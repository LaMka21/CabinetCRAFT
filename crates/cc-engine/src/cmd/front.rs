//! Editor sekcií čiel (ako Section Face v CV, findings 11 §3): delenie otvoru vodorovne
//! a zvislo, typ otvoru, zamknutie výšky/šírky, zmazanie delenia.
//!
//! Otvor = riadok `fronts` skrinky (zhora od 0) a voliteľne stĺpec riadku.

use cc_i18n::tr;
use cc_model::{CabinetSpec, FrontAdjust, FrontKind, FrontSpec, HingeSide};
use serde::Deserialize;
use serde_json::{json, Value};

use super::cabinet::check_cabinet;
use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

/// Najviac častí pri jednom delení.
const MAX_SPLIT: usize = 6;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "front.split",
        menu: false,
        shortcut: None,
        params: r#"{"cabinet": "SP1", "row": 0, "column"?: 1, "direction": "horizontal"|"vertical", "count"?: 2} – vodorovne = nad sebou (riadky), zvislo = vedľa seba (stĺpce); časti dostanú rovnaký rozmer a typ otvoru"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: split,
    },
    CommandSpec {
        id: "front.set",
        menu: false,
        shortcut: None,
        params: r#"{"cabinet": "SP1", "row": 0, "column"?: 1, "kind"?: "door"|"pair"|"drawer"|"false_front"|"open"|"panel", "hinge_side"?: "left"|"right", "lock"?: true|false, "height"?: 300, "width"?: 450, "adjust"?: {"left": 0, "right": 0, "top": 0, "bottom": 0}} – lock zamkne aktuálnu výšku riadku (šírku stĺpca), false ju uvoľní (Equalize v CV); height/width zadá a zamkne rozmer; adjust = úprava rozmeru čela"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set,
    },
    CommandSpec {
        id: "front.remove",
        menu: false,
        shortcut: None,
        params: r#"{"cabinet": "SP1", "row": 0, "column"?: 1} – otvor sa zlúči so susedom (riadok s riadkom pod ním, inak nad ním; stĺpec so susedným stĺpcom)"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: remove,
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    cabinet: String,
    row: usize,
    column: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Split {
    cabinet: String,
    row: usize,
    column: Option<usize>,
    direction: Direction,
    count: Option<usize>,
}

#[derive(Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    cabinet: String,
    row: usize,
    column: Option<usize>,
    kind: Option<FrontKind>,
    hinge_side: Option<HingeSide>,
    lock: Option<bool>,
    height: Option<f64>,
    width: Option<f64>,
    adjust: Option<FrontAdjust>,
}

/// Rozmer otvoru z posledného modelu (výška riadku, šírka stĺpca po čiaru delenia)
/// a škára medzi riadkami.
struct Measured {
    row_h: Option<f64>,
    col_w: Option<f64>,
    gap: f64,
}

fn measure(s: &Session, cabinet: &str, row: usize, column: Option<usize>) -> Measured {
    let (gap, pair) = s.doc().map_or((2.0, 2.0), |d| {
        let c = d.project.cabinets.iter().find(|c| c.id == cabinet);
        let rows = c.map(|c| d.standard.fronts.rows(c.cabinet_type));
        let gap = c
            .and_then(|c| c.reveals.horizontal)
            .or(rows.map(|r| r.gap))
            .unwrap_or(2.0);
        let pair = c
            .and_then(|c| c.reveals.vertical)
            .unwrap_or(d.standard.fronts.pair_gap);
        (gap, pair)
    });
    let built = s.model();
    let openings = built
        .as_ref()
        .and_then(|b| b.model.cabinets.iter().find(|c| c.id == cabinet))
        .map(|c| c.openings.clone())
        .unwrap_or_default();
    let row_h = openings.iter().find(|o| o.row == row).map(|o| o.h);
    // šírka stĺpca po čiaru delenia (súčet šírok stĺpcov = šírka skrinky): čelo + polovičné škáry
    let col_w = column.and_then(|k| {
        let cols: Vec<_> = openings.iter().filter(|o| o.row == row).collect();
        let o = cols.iter().find(|o| o.column == Some(k))?;
        let n = cols.len();
        let half = pair / 2.0;
        let inner = if k == 0 || k + 1 == n {
            half
        } else {
            2.0 * half
        };
        let edge = s.doc().map_or(0.0, |d| d.standard.fronts.reveal_side)
            * f64::from(u8::from(k == 0 || k + 1 == n));
        Some(o.w + inner + edge)
    });
    Measured { row_h, col_w, gap }
}

