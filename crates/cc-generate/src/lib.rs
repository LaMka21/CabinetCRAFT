//! Generovanie dielov zo zadania skriniek (špecifikácia MVP, kap. 5).
//!
//! Orientácia dielov (kap. 3.2, 3.3): plocha F je vždy plocha k interiéru
//! a predná hrana dielu je vždy `Y0`. Všetky sústavy sú pravotočivé.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

mod blenda;

use cc_i18n::tr;
use cc_model::{
    BackMount, CabinetModel, CabinetSpec, CabinetType, Edge, Edges, FrontKind, FrontMount,
    FrontOpening, Hardware, HingeSide, Material, MaterialLibrary, Model, Part, ProjectFile, Role,
    ShelfSpacing, Standard, TopStyle, Transform, EPS,
};
use glam::DVec3;

mod corner;
mod niche;
mod plinth;
mod slanted;

#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("{}", tr!("generate.error", cabinet = .cabinet, message = .message))]
    Invalid { cabinet: String, message: String },
    #[error("{}", tr!("generate.unknown_material", id = .0))]
    UnknownMaterial(String),
}

/// Vygeneruje diely pre všetky skrinky projektu.
pub fn build(
    project: &ProjectFile,
    standard: &Standard,
    materials: &MaterialLibrary,
) -> Result<Model, GenerateError> {
    let panel = material(materials, &project.defaults.panel_material)?;
    let back = material(materials, &project.defaults.back_material)?;
    let front = match &standard.fronts.material {
        Some(id) => material(materials, id)?,
        None => panel,
    };
    let mut cabinets = project
        .cabinets
        .iter()
        .map(|spec| build_cabinet(spec, standard, panel, back, front, materials))
        .collect::<Result<Vec<_>, _>>()?;
    // sokle po zostavách skriniek a krycie boky (výška osadenia podľa umiestnenia, Placement ako v CV)
    plinth::add(&mut cabinets, project, standard, materials, panel)?;
    // LED rampa pod hornou skrinkou (niche.rs)
    for (cab, spec) in cabinets.iter_mut().zip(&project.cabinets) {
        niche::add_led_rail(cab, spec, standard, materials)?;
    }
    // blenda pri skrinke alebo samostatná blenda (blenda.rs)
    for (cab, spec) in cabinets.iter_mut().zip(&project.cabinets) {
        blenda::add(cab, spec, standard, panel, front).map_err(|message| {
            GenerateError::Invalid {
                cabinet: spec.id.clone(),
                message,
            }
        })?;
    }
    Ok(Model {
        project: project.project.name.clone(),
        cabinets,
    })
}

fn material<'a>(lib: &'a MaterialLibrary, id: &str) -> Result<&'a Material, GenerateError> {
    lib.material(id)
        .ok_or_else(|| GenerateError::UnknownMaterial(id.to_owned()))
}

/// Odvodené rozmery skrinky (kap. 5, „Pomocné veličiny“).
struct Dims {
    w: f64,
    h: f64,
    d: f64,
    /// hrúbka korpusu
    t: f64,
    /// hrúbka zadnej steny
    tb: f64,
    /// vnútorná šírka `W − 2T`
    wi: f64,
    /// predná plocha zadnej steny `D − e − gw`
    yb: f64,
    /// šírka drážky `Tb + clearance`
    gw: f64,
    /// spodná skrinka: dno o toľko nad spodkom bokov (`base_end_inset`)
    floor: f64,
}

