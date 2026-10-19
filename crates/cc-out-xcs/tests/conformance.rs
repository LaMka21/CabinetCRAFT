//! Zhoda formátu s reálnymi programmi CV pre CX100 (`tests/fixtures/scm-cx100`).
//!
//! Každý príkaz nášho výstupu musí byť funkcia, ktorá sa vo vzoroch vyskytuje,
//! s rovnakým počtom argumentov; roviny musia byť tie, ktoré používajú vzory.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Rozdelí volanie `Meno(a,"b,c",d);` na meno a argumenty (čiarky v úvodzovkách sa nerátajú).
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

/// Funkcia → povolené počty argumentov, a množina rovín zo vzorov.
fn reference() -> (BTreeMap<String, BTreeSet<usize>>, BTreeSet<String>) {
    let mut funcs: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut planes = BTreeSet::new();
    let dir = root().join("tests/fixtures/scm-cx100");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "xcs") {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).into_owned();
        for line in text.lines() {
            if let Some((name, args)) = parse_call(line) {
                if name == "SelectWorkplane" {
                    planes.insert(args[0].trim_matches('"').to_owned());
                }
                funcs.entry(name).or_default().insert(args.len());
            }
        }
    }
    assert!(
        funcs.contains_key("CreateDrill"),
        "vzory musia obsahovať CreateDrill"
    );
    (funcs, planes)
}

/// Výstupy oboch vzorových konfigurácií: `s1` (špecifikácia) a `dielna` (štandard dielne).
fn our_outputs() -> Vec<cc_out_xcs::XcsOutput> {
    ["s1", "dielna"]
        .into_iter()
        .map(|example| {
            let dir = root().join("examples").join(example);
            let loaded = cc_config::load(&dir.join("project.toml")).unwrap();
            let machine = cc_config::load_machine(&dir.join("machine.toml")).unwrap();
            let model =
                cc_generate::build(&loaded.project, &loaded.standard, &loaded.materials).unwrap();
            let model = cc_joints::apply(model, &loaded.standard).unwrap().model;
            cc_out_xcs::render(&model, machine.xcs.as_ref().unwrap())
        })
        .collect()
}

#[test]
fn every_statement_matches_reference_programs() {
    let (funcs, planes) = reference();
    let outputs = our_outputs();
    let files: Vec<_> = outputs.iter().flat_map(|o| o.files.iter()).collect();
    assert!(files.len() > 10);
    for file in files {
        assert!(file.content.is_ascii(), "{} nie je ASCII", file.name);
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
                "{}: `{name}` má {} argumentov, vzory majú {allowed:?}: {line}",
                file.name,
                args.len()
            );
            if name == "SelectWorkplane" {
                let plane = args[0].trim_matches('"');
                assert!(
                    planes.contains(plane),
                    "{}: neznáma rovina `{plane}`",
                    file.name
                );
            }
            // číselné argumenty nesmú mať exponent ani čiarku
            for a in &args {
                if a.parse::<f64>().is_ok() {
                    assert!(
                        !a.contains(['e', 'E', ',']),
                        "{}: zlé číslo `{a}`",
                        file.name
                    );
                }
            }
        }
    }
}

#[test]
fn reference_parser_sanity() {
    let (name, args) = parse_call(
        r#"CreateDrill("Vrtani 1 DIA=5 Z=13",567,322.5,13,5,"",TypeOfProcess.Drilling,"-1","-1",0,"-1","-1","P",0,0.5);"#,
    )
    .unwrap();
    assert_eq!(name, "CreateDrill");
    assert_eq!(args.len(), 15);
    assert_eq!(args[1], "567");
    let (_, args) = parse_call(r#"SetWorkpieceSetupPosition(0,0, 0, 0);"#).unwrap();
    assert_eq!(args.len(), 4);
    let (_, args) = parse_call("ResetPneumaticHood();").unwrap();
    assert!(args.is_empty());
}
