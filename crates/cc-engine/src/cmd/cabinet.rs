//! Skrinky: pridať, odstrániť, duplikovať, presunúť v zozname, zmeniť polia.

use cc_i18n::tr;
use cc_model::{CabinetSpec, CabinetType};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::{has_doc, parse, positive, CommandSpec, Effect};
use crate::{new_cabinet, EngineError, Session, MAX_CABINETS};

/// Najviac polohovateľných políc v jednej skrinke.
pub const MAX_SHELVES: u32 = 50;
/// Najväčšia absolútna hodnota súradnice alebo rozmeru (mm).
const MAX_MM: f64 = 100_000.0;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "cabinet.snap_corner",
        menu: false,
        shortcut: None,
        params: r#"{"id": "SP1", "side": "left"|"right"} – skrinka na stene sa prisunie do ľavého alebo pravého rohu (za hĺbku skriniek na susednej stene), ako Snap to Corner v CV pre skrinky bez natočenia"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: snap_corner,
    },
    CommandSpec {
        id: "cabinet.add",
        menu: true,
        shortcut: None,
        params: r#"{"type": "base"|"wall"|"tall" alebo "catalog": "S_KUCH_D" (položka katalógu), "from"?: "Kuchyne_CASTEL", "id"?: "SP3", "w"?: 600, "h"?: 720, "d"?: 560, "adjustable_shelves"?: 1, "wall"?: "W1", "at"?: 0, "snap"?: true (pri prekrytí k susedovi), "after"?: "id skrinky"} – bez steny a polohy do stredu steny vybranej skrinky (inak prvej) na najbližšie voľné miesto ako v CV"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: add,
    },
    CommandSpec {
        id: "cabinet.remove",
        menu: true,
        shortcut: Some("Delete"),
        params: r#"{"id": "SP1"}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: remove,
    },
    CommandSpec {
        id: "cabinet.duplicate",
        menu: true,
        shortcut: Some("Ctrl+D"),
        params: r#"{"id": "SP1"} – kópia ide hneď za originál; pri stene sa posunie o šírku"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: duplicate,
    },
    CommandSpec {
        id: "cabinet.move",
        menu: false,
        shortcut: None,
        params: r#"{"id": "SP1", "by": -1|1} alebo {"id": "SP1", "to": 0}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: move_in_list,
    },
    CommandSpec {
        id: "cabinet.set",
        menu: false,
        shortcut: None,
        params: r#"{"id": "SP1", "set": {"w": 650, "adjustable_shelves": 2, "wall": "W1", "at": 0, "fronts": [{"kind": "drawer", "height": 180}, {"kind": "door", "hinge_side": "right"}], "front_mount": "overlay"|"inset", "hinge": "blum-110-inserta", "drawer": "merivobox", ...}} – polia ako v [[cabinet]] v project.toml; null zmaže voliteľné pole"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: set,
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Add {
    #[serde(rename = "type")]
    cabinet_type: Option<CabinetType>,
    catalog: Option<String>,
    from: Option<String>,
    id: Option<String>,
    w: Option<f64>,
    h: Option<f64>,
    d: Option<f64>,
    adjustable_shelves: Option<u32>,
    wall: Option<String>,
    at: Option<f64>,
    after: Option<String>,
    /// s `wall` a `at` (vloženie myšou): pri prekrytí posunúť k susedovi ako v CV
    #[serde(default)]
    snap: bool,
}

fn add(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "cabinet.add";
    let p: Add = parse(C, params)?;
    let selected = s.selection.cabinet.clone();
    let doc = s.doc_mut()?;
    let item = match &p.catalog {
        Some(id) => Some(
            cc_model::find_catalog_item(&doc.catalogs, id, p.from.as_deref())
                .map(|(_, item)| item.clone())
                .ok_or_else(|| EngineError::from(tr!("engine.cabinet.not_in_catalog", id = id)))?,
        ),
        None => None,
    };
    let cabinet_type = match (&item, p.cabinet_type) {
        (Some(i), None) => i.cabinet_type,
        (_, Some(t)) => t,
        (None, None) => {
            return Err(EngineError::Params {
                command: C.into(),
                message: tr!("engine.cabinet.type_or_catalog"),
            })
        }
    };
    if doc.project.cabinets.len() >= MAX_CABINETS {
        return Err(tr!("engine.session.too_many_cabinets", max = MAX_CABINETS).into());
    }
    let id = match p.id {
        Some(id) => {
            let id = id.trim().to_owned();
            if doc.project.cabinets.iter().any(|c| c.id == id) {
                return Err(tr!("engine.errors.cabinet_exists", id = id).into());
            }
            id
        }
        None => next_id(&doc.project.cabinets, cabinet_type),
    };
    let mut c = match &item {
        Some(i) => CabinetSpec {
            cabinet_type,
            ..i.to_spec(&id)
        },
        None => new_cabinet(cabinet_type, &id),
    };
    if let Some(w) = p.w {
        c.w = positive(C, "w", w)?;
    }
    if let Some(h) = p.h {
        c.h = positive(C, "h", h)?;
    }
    if let Some(d) = p.d {
        c.d = positive(C, "d", d)?;
    }
    if let Some(n) = p.adjustable_shelves {
        c.adjustable_shelves = n;
    }
    c.wall = p.wall;
    if let Some(at) = p.at {
        c.at = at;
    }
    if p.snap {
        if let (Some(wall), Some(at)) = (c.wall.clone(), p.at) {
            if let Some(free) = cc_room::place_on_wall(&doc.project, &wall, c.cabinet_type, c.w, at)
            {
                c.at = free;
            }
        }
    }
    // bez steny v miestnosti so stenami: postaviť na stenu vedľa vybranej skrinky
    // (alebo na prvú stenu) do prvého voľného miesta jej radu, ako pri kreslení v CV
    if c.wall.is_none() && p.at.is_none() {
        if let Some((wall, at)) =
            auto_place(&doc.project, &c, p.after.as_deref().or(selected.as_deref()))
        {
            c.wall = Some(wall);
            c.at = at;
        }
    }
    let cabinets = &mut doc.project.cabinets;
    check_cabinet(C, &c)?;
    let index = match &p.after {
        Some(after) => position(cabinets, after)? + 1,
        None => cabinets.len(),
    };
    cabinets.insert(index.min(cabinets.len()), c);
    s.selection.cabinet = Some(id.clone());
    s.selection.part = None;
    Ok(json!({ "id": id }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapCorner {
    id: String,
    side: String,
}

fn snap_corner(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "cabinet.snap_corner";
    let p: SnapCorner = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    let i = position(&project.cabinets, &p.id)?;
    let (left, right) = cc_room::corner_limits(project, i).ok_or_else(|| EngineError::Params {
        command: C.into(),
        message: tr!("engine.cabinet.not_on_wall", id = &p.id),
    })?;
    let c = project
        .cabinets
        .get_mut(i)
        .ok_or_else(|| EngineError::from(tr!("engine.errors.cabinet_missing", id = &p.id)))?;
    c.at = match p.side.as_str() {
        "left" => left,
        "right" => right - c.w,
        other => {
            return Err(EngineError::Params {
                command: C.into(),
                message: tr!("engine.cabinet.bad_side", value = other),
            })
        }
    };
    Ok(json!({ "at": c.at }))
}

/// Stena a poloha pre novú skrinku bez steny: vedľa skrinky `reference` (v jej rade za ňou,
/// v inom rade nad/pod ňou), inak na prvej stene od začiatku. `None` bez stien alebo bez miesta.
fn auto_place(
    project: &cc_model::ProjectFile,
    c: &CabinetSpec,
    reference: Option<&str>,
) -> Option<(String, f64)> {
    // ako v CV bez myši (findings 06 §4.3): do stredu plochy, pri prekrytí na najbližšie
    // voľné miesto; plocha = stena vybranej skrinky, inak prvá stena
    let wall = reference
        .and_then(|id| project.cabinets.iter().find(|r| r.id == id))
        .and_then(|r| r.wall.clone())
        .or_else(|| project.walls.first().map(|w| w.id.clone()))?;
    let room = cc_room::Room::new(project);
    let length = room
        .wall_index(&wall)
        .and_then(|i| room.walls.get(i))
        .map(|w| w.length)?;
    let prefer = (length / 2.0).round() - c.w / 2.0;
    // na stene nie je miesto: ďalšie steny v poradí od začiatku
    let start = project.walls.iter().position(|w| w.id == wall).unwrap_or(0);
    let others = project
        .walls
        .iter()
        .cycle()
        .skip(start + 1)
        .take(project.walls.len().saturating_sub(1));
    cc_room::place_on_wall(project, &wall, c.cabinet_type, c.w, prefer)
        .map(|at| (wall.clone(), at))
        .or_else(|| {
            others
                .filter_map(|w| {
                    let len = room
                        .wall_index(&w.id)
                        .and_then(|i| room.walls.get(i))
                        .map_or(0.0, |w| w.length);
                    let prefer = (len / 2.0).round() - c.w / 2.0;
                    cc_room::place_on_wall(project, &w.id, c.cabinet_type, c.w, prefer)
                        .map(|at| (w.id.clone(), at))
                })
                .next()
        })
}

fn remove(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Id = parse("cabinet.remove", params)?;
    let cabinets = &mut s.doc_mut()?.project.cabinets;
    let i = position(cabinets, &p.id)?;
    if cabinets.len() == 1 {
        return Err(tr!("engine.cabinet.last_cabinet").into());
    }
    cabinets.remove(i);
    s.fix_selection();
    Ok(Value::Null)
}

fn duplicate(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Id = parse("cabinet.duplicate", params)?;
    let cabinets = &mut s.doc_mut()?.project.cabinets;
    if cabinets.len() >= MAX_CABINETS {
        return Err(tr!("engine.session.too_many_cabinets", max = MAX_CABINETS).into());
    }
    let i = position(cabinets, &p.id)?;
    let Some(original) = cabinets.get(i) else {
        return Err(tr!("engine.errors.cabinet_missing", id = &p.id).into());
    };
    let mut copy = original.clone();
    copy.id = next_id(cabinets, copy.cabinet_type);
    if copy.wall.is_some() {
        copy.at += copy.w;
    }
    let id = copy.id.clone();
    cabinets.insert(i + 1, copy);
    s.selection.cabinet = Some(id.clone());
    s.selection.part = None;
    Ok(json!({ "id": id }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Move {
    id: String,
    by: Option<i64>,
    to: Option<usize>,
}

fn move_in_list(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "cabinet.move";
    let p: Move = parse(C, params)?;
    let cabinets = &mut s.doc_mut()?.project.cabinets;
    let i = position(cabinets, &p.id)?;
    let last = cabinets.len().saturating_sub(1);
    let target = match (p.by, p.to) {
        (Some(by), None) => {
            let t = i64::try_from(i).unwrap_or(i64::MAX).saturating_add(by);
            usize::try_from(t.max(0)).unwrap_or(0).min(last)
        }
        (None, Some(to)) => to.min(last),
        _ => {
            return Err(EngineError::Params {
                command: C.into(),
                message: tr!("engine.cabinet.by_or_to"),
            })
        }
    };
    let c = cabinets.remove(i);
    cabinets.insert(target, c);
    Ok(json!({ "index": target }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    id: String,
    set: Map<String, Value>,
}

fn set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "cabinet.set";
    let p: Set = parse(C, params)?;
    let cabinets = &mut s.doc_mut()?.project.cabinets;
    let i = position(cabinets, &p.id)?;
    let Some(current) = cabinets.get(i) else {
        return Err(tr!("engine.errors.cabinet_missing", id = &p.id).into());
    };
    let mut value = serde_json::to_value(current).map_err(|e| e.to_string())?;
    if let Value::Object(fields) = &mut value {
        for (k, v) in p.set {
            // null: pole sa vynechá a použije sa predvolená hodnota (napr. bez steny)
            if v.is_null() {
                fields.remove(&k);
            } else {
                fields.insert(k, v);
            }
        }
    }
    let updated: CabinetSpec = serde_json::from_value(value).map_err(|e| EngineError::Params {
        command: C.into(),
        message: e.to_string(),
    })?;
    check_cabinet(C, &updated)?;
    let renamed = (updated.id != p.id).then(|| updated.id.clone());
    if let Some(new_id) = &renamed {
        if cabinets.iter().any(|c| &c.id == new_id) {
            return Err(tr!("engine.errors.cabinet_exists", id = new_id).into());
        }
    }
    if let Some(slot) = cabinets.get_mut(i) {
        *slot = updated;
    }
    if let Some(new_id) = renamed {
        if s.selection.cabinet.as_deref() == Some(p.id.as_str()) {
            s.selection.cabinet = Some(new_id);
            s.selection.part = None;
        }
    }
    Ok(Value::Null)
}

/// Najviac čiel v skrinke (generátor má rovnaký limit).
const MAX_FRONTS: usize = 12;

/// Rozmery a súradnice skrinky sú konečné a v rozumnom rozsahu.
pub(super) fn check_cabinet(command: &str, c: &CabinetSpec) -> Result<(), EngineError> {
    let bad = |message: String| EngineError::Params {
        command: command.to_owned(),
        message,
    };
    if c.id.trim().is_empty() {
        return Err(bad(tr!("engine.cabinet.empty_id")));
    }
    for (name, v) in [("w", c.w), ("h", c.h), ("d", c.d)] {
        positive(command, name, v)?;
        if v > MAX_MM {
            return Err(bad(tr!("engine.cabinet.too_large", name = name, value = v)));
        }
    }
    let mut numbers = vec![("at", c.at), ("outset", c.outset)];
    if let Some(e) = c.elevation {
        numbers.push(("elevation", e));
    }
    if let Some(cc_model::Placement::AboveElevation(e) | cc_model::Placement::BelowElevation(e)) =
        c.placement
    {
        numbers.push(("placement", e));
    }
    numbers.extend(c.fixed_shelves.iter().map(|&v| ("fixed_shelves", v)));
    for (name, v) in numbers {
        if !v.is_finite() || v.abs() > MAX_MM {
            return Err(bad(tr!(
                "engine.errors.not_finite_max",
                name = name,
                max = MAX_MM
            )));
        }
    }
    if c.adjustable_shelves > MAX_SHELVES {
        let value = c.adjustable_shelves;
        return Err(bad(tr!(
            "engine.cabinet.too_many_adjustable",
            max = MAX_SHELVES,
            value = value
        )));
    }
    if c.fixed_shelves.len() > MAX_SHELVES as usize {
        return Err(bad(tr!("engine.cabinet.too_many_fixed", max = MAX_SHELVES)));
    }
    if c.fronts.len() > MAX_FRONTS {
        return Err(bad(tr!("engine.cabinet.too_many_fronts", max = MAX_FRONTS)));
    }
    for h in c.fronts.iter().filter_map(|f| f.height) {
        if !h.is_finite() || h <= 0.0 || h > MAX_MM {
            return Err(bad(tr!(
                "engine.cabinet.bad_front_height",
                value = h,
                max = MAX_MM
            )));
        }
    }
    if c.hinge
        .as_ref()
        .is_some_and(|h| h.trim().is_empty() || h.len() > 100)
    {
        return Err(bad(tr!("engine.cabinet.bad_hinge")));
    }
    if c.drawer
        .as_ref()
        .is_some_and(|h| h.trim().is_empty() || h.len() > 100)
    {
        return Err(bad(tr!("engine.cabinet.bad_drawer")));
    }
    Ok(())
}

fn position(cabinets: &[CabinetSpec], id: &str) -> Result<usize, EngineError> {
    cabinets
        .iter()
        .position(|c| c.id == id)
        .ok_or_else(|| tr!("engine.errors.cabinet_missing", id = id).into())
}

/// Ďalšie voľné ID: spodné `SP`, horné `HO`, vysoké `VY` + číslo.
pub fn next_id(cabinets: &[CabinetSpec], t: CabinetType) -> String {
    let prefix = match t {
        CabinetType::Base => "SP",
        CabinetType::Wall => "HO",
        CabinetType::Tall => "VY",
    };
    // Pri n skrinkách je medzi číslami 1..=n+1 vždy aspoň jedno voľné.
    (1..=cabinets.len() + 1)
        .map(|n| format!("{prefix}{n}"))
        .find(|id| cabinets.iter().all(|c| &c.id != id))
        .unwrap_or_else(|| format!("{prefix}{}", cabinets.len() + 1))
}
