//! Kolíky lišty blendy (findings 11 §5.3, 12 §2): lišta sa hranou dotýka čela blendy
//! a dna blendy (pri skrinke) alebo slepého čela (samostatná blenda). Do hrany lišty ide
//! kolík Ø × `edge_depth`, do plochy druhého dielu Ø × `face_depth` (`[dowel]` štandardu),
//! rozmiestnenie podľa `[filler] dowels` / `standalone_dowels`.

use cc_i18n::tr;
use cc_model::{CabinetModel, Operation, Role, Standard};
use glam::DVec3;

use crate::{contact, push_holes, unit, Bbox, CabHole, Fail};

/// Diery kolíkov lišty `strip` do jej hrany a do dielov, ktorých sa hranou dotýka.
pub(crate) fn strip_ops(
    cab: &CabinetModel,
    boxes: &[Bbox],
    standard: &Standard,
    strip: &str,
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let si = cab
        .parts
        .iter()
        .position(|p| p.id == strip && p.role == Role::FillerStrip)
        .ok_or_else(|| Fail::from(tr!("joints.filler_strip_missing", strip = strip)))?;
    let Some(sb) = boxes.get(si).copied() else {
        return Ok(());
    };
    let dw = &standard.dowel;
    for (k, part) in cab.parts.iter().enumerate() {
        // čelo a dno blendy pri skrinke; slepé čelo samostatnej blendy
        let pattern = match part.role {
            Role::FillerFront | Role::FillerBottom => &standard.filler.dowels,
            r if r.is_front() => &standard.filler.standalone_dowels,
            _ => continue,
        };
        let Some(pb) = boxes.get(k) else {
            continue;
        };
        let Some(c) = contact(&sb, pb) else {
            continue;
        };
        // hrana lišty: dotyková rovina je kolmá na niektorý rozmer lišty, ktorý nie je hrúbka
        let along: Vec<usize> = (0..3).filter(|a| *a != c.axis).collect();
        let (Some(&a0), Some(&a1)) = (along.first(), along.get(1)) else {
            continue;
        };
        let len = |a: usize| c.overlap[a].1 - c.overlap[a].0;
        // pozdĺž dlhšieho prieniku, naprieč v strede (stred hrúbky lišty)
        let (run, across) = if len(a0) >= len(a1) {
            (a0, a1)
        } else {
            (a1, a0)
        };
        let into_strip = if sb.center()[c.axis] > c.plane {
            unit(c.axis)
        } else {
            -unit(c.axis)
        };
        let mid = (c.overlap[across].0 + c.overlap[across].1) / 2.0;
        let mut strip_holes = Vec::new();
        let mut part_holes = Vec::new();
        for pos in pattern.positions(len(run)) {
            let mut entry = DVec3::ZERO;
            entry[c.axis] = c.plane;
            entry[run] = c.overlap[run].0 + pos;
            entry[across] = mid;
            strip_holes.push(CabHole {
                entry,
                dir: into_strip,
                diameter: dw.diameter,
                depth: dw.edge_depth,
                dowel: true,
            });
            part_holes.push(CabHole {
                entry,
                dir: -into_strip,
                diameter: dw.diameter,
                depth: dw.face_depth,
                dowel: true,
            });
        }
        push_holes(cab, new_ops, si, &strip_holes)?;
        push_holes(cab, new_ops, k, &part_holes)?;
    }
    Ok(())
}
