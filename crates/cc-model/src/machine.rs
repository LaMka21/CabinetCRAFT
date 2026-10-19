use serde::{Deserialize, Serialize};

use crate::template_problem::check_template;
use crate::{NestingProfile, TemplateProblem};

/// Profil stroja, súbor `machine.toml` (kap. 7.3, 7.4).
///
/// `work_area`, `min_part` a `drills` používa validácia (kap. 8),
/// sekciu `[xcs]` writer SCM Maestro (`cc-out-xcs`), sekciu `[mpr]` writer WoodWOP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineProfile {
    pub machine: MachineInfo,
    pub work_area: WorkArea,
    pub min_part: MinPart,
    pub drills: Drills,
    /// Bez tejto sekcie sa `.xcs` negeneruje.
    #[serde(default)]
    pub xcs: Option<XcsProfile>,
    /// Bez tejto sekcie sa `.mpr` negeneruje.
    #[serde(default)]
    pub mpr: Option<MprProfile>,
    /// Rozdelenie výroby medzi stroje; bez sekcie všetko na CX100.
    #[serde(default)]
    pub production: Production,
    /// SCM nesting (programy tabúľ). Používa sa iba pri `production.mode = "nesting"`.
    #[serde(default)]
    pub nesting: Option<NestingProfile>,
    /// Štítky dielov (rozmer, poradie tlače po tabuliach); bez sekcie ako CV dielne 80 × 80.
    #[serde(default)]
    pub labels: LabelSettings,
}

/// Nastavenie štítkov dielov (findings 14): rozmer štítku a poradie tlače štítkov tabule.
/// Návrh štítku je pre 80 × 80 mm; pri inom rozmere sa polia úmerne roztiahnu a písmo
/// sa prispôsobí poliam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelSettings {
    /// Šírka štítku (mm).
    #[serde(default = "default_label_size")]
    pub width: f64,
    /// Výška štítku (mm).
    #[serde(default = "default_label_size")]
    pub height: f64,
    /// Poradie štítkov tabule pri tlači.
    #[serde(default)]
    pub order: LabelOrder,
    /// Zvyšok tabule sa počíta do čísel dielov ako v CV (prvý v poradí, diely od 2).
    #[serde(default = "default_true")]
    pub offcut_number: bool,
    /// Tlačiareň štítkov pre priamu tlač (meno vo Windows); bez hodnoty predvolená.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
}

fn default_label_size() -> f64 {
    80.0
}

impl Default for LabelSettings {
    fn default() -> Self {
        LabelSettings {
            width: default_label_size(),
            height: default_label_size(),
            order: LabelOrder::default(),
            offcut_number: true,
            printer: None,
        }
    }
}

impl LabelSettings {
    /// Rozmer štítku; nezmyselné hodnoty (NaN, pod 20 alebo nad 300 mm) → 80.
    pub fn size(&self) -> (f64, f64) {
        let ok = |v: f64| {
            if v.is_finite() && (20.0..=300.0).contains(&v) {
                v
            } else {
                default_label_size()
            }
        };
        (ok(self.width), ok(self.height))
    }
}

/// Poradie štítkov na tabuli (ako „label order“ v +Label, findings 14 §8.2). Radí sa podľa
/// ľavého dolného rohu dielu na tabuli; „riadok“ = rovnaké Y, „stĺpec“ = rovnaké X.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelOrder {
    /// Poradie obrábania obrysov na tabuli (= číslo dielu na štítku).
    #[default]
    Machining,
    /// Riadky zhora nadol, v riadku zľava doprava.
    RowsTopLeft,
    /// Stĺpce zľava doprava, v stĺpci zhora nadol.
    ColumnsLeftTop,
    /// Riadky zhora nadol, v riadku sprava doľava.
    RowsTopRight,
    /// Stĺpce sprava doľava, v stĺpci zhora nadol.
    ColumnsRightTop,
    /// Riadky zdola nahor, v riadku zľava doprava.
    RowsBottomLeft,
    /// Stĺpce zľava doprava, v stĺpci zdola nahor.
    ColumnsLeftBottom,
    /// Riadky zdola nahor, v riadku sprava doľava.
    RowsBottomRight,
    /// Stĺpce sprava doľava, v stĺpci zdola nahor.
    ColumnsRightBottom,
}

impl LabelOrder {
    pub const ALL: [LabelOrder; 9] = [
        LabelOrder::Machining,
        LabelOrder::RowsTopLeft,
        LabelOrder::ColumnsLeftTop,
        LabelOrder::RowsTopRight,
        LabelOrder::ColumnsRightTop,
        LabelOrder::RowsBottomLeft,
        LabelOrder::ColumnsLeftBottom,
        LabelOrder::RowsBottomRight,
        LabelOrder::ColumnsRightBottom,
    ];