fn build_cabinet(
    spec: &CabinetSpec,
    standard: &Standard,
    panel: &Material,
    back: &Material,
    front: &Material,
    materials: &MaterialLibrary,
) -> Result<CabinetModel, GenerateError> {
    // šikmá skrinka má vlastnú konštrukciu (slanted.rs)
    if let Some(slant) = spec.slant.as_ref().filter(|_| spec.carcass) {
        return slanted::build_cabinet(spec, slant, standard, panel, back, front, materials);
    }
    let invalid = |message: String| GenerateError::Invalid {
        cabinet: spec.id.clone(),
        message,
    };
    // otvorená nika má vlastnú stavbu (niche.rs)
    if spec.niche && spec.carcass {
        return niche::build_niche(spec, standard, front, materials);
    }

    let t = panel.thickness;
    let tb = back.thickness;
    let bs = &standard.back;
    let gw = tb + bs.groove_width_clearance;
    // naložený chrbát: korpus je o hrúbku chrbta plytší, drážka nie je
    let applied = spec.back && bs.mount_for(spec.cabinet_type) == BackMount::Applied;
    let d_carcass = if applied {
        spec.d - bs.applied_depth.unwrap_or(tb)
    } else {
        spec.d
    };
    let dims = Dims {
        w: spec.w,
        h: spec.h,
        d: d_carcass,
        t,
        tb,
        wi: spec.w - 2.0 * t,
        yb: if applied || !spec.back {
            d_carcass
        } else {
            spec.d - bs.groove_offset_from_rear - gw
        },
        gw,
        floor: match spec.cabinet_type {
            CabinetType::Base => standard.carcass.base_end_inset.max(0.0),
            CabinetType::Wall | CabinetType::Tall => 0.0,
        },
    };
    // bez korpusu (umývačka, samostatná blenda šírky lišty) vnútornú šírku nepotrebuje
    if spec.carcass && dims.wi <= EPS {
        return Err(invalid(tr!("generate.inner_width", value = dims.wi)));
    }
    if dims.yb <= EPS {
        return Err(invalid(tr!("generate.depth_before_back", value = dims.yb)));
    }
    if 2.0 * t >= dims.h {
        return Err(invalid(tr!("generate.height", value = dims.h)));
    }

    let top_style = match spec.cabinet_type {
        CabinetType::Base => spec.top_style.unwrap_or(standard.carcass.top_style),
        CabinetType::Wall | CabinetType::Tall => TopStyle::Panel,
    };
    if !spec.fixed_shelves.is_empty() && spec.cabinet_type != CabinetType::Tall {
        return Err(invalid(tr!("generate.fixed_shelves_tall_only")));
    }

    // vrch hornej skrinky môže byť z iného materiálu (dielňa: MDF 19 ako v CV)
    let top_mat = match (spec.cabinet_type, &standard.carcass.wall_top_material) {
        (CabinetType::Wall, Some(id)) => materials
            .material(id)
            .ok_or_else(|| GenerateError::UnknownMaterial(id.clone()))?,
        _ => panel,
    };
    let t_top = match top_style {
        TopStyle::Panel => top_mat.thickness,
        TopStyle::Stretchers | TopStyle::Sink => t,
    };

    // dno a vrch spodnej skrinky odsadené od koncov bokov
    let inset = match spec.cabinet_type {
        CabinetType::Base => standard.carcass.base_end_inset,
        CabinetType::Wall | CabinetType::Tall => 0.0,
    };
    if !inset.is_finite() || inset < 0.0 || 2.0 * (t + inset) >= dims.h {
        return Err(invalid(tr!("generate.bad_inset", value = inset)));
    }
    let z_top = dims.h - inset;
    // horná skrinka ako v CV: vrch na bokoch, boky končia pred chrbtom
    let wall = spec.cabinet_type == CabinetType::Wall;
    let top_on_sides = wall && standard.carcass.wall_top_on_sides;
    let behind = wall && standard.carcass.wall_back_behind_sides && spec.back && !applied;
    let side_h = if top_on_sides { dims.h - t_top } else { dims.h };
    let side_d = if behind { dims.yb } else { dims.d };
    if side_h <= 2.0 * t {
        return Err(invalid(tr!("generate.side_too_low", value = side_h)));
    }

    let mut b = PartBuilder::new(spec, panel);
    // slepý roh: bok pri pevnom paneli vystupuje dopredu až k panelu
    let (ext_l, ext_r) = corner::blind_side_extend(spec, standard);

    // iba čelá (napr. umývačka): bez korpusu, pántov a zásuviek
    if !spec.carcass {
        let (hardware, openings) =
            build_fronts(spec, standard, &dims, front, materials, &mut b).map_err(invalid)?;
        return Ok(CabinetModel {
            id: spec.id.clone(),
            cabinet_type: spec.cabinet_type,
            w: dims.w,
            h: dims.h,
            d: dims.d,
            parts: b.finish(),
            hardware,
            options: spec.options.resolve(&standard.cabinet_options),
            openings,
        });
    }

    // 5.1 Boky, dno
    b.panel(
        Role::SideLeft,
        side_h,
        side_d + ext_l,
        // x = −Z (zhora nadol), y = +Y (spredu dozadu), z = +X (F = vnútorná plocha)
        Transform::from_axes(DVec3::NEG_Z, DVec3::Y, DVec3::new(0.0, -ext_l, side_h)),
    );
    b.panel(
        Role::SideRight,
        side_h,
        side_d + ext_r,
        // x = +Z, y = +Y, z = −X
        Transform::from_axes(DVec3::Z, DVec3::Y, DVec3::new(dims.w, -ext_r, 0.0)),
    );
    b.panel(
        Role::Bottom,
        dims.wi,
        dims.d,
        // x = +X, y = +Y, z = +Z (F = horná plocha)
        Transform::from_axes(DVec3::X, DVec3::Y, DVec3::new(t, 0.0, inset)),
    );

    // 5.2 / 5.3 Vrch
    match top_style {
        TopStyle::Stretchers => {
            let sw = standard.carcass.stretcher_width;
            // vlysy znížené pod varnú dosku (CV REF VARNA: 154,5 pod vrch bokov)
            let z_top = match spec.stretcher_drop {
                Some(drop) if drop.is_finite() && drop > 0.0 => {
                    let z = dims.h - drop;
                    if z - t < inset + t + EPS {
                        return Err(invalid(tr!("generate.stretcher_drop", value = drop)));
                    }
                    z
                }
                _ => z_top,
            };
            if 2.0 * sw > dims.yb + EPS {
                return Err(invalid(tr!(
                    "generate.stretchers_too_wide",
                    width = sw,
                    depth = dims.yb
                )));
            }
            b.panel(
                Role::StretcherFront,
                dims.wi,
                sw,
                top_like(&dims, 0.0, z_top),
            );
            b.panel(
                Role::StretcherRear,
                dims.wi,
                sw,
                top_like(&dims, dims.yb - sw, z_top),
            );
        }
        TopStyle::Sink => {
            // [P] drez REF (findings 12 §4): vlysy na výšku, predný hore lícuje s čelom,
            // zadný stojí na dne a lícuje so zadom bokov; olepenie nemajú (hrana k čelu chýba)
            let sw = standard.carcass.stretcher_width;
            if sw + 2.0 * t + 2.0 * inset > dims.h + EPS || dims.yb < 2.0 * t {
                return Err(invalid(tr!("generate.sink_stretchers", value = sw)));
            }
            // x = −X, y = +Z, z = +Y (F = zadná plocha k interiéru), spodná hrana = Y0
            b.part(
                Role::StretcherFront,
                panel,
                dims.wi,
                sw,
                // lícuje s vrchom bokov (Y 734 … 862 v REF), nie 0,5 pod ním ako ležaté vlysy
                Transform::from_axes(
                    DVec3::NEG_X,
                    DVec3::Z,
                    DVec3::new(dims.w - t, 0.0, dims.h - sw),
                ),
                false,
            );
            // x = +X, y = +Z, z = −Y (F = predná plocha k interiéru), stojí na dne
            b.part(
                Role::StretcherRear,
                panel,
                dims.wi,
                sw,
                Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(t, dims.yb, inset + t)),
                false,
            );
        }
        TopStyle::Panel if top_on_sides => {
            // x = −X, y = +Y, z = −Z (F = spodná plocha na bokoch)
            b.part(
                Role::Top,
                top_mat,
                dims.w,
                dims.d,
                Transform::from_axes(DVec3::NEG_X, DVec3::Y, DVec3::new(dims.w, 0.0, z_top)),
                true,
            );
        }
        TopStyle::Panel => {
            b.part(
                Role::Top,
                top_mat,
                dims.wi,
                dims.d,
                top_like(&dims, 0.0, z_top),
                true,
            );
        }
    }

    // Zadná stena (drezová skrinka môže byť bez nej)
    let gd = bs.groove_depth;
    if applied {
        // [P] spodné kuchynské: chrbát zozadu cez celú šírku − 2 × odsadenie (findings 10 §2.4)
        let side = bs.applied_inset_side.unwrap_or(0.0);
        let top = bs.applied_inset_top.unwrap_or(0.0);
        let bottom = bs.applied_inset_bottom.unwrap_or(0.0);
        let back_width = dims.w - 2.0 * side;
        // pod varnou doskou končí chrbát na hornej ploche znížených vlysov (CV REF: 606)
        let back_top = match (top_style, spec.stretcher_drop) {
            (TopStyle::Stretchers, Some(drop)) if drop.is_finite() && drop > 0.0 => dims.h - drop,
            _ => dims.h - top,
        };
        let back_height = back_top - bottom;
        if back_width <= EPS || back_height <= EPS {
            return Err(invalid(tr!(
                "generate.bad_back",
                width = back_width,
                height = back_height
            )));
        }
        b.part(
            Role::Back,
            back,
            back_width,
            back_height,
            // x = +X, y = +Z, z = −Y (F = predná plocha, na zadných hranách korpusu)
            // predná plocha chrbta na zadných hranách korpusu
            Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(side, d_carcass + tb, bottom)),
            false,
        );
    } else if behind {
        // [P] CV horná: chrbát za bokmi v drážkach dna a vrchu (findings 12 §1)
        let c = bs.fit_clearance;
        let back_width = dims.w - 2.0 * c;
        // medzi dnom a vrchom + zasunutie do oboch drážok
        let back_height = dims.h - t - t_top + 2.0 * (gd - c);
        if back_width <= EPS || back_height <= EPS {
            return Err(invalid(tr!(
                "generate.bad_back",
                width = back_width,
                height = back_height
            )));
        }
        let back_rear_face = dims.yb + (dims.gw - dims.tb) / 2.0 + dims.tb;
        b.part(
            Role::Back,
            back,
            back_width,
            back_height,
            Transform::from_axes(
                DVec3::X,
                DVec3::Z,
                DVec3::new(c, back_rear_face, t - (gd - c)),
            ),
            false,
        );
    } else if spec.back {
        let c = bs.fit_clearance;
        let back_width = dims.wi + 2.0 * gd - 2.0 * c;
        let back_height = match top_style {
            TopStyle::Stretchers | TopStyle::Sink => dims.h - (t - gd) - c,
            TopStyle::Panel => dims.h - (t - gd) - (t_top - gd) - c,
        };
        if back_width <= EPS || back_height <= EPS {
            return Err(invalid(tr!(
                "generate.bad_back",
                width = back_width,
                height = back_height
            )));
        }
        let back_rear_face = dims.yb + (dims.gw - dims.tb) / 2.0 + dims.tb;
        b.part(
            Role::Back,
            back,
            back_width,
            back_height,
            // x = +X, y = +Z, z = −Y (F = predná plocha)
            Transform::from_axes(
                DVec3::X,
                DVec3::Z,
                DVec3::new(t - gd + c, back_rear_face, t - gd),
            ),
            false,
        );
    }

    // 5.4 Pevné police
    let mut sorted = spec.fixed_shelves.clone();
    sorted.sort_by(f64::total_cmp);
    for (i, zf) in sorted.iter().enumerate() {
        if *zf - t < t - EPS || *zf > dims.h - t + EPS {
            return Err(invalid(tr!(
                "generate.fixed_shelf_outside",
                z = *zf,
                min = 2.0 * t,
                max = dims.h - t
            )));
        }
        if i > 0 && *zf - t < sorted[i - 1] - EPS {
            return Err(invalid(tr!(
                "generate.fixed_shelves_overlap",
                a = sorted[i - 1],
                b = *zf
            )));
        }
    }
    for zf in &spec.fixed_shelves {
        b.panel(
            Role::FixedShelf,
            dims.wi,
            dims.yb,
            top_like(&dims, 0.0, *zf),
        );
    }

    let shelf_at = b.parts.len();
    let (hardware, openings) =
        build_fronts(spec, standard, &dims, front, materials, &mut b).map_err(invalid)?;
    let fronts_end = b.parts.len();

    // Polohovateľné police (poloha je nominálna, iba pre model) iba vo výške otvorov s dvierkami
    // alebo otvorených (ako Interior otvoru v CV); pri skrinke iba so zásuvkami žiadne
    let zone = shelf_zone(spec, &openings, inset + t, dims.h - inset - t_top);
    let n = if zone.is_some() {
        spec.adjustable_shelves
    } else {
        0
    };
    if let (true, Some((z_lo, z_hi))) = (n > 0, zone) {
        let sc = standard.carcass.shelf_side_clearance;
        let setback = standard.carcass.shelf_front_setback;
        let length = dims.wi - 2.0 * sc;
        let width = dims.yb - setback;
        if length <= EPS || width <= EPS {
            return Err(invalid(tr!(
                "generate.bad_shelf",
                length = length,
                width = width
            )));
        }
        // výška otvoru pre police: celý vnútrajšok (dno … vrch) alebo časť za dvierkami
        let inner = z_hi - z_lo;
        if inner < f64::from(n) * t + EPS {
            return Err(invalid(tr!(
                "generate.shelves_do_not_fit",
                count = n,
                height = format!("{inner:.0}")
            )));
        }
        for i in 1..=n {
            let top_face = z_lo
                + match standard.carcass.shelf_spacing {
                    ShelfSpacing::InnerHeight => inner * f64::from(i) / f64::from(n + 1),
                    // [P] CV: otvory medzi dnom, policami a vrchom sú rovnaké (findings 10 §2.3)
                    ShelfSpacing::EqualOpenings => {
                        let opening = (inner - f64::from(n) * t) / f64::from(n + 1);
                        f64::from(i) * opening + f64::from(i) * t
                    }
                };
            b.panel(
                Role::AdjustableShelf,
                length,
                width,
                Transform::from_axes(
                    DVec3::X,
                    DVec3::Y,
                    DVec3::new(t + sc, setback, top_face - t),
                ),
            );
        }
    }
    // police v zozname dielov pred čelami (poradie dielov ako doteraz)
    if b.parts.len() > fronts_end {
        let added = b.parts.len() - fronts_end;
        if let Some(tail) = b.parts.get_mut(shelf_at..) {
            tail.rotate_right(added);
        }
    }

    Ok(CabinetModel {
        id: spec.id.clone(),
        cabinet_type: spec.cabinet_type,
        w: dims.w,
        h: dims.h,
        d: dims.d,
        parts: b.finish(),
        hardware,
        options: spec.options.resolve(&standard.cabinet_options),
        openings,
    })
}

