//! Šikmá skrinka (CV REF „S_KUCH_D SIKMA“ a „S_KUCH_NASTENNA SIKMA“, `cc_model::Slant`).
//!
//! Konštrukcia podľa dielov zákazky REF (`tests/fixtures/cv-ref`, findings 12 §1 a §5):
//! - šikmý bok je doska z materiálu `[slanted] side_material` natočená v čelnom pohľade;
//!   jej vonkajšia plocha ide od `bottom` pri spodku po `top` pri vrchu korpusu. Konce
//!   dosky sú v CV zrezané pod uhlom (vodorovne); diel má rozmer obdĺžnika pred zrezaním:
//!   so vrchom `(H + T·sin α) / cos α` (vnútorný roh pri vrchu, 895,8 v REF), bez vrchu
//!   `H / cos α` (vonkajší roh v rohu pri druhom boku, 792 v REF),
//! - rovný bok ako pri rovnej skrinke, dno a vrch medzi bokmi končia na vnútornej ploche
//!   šikmého boku (rozmer = obálka dielu so šikmým koncom: od priesečníka so spodnou plochou),
//! - pohľadový chrbát (`finished_back`, CV `FB`) z dosky korpusu cez celú výšku od šikmého
//!   boku po vonkajšiu plochu rovného boku; korpus je o jeho hrúbku plytší,
//! - chrbát v drážke (horná REF): za rovným bokom, v drážke dna a šikmého boku, hore končí na
//!   priamke drážky šikmého boku; drážka v šikmom boku je pozdĺž hrany chrbta.
//!
//! Tvar šikmých dielov (zrezané konce, šikmá hrana dna, vrchu a chrbta, tvarové dvierka)
//! model zatiaľ nemá: diely sú obdĺžniky s rozmerom obálky ako v rozpise CV.

use cc_i18n::tr;
use cc_model::{
    BackMount, CabinetModel, CabinetSpec, Face, Hardware, Material, MaterialLibrary, Operation,
    Part, Role, Slant, SlantSide, Standard, Transform, EPS, P2,
};
use glam::DVec3;

use super::{build_fronts, material, Dims, GenerateError, PartBuilder};

/// Najkratšie dno šikmej skrinky (mm).
const MIN_BOTTOM: f64 = 50.0;
/// Najväčší presah šikmého boku dozadu (mm).
const MAX_EXTEND: f64 = 2000.0;

/// Chrbát šikmej skrinky.
#[derive(Debug, Clone, Copy, PartialEq)]
enum BackKind {
    None,
    /// pohľadový z dosky korpusu, korpus na ňom
    Finished,
    /// naložený zozadu
    Applied,
    /// v drážke dna, vrchu a šikmého boku, za rovným bokom
    Groove,
}

