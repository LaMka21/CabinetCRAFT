//! Rozmiestnenie kolíkov pozdĺž spoja (kap. 6.1) a radu dier (kap. 6.3).

use cc_model::{DowelLayout, DowelStandard, LineBoreStandard, EPS};

/// Polohy kolíkov na úseku `[s0, s1]` podľa `layout`. `None`, ak je úsek príliš krátky.
pub fn dowel_positions(s0: f64, s1: f64, std: &DowelStandard) -> Option<Vec<f64>> {
    match std.layout {
        DowelLayout::Even => dowel_positions_even(s0, s1, std),
        DowelLayout::Pairs => dowel_positions_pairs(s0, s1, std),
        DowelLayout::Cv333 => cv333(s1 - s0, std).map(|(d, _)| d.iter().map(|v| s0 + v).collect()),
    }
}

/// Kolíky a miesta konfirmátu (vrutu) na úseku `[s0, s1]`. Pri `cv333` podľa vzoru CV,
/// inak konfirmát v strede každej dvojice kolíkov.
pub fn joint_positions(s0: f64, s1: f64, std: &DowelStandard) -> Option<(Vec<f64>, Vec<f64>)> {
    if std.layout == DowelLayout::Cv333 {
        let (d, k) = cv333(s1 - s0, std)?;
        let shift = |v: Vec<f64>| v.into_iter().map(|x| s0 + x).collect();
        return Some((shift(d), shift(k)));
    }
    let dowels = dowel_positions(s0, s1, std)?;
    let connectors = dowels
        .chunks(2)
        .filter_map(|p| match p {
            [a, b] => Some((a + b) / 2.0),
            _ => None,
        })
        .collect();
    Some((dowels, connectors))
}

/// Vzor „spoj 3/3/3“ (CV, findings 11 §1.3) na hrane dĺžky `len`: (kolíky, konfirmáty)
/// od začiatku hrany. Kraj `end_offset` (32), pri krátkych hranách `short_end_offset` (18),
/// kolíky okolo konfirmátu ± polovica `pair_pitch`, skupiny po `group_spacing`.
fn cv333(len: f64, std: &DowelStandard) -> Option<(Vec<f64>, Vec<f64>)> {
    let half = std.pair_pitch / 2.0;
    let edge = |short_limit: f64| {
        if len <= short_limit + EPS {
            std.short_end_offset
        } else {
            std.end_offset
        }
    };
    if !len.is_finite() || len <= 67.0 + EPS {
        return None;
    }
    if len <= 120.0 + EPS {
        let e = edge(98.0);
        return Some((vec![e], vec![len - e]));
    }
    if len <= 210.0 + EPS {
        let e = edge(144.0);
        return Some((vec![e, len - e], vec![len / 2.0]));
    }
    let e = std.end_offset;
    if len <= 270.0 + EPS {
        return Some((vec![e, len / 2.0, len - e], vec![e + half, len - e - half]));
    }
    if len <= 490.0 + EPS {
        let mut d = vec![e, e + 2.0 * half, len - e - 2.0 * half, len - e];
        if len > 380.0 + EPS {
            d.extend([len / 2.0 - half, len / 2.0 + half]);
        }
        d.sort_by(f64::total_cmp);
        return Some((d, vec![e + half, len - e - half]));
    }
    let [near, far] = std.group_spacing;
    let dis = if (len - 3.0 * 2.0 * half) / 3.0 < near {
        near
    } else {
        far
    };
    if dis <= EPS {
        return None;
    }
    let n = ((len - 4.0 * half) / dis).floor().clamp(1.0, 64.0) as usize + 1;
    let (k0, k1) = (e + half, len - e - half);
    let k: Vec<f64> = (0..n)
        .map(|i| k0 + (k1 - k0) * i as f64 / (n - 1) as f64)
        .collect();
    let d = k.iter().flat_map(|&c| [c - half, c + half]).collect();
    Some((d, k))
}

/// Dvojice kolíkov (ako v programoch CV pre CX100): dvojica `pair_pitch` od seba,
/// prvá `end_offset` od začiatku, posledná `end_offset` od konca, medzi nimi ďalšie
/// dvojice rovnomerne tak, aby ich začiatky boli najviac `max_pair_spacing` od seba.
/// Ak sa dve dvojice nezmestia, dva kolíky `short_end_offset` od koncov.
///
/// Príklady z programov CV: 330 → 32, 96, 234, 298 · 388 → 32, 96, 162, 226, 292, 356 ·
/// 506 → 32, 96, 221, 285, 410, 474 · 128 → 18, 110.
pub fn dowel_positions_pairs(s0: f64, s1: f64, std: &DowelStandard) -> Option<Vec<f64>> {
    let len = s1 - s0;
    let a = std.end_offset;
    let p = std.pair_pitch;
    if len < 2.0 * (a + p) - EPS {
        let so = std.short_end_offset;
        if len < 2.0 * so + 8.0 - EPS {
            return None;
        }
        return Some(vec![s0 + so, s1 - so]);
    }
    let span = len - 2.0 * a - p;
    let n = ((span / std.max_pair_spacing - EPS).ceil() as usize + 1).max(2);
    let mut out = Vec::with_capacity(2 * n);
    for i in 0..n {
        let start = s0 + a + span * i as f64 / (n - 1) as f64;
        out.push(start);
        out.push(start + p);
    }
    Some(out)
}