/// Výškový rozsah (spodná plocha najnižšej, horná plocha najvyššej možnej police) pre police:
/// `lo`/`hi` = vnútrajšok korpusu. Bez čiel alebo keď sú všetky riadky s dvierkami celý
/// vnútrajšok; inak najdlhší súvislý úsek riadkov s dvierkami / otvorených (hranica so zásuvkou
/// = spodok / vrch čela). `None` = police nemajú kam ísť (iba zásuvky a slepé čelá).
fn shelf_zone(
    spec: &CabinetSpec,
    openings: &[FrontOpening],
    lo: f64,
    hi: f64,
) -> Option<(f64, f64)> {
    let rows = spec.shelf_rows();
    if rows.is_empty() || rows.iter().all(|r| *r) {
        return Some((lo, hi));
    }
    // najdlhší úsek po sebe idúcich riadkov (zhora nadol) s policami
    let mut best: Option<(usize, usize)> = None;
    let mut start = None;
    for (i, ok) in rows.iter().chain(std::iter::once(&false)).enumerate() {
        match (ok, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                if best.is_none_or(|(a, b)| i - s > b - a) {
                    best = Some((s, i));
                }
                start = None;
            }
            _ => {}
        }
    }
    let (first, end) = best?;
    let span: Vec<&FrontOpening> = openings
        .iter()
        .filter(|o| (first..end).contains(&o.row))
        .collect();
    let bottom = span.iter().map(|o| o.z).fold(f64::INFINITY, f64::min);
    let top = span
        .iter()
        .map(|o| o.z + o.h)
        .fold(f64::NEG_INFINITY, f64::max);
    if !bottom.is_finite() || !top.is_finite() {
        return None;
    }
    // najnižší riadok skrinky siaha po dno, najvyšší po vrch
    let z_lo = if end == rows.len() {
        lo
    } else {
        bottom.max(lo)
    };
    let z_hi = if first == 0 { hi } else { top.min(hi) };
    (z_hi > z_lo).then_some((z_lo, z_hi))
}