    /// Popis pre používateľa.
    pub fn label(self) -> &'static str {
        match self {
            LabelOrder::Machining => "poradie obrábania",
            LabelOrder::RowsTopLeft => "riadky zhora, zľava",
            LabelOrder::ColumnsLeftTop => "stĺpce zľava, zhora",
            LabelOrder::RowsTopRight => "riadky zhora, sprava",
            LabelOrder::ColumnsRightTop => "stĺpce sprava, zhora",
            LabelOrder::RowsBottomLeft => "riadky zdola, zľava",
            LabelOrder::ColumnsLeftBottom => "stĺpce zľava, zdola",
            LabelOrder::RowsBottomRight => "riadky zdola, sprava",
            LabelOrder::ColumnsRightBottom => "stĺpce sprava, zdola",
        }
    }

    /// Kľúč radenia dielu s ľavým dolným rohom (x, y) a poradím obrábania `n`.
    pub fn key(self, x: f64, y: f64, n: usize) -> (f64, f64, usize) {
        match self {
            LabelOrder::Machining => (0.0, 0.0, n),
            LabelOrder::RowsTopLeft => (-y, x, n),
            LabelOrder::ColumnsLeftTop => (x, -y, n),
            LabelOrder::RowsTopRight => (-y, -x, n),
            LabelOrder::ColumnsRightTop => (-x, -y, n),
            LabelOrder::RowsBottomLeft => (y, x, n),
            LabelOrder::ColumnsLeftBottom => (x, y, n),
            LabelOrder::RowsBottomRight => (y, -x, n),
            LabelOrder::ColumnsRightBottom => (-x, y, n),
        }
    }
}

/// Kam ide obrábanie dielov.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProductionMode {
    /// Všetko obrábanie dielu na CX100 (diely sa delia na píle). Pôvodné správanie.
    #[default]
    Cx100,
    /// Obrysy a vŕtanie plochy hore na nestingu, zvyšok na CX100 (findings 08, §3).
    Nesting,
}

impl ProductionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            ProductionMode::Cx100 => "cx100",
            ProductionMode::Nesting => "nesting",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Production {
    #[serde(default)]
    pub mode: ProductionMode,
}

/// Formát SCM Maestro `.xcs` (kap. 7.4). Riadky sú šablóny s premennými
/// v zložených zátvorkách; povolené premenné sú pri každom bloku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsProfile {
    /// Počet desatinných miest (nuly na konci sa vynechajú).
    #[serde(default = "default_xcs_decimals")]
    pub decimals: u32,
    #[serde(default)]
    pub line_ending: LineEnding,
    /// Úvodné riadky. Premenné: `{project} {part_id} {cabinet} {l} {w} {t}`.
    pub header: Vec<String>,
    /// Záverečné riadky. Rovnaké premenné ako `header`.
    #[serde(default)]
    pub footer: Vec<String>,
    pub workplanes: XcsWorkplanes,
    pub drill: XcsDrill,
    pub slot: XcsSlot,
    /// Dávka pre XConverter (`RUN.bat`); bez sekcie sa negeneruje.
    #[serde(default)]
    pub run_bat: Option<XcsRunBat>,
}

fn default_xcs_decimals() -> u32 {
    4
}

/// Mená rovín Maestro pre strany dielu a smer lokálnej osi x v bočných rovinách.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsWorkplanes {
    /// Príkaz výberu roviny. Premenná: `{plane}`.
    pub select: String,
    pub top: String,
    pub x0: String,
    pub x1: String,
    pub y0: String,
    pub y1: String,
    /// `true`: lokálne x sa meria od protiľahlého rohu (`W − y` pri X0/X1, `L − x` pri Y0/Y1).
    pub reverse_x0: bool,
    pub reverse_x1: bool,
    pub reverse_y0: bool,
    pub reverse_y1: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsDrill {
    pub verified: bool,
    /// Meno operácie. Premenné: `{side} {n} {d} {depth}`.
    pub name: String,
    /// Riadok vŕtania. Premenné: `{name} {x} {y} {d} {depth}`.
    pub line: String,
    /// Text `{side}` podľa strany (napr. „Leva “).
    pub side_names: XcsSideNames,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsSideNames {
    pub top: String,
    pub x0: String,
    pub x1: String,
    pub y0: String,
    pub y1: String,
}

/// Drážka pílkou: drážka širšia ako rez sa robí viacerými prechodmi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsSlot {
    pub verified: bool,
    /// Meno prechodu. Premenná: `{n}` (číslo prechodu od `first_number`).
    pub name: String,
    #[serde(default = "default_slot_number")]
    pub first_number: u32,
    /// Riadky jedného prechodu. Premenné: `{name} {xs} {ys} {xe} {ye} {depth}`.
    pub lines: Vec<String>,
    /// Šírka rezu pílky (mm).
    pub saw_kerf: f64,
    /// Max. posun medzi prechodmi (mm).
    pub max_step: f64,
    /// Predĺženie drážky cez celý diel na oboch koncoch (nábeh a výbeh pílky).
    #[serde(default)]
    pub through_extension: f64,
}

