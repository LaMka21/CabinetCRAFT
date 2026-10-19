//! Slepý roh (CV `S_KUC_ROH_P/_L`, findings 11 §5.2, 12 §1): bok na strane pevného
//! panelu vystupuje dopredu až k panelu (REF: LU 391 = D − 4 + 3 na Z = `CAB.DZ + 3`),
//! aby sa panel dal pripojiť na jeho prednú hranu. Spoj panelu robí `cc-joints`.

use cc_model::{CabinetSpec, FrontKind, FrontMount, Standard};

/// O koľko vystupuje ľavý a pravý bok pred čelo korpusu (`[blind_corner] side_extend`).
///
/// Bok vystupuje na strane, kde je pevný panel (`FrontKind::Panel`) v krajnom stĺpci
/// niektorého riadku čiel naložených na korpus. Najviac po zadnú plochu panelu
/// (`[fronts] gap_to_carcass`), aby bok nezasahoval do panelu.
pub(crate) fn blind_side_extend(spec: &CabinetSpec, standard: &Standard) -> (f64, f64) {
    let (ext, gap) = (
        standard.blind_corner.side_extend,
        standard.fronts.gap_to_carcass,
    );
    let mount = spec.front_mount.unwrap_or(standard.fronts.mount);
    if !spec.carcass
        || mount != FrontMount::Overlay
        || !ext.is_finite()
        || !gap.is_finite()
        || ext <= 0.0
        || gap <= 0.0
    {
        return (0.0, 0.0);
    }
    let ext = ext.min(gap);
    let (mut left, mut right) = (false, false);
    for row in &spec.fronts {
        if let Some(first) = row.columns.first() {
            left |= first.kind == FrontKind::Panel;
        }
        if let Some(last) = row.columns.last() {
            right |= last.kind == FrontKind::Panel;
        }
    }
    (if left { ext } else { 0.0 }, if right { ext } else { 0.0 })
}
