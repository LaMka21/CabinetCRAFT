//! Zmena projektu z pôdorysu alebo pohľadu na stenu → príkazy (`wall.*`, `opening.*`,
//! `room.set`, `cabinet.set`). Okno upravuje kópiu projektu; namiesto prepisu celého projektu
//! (`project.set`) pošle tieto príkazy. `None` = zmenu príkazy nevyjadria (iné polia, nové
//! alebo prehodené skrinky, viac otvorov naraz), volajúci potom použije `project.set`.

use serde_json::{json, Map, Value};

use cc_model::{OpeningSpec, ProjectFile};

/// Polia objektu `after`, ktoré sa líšia od `before` (chýbajúce v `after` = `null`).
fn changed_fields(before: &Value, after: &Value) -> Map<String, Value> {
    let mut set = Map::new();
    let (Value::Object(b), Value::Object(a)) = (before, after) else {
        return set;
    };
    for (k, v) in a {
        if b.get(k) != Some(v) {
            set.insert(k.clone(), v.clone());
        }
    }
    for k in b.keys() {
        if !a.contains_key(k) {
            set.insert(k.clone(), Value::Null);
        }
    }
    set
}

fn opening_params(o: &OpeningSpec) -> Value {
    json!({
        "wall": o.wall,
        "kind": o.kind.as_str(),
        "at": o.at,
        "width": o.width,
        "height": o.height,
        "elevation": o.elevation,
    })
}