fn default_slot_number() -> u32 {
    1001
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XcsRunBat {
    /// Názov dávky, napr. `RUN.bat`.
    pub file_name: String,
    /// Priečinok, kde budú `.xcs` na PC pri stroji (napr. `C:\Vyroba\CX100`).
    pub target_dir: String,
    /// Úvodné riadky dávky.
    #[serde(default)]
    pub header: Vec<String>,
    /// Riadok pre jeden program. Premenné: `{input} {output} {stem}`.
    pub line: String,
}

impl XcsProfile {
    pub const HEADER_VARS: &'static [&'static str] =
        &["project", "part_id", "cabinet", "l", "w", "t"];
    pub const SELECT_VARS: &'static [&'static str] = &["plane"];
    pub const DRILL_NAME_VARS: &'static [&'static str] = &["side", "n", "d", "depth"];
    pub const DRILL_LINE_VARS: &'static [&'static str] = &["name", "x", "y", "d", "depth"];
    pub const SLOT_NAME_VARS: &'static [&'static str] = &["n"];
    pub const SLOT_LINE_VARS: &'static [&'static str] = &["name", "xs", "ys", "xe", "ye", "depth"];
    pub const RUN_LINE_VARS: &'static [&'static str] = &["input", "output", "stem"];

    /// Kontrola šablón a číselných parametrov.
    pub fn template_problems(&self) -> Vec<TemplateProblem> {
        let mut blocks: Vec<(&str, Vec<&String>, &[&str])> = vec![
            (
                "xcs.header",
                self.header.iter().collect(),
                Self::HEADER_VARS,
            ),
            (
                "xcs.footer",
                self.footer.iter().collect(),
                Self::HEADER_VARS,
            ),
            (
                "xcs.workplanes.select",
                vec![&self.workplanes.select],
                Self::SELECT_VARS,
            ),
            (
                "xcs.drill.name",
                vec![&self.drill.name],
                Self::DRILL_NAME_VARS,
            ),
            (
                "xcs.drill.line",
                vec![&self.drill.line],
                Self::DRILL_LINE_VARS,
            ),
            ("xcs.slot.name", vec![&self.slot.name], Self::SLOT_NAME_VARS),
            (
                "xcs.slot.lines",
                self.slot.lines.iter().collect(),
                Self::SLOT_LINE_VARS,
            ),
        ];
        if let Some(bat) = &self.run_bat {
            blocks.push(("xcs.run_bat.header", bat.header.iter().collect(), &[]));
            blocks.push(("xcs.run_bat.line", vec![&bat.line], Self::RUN_LINE_VARS));
        }
        let mut problems = Vec::new();
        for (block, templates, allowed) in blocks {
            for template in templates {
                check_template(block, template, allowed, &mut problems);
            }
        }
        if self.decimals > 6 {
            problems.push(TemplateProblem::TooManyDecimals {
                name: "xcs.decimals".into(),
                value: self.decimals,
            });
        }
        if !(self.slot.saw_kerf.is_finite() && self.slot.saw_kerf > 0.0) {
            problems.push(TemplateProblem::NotPositive {
                name: "xcs.slot.saw_kerf".into(),
            });
        }
        if !(self.slot.max_step.is_finite() && self.slot.max_step > 0.0) {
            problems.push(TemplateProblem::NotPositive {
                name: "xcs.slot.max_step".into(),
            });
        }
        if self.slot.max_step > self.slot.saw_kerf {
            problems.push(TemplateProblem::StepOverKerf {
                step: self.slot.max_step,
                kerf: self.slot.saw_kerf,
            });
        }
        if self.slot.lines.is_empty() {
            problems.push(TemplateProblem::Empty {
                name: "xcs.slot.lines".into(),
            });
        }
        problems
    }
}

/// Pole makra: `[meno, šablóna]`, napr. `["XA", "{x}"]`.
pub type MprField = [String; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    #[default]
    Crlf,
    Lf,
}

