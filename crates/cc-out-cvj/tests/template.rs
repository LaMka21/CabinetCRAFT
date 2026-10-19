//! Skúšky nad skutočnou zákazkou CV (súbor nie je v repe): cesta v `CABINETCRAFT_CVJ_REF`
//! alebo `CABINETCRAFT_CVJ`. Bez premennej sa preskočia.

use std::io::Cursor;
use std::path::PathBuf;

fn sample() -> Option<PathBuf> {
    std::env::var_os("CABINETCRAFT_CVJ_REF")
        .or_else(|| std::env::var_os("CABINETCRAFT_CVJ"))
        .map(PathBuf::from)
}

fn standard() -> cc_model::Standard {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/dielna/standards/dielna-cx100.toml");
    toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Nezmenený `Contents` zapísaný nanovo (nové značky) je bajtovo zhodný a kontajner
/// zapísaný späť obsahuje všetky prúdy bez zmeny.
#[test]
fn template_round_trip() {
    let Some(path) = sample() else {
        return;
    };
    let t = cc_out_cvj::Template::open(&path).unwrap();
    let again = cc_out_cvj::emit::rewrite(&t.contents, &t.events).unwrap();
    assert!(again == t.contents, "Contents po prepise nie je zhodný");
    assert!(t.max_object_id() > 0);

    let mut buf = Cursor::new(Vec::new());
    cc_out_cvj::write_cvj(&mut buf, &t, &again, &[]).unwrap();
    buf.set_position(0);
    let back = cc_out_cvj::Template::read_from(buf).unwrap();
    assert_eq!(back.root_clsid, t.root_clsid);
    assert_eq!(back.streams.len(), t.streams.len());
    for ((n1, d1), (n2, d2)) in t.streams.iter().zip(&back.streams) {
        assert_eq!(n1, n2);
        assert!(d1 == d2, "prúd {n1} sa zmenil");
    }
}

/// Projekt z importu zákazky → zúžená skrinka → export podľa tej istej zákazky ako šablóny →
/// výstup sa dá prečítať a skrinky majú rozmery a polohy projektu.
#[test]
fn export_round_trip_with_change() {
    let Some(path) = sample() else {
        return;
    };
    let job = cc_import_cvj::read_job(&path).unwrap();
    let defaults = cc_model::Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    };
    let (mut project, _) = cc_import_cvj::to_project(&job, &defaults);
    let base = project.clone();
    let before: Vec<&cc_import_cvj::CvjCabinet> =
        job.rooms.iter().flat_map(|r| r.cabinets.iter()).collect();
    // jedna spodná skrinka o 100 užšia
    let target = project
        .cabinets
        .iter_mut()
        .filter(|c| c.wall.is_some() && c.cabinet_type == cc_model::CabinetType::Base)
        .last()
        .unwrap();
    target.w -= 100.0;
    let (tid, tw) = (target.id.clone(), target.w);
    // nová skrinka bez pôvodu v CV a bez katalógu: zapíše sa bez väzby na katalóg
    let mut fresh = target.clone();
    fresh.id = "NOVA".into();
    fresh.cv_id = None;
    fresh.catalog = None;
    fresh.w = 450.0;
    fresh.at = 5.0;
    fresh.fronts = vec![cc_model::FrontSpec::new(cc_model::FrontKind::Pair)];
    // delenie čiel z projektu: iná spodná skrinka s dvierkami dostane tri zásuvky ako VARNA,
    // horná s pántmi vľavo pánty vpravo
    use cc_model::{FrontKind, FrontSpec, HingeSide};
    let drawers = project
        .cabinets
        .iter_mut()
        .find(|c| {
            c.id != tid
                && !c.catalog.as_deref().unwrap_or("").contains("SIKMA")
                && c.cabinet_type == cc_model::CabinetType::Base
                && c.fronts.len() == 1
                && c.fronts[0].kind == FrontKind::Door
                && c.fronts[0].columns.is_empty()
                && c.wall.is_some()
        })
        .unwrap();
    drawers.fronts = vec![
        FrontSpec {
            height: Some(160.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
        FrontSpec::new(FrontKind::Drawer),
        FrontSpec::new(FrontKind::Drawer),
    ];
    let drawers_id = drawers.id.clone();
    let flipped = project
        .cabinets
        .iter_mut()
        .find(|c| {
            c.cabinet_type == cc_model::CabinetType::Wall
                && c.fronts.len() == 1
                && c.fronts[0].hinge_side == Some(HingeSide::Left)
        })
        .unwrap();
    flipped.fronts[0].hinge_side = Some(HingeSide::Right);
    let flipped_id = flipped.id.clone();
    // import v aplikácii zjednoduší VARNA (čelo 130 pod minimom stroja): výšky čiel a korpusov
    // preč, odtlačok čiel sa obnoví → delenie čiel ostane zo zákazky CV
    let varna = project
        .cabinets
        .iter_mut()
        .find(|c| c.catalog.as_deref() == Some("S_KUCH_VARNA_D"))
        .unwrap();
    for f in &mut varna.fronts {
        f.height = None;
        f.box_height = None;
    }
    varna.cv_fronts = Some(varna.front_fingerprint());
    let varna_id = varna.id.clone();
    // okno o 100 vľavo a o 66 užšie
    let window = project.openings.first_mut().unwrap();
    window.at -= 100.0;
    window.width -= 66.0;
    let window = window.clone();
    // súbor pre skúšku v CV bez novej skrinky (tá by stála cez rohovú)
    let for_owner = project.clone();
    project.cabinets.push(fresh);

    let t = cc_out_cvj::Template::open(&path).unwrap();
    let ex = cc_out_cvj::export::export_project(&project, &standard(), &t).unwrap();
    assert!(!ex.cabinets.is_empty());

    // výstup prečítame importérom; kostry nemajú diely, kontrola ide z parametrov CV
    let back = cc_import_cvj::parse_contents(&ex.contents).unwrap();
    let all: Vec<&cc_import_cvj::CvjCabinet> =
        back.rooms.iter().flat_map(|r| r.cabinets.iter()).collect();
    let written: Vec<&str> = ex.cabinets.iter().map(|(id, _)| id.as_str()).collect();
    assert!(
        written.contains(&tid.as_str()),
        "{written:?} {:?}",
        ex.notes
    );
    for (id, cabno) in &ex.cabinets {
        let c = project.cabinets.iter().find(|c| &c.id == id).unwrap();
        let cv = all
            .iter()
            .find(|k| k.number == Some(i64::from(*cabno)))
            .unwrap_or_else(|| panic!("CABNO {cabno} ({id}) chýba vo výstupe"));
        let wall = c.wall.as_deref().unwrap().trim_start_matches('W');
        assert_eq!(
            cv.wall.map(|w| w.to_string()).as_deref(),
            Some(wall),
            "{id}"
        );
        // CV mení rozmer o zmenu našej skrinky oproti importu (import berie šikmé skrinky
        // z obrysu dielov); nová skrinka má rozmer projektu
        let (want_x, want_z, want_at) = match (
            base.cabinets.iter().find(|b| &b.id == id),
            before.iter().find(|k| k.number == Some(i64::from(*cabno))),
        ) {
            (Some(b), Some(o)) => (
                o.size.x + c.w - b.w,
                o.size.z + c.d - b.d,
                o.position.x + c.at - b.at,
            ),
            _ => (c.w, c.d, c.at),
        };
        assert!(
            (cv.size.x - want_x).abs() < 0.01,
            "{id}: DX {} vs {want_x}",
            cv.size.x
        );
        assert!(
            (cv.size.z - want_z).abs() < 0.01,
            "{id}: DZ {} vs {want_z}",
            cv.size.z
        );
        assert!(
            (cv.position.x - want_at).abs() < 0.01,
            "{id}: X {} vs {want_at}",
            cv.position.x
        );
    }
    assert!(all.iter().any(|k| (k.size.x - tw).abs() < 0.01));

    // NOVA: meno CC_SPODNA, väzba na katalóg 0, nominál = skutočný rozmer
    fn find<'a>(
        n: &'a cc_import_cvj::classes::Node,
        name: &str,
    ) -> Option<&'a cc_import_cvj::classes::Node> {
        if n.name == name {
            return Some(n);
        }
        n.children.iter().find_map(|c| find(c, name))
    }
    let out_walk0 = cc_import_cvj::classes::walk(&ex.contents).unwrap();
    let nova = find(&out_walk0.root, "CC_SPODNA").expect("CC_SPODNA vo výstupe");
    let link_at = nova.link_at.unwrap();
    assert_eq!(&ex.contents[link_at..link_at + 4], &[0, 0, 0, 0]);
    let nom = nova.nominal_at.unwrap();
    let dx = f64::from_le_bytes(ex.contents[nom..nom + 8].try_into().unwrap()) * 25.4;
    assert!((dx - 450.0).abs() < 1e-6, "nominál {dx}");

    // doplnky bez PID (sokel, LED rampa, krycí bok…) ostanú: CV ich pri prestavbe nevytvorí
    fn extras(n: &cc_import_cvj::classes::Node, out: &mut Vec<String>) {
        if n.params.iter().all(|p| p.name != "PID")
            && ["SOKEL", "LED RAMPA", "KRYCI BOK"]
                .iter()
                .any(|d| n.description.contains(d))
        {
            out.push(n.description.clone());
        }
        for c in &n.children {
            extras(c, out);
        }
    }
    let (mut before, mut after) = (Vec::new(), Vec::new());
    extras(&t.walk.root, &mut before);
    let out_walk = cc_import_cvj::classes::walk(&ex.contents).unwrap();
    extras(&out_walk.root, &mut after);
    before.sort();
    after.sort();
    assert_eq!(before, after);

    // súbor zapísaný pre ručnú skúšku v CV (mimo repa), ak je zadaný priečinok
    // sekcie z projektu: tri zásuvky (FO, DWR, MR, FO, DWR, MR, DWR), pánty vpravo, Pair
    fn face_of(cab: &cc_import_cvj::classes::Node) -> Vec<(String, i32, i32, i32, f64)> {
        let int = |n: &cc_import_cvj::classes::Node, k: &str| match n.param(k) {
            Some(cc_import_cvj::classes::Value::Int(v)) => *v,
            _ => -1,
        };
        let face = cab
            .children
            .iter()
            .find(|g| g.name.starts_with("Face") && g.param("ID").is_none())
            .unwrap();
        face.children
            .iter()
            .map(|s| {
                let dy = match s.param("DY") {
                    Some(cc_import_cvj::classes::Value::Length(v)) => {
                        (v * 25.4 * 10.0).round() / 10.0
                    }
                    _ => -1.0,
                };
                (
                    s.name.clone(),
                    int(s, "ID"),
                    int(s, "STYPE"),
                    int(s, "HNG"),
                    dy,
                )
            })
            .collect()
    }
    fn by_cabno(
        n: &cc_import_cvj::classes::Node,
        no: i32,
    ) -> Option<&cc_import_cvj::classes::Node> {
        if n.class == "CVCabinet"
            && matches!(n.param("CABNO"), Some(cc_import_cvj::classes::Value::Int(v)) if *v == no)
        {
            return Some(n);
        }
        n.children.iter().find_map(|c| by_cabno(c, no))
    }
    let cabno = |id: &str| ex.cabinets.iter().find(|(i, _)| i == id).unwrap().1;
    let d = face_of(by_cabno(&out_walk.root, cabno(&drawers_id)).unwrap());
    let kinds: Vec<(&str, i32)> = d.iter().map(|s| (s.0.as_str(), s.2)).collect();
    assert_eq!(
        kinds,
        [
            ("FO", 0),
            ("DWR_OPEN", 2),
            ("MR", 0),
            ("FO", 0),
            ("DWR_OPEN", 2),
            ("MR", 0),
            ("DWR_OPEN", 2)
        ],
        "{d:?}"
    );
    let v = face_of(by_cabno(&out_walk.root, cabno(&varna_id)).unwrap());
    let heights: Vec<f64> = v
        .iter()
        .filter(|s| s.0 == "DWR_OPEN")
        .map(|s| s.4)
        .collect();
    assert_eq!(heights, [163.0, 300.0, 299.0], "{v:?}");
    let h = face_of(by_cabno(&out_walk.root, cabno(&flipped_id)).unwrap());
    assert_eq!(h[0].3, 2, "{h:?}");
    let pair = face_of(nova);
    assert_eq!(
        pair.iter()
            .map(|s| (s.0.as_str(), s.1, s.2))
            .collect::<Vec<_>>(),
        [("DOR_OPEN", 1, 4), ("DOR_OPEN", 1, 4)]
    );

    // steny bez zmeny: rovnaké hodnoty a konzistentné; okno na novom mieste, jedno
    assert_eq!(
        cc_out_cvj::walls::check(&out_walk.root, &ex.contents),
        Vec::<String>::new()
    );
    let back_job = cc_import_cvj::parse_contents(&ex.contents).unwrap();
    let (back_project, _) = cc_import_cvj::to_project(&back_job, &defaults);
    assert_eq!(back_project.walls, base.walls);
    assert_eq!(back_project.openings, vec![window]);
    // parapet (objekt CV s polohou a šírkou okna) ide s oknom
    let sill = |root: &cc_import_cvj::classes::Node| {
        fn find<'a>(
            n: &'a cc_import_cvj::classes::Node,
            name: &str,
        ) -> Option<&'a cc_import_cvj::classes::Node> {
            if n.class == "CVCabinet" && n.name.trim() == name {
                return Some(n);
            }
            n.children.iter().find_map(|c| find(c, name))
        }
        let d = find(root, "DOSKA DNO/VRCH").unwrap();
        let mm = |k: &str| match d.param(k) {
            Some(cc_import_cvj::classes::Value::Length(v)) => v * 25.4,
            _ => f64::NAN,
        };
        (mm("X"), mm("DX"))
    };
    let (x0, w0) = sill(&t.walk.root);
    let (x1, w1) = sill(&out_walk.root);
    assert!(
        (x1 - (x0 - 100.0)).abs() < 1e-6 && (w1 - (w0 - 66.0)).abs() < 1e-6,
        "{x1} {w1}"
    );

    if let Some(dir) = std::env::var_os("CABINETCRAFT_CVJ_OUT") {
        let ex = cc_out_cvj::export::export_project(&for_owner, &standard(), &t).unwrap();
        let file = std::path::Path::new(&dir).join("cabinetcraft_export.cvj");
        let f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file)
            .unwrap();
        cc_out_cvj::write_cvj(f, &t, &ex.contents, &["Header"]).unwrap();
        eprintln!(
            "zúžená skrinka {tid} na {tw}; zásuvky {drawers_id} (č. {}), pánty vpravo \
             {flipped_id} (č. {}); zapísané {:?}; poznámky {:?}; sekcie {d:?}",
            cabno(&drawers_id),
            cabno(&flipped_id),
            ex.cabinets,
            ex.notes
        );
    }
}