/// Jednotlivé kolíky rovnomerne s max. rozstupom `max_spacing` (kap. 6.1).
/// `None`, ak je úsek kratší ako `2a + 8`.
pub fn dowel_positions_even(s0: f64, s1: f64, std: &DowelStandard) -> Option<Vec<f64>> {
    let len = s1 - s0;
    let a = std.end_offset;
    if len < 2.0 * a + 8.0 - EPS {
        return None;
    }
    let span = len - 2.0 * a;
    let n = ((span / std.max_spacing).ceil() as usize + 1).max(2);
    let mut out: Vec<f64> = (0..n)
        .map(|i| s0 + a + span * i as f64 / (n - 1) as f64)
        .collect();
    if std.snap_32 && n > 2 {
        let first = out[0];
        for s in &mut out[1..n - 1] {
            *s = first + ((*s - first) / 32.0).round() * 32.0;
        }
    }
    Some(out)
}

/// Výšky dier radu v úseku `[lo, hi]` (horná plocha spodného dielu, spodná plocha horného).
pub fn line_bore_heights(lo: f64, hi: f64, std: &LineBoreStandard) -> Vec<f64> {
    let z0 = lo + std.start_above_bottom;
    let z_max = hi - std.end_below_top;
    let mut out = Vec::new();
    let mut k = 0u32;
    loop {
        let z = z0 + f64::from(k) * std.pitch;
        if z > z_max + EPS {
            break;
        }
        out.push(z);
        k += 1;
    }
    out
}