/// Najviac čiel v jednej skrinke.
const MAX_FRONTS: usize = 12;
/// Najmenšia výška a šírka čela.
const MIN_FRONT: f64 = 40.0;

/// Čelá skrinky zhora nadol a pánty dvierok.
///
/// Na korpus: bočné hrany `reveal_side` od hrán korpusu, vrch a spodok podľa typu skrinky
/// (`[fronts.base|wall|tall]`), čelá `gap_to_carcass` pred prednou rovinou (`y < 0`).
/// Vložené: otvor medzi bokmi, dnom a vrchom zmenšený o `inset_gap`, čelá lícujú
/// s prednou rovinou (`0 ≤ y ≤ hrúbka`).
///
/// Diel čela: `x` = nahor (dĺžka = výška čela), `y` = doprava, `z` = dozadu, takže
/// plocha F je vnútorná (k skrinke) a B je pohľadová.
fn build_fronts(
    spec: &CabinetSpec,
    standard: &Standard,
    dims: &Dims,
    mat: &Material,
    materials: &MaterialLibrary,
    b: &mut PartBuilder,
) -> Result<(Vec<Hardware>, Vec<FrontOpening>), String> {
    let fs = &standard.fronts;
    let fronts = &spec.fronts;
    if fronts.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    if fronts.len() > MAX_FRONTS {
        return Err(tr!("generate.fronts.too_many", count = MAX_FRONTS));
    }
    let mount = spec.front_mount.unwrap_or(fs.mount);
    let tf = mat.thickness;
    let rows = fs.rows(spec.cabinet_type);
    let (x0, x1, z0, z1, y0, gap, pair_gap) = match mount {
        FrontMount::Overlay => {
            let r = &spec.reveals;
            (
                r.left.unwrap_or(fs.reveal_side),
                dims.w - r.right.unwrap_or(fs.reveal_side),
                r.bottom.unwrap_or(rows.bottom),
                dims.h - r.top.unwrap_or(rows.top),
                -(fs.gap_to_carcass + tf),
                r.horizontal.unwrap_or(rows.gap),
                r.vertical.unwrap_or(fs.pair_gap),
            )
        }
        FrontMount::Inset => (
            dims.t + fs.inset_gap,
            dims.w - dims.t - fs.inset_gap,
            dims.t + fs.inset_gap,
            dims.h - dims.t - fs.inset_gap,
            0.0,
            fs.inset_gap,
            fs.inset_gap,
        ),
    };
    if [x0, x1, z0, z1, y0, gap, pair_gap]
        .iter()
        .any(|v| !v.is_finite())
        || pair_gap < 0.0
    {
        return Err(tr!("generate.fronts.bad_standard"));
    }
    let width = x1 - x0;
    if !(width > MIN_FRONT && z1 - z0 > MIN_FRONT) || !gap.is_finite() || gap < 0.0 {
        return Err(tr!(
            "generate.fronts.no_room",
            width = format!("{width:.1}"),
            height = format!("{:.1}", z1 - z0)
        ));
    }

    // výšky: zadané, zvyšok rovnako medzi čelá bez výšky
    let available = z1 - z0 - gap * (fronts.len() - 1) as f64;
    let mut fixed = 0.0;
    for f in fronts {
        if let Some(h) = f.height {
            if !h.is_finite() || h < MIN_FRONT {
                return Err(tr!("generate.fronts.too_low", height = h, min = MIN_FRONT));
            }
            fixed += h;
        }
    }
    let free = fronts.iter().filter(|f| f.height.is_none()).count();
    let rest = available - fixed;
    let share = if free > 0 { rest / free as f64 } else { 0.0 };
    if (free > 0 && share < MIN_FRONT) || (free == 0 && rest < -1e-6) {
        return Err(tr!(
            "generate.fronts.too_high",
            available = format!("{available:.1}"),
            fixed = format!("{fixed:.1}")
        ));
    }

    let hinge_id = spec
        .hinge
        .as_deref()
        .or(fs.hinge.as_deref())
        .or(standard.hinges.first().map(|h| h.id.as_str()));
    // bez korpusu nie je kam dať pánty ani výsuvy
    let hinge_id = hinge_id.filter(|_| spec.carcass);
    let drawer_id_ok = spec.carcass;
    let hinge = match hinge_id {
        Some(id) => Some(
            standard
                .hinge(id)
                .ok_or_else(|| tr!("generate.fronts.hinge_missing", id = id))?,
        ),
        None => None,
    };
    let drawer_id = spec
        .drawer
        .as_deref()
        .or(fs.drawer.as_deref())
        .or(standard.drawers.first().map(|d| d.id.as_str()));
    let drawer_id = drawer_id.filter(|_| drawer_id_ok);
    let drawer = match drawer_id {
        Some(id) => Some(
            standard
                .drawer(id)
                .ok_or_else(|| tr!("generate.fronts.drawer_missing", id = id))?,
        ),
        None => None,
    };
    let box_material = match drawer.and_then(|d| d.box_material.as_deref()) {
        Some(id) => Some(
            materials
                .material(id)
                .ok_or_else(|| tr!("generate.fronts.drawer_material", id = id))?,
        ),
        None => None,
    };

    let blind_hinge = match fs.blind_hinge.as_deref().filter(|_| spec.carcass) {
        Some(id) => Some(
            standard
                .hinge(id)
                .ok_or_else(|| tr!("generate.fronts.blind_hinge_missing", id = id))?,
        ),
        None => None,
    };
    let ctx = FrontCtx {
        spec,
        dims,
        mat,
        mount,
        tf,
        y0,
        pair_gap,
        hinge,
        blind_hinge,
        drawer,
        box_material,
        hinge_end: fs.hinge_end_offset,
        hinge_spacing: fs.hinge_max_spacing,
    };

    let mut hardware = Vec::new();
    let mut openings = Vec::new();
    let mut top = z1;
    for (row, f) in fronts.iter().enumerate() {
        let h = f.height.unwrap_or(share);
        let bottom = top - h;
        // pevný panel v hornom rade siaha vyššie (nad ním nie je úchytka); škára hore
        // pri skrinke (`reveals.top`) nahrádza škáru štandardu
        let top_reveal = match mount {
            FrontMount::Overlay => spec.reveals.top.unwrap_or(rows.top),
            FrontMount::Inset => rows.top,
        };
        let panel_extra = if row == 0 {
            top_reveal - rows.panel_top
        } else {
            0.0
        };
        if f.columns.is_empty() {
            openings.push(FrontOpening {
                row,
                column: None,
                x: x0,
                z: bottom,
                w: width,
                h,
            });
            let item = Item {
                spec: f,
                x: x0,
                w: width,
                blind: None,
            };
            place_front(&ctx, &item, bottom, h, b, &mut hardware)?;
        } else {
            let items = split_columns(f, dims.w, x0, x1, pair_gap)?;
            for (k, (_, x, w, _, _)) in items.iter().enumerate() {
                openings.push(FrontOpening {
                    row,
                    column: Some(k),
                    x: *x,
                    z: bottom,
                    w: *w,
                    h,
                });
            }
            // najprv panely (pánt vedľa panelu má podložku na paneli)
            let mut panels: Vec<(usize, String, f64)> = Vec::new();
            for (k, (c, x, w, line_l, line_r)) in items.iter().enumerate() {
                if c.kind == FrontKind::Panel {
                    // úprava rozmeru panelu (Size Adjustments)
                    let a = &c.adjust;
                    let (px, pw) = (*x - a.left, *w + a.left + a.right);
                    let (pb, ph) = (bottom - a.bottom, h + panel_extra + a.bottom + a.top);
                    if !(pw > MIN_FRONT && ph > MIN_FRONT)
                        || ![px, pw, pb, ph].iter().all(|v| v.is_finite())
                    {
                        return Err(tr!(
                            "generate.fronts.bad_panel_adjust",
                            width = format!("{pw:.1}"),
                            height = format!("{ph:.1}")
                        ));
                    }
                    let id = b.front(
                        Role::FalseFront,
                        mat,
                        ph,
                        pw,
                        Transform::from_axes(DVec3::Z, DVec3::X, DVec3::new(px, y0, pb)),
                    );
                    panels.push((k, id, if k == 0 { *line_r } else { *line_l }));
                }
            }
            for (k, (c, x, w, _, _)) in items.iter().enumerate() {
                if c.kind == FrontKind::Panel {
                    continue;
                }
                // susedný panel: (strana pántov, ID panelu, čiara delenia)
                let blind = panels.iter().find_map(|(pk, id, line)| {
                    if *pk + 1 == k {
                        Some((HingeSide::Left, id.clone(), *line))
                    } else if k + 1 == *pk {
                        Some((HingeSide::Right, id.clone(), *line))
                    } else {
                        None
                    }
                });
                let item = Item {
                    spec: c,
                    x: *x,
                    w: *w,
                    blind,
                };
                place_front(&ctx, &item, bottom, h, b, &mut hardware)?;
            }
        }
        top = bottom - gap;
    }
    Ok((hardware, openings))
}