/// Formát WoodWOP `.mpr` (kap. 7.3). Hodnoty polí sú šablóny s premennými
/// v zložených zátvorkách; povolené premenné sú pri každom bloku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MprProfile {
    /// Počet desatinných miest (nuly na konci sa vynechajú).
    #[serde(default = "default_decimals")]
    pub decimals: u32,
    #[serde(default)]
    pub line_ending: LineEnding,
    /// Zapísať do `[001` aj premennú `i = 25.4` („converter“).
    #[serde(default = "default_true")]
    pub inch_converter_var: bool,
    /// Hlavička `[H`. Premenné: `{l} {w} {t}`.
    pub header: Vec<MprField>,
    /// Obrobok `<100 \WerkStck\`. Premenné: `{l} {w} {t}`.
    pub workpiece: Vec<MprField>,
    /// Komentár `<101 \Kommentar\`. Premenné: `{part_id} {project} {cabinet} {l} {w} {t}`.
    pub comment: Vec<MprField>,
    /// `<102 \BohrVert\`: diera do plochy. Premenné: `{x} {y} {d} {depth}`.
    pub bohr_vert: MprMacro,
    /// `<103 \BohrHoriz\`: diera do hrany. Premenné: `{x} {y} {z} {d} {depth} {dir}`.
    pub bohr_horiz: MprBohrHoriz,
    /// `<109 \Nuten\`: drážka. Premenné: `{xs} {ys} {xe} {ye} {width} {depth}`.
    pub nuten: MprNuten,
}

impl MprProfile {
    pub const HEADER_VARS: &'static [&'static str] = &["l", "w", "t"];
    pub const COMMENT_VARS: &'static [&'static str] =
        &["part_id", "project", "cabinet", "l", "w", "t"];
    pub const BOHR_VERT_VARS: &'static [&'static str] = &["x", "y", "d", "depth"];
    pub const BOHR_HORIZ_VARS: &'static [&'static str] = &["x", "y", "z", "d", "depth", "dir"];
    pub const NUTEN_VARS: &'static [&'static str] = &["xs", "ys", "xe", "ye", "width", "depth"];

    /// Kontrola šablón: neznáme premenné, nespárované zátvorky, prázdne mená polí.
    pub fn template_problems(&self) -> Vec<TemplateProblem> {
        let blocks: [(&str, &Vec<MprField>, &[&str]); 6] = [
            ("mpr.header", &self.header, Self::HEADER_VARS),
            ("mpr.workpiece", &self.workpiece, Self::HEADER_VARS),
            ("mpr.comment", &self.comment, Self::COMMENT_VARS),
            (
                "mpr.bohr_vert",
                &self.bohr_vert.fields,
                Self::BOHR_VERT_VARS,
            ),
            (
                "mpr.bohr_horiz",
                &self.bohr_horiz.fields,
                Self::BOHR_HORIZ_VARS,
            ),
            ("mpr.nuten", &self.nuten.fields, Self::NUTEN_VARS),
        ];
        let mut problems = Vec::new();
        for (block, fields, allowed) in blocks {
            for [name, template] in fields {
                if name.trim().is_empty() {
                    problems.push(TemplateProblem::EmptyFieldName {
                        block: block.to_owned(),
                    });
                }
                check_template(&format!("{block}.{name}"), template, allowed, &mut problems);
            }
        }
        if self.decimals > 6 {
            problems.push(TemplateProblem::TooManyDecimals {
                name: "mpr.decimals".into(),
                value: self.decimals,
            });
        }
        problems
    }
}

fn default_decimals() -> u32 {
    3
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MprMacro {
    /// `false` = polia ešte neboli overené na stroji alebo v editore woodWOP (O2).
    pub verified: bool,
    pub fields: Vec<MprField>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MprBohrHoriz {
    pub verified: bool,
    pub fields: Vec<MprField>,
    /// Kód smeru vŕtania `{dir}` podľa hrany dielu (O1).
    pub dir: EdgeCodes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub struct EdgeCodes {
    pub X0: String,
    pub X1: String,
    pub Y0: String,
    pub Y1: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MprNuten {
    pub verified: bool,
    pub fields: Vec<MprField>,
    /// Predĺženie drážky cez celý diel na oboch koncoch (napr. nábeh pílového kotúča).
    #[serde(default)]
    pub through_extension: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineInfo {
    pub name: String,
}

/// Najväčší obrábaný diel: `length` v smere X stroja, `width` v smere Y.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkArea {
    pub max_length: f64,
    pub max_width: f64,
}

/// Najmenší diel, ktorý stroj bezpečne upne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinPart {
    pub length: f64,
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drills {
    /// Dostupné priemery vrtákov (mm).
    pub diameters: Vec<f64>,
}

impl MachineProfile {
    /// Je vrták s daným priemerom k dispozícii (tolerancia 0,001 mm)?
    pub fn has_drill(&self, diameter: f64) -> bool {
        self.drills
            .diameters
            .iter()
            .any(|d| (d - diameter).abs() < 1e-3)
    }
}