/// Uzavretá miestnosť `test_jedna` (findings 13 §11–13): steny 1 a 3 o 200 dlhšie, stena 2
/// posunutá, okno o 100 vpravo → steny, zadné plochy a spoje sedia so vzorcami CV a import
/// výstupu vráti náš projekt.
#[test]
fn walls_lengthened_in_closed_room() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_JEDNA").map(PathBuf::from) else {
        return;
    };
    let job = cc_import_cvj::read_job(&path).unwrap();
    let defaults = cc_model::Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    };
    let (mut project, _) = cc_import_cvj::to_project(&job, &defaults);
    let t = cc_out_cvj::Template::open(&path).unwrap();
    assert_eq!(
        cc_out_cvj::walls::check(&t.walk.root, &t.contents),
        Vec::<String>::new()
    );
    for w in &mut project.walls {
        match w.id.as_str() {
            "W1" => w.length += 200.0,
            "W2" => w.x += 200.0,
            "W3" => {
                w.x += 200.0;
                w.length += 200.0;
            }
            _ => {}
        }
    }
    project.openings[0].at += 100.0;
    let ex = cc_out_cvj::export::export_project(&project, &standard(), &t).unwrap();
    let walk = cc_import_cvj::classes::walk(&ex.contents).unwrap();
    assert_eq!(
        cc_out_cvj::walls::check(&walk.root, &ex.contents),
        Vec::<String>::new()
    );
    let back = cc_import_cvj::parse_contents(&ex.contents).unwrap();
    let (back, _) = cc_import_cvj::to_project(&back, &defaults);
    assert_eq!(back.walls, project.walls);
    assert_eq!(back.openings, project.openings);
    assert!(ex.notes.is_empty(), "{:?}", ex.notes);

    if let Some(dir) = std::env::var_os("CABINETCRAFT_CVJ_OUT") {
        let file = std::path::Path::new(&dir).join("test_jedna_cabinetcraft.cvj");
        let f = std::fs::File::create(&file).unwrap();
        cc_out_cvj::write_cvj(f, &t, &ex.contents, &["Header"]).unwrap();
    }
}