/// Spoločné údaje pre čelá skrinky.
struct FrontCtx<'a> {
    spec: &'a CabinetSpec,
    dims: &'a Dims,
    mat: &'a Material,
    mount: FrontMount,
    tf: f64,
    y0: f64,
    pair_gap: f64,
    hinge: Option<&'a cc_model::HingeSpec>,
    blind_hinge: Option<&'a cc_model::HingeSpec>,
    drawer: Option<&'a cc_model::DrawerSpec>,
    box_material: Option<&'a Material>,
    hinge_end: f64,
    hinge_spacing: f64,
}

/// Čelo v riadku alebo stĺpci: poloha `x`, šírka `w`; `blind` = susedný panel
/// (strana pántov, ID panelu, čiara delenia) pre slepý pánt.
struct Item<'a> {
    spec: &'a cc_model::FrontSpec,
    x: f64,
    w: f64,
    blind: Option<(HingeSide, String, f64)>,
}

/// Najviac stĺpcov v riadku čiel.
const MAX_COLUMNS: usize = 6;

/// Stĺpec riadku: čelo, x, šírka, ľavá a pravá čiara delenia.
type Column<'a> = (&'a cc_model::FrontSpec, f64, f64, f64, f64);

/// Stĺpce riadku. Šírky stĺpcov sú šírky otvorov po čiaru delenia (súčet = šírka skrinky);
/// bez šírky si delia zvyšok. Medzi čelami je škára `gap`.
fn split_columns(
    row: &cc_model::FrontSpec,
    cab_w: f64,
    x0: f64,
    x1: f64,
    gap: f64,
) -> Result<Vec<Column<'_>>, String> {
    let cols = &row.columns;
    if cols.len() > MAX_COLUMNS {
        return Err(tr!("generate.fronts.too_many_columns", count = MAX_COLUMNS));
    }
    let mut fixed = 0.0;
    for c in cols {
        if !c.columns.is_empty() {
            return Err(tr!("generate.fronts.nested_columns"));
        }
        if let Some(w) = c.width {
            if !w.is_finite() || w < MIN_FRONT {
                return Err(tr!(
                    "generate.fronts.column_too_narrow",
                    width = w,
                    min = MIN_FRONT
                ));
            }
            fixed += w;
        }
    }
    let free = cols.iter().filter(|c| c.width.is_none()).count();
    let share = if free > 0 {
        (cab_w - fixed) / free as f64
    } else {
        0.0
    };
    if (free > 0 && share < MIN_FRONT) || (free == 0 && (fixed - cab_w).abs() > 0.5) {
        return Err(tr!(
            "generate.fronts.columns_mismatch",
            fixed = format!("{fixed:.1}"),
            width = format!("{cab_w:.1}")
        ));
    }
    let mut out = Vec::new();
    let mut line = 0.0;
    let n = cols.len();
    for (k, c) in cols.iter().enumerate() {
        let w = c.width.unwrap_or(share);
        let (l, r) = (line, line + w);
        let left = if k == 0 { x0 } else { l + gap / 2.0 };
        let right = if k + 1 == n { x1 } else { r - gap / 2.0 };
        if right - left < MIN_FRONT {
            return Err(tr!(
                "generate.fronts.column_narrow",
                width = format!("{:.1}", right - left)
            ));
        }
        out.push((c, left, right - left, l, r));
        line = r;
    }
    Ok(out)
}

