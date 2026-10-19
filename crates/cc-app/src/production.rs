//! Výroba: rozpis dielov so stavom kontroly a programami pre stroj; Kontrola: zoznam
//! zistení. Obe ako tabuľky v štýle CAD programov.

use std::collections::HashMap;

use cc_i18n::tr;
use cc_model::{Edge, Severity};
use eframe::egui;

use crate::app::{App, Tab};
use crate::grid::{self, col, num, Cell};
use crate::part_view::fmt;
use crate::tabs::role_name;
use crate::theme;

const OK_FILL: egui::Color32 = egui::Color32::from_rgb(178, 226, 128);
const WARN_FILL: egui::Color32 = egui::Color32::from_rgb(250, 222, 140);
const ERR_FILL: egui::Color32 = egui::Color32::from_rgb(244, 160, 150);

// Hlavičky stĺpcov sú kľúče prekladu (preloží ich `grid::show`).
const PART_COLUMNS: [grid::Col; 12] = [
    col("production.parts.status", 46.0),
    num("production.parts.qty", 36.0),
    col("production.parts.part", 130.0),
    col("production.parts.description", 130.0),
    num("production.parts.length", 66.0),
    num("production.parts.width", 66.0),
    num("production.parts.thickness", 58.0),
    col("production.parts.cabinet", 64.0),
    col("production.parts.material", 190.0),
    col("production.parts.edging", 90.0),
    num("production.parts.operations", 70.0),
    col("production.parts.program", 170.0),
];

const ISSUE_COLUMNS: [grid::Col; 4] = [
    col("production.issues.status", 86.0),
    col("production.issues.rule", 170.0),
    col("production.issues.part", 140.0),
    col("production.issues.description", 720.0),
];

impl App {
    pub(crate) fn tab_parts(&mut self, ui: &mut egui::Ui) {
        let Some(built) = self.model_built() else {
            return;
        };
        // najhoršie zistenie pre každý diel
        let mut state: HashMap<&str, Severity> = HashMap::new();
        for i in built.issues() {
            if let Some(p) = &i.part {
                let e = state.entry(p.as_str()).or_insert(i.severity);
                if i.severity == Severity::Error {
                    *e = Severity::Error;
                }
            }
        }
        let parts: Vec<&cc_model::Part> = built.model.parts().collect();
        let current = self.selected_part();
        let selected = current
            .as_ref()
            .and_then(|id| parts.iter().position(|p| &p.id == id));
        let programs = &self.programs;
        let r = grid::show(ui, "parts", &PART_COLUMNS, parts.len(), selected, |i| {
            let Some(p) = parts.get(i) else {
                return Vec::new();
            };
            let status = match state.get(p.id.as_str()) {
                Some(Severity::Error) => Cell::from(tr!("production.status.error")).fill(ERR_FILL),
                Some(Severity::Warning) => Cell::from("⚠").fill(WARN_FILL),
                None => Cell::from("✔").fill(OK_FILL),
            };
            let edges: Vec<&str> = Edge::ALL
                .into_iter()
                .filter(|e| p.edges.get(*e).is_some())
                .map(|e| match e {
                    Edge::X0 => "X0",
                    Edge::X1 => "X1",
                    Edge::Y0 => "Y0",
                    Edge::Y1 => "Y1",
                })
                .collect();
            vec![
                status,
                "1".into(),
                p.id.clone().into(),
                capitalize(role_name(p.role)).into(),
                fmt(p.length).into(),
                fmt(p.width).into(),
                fmt(p.thickness).into(),
                p.cabinet.clone().into(),
                p.material.clone().into(),
                if edges.is_empty() {
                    "–".to_owned()
                } else {
                    edges.join(" ")
                }
                .into(),
                p.operations.len().to_string().into(),
                programs
                    .get(&p.id)
                    .cloned()
                    .unwrap_or_else(|| "–".to_owned())
                    .into(),
            ]
        });
        if let Some(i) = r.clicked.or(r.double_clicked) {
            if let Some(p) = parts.get(i) {
                self.select_part(Some(p.id.clone()));
            }
        }
        if r.double_clicked.is_some() {
            self.tab = Tab::Part;
        }
    }

    pub(crate) fn tab_issues(&mut self, ui: &mut egui::Ui) {
        if let Some(e) = self.build_error() {
            ui.colored_label(theme::ERROR, tr!("production.build_error", error = e));
            ui.separator();
        }
        let machine = self.session.doc().map(|d| d.machine_path.clone());
        if let Some(machine) = machine {
            let text = match machine {
                Some(p) => tr!("production.machine_profile", path = p.display().to_string()),
                None => tr!("production.no_machine"),
            };
            ui.label(
                egui::RichText::new(text)
                    .size(theme::SMALL)
                    .color(theme::TEXT_WEAK),
            );
        }
        let Some(built) = self.model_built() else {
            return;
        };
        let issues: Vec<&cc_model::Issue> = built.issues().collect();
        if issues.is_empty() {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(tr!("production.no_issues"))
                    .color(theme::OK)
                    .size(16.0),
            );
            return;
        }
        let r = grid::show(ui, "issues", &ISSUE_COLUMNS, issues.len(), None, |i| {
            let Some(issue) = issues.get(i) else {
                return Vec::new();
            };
            let status = match issue.severity {
                Severity::Error => Cell::from(tr!("production.status.error")).fill(ERR_FILL),
                Severity::Warning => Cell::from(tr!("production.status.warning")).fill(WARN_FILL),
            };
            vec![
                status,
                issue.code.to_string().into(),
                Cell::from(issue.part.clone().unwrap_or_else(|| "–".to_owned())).color(theme::BLUE),
                issue.message.clone().into(),
            ]
        });
        if let Some(i) = r.clicked.or(r.double_clicked) {
            if let Some(part) = issues.get(i).and_then(|x| x.part.clone()) {
                self.select_part(Some(part));
                if r.double_clicked.is_some() {
                    self.tab = Tab::Part;
                }
            }
        }
    }
}

fn capitalize(s: impl AsRef<str>) -> String {
    let mut c = s.as_ref().chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hlavičky stĺpcov sú kľúče, ktoré `cargo xtask i18n` nevidí: musia byť v oboch
    /// vstavaných jazykoch.
    #[test]
    fn column_titles_are_translated() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for c in PART_COLUMNS.iter().chain(ISSUE_COLUMNS.iter()) {
                assert!(catalog.contains(c.title), "{code}.toml: chýba {}", c.title);
            }
        }
    }
}