/// `test_jedna`: stena 4 zmazaná a nová stena na jej mieste so skrinkou, plus voľne stojaca
/// stena → miestnosť ostane uzavretá (čísla 1–4, susedia, kódy plôch, zadné plochy) a import
/// výstupu vráti geometriu projektu.
#[test]
fn walls_added_and_removed() {
    let Some(path) = std::env::var_os("CABINETCRAFT_CVJ_JEDNA").map(PathBuf::from) else {
        return;
    };
    let job = cc_import_cvj::read_job(&path).unwrap();
    let defaults = cc_model::Defaults {
        panel_material: "P".into(),
        back_material: "B".into(),
        construction: "C".into(),
    };
    let (mut project, _) = cc_import_cvj::to_project(&job, &defaults);
    let t = cc_out_cvj::Template::open(&path).unwrap();
    let old4 = project.walls.iter().position(|w| w.id == "W4").unwrap();
    let mut w5 = project.walls.remove(old4);
    w5.id = "W5".into();
    project.walls.push(w5.clone());
    // voľne stojaca stena mimo miestnosti
    let mut w6 = w5.clone();
    w6.id = "W6".into();
    w6.x -= 3000.0;
    project.walls.push(w6);
    // skrinka na novej stene
    let mut cab = project
        .cabinets
        .iter()
        .find(|c| c.wall.is_some())
        .unwrap()
        .clone();
    cab.id = "NA_W5".into();
    cab.wall = Some("W5".into());
    cab.at = 400.0;
    cab.cv_id = None;
    project.cabinets.retain(|c| c.wall.as_deref() != Some("W4"));
    project.cabinets.push(cab);
    // dvere na stene 2: šablóna má iba okno → obálka s TYPE 20
    project.openings.push(cc_model::OpeningSpec {
        wall: "W2".into(),
        kind: cc_model::OpeningKind::Door,
        at: 1000.0,
        width: 800.0,
        height: 2000.0,
        elevation: 0.0,
    });

    let ex = cc_out_cvj::export::export_project(&project, &standard(), &t).unwrap();
    let walk = cc_import_cvj::classes::walk(&ex.contents).unwrap();
    assert_eq!(
        cc_out_cvj::walls::check(&walk.root, &ex.contents),
        Vec::<String>::new()
    );
    let back = cc_import_cvj::parse_contents(&ex.contents).unwrap();
    let (back, _) = cc_import_cvj::to_project(&back, &defaults);
    let geo = |w: &cc_model::WallSpec| {
        (
            (w.x * 10.0).round(),
            (w.y * 10.0).round(),
            (w.length * 10.0).round(),
            (w.angle * 10.0).round(),
        )
    };
    let mut want: Vec<_> = project.walls.iter().map(geo).collect();
    let mut got: Vec<_> = back.walls.iter().map(geo).collect();
    want.sort_by(|a, b| a.partial_cmp(b).unwrap());
    got.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(got, want);
    assert!(
        back.openings
            .iter()
            .any(|o| o.kind == cc_model::OpeningKind::Door
                && o.wall == "W2"
                && (o.width - 800.0).abs() < 0.5
                && (o.height - 2000.0).abs() < 0.5),
        "{:?}",
        back.openings
    );
    // nová stena č. 4 (W5) so skrinkou, voľne stojaca č. 5
    let job2 = cc_import_cvj::parse_contents(&ex.contents).unwrap();
    let on4 = job2
        .rooms
        .iter()
        .flat_map(|r| r.cabinets.iter())
        .filter(|c| c.wall == Some(4))
        .count();
    assert!(on4 >= 1, "skrinka na novej stene chýba");
    assert_eq!(job2.rooms[0].walls.len(), 5);
    let w5 = job2.rooms[0].walls.iter().find(|w| w.number == 5).unwrap();
    assert_eq!((w5.prev, w5.next), (None, None));

    if let Some(dir) = std::env::var_os("CABINETCRAFT_CVJ_OUT") {
        let file = std::path::Path::new(&dir).join("test_jedna_steny.cvj");
        let f = std::fs::File::create(&file).unwrap();
        cc_out_cvj::write_cvj(f, &t, &ex.contents, &["Header"]).unwrap();
        eprintln!("poznámky {:?}", ex.notes);
    }
}
