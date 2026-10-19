//! Profil SCM nestingu: sekcia `[nesting]` v `machine.toml`.
//!
//! Formát programov tabúľ vychádza z programov, ktoré dielňa vyrobila pre svoj
//! nesting (zákazka REF2026, popis v `findings/08-nesting-scm.md`).
//! Celý text riadkov je v šablónach; kód rieši rozloženie, poradie a súradnice.

use serde::{Deserialize, Serialize};

use crate::template_problem::check_template;
use crate::LineEnding;
use crate::TemplateProblem;

/// Sekcia `[nesting]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestingProfile {
    /// `false` = hlavička a päta programu tabule ešte neboli overené na stroji.
    pub verified: bool,
    /// Počet desatinných miest (nuly na konci sa vynechajú).
    #[serde(default = "default_decimals")]
    pub decimals: u32,
    #[serde(default)]
    pub line_ending: LineEnding,
    /// Meno programu tabule bez prípony. Premenné: `{mat}` (poradie materiálu, 2 číslice),
    /// `{sheet}` (poradie tabule v materiáli, 2 číslice), `{material}` (id materiálu).
    pub sheet_name: String,
    /// Úvodné riadky programu tabule. Premenné: `{project} {program} {material} {l} {w} {t}`.
    pub header: Vec<String>,
    /// Záverečné riadky (parkovanie). Rovnaké premenné ako `header`.
    #[serde(default)]
    pub footer: Vec<String>,
    /// Materiály, ktoré na nesting nejdú (delia sa na píle): celé ich obrábanie ide na CX100.
    #[serde(default)]
    pub skip_materials: Vec<String>,
    pub layout: NestLayout,
    /// Rozmer tabule podľa materiálu (inak `layout.sheet_length × sheet_width`).
    #[serde(default, rename = "sheet")]
    pub sheets: Vec<NestSheetSize>,
    pub drill: NestDrill,
    pub route: NestRoute,
    pub remnant: NestRemnant,
    /// Súpis behu `CncRun.txt` pre obsluhu; bez sekcie sa nezapisuje.
    #[serde(default)]
    pub cnc_run: Option<NestCncRun>,
}

fn default_decimals() -> u32 {
    4
}

/// Rozloženie dielov na tabuli. Počiatok tabule je vľavo dole, X po dĺžke tabule.
///
/// Vlákno tabule ide po jej dĺžke (X): diel s vláknom po dĺžke sa neotáča,
/// diel s vláknom po šírke sa vždy otočí o 90°, diel bez vlákna sa otáča voľne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestLayout {
    /// Predvolený rozmer tabule (mm), napr. 2800 × 2070.
    pub sheet_length: f64,
    pub sheet_width: f64,
    /// Okraj tabule zo všetkých strán (mm).
    pub margin: f64,
    /// Medzera medzi dielmi (mm), napr. Ø frézy 12 + 2,2.
    pub spacing: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestSheetSize {
    pub material: String,
    pub length: f64,
    pub width: f64,
}

/// Vŕtanie do plochy hore (iba rovina Top).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestDrill {
    pub verified: bool,
    /// Výber roviny pred každou dierou (napr. `SelectWorkplane("Top");`). Bez premenných.
    pub select: String,
    /// Meno operácie. Premenné: `{n} {d} {depth}`.
    pub name: String,
    /// Riadok vŕtania. Premenné: `{name} {x} {y} {d} {depth} {tool} {kind}`.
    pub line: String,
    /// `{tool}`, ak ho neurčí variant (`-1` = stroj vyberie podľa priemeru).
    pub default_tool: String,
    /// `{kind}`, ak ho neurčí variant (`P` = plochý vrták).
    pub default_kind: String,
    /// Poradie skupín vŕtania podľa priemeru; ostatné priemery idú za nimi vzostupne.
    #[serde(default)]
    pub order: Vec<f64>,
    /// Výnimky podľa priemeru (a hĺbky): iný nástroj alebo typ vrtáka.
    #[serde(default, rename = "variant")]
    pub variants: Vec<NestDrillVariant>,
}