fn with_cabinet<R>(
    s: &mut Session,
    command: &str,
    id: &str,
    f: impl FnOnce(&mut CabinetSpec) -> Result<R, String>,
) -> Result<R, EngineError> {
    let cabinets = &mut s.doc_mut()?.project.cabinets;
    let c = cabinets
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or_else(|| EngineError::from(tr!("engine.errors.cabinet_missing", id = id)))?;
    let mut updated = c.clone();
    let r = f(&mut updated).map_err(|message| EngineError::Params {
        command: command.to_owned(),
        message,
    })?;
    check_cabinet(command, &updated)?;
    *c = updated;
    Ok(r)
}

fn row_mut(c: &mut CabinetSpec, row: usize) -> Result<&mut FrontSpec, String> {
    let n = c.fronts.len();
    c.fronts
        .get_mut(row)
        .ok_or_else(|| tr!("engine.front.row_missing_of", count = n, row = row))
}

fn split(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "front.split";
    let p: Split = parse(C, params)?;
    let n = p.count.unwrap_or(2);
    if !(2..=MAX_SPLIT).contains(&n) {
        return Err(EngineError::Params {
            command: C.into(),
            message: tr!("engine.front.bad_count", max = MAX_SPLIT),
        });
    }
    let m = measure(s, &p.cabinet, p.row, p.column);
    with_cabinet(s, C, &p.cabinet, |c| {
        let row = row_mut(c, p.row)?.clone();
        match (p.direction, p.column) {
            (Direction::Horizontal, None) => {
                if !row.columns.is_empty() {
                    return Err(tr!("engine.front.row_has_columns_h"));
                }
                // časti rovnakej výšky; pri voľnej výške ostane posledná voľná
                let total = row.height.or(m.row_h);
                let part = total.map(|h| round1((h - (n - 1) as f64 * m.gap) / n as f64));
                let parts: Vec<FrontSpec> = (0..n)
                    .map(|k| FrontSpec {
                        height: if row.height.is_none() && k + 1 == n {
                            None
                        } else {
                            part
                        },
                        ..row.clone()
                    })
                    .collect();
                c.fronts.splice(p.row..=p.row, parts);
            }
            (Direction::Horizontal, Some(_)) => {
                return Err(tr!("engine.front.column_split_h"));
            }
            (Direction::Vertical, None) => {
                if !row.columns.is_empty() {
                    return Err(tr!("engine.front.row_has_columns_v"));
                }
                let kind = match row.kind {
                    FrontKind::Pair => FrontKind::Door,
                    k => k,
                };
                let r = row_mut(c, p.row)?;
                r.columns = (0..n)
                    .map(|k| FrontSpec {
                        // dvierka: krajné vpravo s pántami vpravo
                        hinge_side: (kind == FrontKind::Door && k + 1 == n)
                            .then_some(HingeSide::Right),
                        ..FrontSpec::new(kind)
                    })
                    .collect();
            }
            (Direction::Vertical, Some(col)) => {
                let r = row_mut(c, p.row)?;
                if r.columns.len() + n - 1 > MAX_SPLIT {
                    return Err(tr!("engine.front.too_many_columns", max = MAX_SPLIT));
                }
                let cell = r
                    .columns
                    .get(col)
                    .cloned()
                    .ok_or_else(|| tr!("engine.front.column_missing", column = col))?;
                let w = cell.width.or(m.col_w).map(|w| round1(w / n as f64));
                let parts: Vec<FrontSpec> = (0..n)
                    .map(|k| FrontSpec {
                        width: if cell.width.is_none() && k + 1 == n {
                            None
                        } else {
                            w
                        },
                        ..cell.clone()
                    })
                    .collect();
                r.columns.splice(col..=col, parts);
            }
        }
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

fn set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "front.set";
    let p: Set = parse(C, params)?;
    let m = measure(s, &p.cabinet, p.row, p.column);
    for (name, v) in [("height", p.height), ("width", p.width)] {
        if let Some(v) = v {
            if !(v.is_finite() && v > 0.0 && v < 10_000.0) {
                return Err(EngineError::Params {
                    command: C.into(),
                    message: tr!("engine.front.bad_size", name = name),
                });
            }
        }
    }
    if let Some(a) = &p.adjust {
        if ![a.left, a.right, a.top, a.bottom]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1000.0)
        {
            return Err(EngineError::Params {
                command: C.into(),
                message: tr!("engine.front.bad_adjust"),
            });
        }
    }
    with_cabinet(s, C, &p.cabinet, |c| {
        let row = row_mut(c, p.row)?;
        if let Some(h) = p.height {
            if p.column.is_some() {
                return Err(tr!("engine.front.height_on_row"));
            }
            row.height = Some(round1(h));
        }
        if let Some(w) = p.width {
            let col = p
                .column
                .ok_or_else(|| tr!("engine.front.width_on_column"))?;
            let cell = row
                .columns
                .get_mut(col)
                .ok_or_else(|| tr!("engine.front.column_missing", column = col))?;
            cell.width = Some(round1(w));
        }
        if let Some(lock) = p.lock {
            match p.column {
                None => row.height = if lock { m.row_h.map(round1) } else { None },
                Some(col) => {
                    let cell = row
                        .columns
                        .get_mut(col)
                        .ok_or_else(|| tr!("engine.front.column_missing", column = col))?;
                    cell.width = if lock { m.col_w.map(round1) } else { None };
                }
            }
        }
        let target = match p.column {
            None => row,
            Some(col) => row
                .columns
                .get_mut(col)
                .ok_or_else(|| tr!("engine.front.column_missing", column = col))?,
        };
        if let Some(kind) = p.kind {
            if p.column.is_some() && kind == FrontKind::Pair {
                return Err(tr!("engine.front.pair_in_column"));
            }
            target.kind = kind;
            if kind != FrontKind::Door {
                target.hinge_side = None;
            }
            if kind != FrontKind::Drawer {
                target.box_height = None;
            }
        }
        if let Some(side) = p.hinge_side {
            if target.kind != FrontKind::Door {
                return Err(tr!("engine.front.hinge_side_door"));
            }
            target.hinge_side = (side == HingeSide::Right).then_some(side);
        }
        if let Some(a) = p.adjust {
            target.adjust = a;
        }
        Ok(())
    })?;
    Ok(Value::Null)
}

fn remove(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "front.remove";
    let p: Target = parse(C, params)?;
    let m = measure(s, &p.cabinet, p.row, p.column);
    with_cabinet(s, C, &p.cabinet, |c| {
        match p.column {
            None => {
                if p.row >= c.fronts.len() {
                    return Err(tr!("engine.front.row_missing", row = p.row));
                }
                if c.fronts.len() == 1 {
                    // jediný otvor: skrinka ostane bez čiel
                    c.fronts.clear();
                    return Ok(());
                }
                let removed = c.fronts.remove(p.row);
                // sused pod (inak nad) prevezme výšku, ak je zamknutá
                let k = if p.row < c.fronts.len() {
                    p.row
                } else {
                    p.row - 1
                };
                if let Some(n) = c.fronts.get_mut(k) {
                    if let (Some(a), Some(b)) = (n.height, removed.height.or(m.row_h)) {
                        n.height = Some(round1(a + b + m.gap));
                    }
                }
            }
            Some(col) => {
                let row = row_mut(c, p.row)?;
                if col >= row.columns.len() {
                    return Err(tr!("engine.front.column_missing", column = col));
                }
                let removed = row.columns.remove(col);
                let k = if col < row.columns.len() {
                    col
                } else {
                    col - 1
                };
                if let Some(n) = row.columns.get_mut(k) {
                    if let (Some(a), Some(b)) = (n.width, removed.width.or(m.col_w)) {
                        n.width = Some(round1(a + b));
                    }
                }
                // jeden stĺpec = celý riadok
                if row.columns.len() == 1 {
                    let only = row.columns.remove(0);
                    row.kind = only.kind;
                    row.hinge_side = only.hinge_side;
                }
            }
        }
        Ok(())
    })?;
    Ok(Value::Null)
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}
