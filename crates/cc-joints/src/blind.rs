//! Spoj pevného panelu slepého rohu s korpusom (CV REF skr. 10 `S_KUC_ROH_P`, findings 11 §5.2).
//!
//! Panel (rola `FalseFront`) leží zadnou plochou F na prednej hrane boku, ktorý k nemu
//! vystupuje (`cc-generate`, `[blind_corner] side_extend`):
//! - bok ↔ panel: vzor `[dowel]` (3/3/3) po celej prednej hrane boku – kolíky Ø8 × 26 do hrany
//!   boku a Ø8 × 13 do plochy F panelu, konfirmát/vrut Ø5 × 13 iba do panelu
//!   (REF: 32/96/349/413/666/730, K 64/381/698, 9 od kraja panelu = stred boku),
//! - dno ↔ panel cez medzeru pred korpusom: kolíky v polohách `panel_bottom_dowels` od
//!   vnútornej plochy boku (REF: 32/96/350/414, v paneli 9,5 nad spodnou hranou).

use cc_model::{CabinetModel, JointType, Operation, Role, Standard};
use glam::DVec3;

use crate::placement::joint_positions;
use crate::{push_holes, Bbox, CabHole, Fail};

/// Tolerancia dotyku plôch (mm).
const TOUCH: f64 = 1e-3;

/// Diery spoja panelu s bokom a s dnom do `new_ops`.
pub(crate) fn blind_panel_ops(
    cab: &CabinetModel,
    boxes: &[Bbox],
    standard: &Standard,
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let bc = &standard.blind_corner;
    if !cab.options.joints || (!bc.panel_side_joint && bc.panel_bottom_dowels.is_empty()) {
        return Ok(());
    }
    let parts = || cab.parts.iter().zip(boxes).enumerate();
    for (pi, (panel, pb)) in parts() {
        if panel.role != Role::FalseFront {
            continue;
        }
        // bok, ktorý sa prednou hranou dotýka zadnej plochy panelu
        let side = parts().find(|(_, (p, b))| {
            matches!(p.role, Role::SideLeft | Role::SideRight)
                && (b.min.y - pb.max.y).abs() <= TOUCH
                && b.min.x < pb.max.x - TOUCH
                && pb.min.x < b.max.x - TOUCH
                && b.min.z < pb.max.z - TOUCH
                && pb.min.z < b.max.z - TOUCH
        });
        let Some((si, (side_part, sb))) = side else {
            continue;
        };
        if bc.panel_side_joint {
            side_joint(cab, standard, (pi, pb), (si, sb), new_ops)?;
        }
        let from_left = side_part.role == Role::SideLeft;
        for (bi, (bottom, bb)) in parts() {
            if bottom.role == Role::Bottom {
                bottom_joint(cab, standard, (pi, pb), (bi, bb), from_left, new_ops)?;
            }
        }
    }
    Ok(())
}

/// Kolíky (a konfirmáty) medzi prednou hranou boku a plochou F panelu.
fn side_joint(
    cab: &CabinetModel,
    standard: &Standard,
    (pi, pb): (usize, &Bbox),
    (si, sb): (usize, &Bbox),
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let std = &standard.dowel;
    let r = std.diameter / 2.0;
    // v strede hrúbky boku, iba ak tam je panel
    let x = (sb.min.x + sb.max.x) / 2.0;
    if x - r < pb.min.x - TOUCH || x + r > pb.max.x + TOUCH {
        return Ok(());
    }
    // vzor z celej hrany boku, diery mimo panelu sa vynechajú
    let (lo, hi) = (sb.min.z.max(pb.min.z), sb.max.z.min(pb.max.z));
    let inside = |z: &f64| *z - r >= lo - TOUCH && *z + r <= hi + TOUCH;
    let Some((dowels, connectors)) = joint_positions(sb.min.z, sb.max.z, std) else {
        return Ok(());
    };
    let (y_panel, y_side) = (pb.max.y, sb.min.y);
    let mut panel_holes = Vec::new();
    let mut side_holes = Vec::new();
    for z in dowels.iter().filter(|z| inside(z)) {
        panel_holes.push(hole(
            x,
            y_panel,
            *z,
            -1.0,
            std.diameter,
            std.face_depth,
            true,
        ));
        side_holes.push(hole(x, y_side, *z, 1.0, std.diameter, std.edge_depth, true));
    }
    if matches!(
        cab.options.joint_type,
        JointType::Confirmat | JointType::Screw
    ) {
        for z in connectors.iter().filter(|z| inside(z)) {
            panel_holes.push(hole(
                x,
                y_panel,
                *z,
                -1.0,
                std.confirmat_diameter,
                std.confirmat_depth,
                false,
            ));
            if let (JointType::Confirmat, Some([d, depth])) =
                (cab.options.joint_type, std.confirmat_edge)
            {
                side_holes.push(hole(x, y_side, *z, 1.0, d, depth, false));
            }
        }
    }
    push_holes(cab, new_ops, pi, &panel_holes)?;
    push_holes(cab, new_ops, si, &side_holes)
}

/// Kolíky medzi prednou hranou dna a plochou F panelu cez medzeru pred korpusom.
fn bottom_joint(
    cab: &CabinetModel,
    standard: &Standard,
    (pi, pb): (usize, &Bbox),
    (bi, bb): (usize, &Bbox),
    from_left: bool,
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let positions = &standard.blind_corner.panel_bottom_dowels;
    let std = &standard.dowel;
    let r = std.diameter / 2.0;
    // panel je pred dnom najviac o medzeru čiel pred korpusom a kryje celú hrúbku dna
    let gap = bb.min.y - pb.max.y;
    let max_gap = standard.fronts.gap_to_carcass;
    if positions.is_empty()
        || !max_gap.is_finite()
        || gap < -TOUCH
        || gap > max_gap + TOUCH
        || bb.min.z < pb.min.z - TOUCH
        || bb.max.z > pb.max.z + TOUCH
    {
        return Ok(());
    }
    let z = (bb.min.z + bb.max.z) / 2.0;
    let (x_lo, x_hi) = (bb.min.x.max(pb.min.x), bb.max.x.min(pb.max.x));
    let mut panel_holes = Vec::new();
    let mut bottom_holes = Vec::new();
    for s in positions.iter().filter(|s| s.is_finite() && **s > 0.0) {
        let x = if from_left {
            bb.min.x + s
        } else {
            bb.max.x - s
        };
        if x - r < x_lo - TOUCH || x + r > x_hi + TOUCH {
            continue;
        }
        panel_holes.push(hole(
            x,
            pb.max.y,
            z,
            -1.0,
            std.diameter,
            std.face_depth,
            true,
        ));
        bottom_holes.push(hole(
            x,
            bb.min.y,
            z,
            1.0,
            std.diameter,
            std.edge_depth,
            true,
        ));
    }
    push_holes(cab, new_ops, pi, &panel_holes)?;
    push_holes(cab, new_ops, bi, &bottom_holes)
}

/// Diera vŕtaná v smere ±Y skrinky (`dir_y` = −1 dopredu do panelu, +1 dozadu do hrany).
fn hole(x: f64, y: f64, z: f64, dir_y: f64, diameter: f64, depth: f64, dowel: bool) -> CabHole {
    CabHole {
        entry: DVec3::new(x, y, z),
        dir: DVec3::new(0.0, dir_y, 0.0),
        diameter,
        depth,
        dowel,
    }
}
