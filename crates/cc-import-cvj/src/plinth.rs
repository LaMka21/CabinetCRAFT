//! Sokel, bočný sokel a krycí bok zo skrinky CV (findings 11 §5.5, 12 §6).
//!
//! Dielňa v CV nechá sokel `BO „SOKEL“` iba na jednej skrinke zostavy a ručne mu zadá
//! polohu a dĺžku cez celú zostavu (REF: skr. 1 = 2680 od X −60, skr. 13 = 1944 od
//! X −1372); ostatné sokle zmaže. Bočný sokel „SOKEL BOCNY“ (kód boku `RU`) a krycí bok
//! „KRYCI BOK“ sú samostatné diely bez `PID`. Import ich prenesie ako atribúty skrinky:
//! sokel s polohou a dĺžkou odlišnou od skrinky ako `plinth_span`, strany bočného sokla
//! a krycieho boku ako `side_plinth_*` / `cover_side_*`.
//!
//! Rám: CV skrinka má `X` doprava, `Y` nahor, `Z` dopredu; náš `x` = `X − ľavý okraj
//! korpusu`, výška od spodku korpusu.

use cc_model::{CabinetOptions, PlinthSpan};

use crate::job::{Bbox, CvjCabinet, PartRole};

/// Tolerancia „rovnaká poloha / dĺžka“ (mm).
const TOL_LEN: f64 = 0.5;
/// Tolerancia „rovnaká výška“ (mm).
const TOL_HEIGHT: f64 = 0.05;

fn loose<'a>(
    c: &'a CvjCabinet,
    pred: impl Fn(&str) -> bool + 'a,
) -> impl Iterator<Item = Bbox> + 'a {
    c.parts
        .iter()
        .filter(move |p| p.role == PartRole::Loose && pred(&p.description.trim().to_uppercase()))
        .map(|p| p.bbox)
}

fn finite(b: &Bbox) -> bool {
    b.min.is_finite() && b.max.is_finite()
}

/// Doplní atribúty sokla a krycieho boku skrinky `c`; `gap_top` je medzera sokla pod
/// skrinkou podľa štandardu (výška sokla, ktorá sa nemusí zapísať).
pub(crate) fn options(c: &CvjCabinet, gap_top: f64, o: &mut CabinetOptions) {
    let carcass = c.carcass;
    if !finite(&carcass) {
        return;
    }
    let mid = (carcass.min.x + carcass.max.x) / 2.0;
    let is_left = |b: &Bbox| (b.min.x + b.max.x) / 2.0 < mid;
    // krycí bok pri boku skrinky
    for b in loose(c, |d| d == "KRYCI BOK").filter(finite) {
        if is_left(&b) {
            o.cover_side_left = Some(true);
        } else {
            o.cover_side_right = Some(true);
        }
    }
    // predný sokel (prvý diel „SOKEL“)
    let Some(front) = loose(c, |d| d == "SOKEL").find(finite) else {
        return;
    };
    let size = front.size();
    let from = front.min.x - carcass.min.x;
    let length = size.x;
    let height = size.y;
    let computed = carcass.min.y - front.min.y - gap_top;
    let own_height = (height - computed).abs() > TOL_HEIGHT;
    let default_span =
        from.abs() <= TOL_LEN && (length - carcass.size().x).abs() <= TOL_LEN && !own_height;
    if !default_span && length > 0.0 && height > 0.0 {
        o.plinth_span = Some(PlinthSpan {
            from: round(from),
            length: round(length),
            height: own_height.then(|| round(height)),
        });
    }
    // bočné sokle: zapnuté tam, kde sú v CV, inak vypnuté (zostava podľa CV)
    let mut left = false;
    let mut right = false;
    let mut side_height = None;
    for b in loose(c, |d| d.starts_with("SOKEL BOC")).filter(finite) {
        if is_left(&b) {
            left = true;
        } else {
            right = true;
        }
        side_height.get_or_insert(b.size().y);
    }
    o.side_plinth_left = Some(left);
    o.side_plinth_right = Some(right);
    o.side_plinth_height = side_height
        .filter(|h| *h > 0.0 && (h - height).abs() > TOL_HEIGHT)
        .map(round);
}

/// Zaokrúhlenie na tisíciny mm (čistý zápis do TOML), bez −0.
fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0 + 0.0
}
