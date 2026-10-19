//! Polohovateľné police iba v otvore s dvierkami alebo otvorenom (ako Interior otvoru v CV).

use std::path::PathBuf;

use cc_generate::build;
use cc_model::{CabinetSpec, FrontKind, FrontSpec, Model, Part, Role};
use glam::DVec3;

fn kitchen() -> cc_config::LoadedProject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/dielna/kuchyna.toml");
    cc_config::load(&path).expect("vzorová kuchyňa sa musí načítať")
}

/// Skrinka SP3 vzorovej kuchyne s danými čelami a 4 policami.
fn with_fronts(fronts: Vec<FrontSpec>) -> (Model, CabinetSpec) {
    let mut k = kitchen();
    let c = k
        .project
        .cabinets
        .iter_mut()
        .find(|c| c.id == "SP3")
        .expect("SP3 vo vzorovej kuchyni");
    c.fronts = fronts;
    c.adjustable_shelves = 4;
    let spec = c.clone();
    let model = build(&k.project, &k.standard, &k.materials).expect("model sa postaví");
    (model, spec)
}

fn shelves(m: &Model) -> Vec<&Part> {
    m.cabinets
        .iter()
        .find(|c| c.id == "SP3")
        .map(|c| {
            c.parts
                .iter()
                .filter(|p| p.role == Role::AdjustableShelf)
                .collect()
        })
        .unwrap_or_default()
}

/// Výška hornej plochy police v skrinke.
fn top(p: &Part) -> f64 {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    a.z.max(b.z)
}

#[test]
fn drawers_only_get_no_shelves() {
    let (m, spec) = with_fronts(vec![
        FrontSpec::new(FrontKind::Drawer),
        FrontSpec::new(FrontKind::Drawer),
    ]);
    assert!(shelves(&m).is_empty());
    assert!(!spec.has_shelf_opening());
}

#[test]
fn shelves_stay_behind_the_door_below_a_drawer() {
    let (m, spec) = with_fronts(vec![
        FrontSpec {
            height: Some(150.0),
            ..FrontSpec::new(FrontKind::Drawer)
        },
        FrontSpec::new(FrontKind::Door),
    ]);
    assert_eq!(spec.shelf_rows(), [false, true]);
    let s = shelves(&m);
    assert_eq!(s.len(), 4);
    let cab = m.cabinets.iter().find(|c| c.id == "SP3").unwrap();
    // čelo zásuvky 150 hore: police pod spodkom jej čela
    let drawer = cab
        .parts
        .iter()
        .find(|p| p.role == Role::DrawerFront)
        .unwrap();
    let a = drawer.transform.point_to_cabinet(DVec3::ZERO);
    let b = drawer.transform.point_to_cabinet(DVec3::new(
        drawer.length,
        drawer.width,
        drawer.thickness,
    ));
    let drawer_bottom = a.z.min(b.z);
    for p in &s {
        assert!(
            top(p) < drawer_bottom,
            "polica {} nad {drawer_bottom}",
            top(p)
        );
    }
    // rovnaké otvory: police vzostupne a v rovnakých rozostupoch
    let mut tops: Vec<f64> = s.iter().map(|p| top(p)).collect();
    tops.sort_by(f64::total_cmp);
    let d: Vec<f64> = tops.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(d.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6), "{tops:?}");
}

#[test]
fn door_cabinet_keeps_full_height() {
    let (m, spec) = with_fronts(vec![FrontSpec::new(FrontKind::Door)]);
    assert!(spec.has_shelf_opening());
    let s = shelves(&m);
    assert_eq!(s.len(), 4);
    let cab = m.cabinets.iter().find(|c| c.id == "SP3").unwrap();
    // najvyššia polica je v hornej pätine korpusu (celý vnútrajšok, nie iba časť)
    let max = s.iter().map(|p| top(p)).fold(f64::NEG_INFINITY, f64::max);
    assert!(max > cab.h * 0.7, "{max} pri výške {}", cab.h);
}
