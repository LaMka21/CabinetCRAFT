//! Režim nesting (SCM nesting + CX100): golden súbory, zhoda so vzormi tabúľ a bezpečnosť dráh.
//!
//! Golden súbory: `tests/golden/dielna-nesting/<projekt>/{nesting,xcs}`. Zámerná zmena výstupu:
//! `CABINETCRAFT_BLESS=1 cargo test -p cc-out-xcs --test nesting`, potom rozdiel skontrolovať
//! riadok po riadku.
//!
//! Vzory programov tabúľ nie sú v repe (rozhodnutie vlastníka): číta sa
//! `CABINETCRAFT_NEST_FIXTURES`, `tests/fixtures/scm-nesting` alebo
//! `F:\AI\CabinetCRAFT\cabinetcraft\tests\fixtures\scm-nesting`; ak chýbajú, test sa preskočí.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cc_model::{MachineProfile, Model, ProductionMode};
use cc_out_xcs::nest::{render_nesting, NestOutput};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn nest_fixtures() -> Option<PathBuf> {
    [
        std::env::var_os("CABINETCRAFT_NEST_FIXTURES").map(PathBuf::from),
        Some(root().join("tests/fixtures/scm-nesting")),
        Some(PathBuf::from(
            r"F:\AI\CabinetCRAFT\cabinetcraft\tests\fixtures\scm-nesting",
        )),
    ]
    .into_iter()
    .flatten()
    .find(|d| d.join("nest").is_dir())
}

fn machine(name: &str) -> MachineProfile {
    cc_config::load_machine(&root().join("examples/dielna").join(name)).unwrap()
}

fn model(project: &str) -> Model {
    let loaded = cc_config::load(&root().join("examples/dielna").join(project)).unwrap();
    let model = cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
    cc_joints::apply(model, &loaded.standard).unwrap().model
}

fn render(project: &str) -> (Model, NestOutput, MachineProfile) {
    let m = machine("machine-nesting.toml");
    let model = model(project);
    let out = render_nesting(&model, m.xcs.as_ref().unwrap(), m.nesting.as_ref().unwrap()).unwrap();
    (model, out, m)
}

/// Všetky súbory výstupu: relatívna cesta → obsah.
fn files(out: &NestOutput) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    for s in &out.sheets {
        map.insert(format!("nesting/{}", s.name), s.content.clone());
    }
    if let Some((name, content)) = &out.cnc_run {
        map.insert(format!("nesting/{name}"), content.clone());
    }
    for f in &out.cx100.files {
        map.insert(format!("xcs/{}", f.name), f.content.clone());
    }
    if let Some((name, content)) = &out.cx100.run_bat {
        map.insert(format!("xcs/{name}"), content.clone());
    }
    map
}

fn read_dir_files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    for sub in ["nesting", "xcs"] {
        let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for e in entries {
            let p = e.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            map.insert(format!("{sub}/{name}"), std::fs::read(&p).unwrap());
        }
    }
    map
}

