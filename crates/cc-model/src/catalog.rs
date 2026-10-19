//! Katalógy skriniek dielne (`catalogs/<meno>.toml` vedľa projektu): pomenované skrinky,
//! ktoré sa pridajú jedným klikom. Položka má rovnaké polia ako `[[cabinet]]` okrem polohy.
//! Používateľ si katalógy vytvára a ukladá do nich skrinky zo zákazky.

use serde::{Deserialize, Serialize};

use crate::{CabinetOptions, CabinetSpec, CabinetType, FrontMount, FrontReveals, FrontSpec};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    #[serde(default, rename = "item", skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<CatalogItem>,
}

impl Catalog {
    pub fn item(&self, id: &str) -> Option<&CatalogItem> {
        self.items.iter().find(|i| i.id == id)
    }
}

/// Katalóg s menom (meno = meno súboru bez `.toml`).
#[derive(Debug, Clone, PartialEq)]
pub struct NamedCatalog {
    pub name: String,
    pub catalog: Catalog,
}

/// Položka podľa ID: z katalógu `from`, inak z prvého katalógu, ktorý ju má.
pub fn find_catalog_item<'a>(
    catalogs: &'a [NamedCatalog],
    id: &str,
    from: Option<&str>,
) -> Option<(&'a str, &'a CatalogItem)> {
    catalogs
        .iter()
        .filter(|c| from.is_none_or(|f| f == c.name))
        .find_map(|c| c.catalog.item(id).map(|i| (c.name.as_str(), i)))
}

/// Platné meno katalógu (aj meno súboru): písmená, číslice, medzera, `_`, `-`, `.`; nie `..`.
pub fn valid_catalog_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty()
        && n.chars().count() <= 60
        && n == name
        && !n.starts_with('.')
        && !n.contains("..")
        && n.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.'))
}

/// Jedna skrinka katalógu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogItem {
    /// Označenie v katalógu (napr. `S_KUCH_D`).
    pub id: String,
    /// Meno v ponuke.
    pub name: String,
    /// Skupina v ponuke (napr. „Spodné“).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    #[serde(rename = "type")]
    pub cabinet_type: CabinetType,
    pub w: f64,
    pub h: f64,
    pub d: f64,
    #[serde(default)]
    pub adjustable_shelves: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed_shelves: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fronts: Vec<FrontSpec>,
    #[serde(default, skip_serializing_if = "FrontReveals::is_empty")]
    pub reveals: FrontReveals,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub front_mount: Option<FrontMount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hinge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawer_length: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_style: Option<crate::TopStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stretcher_drop: Option<f64>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub back: bool,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub carcass: bool,
    /// Výškové umiestnenie (Placement v CV), napr. horné `{ above_elevation = 1518 }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<crate::Placement>,
    /// Zámok šírky (v CV „Stretching – Width“ položky katalógu vypnuté).
    #[serde(default, skip_serializing_if = "is_false")]
    pub lock_width: bool,
    /// Blenda (napr. katalógová BLENDA: skrinka bez korpusu, lišta a slepé čelo s presahom).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filler: Option<crate::FillerSpec>,
    #[serde(default, skip_serializing_if = "CabinetOptions::is_empty")]
    pub options: CabinetOptions,
    /// Otvorená nika (`[niche]` štandardu).
    #[serde(default, skip_serializing_if = "is_false")]
    pub niche: bool,
    /// LED rampa pod skrinkou.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub led_rail: Option<crate::LedRail>,
}

fn yes() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl CatalogItem {
    /// Položka zo skrinky zákazky (bez polohy v miestnosti).
    pub fn from_spec(spec: &CabinetSpec, id: &str, name: &str, group: &str) -> CatalogItem {
        CatalogItem {
            id: id.to_owned(),
            name: name.to_owned(),
            group: group.to_owned(),
            cabinet_type: spec.cabinet_type,
            w: spec.w,
            h: spec.h,
            d: spec.d,
            adjustable_shelves: spec.adjustable_shelves,
            fixed_shelves: spec.fixed_shelves.clone(),
            fronts: spec.fronts.clone(),
            reveals: spec.reveals.clone(),
            front_mount: spec.front_mount,
            hinge: spec.hinge.clone(),
            drawer: spec.drawer.clone(),
            drawer_length: spec.drawer_length,
            top_style: spec.top_style,
            stretcher_drop: spec.stretcher_drop,
            back: spec.back,
            carcass: spec.carcass,
            lock_width: spec.lock_width,
            placement: spec.placement,
            filler: spec.filler.clone(),
            options: spec.options.clone(),
            niche: spec.niche,
            led_rail: spec.led_rail,
        }
    }

    /// Zadanie skrinky s ID `id` (bez steny).
    pub fn to_spec(&self, id: &str) -> CabinetSpec {
        CabinetSpec {
            id: id.to_owned(),
            cabinet_type: self.cabinet_type,
            w: self.w,
            h: self.h,
            d: self.d,
            adjustable_shelves: self.adjustable_shelves,
            fixed_shelves: self.fixed_shelves.clone(),
            wall: None,
            at: 0.0,
            elevation: None,
            outset: 0.0,
            fronts: self.fronts.clone(),
            reveals: self.reveals.clone(),
            front_mount: self.front_mount,
            hinge: self.hinge.clone(),
            drawer: self.drawer.clone(),
            drawer_length: self.drawer_length,
            top_style: self.top_style,
            stretcher_drop: self.stretcher_drop,
            back: self.back,
            carcass: self.carcass,
            lock_width: self.lock_width,
            comment: None,
            placement: self.placement,
            catalog: Some(self.id.clone()),
            cv_id: None,
            cv_fronts: None,
            filler: self.filler.clone(),
            options: self.options.clone(),
            slant: None,
            niche: self.niche,
            led_rail: self.led_rail,
        }
    }
}