/// Jedno čelo (dvierka, dvojité, zásuvka, slepé čelo) a jeho kovanie.
fn place_front(
    ctx: &FrontCtx,
    item: &Item,
    bottom: f64,
    h: f64,
    b: &mut PartBuilder,
    hardware: &mut Vec<Hardware>,
) -> Result<(), String> {
    let f = item.spec;
    // úprava rozmeru čela pri hranách (Size Adjustments v CV)
    let a = &f.adjust;
    let (x, width) = (item.x - a.left, item.w + a.left + a.right);
    let (bottom, h) = (bottom - a.bottom, h + a.bottom + a.top);
    if !(width > MIN_FRONT && h > MIN_FRONT) || ![x, width, bottom, h].iter().all(|v| v.is_finite())
    {
        return Err(tr!(
            "generate.fronts.bad_front_adjust",
            width = format!("{width:.1}"),
            height = format!("{h:.1}")
        ));
    }
    let place = |x: f64| Transform::from_axes(DVec3::Z, DVec3::X, DVec3::new(x, ctx.y0, bottom));
    let mut doors: Vec<(f64, f64, HingeSide)> = Vec::new();
    match f.kind {
        FrontKind::Door => {
            let side = item
                .blind
                .as_ref()
                .map_or(f.hinge_side.unwrap_or_default(), |(s, _, _)| *s);
            doors.push((x, width, side));
        }
        FrontKind::Pair => {
            let half = (width - ctx.pair_gap) / 2.0;
            if half < MIN_FRONT {
                return Err(tr!(
                    "generate.fronts.pair_narrow",
                    width = format!("{width:.1}")
                ));
            }
            doors.push((x, half, HingeSide::Left));
            doors.push((x + width - half, half, HingeSide::Right));
        }
        FrontKind::Drawer => {
            let front_id = b.front(Role::DrawerFront, ctx.mat, h, width, place(x));
            if let Some(ds) = ctx.drawer {
                let hw = drawer_box(
                    ctx.spec,
                    ctx.dims,
                    ds,
                    ctx.box_material,
                    &front_id,
                    bottom,
                    bottom + h,
                    f.box_height,
                    b,
                )?;
                hardware.push(hw);
            }
        }
        FrontKind::FalseFront | FrontKind::Panel => {
            b.front(Role::FalseFront, ctx.mat, h, width, place(x));
        }
        FrontKind::Open => {}
    }
    for (dx, w, side) in doors {
        let id = b.front(Role::Door, ctx.mat, h, w, place(dx));
        // slepý pánt pri pevnom paneli: podložka na paneli, inak na boku
        let (hs, target, plate_x) = match (&item.blind, ctx.blind_hinge) {
            (Some((_, panel, line)), Some(bh)) => {
                let toward_panel = match side {
                    HingeSide::Left => -1.0,
                    HingeSide::Right => 1.0,
                };
                (
                    bh,
                    panel.clone(),
                    Some(line + toward_panel * bh.plate_setback),
                )
            }
            _ => {
                let Some(hs) = ctx.hinge else {
                    continue;
                };
                let side_role = match side {
                    HingeSide::Left => Role::SideLeft,
                    HingeSide::Right => Role::SideRight,
                };
                (hs, format!("{}-{}", ctx.spec.id, side_role.id_code()), None)
            }
        };
        let cup_x = match side {
            HingeSide::Left => dx + hs.cup_center,
            HingeSide::Right => dx + w - hs.cup_center,
        };
        let plate_y = hs.plate_setback
            + match ctx.mount {
                FrontMount::Overlay => 0.0,
                FrontMount::Inset => ctx.tf,
            };
        let end = hs.end_offset.unwrap_or(ctx.hinge_end);
        let spacing = hs.max_spacing.unwrap_or(ctx.hinge_spacing);
        let count = cc_model::FrontStandard::hinge_count(h, end, spacing);
        for z in hinge_positions(h, end, count) {
            hardware.push(Hardware::Hinge {
                hinge: hs.id.clone(),
                door: id.clone(),
                side: target.clone(),
                z: bottom + z,
                cup_x,
                plate_y,
                plate_x,
            });
        }
    }
    Ok(())
}