/// Prvý variant, ktorému diera vyhovie, určí `{tool}` a/alebo `{kind}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestDrillVariant {
    pub diameter: f64,
    /// Variant platí iba pre diery s hĺbkou ≤ `max_depth`.
    #[serde(default)]
    pub max_depth: Option<f64>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

/// Frézovanie po dráhe: obrysy dielov, onion skin, odrezanie zvyšku tabule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestRoute {
    pub verified: bool,
    /// Kód frézy obrysov (napr. `E002`).
    pub tool: String,
    /// Priemer frézy (mm).
    pub tool_diameter: f64,
    /// Prerezanie: hĺbka = hrúbka + `through_extra` (do podkladovej dosky).
    pub through_extra: f64,
    /// Onion skin malých dielov: prvý prechod v hĺbke hrúbka − `skin`.
    pub skin: f64,
    /// Malý diel: plocha < `small_area` (m²) …
    pub small_area: f64,
    /// … alebo menší rozmer < `small_min_dim` (mm).
    pub small_min_dim: f64,
    /// Nábeh: prvý bod je `lead_in` pred štartom na priamke prvej hrany.
    pub lead_in: f64,
    /// Hĺbka prvého bodu nábehu (záporná = nad materiálom).
    pub lead_in_depth: f64,
    /// Prekrytie za štartom po uzavretí obrysu (mm), aspoň Ø frézy.
    pub overlap: f64,
    /// Výbeh onion skinu za prekrytím: `skin_runout_factor × (hĺbka − lead_in_depth)`.
    pub skin_runout_factor: f64,
    /// Meno operácie. Premenná: `{k}` (číslo od `first_number`).
    pub name: String,
    pub first_number: u32,
    /// Začiatok dráhy s korekciou (obrys, onion skin). Bez premenných.
    pub start: Vec<String>,
    /// Začiatok dráhy v osi nástroja (odrezanie zvyšku). Bez premenných.
    pub start_centerline: Vec<String>,
    /// Prvý bod dráhy. Premenné: `{x} {y}`.
    pub first_point: String,
    /// Ďalší bod dráhy. Premenné: `{x} {y}`.
    pub point: String,
    /// Hĺbka naposledy pridaného bodu. Premenná: `{z}`.
    pub depth: String,
    /// Koniec dráhy. Premenné: `{name} {z} {tool} {comp} {d}`.
    pub finish: Vec<String>,
    /// `{comp}` pri obryse v smere hodín (korekcia vľavo = mimo dielu).
    pub comp_outside: String,
    /// `{comp}` pri dráhe v osi nástroja.
    pub comp_centerline: String,
}

/// Odrezanie zvyšku tabule nad dielmi (obdĺžnikový zvyšok).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestRemnant {
    pub enabled: bool,
    /// Rez sa robí iba vtedy, keď ostane pás široký aspoň `min_width` (mm).
    pub min_width: f64,
    /// Os rezu = najvyššia hrana dielov + `offset` (polomer frézy + 2,2).
    pub offset: f64,
    /// Rez začína a končí `overrun` mm za hranou tabule.
    pub overrun: f64,
}

/// Súpis behu `CncRun.txt` (UTF-16 LE s BOM, CRLF).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NestCncRun {
    pub file_name: String,
    /// Hlavička. Premenné: `{project} {parts} {sheets}`.
    pub header: Vec<String>,
    /// Riadok tabule. Premenné: `{program} {n} {l} {w} {t} {material} {grain} {face}`.
    pub sheet_line: String,
    /// Riadok programu CX100. Premenné:
    /// `{program} {part_id} {code} {l} {w} {t} {material} {cabinet} {grain} {face}`.
    pub part_line: String,
}

