//! Problémy v šablónach a číselných parametroch profilu stroja (`[xcs]`, `[mpr]`, `[nesting]`).
//!
//! `cc-model` je na L0 vedľa `cc-i18n` a nesmie od neho závisieť: kontrola vráti kód
//! s hodnotami a text v zvolenom jazyku zostaví `cc-config` (`validate_machine`, ADR 0007).

/// Nespárovaná zložená zátvorka v šablóne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnmatchedBrace {
    /// `{` bez `}`
    Open,
    /// `}` bez `{`
    Close,
}

/// Jeden problém profilu stroja. `block` a `name` sú cesty v `machine.toml`
/// (napr. `xcs.drill.line`, `mpr.header.XA`), čísla sú pôvodné hodnoty zo súboru.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateProblem {
    /// Nespárovaná zátvorka v šablóne.
    Unmatched {
        block: String,
        brace: UnmatchedBrace,
        template: String,
    },
    /// Premenná, ktorú blok nepozná; `allowed` prázdne = blok nemá premenné.
    UnknownVariable {
        block: String,
        var: String,
        allowed: Vec<String>,
    },
    /// Prázdne meno poľa makra `.mpr`.
    EmptyFieldName { block: String },
    /// Viac ako 6 desatinných miest.
    TooManyDecimals { name: String, value: u32 },
    /// Hodnota musí byť > 0.
    NotPositive { name: String },
    /// Hodnota musí byť ≥ 0.
    Negative { name: String },
    /// Hodnota musí byť ≥ 0 (s hodnotou).
    NegativeValue { name: String, value: f64 },
    /// Zoznam riadkov je prázdny.
    Empty { name: String },
    /// `xcs.slot.max_step` je väčší ako `saw_kerf`.
    StepOverKerf { step: f64, kerf: f64 },
    /// `nesting.sheet_name` bez `{sheet}` a `{mat}`/`{material}`.
    SheetNameVars,
    /// Rozmer tabule mimo (0, max].
    SheetSize { name: String, max: f64 },
    /// Tabuľa je menšia ako dva okraje.
    SheetBelowMargins { name: String },
    /// `nesting.layout.spacing` je menšia ako priemer frézy.
    SpacingBelowTool { spacing: f64, tool: f64 },
    /// `nesting.route.through_extra` je viac ako 2 mm.
    ThroughExtra { value: f64 },
    /// `nesting.route.lead_in_depth` nie je záporná.
    LeadInDepth,
    /// `nesting.route.overlap` je menší ako priemer frézy.
    OverlapBelowTool { overlap: f64, tool: f64 },
    /// `nesting.remnant.offset` nie je väčší ako polomer frézy.
    RemnantOffset { offset: f64, radius: f64 },
    /// `nesting.remnant.overrun` je menší ako polomer frézy.
    RemnantOverrun { overrun: f64, radius: f64 },
    /// `nesting.drill.order`: priemer ≤ 0.
    DrillOrderDiameter { value: f64 },
    /// `nesting.drill.variant`: priemer ≤ 0.
    VariantDiameter { value: f64 },
    /// `nesting.drill.variant`: `max_depth` ≤ 0.
    VariantMaxDepth { diameter: f64 },
    /// `nesting.drill.variant` bez `tool` aj `kind`.
    VariantNoTool { diameter: f64 },
}

/// Mená premenných `{…}` v šablóne. Chybu vráti pri nespárovanej zátvorke.
pub fn template_placeholders(template: &str) -> Result<Vec<&str>, UnmatchedBrace> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let close = after.find('}').ok_or(UnmatchedBrace::Open)?;
        out.push(&after[..close]);
        rest = &after[close + 1..];
    }
    if rest.contains('}') {
        return Err(UnmatchedBrace::Close);
    }
    Ok(out)
}

/// Kontrola šablón jedného bloku: nespárované zátvorky a neznáme premenné.
pub(crate) fn check_template(
    block: &str,
    template: &str,
    allowed: &[&str],
    problems: &mut Vec<TemplateProblem>,
) {
    match template_placeholders(template) {
        Err(brace) => problems.push(TemplateProblem::Unmatched {
            block: block.to_owned(),
            brace,
            template: template.to_owned(),
        }),
        Ok(vars) => {
            for v in vars {
                if !allowed.contains(&v) {
                    problems.push(TemplateProblem::UnknownVariable {
                        block: block.to_owned(),
                        var: v.to_owned(),
                        allowed: allowed.iter().map(|a| (*a).to_owned()).collect(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        assert_eq!(template_placeholders("a{x}b{y}"), Ok(vec!["x", "y"]));
        assert_eq!(template_placeholders("a{x"), Err(UnmatchedBrace::Open));
        assert_eq!(template_placeholders("a}x"), Err(UnmatchedBrace::Close));
    }

    #[test]
    fn unknown_variable_keeps_allowed_list() {
        let mut p = Vec::new();
        check_template("xcs.drill.line", "{x}{zz}", &["x", "y"], &mut p);
        assert_eq!(
            p,
            vec![TemplateProblem::UnknownVariable {
                block: "xcs.drill.line".into(),
                var: "zz".into(),
                allowed: vec!["x".into(), "y".into()],
            }]
        );
    }
}