/// Postaví šikmú skrinku; diely sa rátajú pre šikmý ľavý bok a pri pravom sa zrkadlia.
pub(crate) fn build_cabinet(
    spec: &CabinetSpec,
    slant: &Slant,
    standard: &Standard,
    panel: &Material,
    back: &Material,
    front: &Material,
    materials: &MaterialLibrary,
) -> Result<CabinetModel, GenerateError> {
    let invalid = |message: String| GenerateError::Invalid {
        cabinet: spec.id.clone(),
        message,
    };
    let ss = &standard.slanted;
    let side_mat = match &ss.side_material {
        Some(id) => material(materials, id)?,
        None => panel,
    };
    let (w, h, d) = (spec.w, spec.h, spec.d);
    let t = panel.thickness;
    let tv = side_mat.thickness;
    let tb = back.thickness;
    let finite = [
        w,
        h,
        d,
        slant.bottom,
        slant.top,
        slant.extend_back,
        ss.end_inset,
        ss.back_insert_bottom,
        ss.back_insert_side,
        ss.min_top,
        ss.max_angle,
    ];
    if finite.iter().any(|v| !v.is_finite()) {
        return Err(invalid(tr!("generate.slanted.bad_values")));
    }
    if !(0.0..=MAX_EXTEND).contains(&slant.extend_back) {
        return Err(invalid(tr!(
            "generate.slanted.bad_extend",
            value = slant.extend_back,
            max = MAX_EXTEND
        )));
    }
    let inset = ss.end_inset;
    if inset < 0.0 || 2.0 * (t + inset) >= h || w <= 2.0 * t || d <= 2.0 * t {
        return Err(invalid(tr!(
            "generate.slanted.too_small",
            w = w,
            h = h,
            d = d
        )));
    }
    if spec.adjustable_shelves > 0 || !spec.fixed_shelves.is_empty() {
        return Err(invalid(tr!("generate.slanted.no_shelves")));
    }

    // priamka vonkajšej plochy šikmého boku v čelnom pohľade (x od hrany so šikmým bokom)
    let shift = slant.top - slant.bottom;
    if shift < 0.0 {
        return Err(invalid(tr!("generate.slanted.must_narrow")));
    }
    let tan = shift / h;
    let angle = tan.atan();
    if angle.to_degrees() > ss.max_angle {
        return Err(invalid(tr!(
            "generate.slanted.too_steep",
            angle = format!("{:.1}", angle.to_degrees()),
            max = format!("{:.1}", ss.max_angle)
        )));
    }
    let (s, c) = angle.sin_cos();
    let outer = |z: f64| slant.bottom + z * tan;
    let inner = |z: f64| outer(z) + tv / c;

    let bs = &standard.back;
    let kind = if slant.finished_back {
        BackKind::Finished
    } else if !spec.back {
        BackKind::None
    } else {
        match bs.mount_for(spec.cabinet_type) {
            BackMount::Applied => BackKind::Applied,
            BackMount::Groove => BackKind::Groove,
        }
    };
    let gw = tb + bs.groove_width_clearance;
    // hĺbka korpusu, bokov a dna/vrchu
    let d_carcass = match kind {
        BackKind::Finished => d - t,
        BackKind::Applied => d - bs.applied_depth.unwrap_or(tb),
        BackKind::Groove | BackKind::None => d,
    };
    let yb = match kind {
        BackKind::Groove => d - bs.groove_offset_from_rear - gw,
        _ => d_carcass,
    };
    if d_carcass <= 2.0 * t || yb <= 2.0 * t {
        return Err(invalid(tr!(
            "generate.slanted.too_shallow",
            depth = d_carcass
        )));
    }
    let side_depth = yb;
    let panel_depth = match kind {
        BackKind::Groove => d,
        _ => d_carcass,
    };

    // (rola rovného boku, rola šikmého boku)
    let (straight_role, slanted_role) = match slant.side {
        SlantSide::Left => (Role::SideRight, Role::SideLeft),
        SlantSide::Right => (Role::SideLeft, Role::SideRight),
    };
    let mut b = PartBuilder::new(spec, panel);

    // rovný bok: x = +Z, y = +Y, z = −X (F = vnútorná plocha)
    b.part(
        straight_role,
        panel,
        h,
        side_depth,
        Transform::from_axes(DVec3::Z, DVec3::Y, DVec3::new(w, 0.0, 0.0)),
        true,
    );

    // dno: od šikmého boku (priesečník so spodnou plochou) po rovný bok, F = horná plocha
    let x_bottom = inner(inset);
    let bottom_len = w - t - x_bottom;
    if bottom_len < MIN_BOTTOM {
        return Err(invalid(tr!(
            "generate.slanted.bottom_too_short",
            length = format!("{bottom_len:.1}")
        )));
    }
    b.part(
        Role::Bottom,
        panel,
        bottom_len,
        panel_depth,
        Transform::from_axes(DVec3::X, DVec3::Y, DVec3::new(x_bottom, 0.0, inset)),
        true,
    );

    // vrch medzi bokmi, ak je pri vrchu medzi bokmi miesto; inak bok končí v rohu
    let z_top = h - inset;
    let x_top = inner(z_top - t);
    let top_len = w - t - x_top;
    let has_top = top_len >= ss.min_top;
    if has_top {
        // x = −X, y = +Y, z = −Z (F = spodná plocha)
        b.part(
            Role::Top,
            panel,
            top_len,
            panel_depth,
            Transform::from_axes(DVec3::NEG_X, DVec3::Y, DVec3::new(w - t, 0.0, z_top)),
            true,
        );
    }

    // šikmý bok: x = nadol po sklone, y = +Y (spredu dozadu), z = k interiéru (F)
    let x_axis = DVec3::new(-s, 0.0, -c);
    let (length, origin) = if has_top {
        // vnútorný roh pri vrchu korpusu
        (
            (h + tv * s) / c,
            DVec3::new(inner(h) - tv * c, 0.0, h + tv * s),
        )
    } else {
        // vonkajší roh pri vrchu (roh pri rovnom boku)
        (h / c, DVec3::new(outer(h), 0.0, h))
    };
    let slanted_width = match kind {
        BackKind::Applied => d_carcass,
        _ => d,
    } + slant.extend_back;
    let slanted_tf = Transform::from_axes(x_axis, DVec3::Y, origin);
    b.part(
        slanted_role,
        side_mat,
        length,
        slanted_width,
        slanted_tf,
        true,
    );

    // chrbát
    let mut side_groove = None;
    match kind {
        BackKind::None => {}
        BackKind::Finished => {
            // x = +X, y = +Z, z = −Y (F = predná plocha k interiéru)
            let x0 = inner(0.0);
            b.front(
                Role::FinishedBack,
                panel,
                w - x0,
                h,
                Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(x0, d, 0.0)),
            );
        }
        BackKind::Applied => {
            let side = bs.applied_inset_side.unwrap_or(0.0);
            let x0 = outer(0.0).max(0.0) + side;
            let z0 = bs.applied_inset_bottom.unwrap_or(0.0);
            let z1 = h - bs.applied_inset_top.unwrap_or(0.0);
            let (bw, bh) = (w - side - x0, z1 - z0);
            if bw <= EPS || bh <= EPS {
                return Err(invalid(tr!("generate.bad_back", width = bw, height = bh)));
            }
            b.part(
                Role::Back,
                back,
                bw,
                bh,
                Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(x0, d_carcass + tb, z0)),
                false,
            );
        }
        BackKind::Groove => {
            // ľavá hrana chrbta na priamke drážky v šikmom boku
            let groove_x = |z: f64| inner(z) - ss.back_insert_side / c;
            let x1 = w - bs.fit_clearance;
            let z0 = inset + t - ss.back_insert_bottom;
            let z1 = if has_top {
                z_top - t + ss.back_insert_bottom
            } else if tan > EPS {
                ((x1 - groove_x(0.0)) / tan).min(h)
            } else {
                h
            };
            let x0 = groove_x(z0);
            let (bw, bh) = (x1 - x0, z1 - z0);
            if bw <= EPS || bh <= EPS {
                return Err(invalid(tr!(
                    "generate.bad_back",
                    width = format!("{bw:.1}"),
                    height = format!("{bh:.1}")
                )));
            }
            let rear = yb + (gw - tb) / 2.0 + tb;
            b.part(
                Role::Back,
                back,
                bw,
                bh,
                Transform::from_axes(DVec3::X, DVec3::Z, DVec3::new(x0, rear, z0)),
                false,
            );
            // drážka v šikmom boku pozdĺž šikmej hrany chrbta (od spodku po vrch chrbta)
            let along = |z: f64| {
                (DVec3::new(groove_x(z), 0.0, z) - origin)
                    .dot(x_axis)
                    .clamp(0.0, length)
            };
            let (a, e) = (along(z1), along(z0));
            if e - a > EPS {
                let y = yb + gw / 2.0;
                side_groove = Some(Operation::Groove {
                    face: Face::F,
                    start: P2 { x: a, y },
                    end: P2 { x: e, y },
                    width: gw,
                    depth: bs.groove_depth,
                });
            }
        }
    }
    let mut parts = b.finish();
    if let (Some(op), Some(p)) = (
        side_groove,
        parts.iter_mut().find(|p| p.role == slanted_role),
    ) {
        p.operations.push(op);
    }
    if slant.side == SlantSide::Right {
        for p in &mut parts {
            mirror(p, w);
        }
    }

    // čelá ako pri rovnej skrinke (tvar šikmých dvierok model zatiaľ nemá)
    let dims = Dims {
        w,
        h,
        d: d_carcass,
        t,
        tb,
        wi: w - 2.0 * t,
        yb,
        gw,
        floor: inset,
    };
    let mut fb = PartBuilder::new(spec, panel);
    let (hardware, openings) =
        build_fronts(spec, standard, &dims, front, materials, &mut fb).map_err(invalid)?;
    let slanted_id = format!("{}-{}", spec.id, slanted_role.id_code());
    for hw in &hardware {
        match hw {
            Hardware::Hinge { side, .. } if *side == slanted_id => {
                return Err(invalid(tr!("generate.slanted.hinges_on_slant")));
            }
            Hardware::Drawer { .. } => {
                return Err(invalid(tr!("generate.slanted.no_drawers")));
            }
            _ => {}
        }
    }
    parts.extend(fb.finish());

    Ok(CabinetModel {
        id: spec.id.clone(),
        cabinet_type: spec.cabinet_type,
        w,
        h,
        d: d_carcass,
        parts,
        hardware,
        options: spec.options.resolve(&standard.cabinet_options),
        openings,
    })
}

/// Zrkadlí diel podľa roviny `x = w / 2` (šikmý pravý bok). Os `x` dielu sa obráti, aby rámec
/// ostal pravotočivý: plochy F/B ostanú, hrany `X0`/`X1` sa vymenia.
fn mirror(p: &mut Part, w: f64) {
    let m = |v: DVec3| DVec3::new(-v.x, v.y, v.z);
    let tf = p.transform;
    let mx = m(tf.x_axis);
    let origin = DVec3::new(w - tf.origin.x, tf.origin.y, tf.origin.z) + mx * p.length;
    p.transform = Transform::from_axes(-mx, m(tf.y_axis), origin);
    std::mem::swap(&mut p.edges.x0, &mut p.edges.x1);
    let len = p.length;
    for op in &mut p.operations {
        match op {
            Operation::Groove { start, end, .. } => {
                start.x = len - start.x;
                end.x = len - end.x;
            }
            Operation::Hole { face, x, .. } => {
                *x = len - *x;
                *face = match *face {
                    Face::X0 => Face::X1,
                    Face::X1 => Face::X0,
                    other => other,
                };
            }
        }
    }
}
