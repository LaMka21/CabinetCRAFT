//! Miestnosť (kap. 13): nastavenie miestnosti, steny, otvory, vyplnenie steny skrinkami.

use cc_i18n::tr;
use cc_model::{CabinetType, OpeningKind, OpeningSpec, ProjectFile, RoomSpec, WallSpec};
use cc_room::{FillParams, Row};
use glam::DVec2;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::{has_doc, parse, positive, CommandSpec, Effect};
use crate::{new_cabinet, EngineError, Session, MAX_CABINETS};

/// Najviac stien a otvorov v projekte (zhodne s kontrolou konfigurácie).
const MAX_WALLS: usize = 200;
const MAX_OPENINGS: usize = 500;
const MAX_MM: f64 = 100_000.0;

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        id: "room.corner",
        menu: false,
        shortcut: None,
        params: r#"{"wall": "W1", "row"?: "base"|"upper", "extend": "W1"|"W2"} – roh na konci steny `wall`; predĺži sa skrinka pri rohu na stene `extend` (ako Extend Left/Right End v CV), druhá sa odsunie, aby sa jej dotýkala"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: corner,
    },
    CommandSpec {
        id: "room.set",
        menu: true,
        shortcut: None,
        params: r#"{"set": {"wall_height"?: 2600, "wall_thickness"?: 100, "soffit"?: 0, "toe_height"?: 100, "upper_top"?: 2200|null}}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: room_set,
    },
    CommandSpec {
        id: "room.add_rect",
        menu: true,
        shortcut: None,
        params: r#"{"w": 3600, "d": 2800, "x"?: 0, "y"?: 0} – 4 steny v smere hodinových ručičiek, prvá je zadná"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: add_rect,
    },
    CommandSpec {
        id: "wall.add",
        menu: true,
        shortcut: None,
        params: r#"{"id"?: "W5", "x": 0, "y": 0, "length": 3000, "angle"?: 0, "height"?: 2600, "thickness"?: 100} – miestnosť je napravo od smeru steny"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: wall_add,
    },
    CommandSpec {
        id: "wall.set",
        menu: false,
        shortcut: None,
        params: r#"{"id": "W1", "set": {"length"?: 3200, "angle"?: 0, "x"?: 0, "y"?: 0, "height"?: null, "thickness"?: 120, "id"?: "nové"}} – null vráti predvolenú hodnotu miestnosti"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: wall_set,
    },
    CommandSpec {
        id: "wall.remove",
        menu: true,
        shortcut: None,
        params: r#"{"id": "W1"} – skrinky pri stene ostanú bez steny, otvory sa zmažú"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: wall_remove,
    },
    CommandSpec {
        id: "opening.add",
        menu: true,
        shortcut: None,
        params: r#"{"wall": "W1", "kind": "window"|"door"|"opening", "at"?: stred steny, "width"?: 900, "height"?: 1200|2000, "elevation"?: 1000|0}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: opening_add,
    },
    CommandSpec {
        id: "opening.set",
        menu: false,
        shortcut: None,
        params: r#"{"index": 0, "set": {"at"?: 1200, "width"?: 900, "height"?: 1200, "elevation"?: 1000, "kind"?: "door", "wall"?: "W2"}} – index v zozname [[opening]]"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: opening_set,
    },
    CommandSpec {
        id: "opening.remove",
        menu: false,
        shortcut: None,
        params: r#"{"index": 0}"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: opening_remove,
    },
    CommandSpec {
        id: "room.orient",
        menu: true,
        shortcut: None,
        params: "{} – uzavretú miestnosť nakreslenú proti smeru hodinových ručičiek otočí (miestnosť napravo od smeru steny ako v CV); skrinky a otvory ostanú na mieste",
        effect: Effect::Edit,
        enabled: has_doc,
        run: room_orient,
    },
    CommandSpec {
        id: "cabinet.fill",
        menu: true,
        shortcut: None,
        params: r#"{"wall": "W1", "row": "base"|"upper", "nominal"?: 600, "max"?: 900, "increment"?: 50, "min"?: 150} – rozmery okrem šírky podľa poslednej skrinky toho typu"#,
        effect: Effect::Edit,
        enabled: has_doc,
        run: fill,
    },
];