/// Korpus zásuvky za čelom `front_bottom … front_top`: výber dĺžky a výšky systému,
/// dno a zadná stena z dosky (ak má systém materiál), kovanie pre obrábanie.
#[allow(clippy::too_many_arguments)]
fn drawer_box(
    spec: &CabinetSpec,
    dims: &Dims,
    ds: &cc_model::DrawerSpec,
    box_material: Option<&Material>,
    front_id: &str,
    front_bottom: f64,
    front_top: f64,
    box_height: Option<f64>,
    b: &mut PartBuilder,
) -> Result<Hardware, String> {
    let t = dims.t;
    // nad dnom: dno spodnej skrinky je o `floor` nad spodkom bokov (CV REF: rady výsuvov 75,5)
    let box_bottom = (front_bottom + ds.above_front).max(dims.floor + t + ds.above_bottom);
    let length = ds
        .lengths
        .iter()
        .copied()
        .filter(|l| {
            l.is_finite()
                && *l > 0.0
                && l + ds.depth_reserve <= dims.d + 1e-6
                && spec.drawer_length.is_none_or(|max| *l <= max + 1e-6)
        })
        .fold(None, |best: Option<f64>, l| {
            Some(best.map_or(l, |b| b.max(l)))
        })
        .ok_or_else(|| tr!("generate.fronts.drawer_depth", id = &ds.id, depth = dims.d))?;
    let room_top = (front_top - ds.top_reserve).min(dims.h - t - ds.top_reserve);
    let height = ds
        .heights
        .iter()
        .copied()
        .filter(|h| {
            h.is_finite()
                && *h > 0.0
                && box_bottom + h <= room_top + 1e-6
                && box_height.is_none_or(|want| (h - want).abs() < 0.01)
        })
        .fold(None, |best: Option<f64>, h| {
            Some(best.map_or(h, |b| b.max(h)))
        })
        .ok_or_else(|| {
            tr!(
                "generate.fronts.drawer_box_height",
                id = &ds.id,
                height = format!("{:.1}", front_top - front_bottom)
            )
        })?;
    let box_left = t + ds.side_clearance;
    let box_right = dims.w - t - ds.side_clearance;
    if box_right - box_left < 100.0 {
        return Err(tr!("generate.fronts.drawer_narrow", id = &ds.id));
    }

    if let Some(m) = box_material {
        let inner = dims.w - 2.0 * t;
        let w = inner - ds.bottom_narrower;
        let l = length - ds.bottom_shorter;
        if w > 0.0 && l > 0.0 {
            let x0 = t + ds.bottom_narrower / 2.0;
            let y0 = ds.bottom_shorter / 2.0;
            let z_top = box_bottom + ds.bottom_top;
            // dno: x = +X, y = +Y, F = horná plocha
            b.part(
                Role::DrawerBottom,
                m,
                w,
                l,
                Transform::from_axes(DVec3::X, DVec3::Y, DVec3::new(x0, y0, z_top - m.thickness)),
                false,
            );
            let back_h = height - ds.back_lower;
            if back_h > 0.0 {
                // zadná stena na dne vzadu: x = +X, y = +Z, F = predná plocha (z = −Y)
                b.part(
                    Role::DrawerBack,
                    m,
                    w,
                    back_h,
                    Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(x0, y0 + l, z_top)),
                    false,
                );
            }
        }
    }
    Ok(Hardware::Drawer {
        drawer: ds.id.clone(),
        front: front_id.to_owned(),
        side_left: format!("{}-{}", spec.id, Role::SideLeft.id_code()),
        side_right: format!("{}-{}", spec.id, Role::SideRight.id_code()),
        box_bottom,
        box_height: height,
        length,
        box_left,
        box_right,
    })
}