fn check_golden(project: &str, golden: &str) {
    let (_, out, _) = render(project);
    let ours = files(&out);
    let dir = root().join("tests/golden/dielna-nesting").join(golden);
    if std::env::var_os("CABINETCRAFT_BLESS").is_some() {
        let _ = std::fs::remove_dir_all(&dir);
        for (path, content) in &ours {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
    }
    let expected = read_dir_files(&dir);
    assert_eq!(
        ours.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "zoznam súborov {golden}"
    );
    for (path, content) in &ours {
        assert!(
            *content == expected[path],
            "{golden}/{path} sa líši od golden súboru"
        );
    }
}

#[test]
fn project_matches_golden_files() {
    check_golden("project.toml", "project");
}

#[test]
fn kuchyna_matches_golden_files() {
    check_golden("kuchyna.toml", "kuchyna");
}

#[test]
fn nesting_profile_differs_from_cx100_profile_only_in_mode() {
    let a = machine("machine.toml");
    let mut b = machine("machine-nesting.toml");
    assert_eq!(a.production.mode, ProductionMode::Cx100);
    assert_eq!(b.production.mode, ProductionMode::Nesting);
    b.production.mode = ProductionMode::Cx100;
    assert_eq!(a, b);
}

#[test]
fn deterministic_output() {
    let (_, a, _) = render("kuchyna.toml");
    let (_, b, _) = render("kuchyna.toml");
    assert_eq!(files(&a), files(&b));
}

/// CX100 dostane iba zvyšnú prácu: žiadne vŕtanie plochy, ktorú spravil nesting.
#[test]
fn cx100_gets_only_remaining_work() {
    let (model, out, m) = render("kuchyna.toml");
    let full = cc_out_xcs::render(&model, m.xcs.as_ref().unwrap());
    assert!(out.cx100.files.len() <= full.files.len());
    let mut nest_holes = 0;
    for part in model.parts() {
        let plan = &out.plan.parts[&part.id];
        nest_holes += plan.nest_ops.len();
        let has_program = out.cx100.files.iter().any(|f| f.part_id == part.id);
        assert_eq!(has_program, !plan.cx100_ops.is_empty(), "{}", part.id);
    }
    assert!(nest_holes > 0);
    // bočnica hornej skrinky (ako CV 966b0060): na CX100 iba kolíky do hornej hrany pod vrch,
    // drážku nemá (končí pred chrbtom); drážku chrbta má vrch. Spodné boky na CX100 nejdú.
    assert!(!out.cx100.files.iter().any(|f| f.name == "SP1-SIDE-L.xcs"));
    let text_of = |name: &str| {
        let f = out.cx100.files.iter().find(|f| f.name == name).unwrap();
        String::from_utf8(f.content.clone()).unwrap()
    };
    let side = text_of("HO1-SIDE-L.xcs");
    assert!(side.contains("CreateDrill("));
    assert!(!side.contains("SelectWorkplane(\"Top\")"));
    assert!(!side.contains("CreateSlot("));
    assert_eq!(text_of("HO1-TOP.xcs").matches("CreateSlot(").count(), 1);
    // RUN.bat obsahuje iba programy CX100, nie tabule
    let bat = String::from_utf8(out.cx100.run_bat.as_ref().unwrap().1.clone()).unwrap();
    assert!(!bat.contains("N.xcs"));
    assert_eq!(bat.matches("XConverter.exe").count(), out.cx100.files.len());
}

#[test]
fn unverified_templates_are_reported() {
    let (_, out, _) = render("project.toml");
    for t in [
        "nesting.header",
        "nesting.CreateDrill",
        "nesting.CreateRoughFinish",
    ] {
        assert!(out.unverified.contains(t), "{t}: {:?}", out.unverified);
    }
}

#[test]
fn cnc_run_is_utf16_with_bom_and_crlf() {
    let (_, out, _) = render("project.toml");
    let (name, bytes) = out.cnc_run.as_ref().unwrap();
    assert_eq!(name, "CncRun.txt");
    assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
    let units: Vec<u16> = bytes[2..]
        .chunks(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16(&units).unwrap();
    assert!(text.starts_with("^Job        : Dielna"));
    // DTDL 18 a MDF 19 (vrch hornej skrinky)
    assert!(text.contains("\r\n^Sheets      : 2\r\n"));
    assert!(text.contains("0101N,Nest Sheet 1,2070,2800,18,DTDL_18_W960_biela,0,1,0,F,0,\r\n"));
    let parts = text.lines().filter(|l| l.contains(",CX1,CX100")).count();
    assert_eq!(parts, out.cx100.files.len());
}

// ---------------------------------------------------------------------------
// Rozbor programu tabule
// ---------------------------------------------------------------------------

/// Rozdelí volanie `Meno(a,"b,c",d);` na meno a argumenty.
fn parse_call(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") {
        return None;
    }
    let open = line.find('(')?;
    let close = line.rfind(')')?;
    assert!(
        line[close..].trim() == ");",
        "príkaz musí končiť `);`: {line}"
    );
    let name = line[..open].trim().to_owned();
    let inner = &line[open + 1..close];
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    for c in inner.chars() {
        match c {
            '"' => {
                in_str = !in_str;
                cur.push(c);
            }
            ',' if !in_str => {
                args.push(cur.trim().to_owned());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !inner.trim().is_empty() {
        args.push(cur.trim().to_owned());
    }
    Some((name, args))
}

/// Dráha: body (x, y, hĺbka), nástroj, korekcia, Ø.
#[derive(Debug, Default)]
struct Path_ {
    pts: Vec<(f64, f64, f64)>,
    comp: String,
    d: f64,
    z: f64,
}

#[derive(Debug, Default)]
struct SheetProgram {
    l: f64,
    w: f64,
    t: f64,
    drills: Vec<(f64, f64, f64, f64)>,
    paths: Vec<Path_>,
    parked: bool,
}

fn num(s: &str) -> f64 {
    s.parse::<f64>().unwrap()
}

fn parse_sheet(text: &str) -> SheetProgram {
    let mut p = SheetProgram::default();
    let mut cur: Option<Path_> = None;
    for line in text.lines() {
        let Some((name, a)) = parse_call(line) else {
            continue;
        };
        match name.as_str() {
            "CreateFinishedWorkpieceBox" => {
                p.l = num(&a[1]);
                p.w = num(&a[2]);
                p.t = num(&a[3]);
            }
            "SelectWorkplane" => assert_eq!(a[0], "\"Top\"", "nesting má iba rovinu Top"),
            "CreateDrill" => p
                .drills
                .push((num(&a[1]), num(&a[2]), num(&a[3]), num(&a[4]))),
            "CreatePolyline" => {
                assert!(cur.is_none());
                cur = Some(Path_ {
                    pts: vec![(num(&a[1]), num(&a[2]), f64::NAN)],
                    ..Path_::default()
                });
            }
            "AddSegmentToPolyline" => {
                cur.as_mut()
                    .unwrap()
                    .pts
                    .push((num(&a[0]), num(&a[1]), f64::NAN));
            }
            "SetAttribute" => {
                let pts = &mut cur.as_mut().unwrap().pts;
                pts.last_mut().unwrap().2 = num(&a[1]);
            }
            "CreateRoughFinish" => {
                let mut path = cur.take().unwrap();
                path.z = num(&a[1]);
                path.comp = a[6].clone();
                path.d = num(&a[7]);
                p.paths.push(path);
            }
            "CreatePark" => p.parked = true,
            _ => {}
        }
    }
    assert!(cur.is_none(), "neukončená dráha");
    p
}

/// Bezpečnosť: diery v dieloch, obrysy po hrane dielu, fréza nezasiahne susedný diel,
/// odrezanie zvyšku nad dielmi, hĺbky podľa hrúbky.
#[test]
fn sheet_programs_are_geometrically_safe() {
    for project in ["project.toml", "kuchyna.toml"] {
        let (_, out, m) = render(project);
        let prof = m.nesting.as_ref().unwrap();
        for (sheet, file) in out
            .plan
            .groups
            .iter()
            .flat_map(|g| g.sheets.iter())
            .zip(&out.sheets)
        {
            let text = String::from_utf8(file.content.clone()).unwrap();
            let p = parse_sheet(&text);
            assert!(p.parked, "{}", file.name);
            let rects: Vec<(f64, f64, f64, f64)> = sheet
                .parts
                .iter()
                .map(|q| (q.x, q.y, q.x + q.dx, q.y + q.dy))
                .collect();
            // diera celá v jednom diele, hĺbka < hrúbka (slepé diery)
            for &(x, y, depth, d) in &p.drills {
                let r = d / 2.0;
                assert!(
                    rects.iter().any(|&(x0, y0, x1, y1)| x - r >= x0 - 1e-6
                        && x + r <= x1 + 1e-6
                        && y - r >= y0 - 1e-6
                        && y + r <= y1 + 1e-6),
                    "{}: diera ({x}, {y}) Ø{d} mimo dielu",
                    file.name
                );
                assert!(depth > 0.0 && depth < p.t, "{}: hĺbka {depth}", file.name);
            }
            let through = p.t + prof.route.through_extra;
            let skin = p.t - prof.route.skin;
            let mut contours = 0;
            for path in &p.paths {
                assert_eq!(path.d, prof.route.tool_diameter);
                let r = path.d / 2.0;
                assert!(path.pts.iter().all(|q| q.2.is_finite()), "bod bez hĺbky");
                if path.comp == "0" {
                    // odrezanie zvyšku: vodorovná priamka cez celú tabuľu nad dielmi
                    let y = path.pts[0].1;
                    assert!(path.pts.iter().all(|q| q.1 == y));
                    let top = rects.iter().map(|q| q.3).fold(0.0, f64::max);
                    assert!(y - r > top, "{}: rez zvyšku zasiahne diel", file.name);
                    assert!(path.pts[0].0 >= p.l + r && path.pts[1].0 <= -r);
                    assert_eq!(path.z, through);
                    continue;
                }
                assert_eq!(path.comp, "1");
                assert!(path.z == through || path.z == skin, "hĺbka {}", path.z);
                // nábeh nad materiálom, ostatné body v plnej hĺbke
                assert!(path.pts[0].2 < 0.0);
                assert!(path.pts[1..].iter().all(|q| q.2 == path.z));
                // všetky body ležia na hranici jedného dielu
                let on = |q: &(f64, f64, f64), &(x0, y0, x1, y1): &(f64, f64, f64, f64)| {
                    let inside_x = q.0 >= x0 - 1e-6 && q.0 <= x1 + 1e-6;
                    let inside_y = q.1 >= y0 - 1e-6 && q.1 <= y1 + 1e-6;
                    ((q.0 - x0).abs() < 1e-6 || (q.0 - x1).abs() < 1e-6) && inside_y
                        || ((q.1 - y0).abs() < 1e-6 || (q.1 - y1).abs() < 1e-6) && inside_x
                };
                let owner = rects
                    .iter()
                    .find(|rc| path.pts.iter().all(|q| on(q, rc)))
                    .unwrap_or_else(|| panic!("{}: dráha nie je po hrane dielu", file.name));
                // obeh v smere hodín (korekcia vľavo = mimo dielu): záporná orientovaná plocha
                let mut area2 = 0.0;
                for w in path.pts[1..7].windows(2) {
                    area2 += w[0].0 * w[1].1 - w[1].0 * w[0].1;
                }
                assert!(area2 < 0.0, "{}: obrys nie je v smere hodín", file.name);
                // fréza ide mimo dielu v páse šírky D: iné diely musia byť ďalej ako D
                for other in rects.iter().filter(|o| *o != owner) {
                    let dx = (other.0 - owner.2).max(owner.0 - other.2).max(0.0);
                    let dy = (other.1 - owner.3).max(owner.1 - other.3).max(0.0);
                    assert!(
                        dx.max(dy) > path.d + 1e-6,
                        "{}: fréza Ø{} zasiahne susedný diel",
                        file.name,
                        path.d
                    );
                }
                // prekrytie za štartom aspoň Ø frézy (po obryse)
                if path.z == through {
                    contours += 1;
                }
            }
            assert_eq!(
                contours,
                sheet.parts.len(),
                "{}: každý diel má obrys",
                file.name
            );
        }
    }
}

/// Každý príkaz programu tabule je funkcia zo vzorov nestingu s rovnakým počtom argumentov.
#[test]
fn sheet_programs_match_reference_programs() {
    let Some(dir) = nest_fixtures() else {
        return;
    };
    let mut funcs: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut planes = BTreeSet::new();
    for entry in std::fs::read_dir(dir.join("nest")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "xcs") {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        for line in text.lines() {
            if let Some((name, args)) = parse_call(line) {
                if name == "SelectWorkplane" {
                    planes.insert(args[0].clone());
                }
                funcs.entry(name).or_default().insert(args.len());
            }
        }
    }
    assert!(funcs.contains_key("CreateRoughFinish"));
    for project in ["project.toml", "kuchyna.toml"] {
        let (_, out, _) = render(project);
        assert!(!out.sheets.is_empty());
        for file in &out.sheets {
            assert!(file.content.is_ascii());
            let text = String::from_utf8(file.content.clone()).unwrap();
            assert!(text.contains("\r\n"), "{}: očakáva sa CRLF", file.name);
            for line in text.lines() {
                let Some((name, args)) = parse_call(line) else {
                    continue;
                };
                let allowed = funcs.get(&name).unwrap_or_else(|| {
                    panic!("{}: funkcia `{name}` sa vo vzoroch nevyskytuje", file.name)
                });
                assert!(
                    allowed.contains(&args.len()),
                    "{}: `{name}` má {} argumentov, vzory {allowed:?}",
                    file.name,
                    args.len()
                );
                if name == "SelectWorkplane" {
                    assert!(
                        planes.contains(&args[0]),
                        "{}: rovina {}",
                        file.name,
                        args[0]
                    );
                }
                for a in &args {
                    if a.parse::<f64>().is_ok() {
                        assert!(!a.contains(['e', 'E', ',']), "{}: číslo `{a}`", file.name);
                    }
                }
            }
        }
    }
}

/// Riadky, ktoré sú vo vzoroch konštantné, píšeme doslovne rovnako.
#[test]
fn constant_lines_equal_reference() {
    let Some(dir) = nest_fixtures() else {
        return;
    };
    let reference = String::from_utf8_lossy(&std::fs::read(dir.join("nest/9660401N.xcs")).unwrap())
        .into_owned();
    let ref_lines: BTreeSet<&str> = reference.lines().collect();
    let (_, out, _) = render("project.toml");
    let text = String::from_utf8(out.sheets[0].content.clone()).unwrap();
    for line in [
        "SetMachiningParameters(\"AD\",1,10,16973824,false);",
        "SetWorkpieceSetupPosition(0,0, 0, 0);",
        "SetApproachStrategy(true, false, 2,120);",
        "SetApproachStrategy(true, false, 0.5);",
        "CreatePark(\"Parkovani\",\"Nothing\",false);",
        "CreateRoughFinish(\"Frezovani 1001 \",14,\"\", TypeOfProcess.GeneralRouting ,\"E002\",\"-1\",1,12,\"\",\"\",0);",
    ] {
        assert!(ref_lines.contains(line), "vzor neobsahuje {line}");
    }
    for line in [
        "SetMachiningParameters(\"AD\",1,10,16973824,false);",
        "SetWorkpieceSetupPosition(0,0, 0, 0);",
        "SetApproachStrategy(true, false, 2,120);",
        "CreatePark(\"Parkovani\",\"Nothing\",false);",
    ] {
        assert!(
            text.lines().any(|l| l == line),
            "náš program neobsahuje {line}"
        );
    }
    // vŕtanie rovnako ako vo vzore 9660301N (s medzerou na začiatku)
    let drill = text.lines().find(|l| l.contains("CreateDrill(")).unwrap();
    assert!(drill.starts_with(" CreateDrill(\"Vrtani 1\","));
    assert!(
        drill.ends_with(",\"\",TypeOfProcess.Drilling,\"-1\",\"-1\",0,\"-1\",\"-1\",\"P\",0,0.5);")
    );
}