fn room_orient(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let _: Map<String, Value> = parse("room.orient", params)?;
    let project = &mut s.doc_mut()?.project;
    let turned = cc_room::orient_rooms(project);
    Ok(json!({ "walls": turned }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corner {
    wall: String,
    row: Option<String>,
    extend: String,
}

fn corner(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "room.corner";
    let p: Corner = parse(C, params)?;
    let row = match p.row.as_deref() {
        None | Some("base") => Row::Base,
        Some("upper") => Row::Upper,
        Some(other) => return Err(bad(C, tr!("engine.room.bad_row", value = other))),
    };
    let project = &mut s.doc_mut()?.project;
    let (_, _, next) = cc_room::corner_cabinets(project, &p.wall, row)
        .ok_or_else(|| bad(C, tr!("engine.room.no_corner", wall = &p.wall)))?;
    let extend_next = if p.extend == p.wall {
        false
    } else if p.extend == next {
        true
    } else {
        return Err(bad(
            C,
            tr!("engine.room.bad_extend", wall = &p.wall, next = &next),
        ));
    };
    let mut updated = project.clone();
    let id = cc_room::extend_into_corner(&mut updated, &p.wall, row, extend_next)
        .map_err(|m| bad(C, m))?;
    for c in &updated.cabinets {
        super::cabinet::check_cabinet(C, c)?;
    }
    *project = updated;
    Ok(json!({ "id": id }))
}

fn bad(command: &str, message: impl Into<String>) -> EngineError {
    EngineError::Params {
        command: command.to_owned(),
        message: message.into(),
    }
}

fn finite(command: &str, name: &str, v: f64) -> Result<f64, EngineError> {
    if v.is_finite() && v.abs() <= MAX_MM {
        Ok(v)
    } else {
        Err(bad(
            command,
            tr!(
                "engine.room.not_finite",
                name = name,
                max = MAX_MM,
                value = v
            ),
        ))
    }
}

fn non_negative(command: &str, name: &str, v: f64) -> Result<f64, EngineError> {
    if finite(command, name, v)? >= 0.0 {
        Ok(v)
    } else {
        Err(bad(
            command,
            tr!("engine.room.negative", name = name, value = v),
        ))
    }
}

/// Zlúči `set` do serializovanej hodnoty: `null` pole vynechá (predvolená hodnota).
fn merged<T: Serialize + DeserializeOwned>(
    command: &str,
    current: &T,
    set: Map<String, Value>,
) -> Result<T, EngineError> {
    let mut value = serde_json::to_value(current).map_err(|e| e.to_string())?;
    if let Value::Object(fields) = &mut value {
        for (k, v) in set {
            if v.is_null() {
                fields.remove(&k);
            } else {
                fields.insert(k, v);
            }
        }
    }
    serde_json::from_value(value).map_err(|e| bad(command, e.to_string()))
}

fn check_room(command: &str, r: &RoomSpec) -> Result<(), EngineError> {
    positive(command, "wall_height", r.wall_height)?;
    positive(command, "wall_thickness", r.wall_thickness)?;
    non_negative(command, "soffit", r.soffit)?;
    non_negative(command, "toe_height", r.toe_height)?;
    if let Some(top) = r.upper_top {
        positive(command, "upper_top", top)?;
    }
    for (name, v) in [
        ("wall_height", r.wall_height),
        ("wall_thickness", r.wall_thickness),
    ] {
        finite(command, name, v)?;
    }
    Ok(())
}

fn check_wall(command: &str, w: &WallSpec) -> Result<(), EngineError> {
    if w.id.trim().is_empty() {
        return Err(bad(command, tr!("engine.room.empty_wall_id")));
    }
    finite(command, "x", w.x)?;
    finite(command, "y", w.y)?;
    finite(command, "length", positive(command, "length", w.length)?)?;
    if !w.angle.is_finite() || w.angle.abs() > 3600.0 {
        return Err(bad(command, tr!("engine.room.bad_angle")));
    }
    for (name, v) in [("height", w.height), ("thickness", w.thickness)] {
        if let Some(v) = v {
            finite(command, name, positive(command, name, v)?)?;
        }
    }
    Ok(())
}

fn check_opening(command: &str, project: &ProjectFile, o: &OpeningSpec) -> Result<(), EngineError> {
    if !project.walls.iter().any(|w| w.id == o.wall) {
        return Err(tr!("engine.errors.wall_missing", id = &o.wall).into());
    }
    finite(command, "width", positive(command, "width", o.width)?)?;
    finite(command, "height", positive(command, "height", o.height)?)?;
    non_negative(command, "at", o.at)?;
    non_negative(command, "elevation", o.elevation)?;
    Ok(())
}

fn ensure_room(project: &mut ProjectFile) {
    if project.room.is_none() {
        project.room = Some(RoomSpec::default());
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetOnly {
    set: Map<String, Value>,
}

fn room_set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "room.set";
    let p: SetOnly = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    let current = project.room.clone().unwrap_or_default();
    let updated: RoomSpec = merged(C, &current, p.set)?;
    check_room(C, &updated)?;
    project.room = Some(updated);
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rect {
    w: f64,
    d: f64,
    x: Option<f64>,
    y: Option<f64>,
}

fn add_rect(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "room.add_rect";
    let p: Rect = parse(C, params)?;
    let w = finite(C, "w", positive(C, "w", p.w)?)?;
    let d = finite(C, "d", positive(C, "d", p.d)?)?;
    let x = finite(C, "x", p.x.unwrap_or(0.0))?;
    let y = finite(C, "y", p.y.unwrap_or(0.0))?;
    let project = &mut s.doc_mut()?.project;
    if project.walls.len() + 4 > MAX_WALLS {
        return Err(tr!("engine.room.too_many_walls", max = MAX_WALLS).into());
    }
    ensure_room(project);
    let walls = cc_room::rect_walls(project, DVec2::new(x, y), w, d);
    let ids: Vec<String> = walls.iter().map(|w| w.id.clone()).collect();
    project.walls.extend(walls);
    Ok(json!({ "walls": ids }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WallAdd {
    id: Option<String>,
    x: f64,
    y: f64,
    length: f64,
    angle: Option<f64>,
    height: Option<f64>,
    thickness: Option<f64>,
}

fn wall_add(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "wall.add";
    let p: WallAdd = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    if project.walls.len() >= MAX_WALLS {
        return Err(tr!("engine.room.too_many_walls", max = MAX_WALLS).into());
    }
    let id = match p.id {
        Some(id) => {
            let id = id.trim().to_owned();
            if project.walls.iter().any(|w| w.id == id) {
                return Err(tr!("engine.errors.wall_exists", id = id).into());
            }
            id
        }
        None => cc_room::next_wall_id(project),
    };
    let wall = WallSpec {
        id: id.clone(),
        x: p.x,
        y: p.y,
        length: p.length,
        angle: p.angle.unwrap_or(0.0),
        height: p.height,
        thickness: p.thickness,
    };
    check_wall(C, &wall)?;
    ensure_room(project);
    project.walls.push(wall);
    Ok(json!({ "id": id }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WallSet {
    id: String,
    set: Map<String, Value>,
}

fn wall_set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "wall.set";
    let p: WallSet = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    let i = wall_position(project, &p.id)?;
    let Some(current) = project.walls.get(i) else {
        return Err(tr!("engine.errors.wall_missing", id = &p.id).into());
    };
    let updated: WallSpec = merged(C, current, p.set)?;
    check_wall(C, &updated)?;
    let new_id = updated.id.clone();
    if new_id != p.id {
        if project.walls.iter().any(|w| w.id == new_id) {
            return Err(tr!("engine.errors.wall_exists", id = &new_id).into());
        }
        // premenovanie: odkazy skriniek a otvorov idú so stenou
        for c in &mut project.cabinets {
            if c.wall.as_deref() == Some(p.id.as_str()) {
                c.wall = Some(new_id.clone());
            }
        }
        for o in &mut project.openings {
            if o.wall == p.id {
                o.wall = new_id.clone();
            }
        }
    }
    if let Some(slot) = project.walls.get_mut(i) {
        *slot = updated;
    }
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    id: String,
}

fn wall_remove(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Id = parse("wall.remove", params)?;
    let project = &mut s.doc_mut()?.project;
    let i = wall_position(project, &p.id)?;
    project.walls.remove(i);
    project.openings.retain(|o| o.wall != p.id);
    let mut detached = Vec::new();
    for c in &mut project.cabinets {
        if c.wall.as_deref() == Some(p.id.as_str()) {
            c.wall = None;
            c.at = 0.0;
            detached.push(c.id.clone());
        }
    }
    Ok(json!({ "detached": detached }))
}

fn wall_position(project: &ProjectFile, id: &str) -> Result<usize, EngineError> {
    project
        .walls
        .iter()
        .position(|w| w.id == id)
        .ok_or_else(|| tr!("engine.errors.wall_missing", id = id).into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpeningAdd {
    wall: String,
    kind: OpeningKind,
    at: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    elevation: Option<f64>,
}

fn opening_add(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "opening.add";
    let p: OpeningAdd = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    if project.openings.len() >= MAX_OPENINGS {
        return Err(tr!("engine.room.too_many_openings", max = MAX_OPENINGS).into());
    }
    let length = project
        .walls
        .iter()
        .find(|w| w.id == p.wall)
        .map(|w| w.length)
        .ok_or_else(|| EngineError::from(tr!("engine.errors.wall_missing", id = &p.wall)))?;
    let (dh, de) = match p.kind {
        OpeningKind::Window => (1200.0, 1000.0),
        OpeningKind::Door | OpeningKind::Opening => (2000.0, 0.0),
    };
    let width = p.width.unwrap_or(900.0);
    let o = OpeningSpec {
        wall: p.wall,
        kind: p.kind,
        at: p
            .at
            .unwrap_or_else(|| ((length - width) / 2.0).max(0.0).round()),
        width,
        height: p.height.unwrap_or(dh),
        elevation: p.elevation.unwrap_or(de),
    };
    check_opening(C, project, &o)?;
    project.openings.push(o);
    Ok(json!({ "index": project.openings.len() - 1 }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpeningSet {
    index: usize,
    set: Map<String, Value>,
}

fn opening_set(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "opening.set";
    let p: OpeningSet = parse(C, params)?;
    let project = &mut s.doc_mut()?.project;
    let Some(current) = project.openings.get(p.index) else {
        return Err(tr!("engine.room.opening_missing", index = p.index).into());
    };
    let updated: OpeningSpec = merged(C, current, p.set)?;
    check_opening(C, project, &updated)?;
    if let Some(slot) = project.openings.get_mut(p.index) {
        *slot = updated;
    }
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    index: usize,
}

fn opening_remove(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    let p: Index = parse("opening.remove", params)?;
    let project = &mut s.doc_mut()?.project;
    if p.index >= project.openings.len() {
        return Err(tr!("engine.room.opening_missing", index = p.index).into());
    }
    project.openings.remove(p.index);
    Ok(Value::Null)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum RowParam {
    Base,
    Upper,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    wall: String,
    row: RowParam,
    nominal: Option<f64>,
    max: Option<f64>,
    increment: Option<f64>,
    min: Option<f64>,
}

fn fill(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "cabinet.fill";
    let p: Fill = parse(C, params)?;
    let defaults = FillParams::default();
    let fp = FillParams {
        nominal: finite(
            C,
            "nominal",
            positive(C, "nominal", p.nominal.unwrap_or(defaults.nominal))?,
        )?,
        max: finite(C, "max", positive(C, "max", p.max.unwrap_or(defaults.max))?)?,
        increment: finite(
            C,
            "increment",
            positive(C, "increment", p.increment.unwrap_or(defaults.increment))?,
        )?,
        min: finite(C, "min", positive(C, "min", p.min.unwrap_or(defaults.min))?)?,
    };
    if fp.nominal > fp.max {
        return Err(bad(C, tr!("engine.room.nominal_over_max")));
    }
    if fp.increment < 1.0 {
        return Err(bad(C, tr!("engine.room.bad_increment")));
    }
    let project = &mut s.doc_mut()?.project;
    wall_position(project, &p.wall)?;
    let (row, t) = match p.row {
        RowParam::Base => (Row::Base, CabinetType::Base),
        RowParam::Upper => (Row::Upper, CabinetType::Wall),
    };
    let template = project
        .cabinets
        .iter()
        .rev()
        .find(|c| c.cabinet_type == t)
        .cloned()
        .unwrap_or_else(|| new_cabinet(t, ""));
    let added = cc_room::autofill(project, &p.wall, row, &fp, &template);
    if project.cabinets.len() + added.len() > MAX_CABINETS {
        return Err(tr!("engine.session.too_many_cabinets", max = MAX_CABINETS).into());
    }
    let ids: Vec<String> = added.iter().map(|c| c.id.clone()).collect();
    project.cabinets.extend(added);
    if let Some(last) = ids.last() {
        s.selection.cabinet = Some(last.clone());
        s.selection.part = None;
    }
    Ok(json!({ "added": ids }))
}