/// Polohy pántov od spodnej hrany dvierok výšky `h`: krajné `end` od hrán, ostatné
/// rovnomerne medzi nimi.
pub fn hinge_positions(h: f64, end: f64, count: usize) -> Vec<f64> {
    let end = end.clamp(0.0, h / 2.0);
    match count {
        0 => Vec::new(),
        1 => vec![h / 2.0],
        n => (0..n)
            .map(|i| end + (h - 2.0 * end) * i as f64 / (n - 1) as f64)
            .collect(),
    }
}

/// Orientácia vrchu, spojok a pevných políc: x = −X, y = +Y, z = −Z
/// (F = spodná plocha). `y0` je predná hrana dielu v skrinke, `z_top` horná plocha.
fn top_like(dims: &Dims, y0: f64, z_top: f64) -> Transform {
    Transform::from_axes(
        DVec3::NEG_X,
        DVec3::Y,
        DVec3::new(dims.w - dims.t, y0, z_top),
    )
}

/// Zbiera diely jednej skrinky, prideľuje ID a olepuje prednú hranu.
struct PartBuilder<'a> {
    spec: &'a CabinetSpec,
    panel: &'a Material,
    parts: Vec<Part>,
}

impl<'a> PartBuilder<'a> {
    fn new(spec: &'a CabinetSpec, panel: &'a Material) -> Self {
        PartBuilder {
            spec,
            panel,
            parts: Vec::new(),
        }
    }

    /// Korpusový diel s olepenou prednou hranou (kap. 5.5).
    fn panel(&mut self, role: Role, length: f64, width: f64, transform: Transform) {
        self.part(role, self.panel, length, width, transform, true);
    }

    fn part(
        &mut self,
        role: Role,
        material: &Material,
        length: f64,
        width: f64,
        transform: Transform,
        band_front: bool,
    ) {
        debug_assert!(transform.is_right_handed_orthonormal());
        let mut edges = Edges::default();
        if band_front {
            // Transformácie tvorí generátor, preto predná hrana existuje vždy;
            // keby nie, diel ostane neolepený a nespadne.
            let front = transform.edge_facing(DVec3::NEG_Y);
            debug_assert!(front.is_some(), "korpusový diel musí mať prednú hranu");
            if let Some(front) = front {
                edges.set(front, material.edge_band.clone());
            }
        }
        let index = self.parts.iter().filter(|p| p.role == role).count() + 1;
        let id = match role {
            Role::AdjustableShelf
            | Role::FixedShelf
            | Role::Door
            | Role::DrawerFront
            | Role::FalseFront
            | Role::DrawerBottom
            | Role::DrawerBack => {
                format!("{}-{}-{index}", self.spec.id, role.id_code())
            }
            _ => format!("{}-{}", self.spec.id, role.id_code()),
        };
        self.parts.push(Part {
            id,
            cabinet: self.spec.id.clone(),
            role,
            material: material.id.clone(),
            length,
            width,
            thickness: material.thickness,
            grain: material.grain,
            edges,
            transform,
            operations: Vec::new(),
        });
    }

    /// Čelo olepené dookola; vráti jeho ID (`SP1-DOOR-1`).
    fn front(
        &mut self,
        role: Role,
        material: &Material,
        length: f64,
        width: f64,
        transform: Transform,
    ) -> String {
        self.part(role, material, length, width, transform, false);
        let Some(last) = self.parts.last_mut() else {
            return String::new();
        };
        for e in Edge::ALL {
            last.edges.set(e, material.edge_band.clone());
        }
        last.id.clone()
    }

    fn finish(self) -> Vec<Part> {
        self.parts
    }
}

/// Hrana, ktorá je v skrinke vpredu (pre testy a výstupy).
pub fn front_edge(part: &Part) -> Option<Edge> {
    part.transform.edge_facing(DVec3::NEG_Y)
}