/// Príkazy, ktoré z `before` urobia `after`, v poradí vykonania.
pub fn room_edit_commands(
    before: &ProjectFile,
    after: &ProjectFile,
) -> Option<Vec<(&'static str, Value)>> {
    if before.project != after.project || before.defaults != after.defaults {
        return None;
    }
    let mut cmds: Vec<(&'static str, Value)> = Vec::new();

    // miestnosť
    if before.room != after.room {
        let (Some(b), Some(a)) = (&before.room, &after.room) else {
            return None;
        };
        let set = changed_fields(
            &serde_json::to_value(b).ok()?,
            &serde_json::to_value(a).ok()?,
        );
        cmds.push(("room.set", json!({ "set": set })));
    }

    // steny: nové a zmenené (podľa ID); zmazané až na konci
    for w in &after.walls {
        match before.walls.iter().find(|b| b.id == w.id) {
            None => cmds.push((
                "wall.add",
                json!({
                    "id": w.id,
                    "x": w.x,
                    "y": w.y,
                    "length": w.length,
                    "angle": w.angle,
                    "height": w.height,
                    "thickness": w.thickness,
                }),
            )),
            Some(b) if b != w => {
                let set = changed_fields(
                    &serde_json::to_value(b).ok()?,
                    &serde_json::to_value(w).ok()?,
                );
                cmds.push(("wall.set", json!({ "id": w.id, "set": set })));
            }
            Some(_) => {}
        }
    }
    // poradie stien sa nemení príkazmi
    let kept_order: Vec<&str> = after
        .walls
        .iter()
        .filter(|w| before.walls.iter().any(|b| b.id == w.id))
        .map(|w| w.id.as_str())
        .collect();
    let before_order: Vec<&str> = before
        .walls
        .iter()
        .filter(|b| after.walls.iter().any(|w| w.id == b.id))
        .map(|b| b.id.as_str())
        .collect();
    if kept_order != before_order {
        return None;
    }
    let removed_walls: Vec<&str> = before
        .walls
        .iter()
        .filter(|b| !after.walls.iter().any(|w| w.id == b.id))
        .map(|b| b.id.as_str())
        .collect();

    // skrinky: rovnaké ID v rovnakom poradí, iba zmenené polia
    if before.cabinets.len() != after.cabinets.len()
        || before
            .cabinets
            .iter()
            .zip(&after.cabinets)
            .any(|(b, a)| b.id != a.id)
    {
        return None;
    }
    for (b, a) in before.cabinets.iter().zip(&after.cabinets) {
        if b != a {
            let set = changed_fields(
                &serde_json::to_value(b).ok()?,
                &serde_json::to_value(a).ok()?,
            );
            // skrinka odpojená od zmazanej steny: urobí to wall.remove
            let detach_only = a.wall.is_none()
                && b.wall
                    .as_deref()
                    .is_some_and(|w| removed_walls.contains(&w))
                && set.keys().all(|k| k == "wall" || k == "at");
            if !detach_only {
                cmds.push(("cabinet.set", json!({ "id": a.id, "set": set })));
            }
        }
    }

    // otvory (index v zozname): otvory na zmazaných stenách zmaže wall.remove
    let survive = |o: &&OpeningSpec| !removed_walls.contains(&o.wall.as_str());
    let b_open: Vec<&OpeningSpec> = before.openings.iter().filter(survive).collect();
    let a_open: Vec<&OpeningSpec> = after.openings.iter().collect();
    if b_open.len() != before.openings.len() {
        // otvory zmazaných stien sú v strede zoznamu: indexy by sa posunuli
        if !before.openings[b_open.len()..]
            .iter()
            .all(|o| removed_walls.contains(&o.wall.as_str()))
        {
            return None;
        }
    }
    match a_open.len().cmp(&b_open.len()) {
        std::cmp::Ordering::Equal => {
            for (i, (b, a)) in b_open.iter().zip(&a_open).enumerate() {
                if b != a {
                    let set = changed_fields(
                        &serde_json::to_value(b).ok()?,
                        &serde_json::to_value(a).ok()?,
                    );
                    cmds.push(("opening.set", json!({ "index": i, "set": set })));
                }
            }
        }
        std::cmp::Ordering::Greater => {
            if a_open[..b_open.len()] != b_open[..] {
                return None;
            }
            for o in &a_open[b_open.len()..] {
                cmds.push(("opening.add", opening_params(o)));
            }
        }
        std::cmp::Ordering::Less => {
            // jeden zmazaný otvor
            if a_open.len() + 1 != b_open.len() {
                return None;
            }
            let i = (0..b_open.len()).find(|&i| {
                let mut rest = b_open.clone();
                rest.remove(i);
                rest == a_open
            })?;
            cmds.push(("opening.remove", json!({ "index": i })));
        }
    }

    for id in removed_walls {
        cmds.push(("wall.remove", json!({ "id": id })));
    }
    Some(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> ProjectFile {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna/kuchyna.toml");
        cc_config::load(&path).unwrap().project
    }

    /// Príkazy aplikované na `before` dajú presne `after`.
    fn apply(before: &ProjectFile, after: &ProjectFile) -> Vec<&'static str> {
        let cmds = room_edit_commands(before, after).expect("vyjadriteľné príkazmi");
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("cc_room_diff_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
        for f in ["materials.toml", "machine.toml"] {
            std::fs::copy(src.join(f), dir.join(f)).unwrap();
        }
        std::fs::create_dir_all(dir.join("standards")).unwrap();
        for e in std::fs::read_dir(src.join("standards")).unwrap() {
            let e = e.unwrap();
            std::fs::copy(e.path(), dir.join("standards").join(e.file_name())).unwrap();
        }
        let path = dir.join("p.toml");
        std::fs::write(&path, toml::to_string(before).unwrap()).unwrap();
        let mut s = crate::Session::new();
        s.run("project.open", json!({ "path": path })).unwrap();
        let ids: Vec<&'static str> = cmds.iter().map(|(id, _)| *id).collect();
        for (id, p) in cmds {
            s.run(id, p.clone())
                .unwrap_or_else(|e| panic!("{id} {p}: {e}"));
        }
        assert_eq!(&s.doc().unwrap().project, after);
        let _ = std::fs::remove_dir_all(&dir);
        ids
    }

    #[test]
    fn wall_move_cabinet_drag_and_opening() {
        let before = project();
        let mut after = before.clone();
        after.walls[0].length += 100.0;
        after.walls[1].x += 100.0;
        if let Some(c) = after.cabinets.iter_mut().find(|c| c.wall.is_some()) {
            c.at += 50.0;
        }
        let ids = apply(&before, &after);
        assert!(
            ids.contains(&"wall.set") && ids.contains(&"cabinet.set"),
            "{ids:?}"
        );

        let mut with_window = before.clone();
        with_window.openings.push(OpeningSpec {
            wall: before.walls[0].id.clone(),
            kind: cc_model::OpeningKind::Window,
            at: 500.0,
            width: 900.0,
            height: 1200.0,
            elevation: 1000.0,
        });
        assert_eq!(apply(&before, &with_window), ["opening.add"]);
        let mut moved = with_window.clone();
        if let Some(o) = moved.openings.last_mut() {
            o.at = 700.0;
        }
        assert_eq!(apply(&with_window, &moved), ["opening.set"]);
        assert_eq!(apply(&with_window, &before), ["opening.remove"]);
    }

    #[test]
    fn new_wall_and_reordered_cabinets() {
        let before = project();
        let mut after = before.clone();
        let mut w = after.walls[0].clone();
        w.id = "W9".into();
        w.y -= 4000.0;
        after.walls.push(w);
        assert_eq!(apply(&before, &after), ["wall.add"]);
        // prehodené skrinky sa príkazmi nevyjadria
        let mut swapped = before.clone();
        if swapped.cabinets.len() > 1 {
            swapped.cabinets.swap(0, 1);
            assert!(room_edit_commands(&before, &swapped).is_none());
        }
    }
}