/// Skupiny dier pri policiach (`per_shelf`): stredná diera skupiny je na mriežke
/// `pitch` (od `lo + start_above_bottom`) najbližšie k spodnej ploche police `shelf_z`.
/// Diery mimo `[lo + start_above_bottom, hi − end_below_top]` sa vynechajú.
pub fn shelf_hole_heights(lo: f64, hi: f64, shelf_z: &[f64], std: &LineBoreStandard) -> Vec<f64> {
    let z0 = lo + std.start_above_bottom;
    let z_max = hi - std.end_below_top;
    let n = std.holes_per_shelf.max(1) as i64;
    let mut out: Vec<f64> = Vec::new();
    for &zs in shelf_z {
        if zs < lo - EPS || zs > hi + EPS {
            continue;
        }
        let center = match std.center_below_shelf {
            Some(below) => zs - below,
            None => z0 + ((zs - z0) / std.pitch).round() * std.pitch,
        };
        for k in 0..n {
            let z = center + (k as f64 - (n - 1) as f64 / 2.0) * std.pitch;
            if z >= z0 - EPS && z <= z_max + EPS && !out.iter().any(|o| (o - z).abs() < EPS) {
                out.push(z);
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn dowel() -> DowelStandard {
        DowelStandard {
            diameter: 8.0,
            face_depth: 12.0,
            edge_depth: 25.0,
            end_offset: 32.0,
            max_spacing: 128.0,
            snap_32: false,
            layout: DowelLayout::Even,
            pair_pitch: 64.0,
            max_pair_spacing: 250.0,
            short_end_offset: 18.0,
            group_spacing: [160.0, 256.0],
            stop_at_back: true,
            confirmat_diameter: 5.0,
            confirmat_depth: 13.0,
            confirmat_edge: None,
        }
    }

    #[test]
    fn spec_chapter_10_dowels() {
        let p = dowel_positions(0.0, 541.5, &dowel()).unwrap();
        let expected = [32.0, 151.375, 270.75, 390.125, 509.5];
        assert_eq!(p.len(), expected.len());
        for (a, b) in p.iter().zip(expected) {
            assert_abs_diff_eq!(*a, b, epsilon = 1e-9);
        }
        assert_eq!(
            dowel_positions(0.0, 100.0, &dowel()).unwrap(),
            vec![32.0, 68.0]
        );
        assert_eq!(
            dowel_positions(441.5, 541.5, &dowel()).unwrap(),
            vec![473.5, 509.5]
        );
    }

    #[test]
    fn too_short_joint() {
        assert!(dowel_positions(0.0, 71.9, &dowel()).is_none());
        assert!(dowel_positions(0.0, 72.0, &dowel()).is_some());
    }

    /// Vlastnosti pre všetky dĺžky 72–3000 mm po 0,5 mm (kap. 9: property testy).
    #[test]
    fn dowel_properties() {
        for snap in [false, true] {
            let std = DowelStandard {
                snap_32: snap,
                ..dowel()
            };
            let mut len = 72.0;
            while len <= 3000.0 {
                let p = dowel_positions(0.0, len, &std).unwrap();
                assert!(p.len() >= 2);
                assert_abs_diff_eq!(p[0], 32.0, epsilon = 1e-9);
                assert_abs_diff_eq!(*p.last().unwrap(), len - 32.0, epsilon = 1e-9);
                for w in p.windows(2) {
                    assert!(w[1] > w[0], "dĺžka {len}: poradie {p:?}");
                    if !snap {
                        assert!(w[1] - w[0] <= 128.0 + 1e-9, "dĺžka {len}: {p:?}");
                    }
                }
                len += 0.5;
            }
        }
    }

    #[test]
    fn spec_chapter_10_line_bore() {
        let std = LineBoreStandard {
            diameter: 5.0,
            depth: 12.0,
            front_offset: 37.0,
            rear_offset: 37.0,
            pitch: 32.0,
            start_above_bottom: 64.0,
            end_below_top: 64.0,
            layout: cc_model::LineBoreLayout::Full,
            holes_per_shelf: 3,
            center_below_shelf: None,
            shelf_pin_holes: None,
        };
        let z = line_bore_heights(18.0, 702.0, &std);
        assert_eq!(z.len(), 18);
        assert_eq!(z[0], 82.0);
        assert_eq!(*z.last().unwrap(), 626.0);
    }

    fn pairs() -> DowelStandard {
        DowelStandard {
            diameter: 8.0,
            face_depth: 13.0,
            edge_depth: 26.0,
            layout: DowelLayout::Pairs,
            ..dowel()
        }
    }

    /// Polohy dvojíc z reálnych programov CV pre CX100 (zákazka REF2026).
    #[test]
    fn pairs_reproduce_cv_programs() {
        let p = |len: f64| dowel_positions(0.0, len, &pairs()).unwrap();
        // 966f0040 Dno 330
        assert_eq!(p(330.0), [32.0, 96.0, 234.0, 298.0]);
        // 966b0036 Lavy BOK A 309.5
        assert_eq!(p(309.5), [32.0, 96.0, 213.5, 277.5]);
        // 966b0047 NES_Lavy BOK B 351
        assert_eq!(p(351.0), [32.0, 96.0, 255.0, 319.0]);
        // 966b0073 Dno spodne 388
        assert_eq!(p(388.0), [32.0, 96.0, 162.0, 226.0, 292.0, 356.0]);
        // 966b0006 Dno spodne 506
        assert_eq!(p(506.0), [32.0, 96.0, 221.0, 285.0, 410.0, 474.0]);
        // 966f0004 Vlys vrchu 128 (krátky spoj)
        assert_eq!(p(128.0), [18.0, 110.0]);
        // príliš krátky
        assert!(dowel_positions(0.0, 40.0, &pairs()).is_none());
    }

    /// Vzor 3/3/3 z CV: kolíky aj konfirmáty ako v programoch zákazky REF.
    #[test]
    fn cv333_matches_cv_programs() {
        let std = DowelStandard {
            layout: DowelLayout::Cv333,
            ..pairs()
        };
        let p = |len: f64| joint_positions(0.0, len, &std).unwrap();
        assert_eq!(
            p(330.0),
            (vec![32.0, 96.0, 234.0, 298.0], vec![64.0, 266.0])
        );
        assert_eq!(
            p(388.0),
            (
                vec![32.0, 96.0, 162.0, 226.0, 292.0, 356.0],
                vec![64.0, 324.0]
            )
        );
        assert_eq!(
            p(506.0),
            (
                vec![32.0, 96.0, 221.0, 285.0, 410.0, 474.0],
                vec![64.0, 253.0, 442.0]
            )
        );
        assert_eq!(p(128.0), (vec![18.0, 110.0], vec![64.0]));
        assert_eq!(p(309.5).0, [32.0, 96.0, 213.5, 277.5]);
        // dlhá hrana: rozostup skupín 256 → 4 skupiny
        assert_eq!(p(900.0).1.len(), 4);
        assert!(joint_positions(0.0, 60.0, &std).is_none());
    }

    #[test]
    fn shelf_groups() {
        let std = LineBoreStandard {
            diameter: 5.0,
            depth: 10.0,
            front_offset: 37.0,
            rear_offset: 37.0,
            pitch: 32.0,
            start_above_bottom: 64.0,
            end_below_top: 64.0,
            layout: cc_model::LineBoreLayout::PerShelf,
            holes_per_shelf: 3,
            center_below_shelf: None,
            shelf_pin_holes: None,
        };
        // úsek 18 … 702, polica so spodnou plochou vo výške 342 → mriežka od 82: stred 338
        let z = shelf_hole_heights(18.0, 702.0, &[342.0], &std);
        assert_eq!(z, [306.0, 338.0, 370.0]);
        // dve police; skupina pri spodnom okraji sa oreže na 82
        let z = shelf_hole_heights(18.0, 702.0, &[90.0, 500.0], &std);
        assert_eq!(z, [82.0, 114.0, 466.0, 498.0, 530.0]);
    }
}