impl NestingProfile {
    pub const SHEET_NAME_VARS: &'static [&'static str] = &["mat", "sheet", "material"];
    pub const HEADER_VARS: &'static [&'static str] =
        &["project", "program", "material", "l", "w", "t"];
    pub const DRILL_NAME_VARS: &'static [&'static str] = &["n", "d", "depth"];
    pub const DRILL_LINE_VARS: &'static [&'static str] =
        &["name", "x", "y", "d", "depth", "tool", "kind"];
    pub const ROUTE_NAME_VARS: &'static [&'static str] = &["k"];
    pub const POINT_VARS: &'static [&'static str] = &["x", "y"];
    pub const DEPTH_VARS: &'static [&'static str] = &["z"];
    pub const FINISH_VARS: &'static [&'static str] = &["name", "z", "tool", "comp", "d"];
    pub const CNC_HEADER_VARS: &'static [&'static str] = &["project", "parts", "sheets"];
    pub const CNC_SHEET_VARS: &'static [&'static str] =
        &["program", "n", "l", "w", "t", "material", "grain", "face"];
    pub const CNC_PART_VARS: &'static [&'static str] = &[
        "program", "part_id", "code", "l", "w", "t", "material", "cabinet", "grain", "face",
    ];

    /// Rozmer tabule pre materiál (dĺžka, šírka).
    pub fn sheet_size(&self, material: &str) -> (f64, f64) {
        self.sheets
            .iter()
            .find(|s| s.material == material)
            .map_or((self.layout.sheet_length, self.layout.sheet_width), |s| {
                (s.length, s.width)
            })
    }

    /// Ide materiál na nesting?
    pub fn nests_material(&self, material: &str) -> bool {
        !self.skip_materials.iter().any(|m| m == material)
    }

    /// Kontrola šablón a číselných parametrov.
    pub fn template_problems(&self) -> Vec<TemplateProblem> {
        let r = &self.route;
        let mut blocks: Vec<(&str, Vec<&String>, &[&str])> = vec![
            (
                "nesting.sheet_name",
                vec![&self.sheet_name],
                Self::SHEET_NAME_VARS,
            ),
            (
                "nesting.header",
                self.header.iter().collect(),
                Self::HEADER_VARS,
            ),
            (
                "nesting.footer",
                self.footer.iter().collect(),
                Self::HEADER_VARS,
            ),
            ("nesting.drill.select", vec![&self.drill.select], &[]),
            (
                "nesting.drill.name",
                vec![&self.drill.name],
                Self::DRILL_NAME_VARS,
            ),
            (
                "nesting.drill.line",
                vec![&self.drill.line],
                Self::DRILL_LINE_VARS,
            ),
            ("nesting.route.name", vec![&r.name], Self::ROUTE_NAME_VARS),
            ("nesting.route.start", r.start.iter().collect(), &[]),
            (
                "nesting.route.start_centerline",
                r.start_centerline.iter().collect(),
                &[],
            ),
            (
                "nesting.route.first_point",
                vec![&r.first_point],
                Self::POINT_VARS,
            ),
            ("nesting.route.point", vec![&r.point], Self::POINT_VARS),
            ("nesting.route.depth", vec![&r.depth], Self::DEPTH_VARS),
            (
                "nesting.route.finish",
                r.finish.iter().collect(),
                Self::FINISH_VARS,
            ),
        ];
        if let Some(c) = &self.cnc_run {
            blocks.push((
                "nesting.cnc_run.header",
                c.header.iter().collect(),
                Self::CNC_HEADER_VARS,
            ));
            blocks.push((
                "nesting.cnc_run.sheet_line",
                vec![&c.sheet_line],
                Self::CNC_SHEET_VARS,
            ));
            blocks.push((
                "nesting.cnc_run.part_line",
                vec![&c.part_line],
                Self::CNC_PART_VARS,
            ));
        }
        let mut problems = Vec::new();
        for (block, templates, allowed) in blocks {
            for template in templates {
                check_template(block, template, allowed, &mut problems);
            }
        }
        if !self.sheet_name.contains("{sheet}")
            || !(self.sheet_name.contains("{mat}") || self.sheet_name.contains("{material}"))
        {
            problems.push(TemplateProblem::SheetNameVars);
        }
        if self.decimals > 6 {
            problems.push(TemplateProblem::TooManyDecimals {
                name: "nesting.decimals".into(),
                value: self.decimals,
            });
        }
        if r.finish.is_empty() {
            problems.push(TemplateProblem::Empty {
                name: "nesting.route.finish".into(),
            });
        }
        problems.extend(self.number_problems());
        problems
    }

    fn number_problems(&self) -> Vec<TemplateProblem> {
        const MAX_SHEET: f64 = 20_000.0;
        let mut problems = Vec::new();
        let positive = |v: f64| v.is_finite() && v > 0.0;
        let non_negative = |v: f64| v.is_finite() && v >= 0.0;
        let l = &self.layout;
        let r = &self.route;
        let mut sizes = vec![("nesting.layout".to_owned(), l.sheet_length, l.sheet_width)];
        for s in &self.sheets {
            sizes.push((format!("nesting.sheet `{}`", s.material), s.length, s.width));
        }
        for (name, len, wid) in sizes {
            if !(positive(len) && positive(wid) && len <= MAX_SHEET && wid <= MAX_SHEET) {
                problems.push(TemplateProblem::SheetSize {
                    name,
                    max: MAX_SHEET,
                });
            } else if non_negative(l.margin) && (len <= 2.0 * l.margin || wid <= 2.0 * l.margin) {
                problems.push(TemplateProblem::SheetBelowMargins { name });
            }
        }
        if !non_negative(l.margin) {
            problems.push(TemplateProblem::Negative {
                name: "nesting.layout.margin".into(),
            });
        }
        if !positive(r.tool_diameter) {
            problems.push(TemplateProblem::NotPositive {
                name: "nesting.route.tool_diameter".into(),
            });
        }
        if !non_negative(l.spacing) || l.spacing < r.tool_diameter {
            problems.push(TemplateProblem::SpacingBelowTool {
                spacing: l.spacing,
                tool: r.tool_diameter,
            });
        }
        for (name, v) in [
            ("nesting.route.through_extra", r.through_extra),
            ("nesting.route.skin", r.skin),
            ("nesting.route.small_area", r.small_area),
            ("nesting.route.small_min_dim", r.small_min_dim),
            ("nesting.route.skin_runout_factor", r.skin_runout_factor),
            ("nesting.remnant.min_width", self.remnant.min_width),
            ("nesting.remnant.overrun", self.remnant.overrun),
        ] {
            if !non_negative(v) {
                problems.push(TemplateProblem::NegativeValue {
                    name: name.to_owned(),
                    value: v,
                });
            }
        }
        if r.through_extra > 2.0 {
            problems.push(TemplateProblem::ThroughExtra {
                value: r.through_extra,
            });
        }
        if !positive(r.lead_in) {
            problems.push(TemplateProblem::NotPositive {
                name: "nesting.route.lead_in".into(),
            });
        }
        if !(r.lead_in_depth.is_finite() && r.lead_in_depth < 0.0) {
            problems.push(TemplateProblem::LeadInDepth);
        }
        if !non_negative(r.overlap) || r.overlap < r.tool_diameter {
            problems.push(TemplateProblem::OverlapBelowTool {
                overlap: r.overlap,
                tool: r.tool_diameter,
            });
        }
        // os rezu zvyšku musí byť aspoň polomer frézy nad dielmi
        if !(self.remnant.offset.is_finite() && self.remnant.offset > r.tool_diameter / 2.0) {
            problems.push(TemplateProblem::RemnantOffset {
                offset: self.remnant.offset,
                radius: r.tool_diameter / 2.0,
            });
        }
        if self.remnant.overrun < r.tool_diameter / 2.0 {
            problems.push(TemplateProblem::RemnantOverrun {
                overrun: self.remnant.overrun,
                radius: r.tool_diameter / 2.0,
            });
        }
        for d in &self.drill.order {
            if !positive(*d) {
                problems.push(TemplateProblem::DrillOrderDiameter { value: *d });
            }
        }
        for v in &self.drill.variants {
            if !positive(v.diameter) {
                problems.push(TemplateProblem::VariantDiameter { value: v.diameter });
            }
            if let Some(m) = v.max_depth {
                if !positive(m) {
                    problems.push(TemplateProblem::VariantMaxDepth {
                        diameter: v.diameter,
                    });
                }
            }
            if v.tool.is_none() && v.kind.is_none() {
                problems.push(TemplateProblem::VariantNoTool {
                    diameter: v.diameter,
                });
            }
        }
        problems
    }
}
