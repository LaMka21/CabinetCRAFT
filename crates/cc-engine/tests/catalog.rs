//! Katalógy skriniek: zoznam, pridanie, vlastný katalóg, uloženie a odstránenie položky.

use std::path::{Path, PathBuf};

use cc_engine::Session;
use serde_json::{json, Value};

fn temp_copy(name: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna");
    let dir = std::env::temp_dir().join(format!("cc-catalog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&src, &dir);
    dir
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let target = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    s.run(id, params).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn spec<'a>(s: &'a Session, id: &str) -> &'a cc_model::CabinetSpec {
    s.doc()
        .unwrap()
        .project
        .cabinets
        .iter()
        .find(|c| c.id == id)
        .unwrap()
}

#[test]
fn shop_catalog_adds_cabinets() {
    let dir = temp_copy("shop");
    let mut s = Session::open(&dir.join("project.toml")).unwrap();
    let list = run(&mut s, "catalog.list", json!({}));
    assert_eq!(list[0]["name"], "Kuchyne_CASTEL", "{list}");
    let items = run(&mut s, "catalog.list", json!({"catalog": "Kuchyne_CASTEL"}));
    assert!(items.as_array().unwrap().len() >= 14);

    let id = run(
        &mut s,
        "cabinet.add",
        json!({"catalog": "S_KUCH_VARNA_D", "from": "Kuchyne_CASTEL"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let c = spec(&s, &id);
    assert_eq!(c.fronts.len(), 3);
    assert_eq!(c.fronts[0].height, Some(130.0));

    // drez bez zadnej steny, umývačka iba čelo
    let drez = run(&mut s, "cabinet.add", json!({"catalog": "S_KUCH_DREZ"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let umyv = run(&mut s, "cabinet.add", json!({"catalog": "S_UMYVACKA"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let built = s.model().unwrap();
    let parts = |cab: &str| -> Vec<cc_model::Role> {
        built
            .model
            .cabinets
            .iter()
            .find(|c| c.id == cab)
            .unwrap()
            .parts
            .iter()
            .map(|p| p.role)
            .collect()
    };
    assert!(!parts(&drez).contains(&cc_model::Role::Back));
    assert!(parts(&drez).contains(&cc_model::Role::DrawerFront));
    assert_eq!(parts(&umyv), vec![cc_model::Role::FalseFront]);
    // drez a umývačka bez chýb (čelo 130 varnej skrinky je pri režime cx100 pod minimom CX100)
    let ours: Vec<_> = built
        .issues()
        .filter(|i| {
            i.part
                .as_deref()
                .is_some_and(|p| p.starts_with(&drez) || p.starts_with(&umyv))
        })
        .collect();
    assert!(ours.is_empty(), "{ours:?}");

    assert!(s
        .run("cabinet.add", json!({"catalog": "S_KUCH_D", "from": "Iný"}))
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn own_catalog_create_save_remove() {
    let dir = temp_copy("own");
    let mut s = Session::open(&dir.join("project.toml")).unwrap();
    for bad in ["", "../x", "a/b", ".skryty", "x".repeat(61).as_str()] {
        assert!(
            s.run("catalog.create", json!({"name": bad})).is_err(),
            "{bad}"
        );
    }
    run(&mut s, "catalog.create", json!({"name": "Moje skrinky"}));
    assert!(dir.join("catalogs/Moje skrinky.toml").is_file());
    assert!(s
        .run("catalog.create", json!({"name": "moje skrinky"}))
        .is_err());

    // skrinka zo zákazky s čelami → položka
    let cab = s.doc().unwrap().project.cabinets[0].id.clone();
    run(
        &mut s,
        "cabinet.set",
        json!({"id": cab, "set": {"w": 450, "fronts": [{"kind": "drawer", "height": 200}, {"kind": "door", "hinge_side": "right"}]}}),
    );
    let r = run(
        &mut s,
        "catalog.save_item",
        json!({"catalog": "Moje skrinky", "cabinet": cab, "id": "S_MOJA", "name": "Moja 450", "group": "Spodné"}),
    );
    assert_eq!(r["replaced"], false);
    let r = run(
        &mut s,
        "catalog.save_item",
        json!({"catalog": "Moje skrinky", "cabinet": cab, "id": "S_MOJA"}),
    );
    assert_eq!(r["replaced"], true);

    // po znovuotvorení je položka v katalógu a pridá sa s čelami
    let mut s2 = Session::open(&dir.join("project.toml")).unwrap();
    let items = run(&mut s2, "catalog.list", json!({"catalog": "Moje skrinky"}));
    assert_eq!(items[0]["name"], "Moja 450", "meno ostane pri prepísaní");
    let id = run(
        &mut s2,
        "cabinet.add",
        json!({"catalog": "S_MOJA", "from": "Moje skrinky"}),
    )["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let c = spec(&s2, &id);
    assert_eq!(c.w, 450.0);
    assert_eq!(c.fronts.len(), 2);
    assert_eq!(c.wall, None);

    run(
        &mut s2,
        "catalog.remove_item",
        json!({"catalog": "Moje skrinky", "id": "S_MOJA"}),
    );
    assert!(s2
        .run(
            "catalog.remove_item",
            json!({"catalog": "Moje skrinky", "id": "S_MOJA"})
        )
        .is_err());
    let text = std::fs::read_to_string(dir.join("catalogs/Moje skrinky.toml")).unwrap();
    assert!(!text.contains("S_MOJA"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Slepý roh S_KUC_ROH_P (findings 11 §5.2): panel 0–500 a dvierka 500–1000, slepý pánt
/// 80/650 s krížovou podložkou na paneli 21,5 od jeho hrany (±16), 6 nožičiek, konfirmáty.
#[test]
fn blind_corner_from_catalog() {
    let dir = temp_copy("corner");
    let mut s = Session::open(&dir.join("project.toml")).unwrap();
    let id = run(&mut s, "cabinet.add", json!({"catalog": "S_KUC_ROH_P"}))["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let built = s.model().unwrap();
    let cab = built.model.cabinets.iter().find(|c| c.id == id).unwrap();
    let part = |pid: &str| cab.parts.iter().find(|p| p.id == pid).unwrap();
    let panel = part(&format!("{id}-FF-1"));
    let door = part(&format!("{id}-DOOR-1"));
    // panel bez úchytky 762 − 2 = 760, dvierka 762 − 32 = 730; šírky 500 − 1 − 1
    assert_eq!((panel.length, panel.width), (760.0, 498.0));
    assert_eq!((door.length, door.width), (730.0, 498.0));
    let holes = |p: &cc_model::Part, d: f64| -> Vec<(f64, f64)> {
        let mut v: Vec<(f64, f64)> = p
            .operations
            .iter()
            .filter_map(|op| match *op {
                cc_model::Operation::Hole { x, y, diameter, .. } if diameter == d => Some((x, y)),
                _ => None,
            })
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    };
    // slepý pánt vľavo pri paneli: misky 22,5 od ľavej hrany dvierok, 80 a 650
    assert_eq!(holes(door, 35.0), vec![(80.0, 22.5), (650.0, 22.5)]);
    // krížová podložka na paneli: 21,5 od pravej hrany panelu, výšky 64/96 a 634/666;
    // konfirmáty spoja s bokom 64/381/698 v strede boku (panel od X 1 → 8 od hrany)
    assert_eq!(
        holes(panel, 5.0),
        vec![
            (64.0, 8.0),
            (64.0, 476.5),
            (96.0, 476.5),
            (381.0, 8.0),
            (634.0, 476.5),
            (666.0, 476.5),
            (698.0, 8.0)
        ]
    );
    // slepý roh (findings 11 §5.2): ľavý bok vystupuje 3 k panelu (D − 4 + 3), pravý nie;
    // panel s bokom 6 kolíkov, s dnom 4 (32/96/350/414 od boku)
    assert_eq!(part(&format!("{id}-SIDE-L")).width, 509.0);
    assert_eq!(part(&format!("{id}-SIDE-R")).width, 506.0);
    let panel_dowels = holes(panel, 8.0);
    assert_eq!(panel_dowels.len(), 10, "{panel_dowels:?}");
    let low: Vec<f64> = panel_dowels
        .iter()
        .filter(|(x, _)| *x < 10.0)
        .map(|(_, y)| *y)
        .collect();
    // dno od X 18, panel od X 1: 18 − 1 + 32 = 49 …
    assert_eq!(low, vec![49.0, 113.0, 367.0, 431.0]);
    let bill = run(&mut s, "model.hardware", json!({"cabinet": id}));
    let qty = |name: &str| {
        bill.as_array()
            .unwrap()
            .iter()
            .find(|l| l["name"].as_str().unwrap().starts_with(name))
            .map_or(0, |l| l["qty"].as_u64().unwrap())
    };
    assert_eq!(qty("Nožička"), 6, "{bill}");
    assert_eq!(qty("Blum slepý"), 2, "{bill}");
    assert!(qty("Konfirmát") > 0, "{bill}");
    assert_eq!(qty("Häfele podperka"), 4, "{bill}");
    let _ = std::fs::remove_dir_all(&dir);
}
