//! Zostavy: zoznam skriniek zákazky ako tabuľka (ako vstup objednávky v CAD programoch)
//! a súpis kovania.

use cc_i18n::tr;
use cc_model::Hardware;
use eframe::egui;

use crate::app::{App, Tab};
use crate::grid::{self, col, num, Cell};
use crate::part_view::fmt;
use crate::side_panel::{fronts_text, type_name};
use crate::theme;

/// Stĺpce: nadpis je kľúč prekladu (tabuľka ho preloží pri kreslení; „#“ a „ID“ sa
/// neprekladajú, `tr` vráti chýbajúci kľúč bez zmeny).
const COLUMNS: [grid::Col; 14] = [
    num("#", 34.0),
    col("ID", 70.0),
    col("reports.col.type", 70.0),
    num("reports.col.width", 64.0),
    num("reports.col.height", 64.0),
    num("reports.col.depth", 64.0),
    num("reports.col.shelves", 54.0),
    col("reports.col.fixed_shelves", 100.0),
    col("reports.col.wall", 60.0),
    num("reports.col.from_left", 110.0),
    col("reports.col.fronts", 190.0),
    num("reports.col.hinges", 54.0),
    num("reports.col.parts", 54.0),
    num("reports.col.operations", 70.0),
];

const BILL_COLUMNS: [grid::Col; 2] = [
    num("reports.bill.qty", 54.0),
    col("reports.bill.hardware", 520.0),
];

impl App {
    pub(crate) fn tab_reports(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let built = self.model_built();
        // súpis kovania celej zákazky (pánty, zásuvky, spoje, nožičky, podperky, čočky)
        let bill: Vec<cc_joints::BillLine> = built
            .as_ref()
            .map(|b| cc_joints::hardware_bill(&b.model, &doc.standard))
            .unwrap_or_default();
        let rows: Vec<Vec<Cell>> = doc
            .project
            .cabinets
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (parts, ops, hinges) = built
                    .as_ref()
                    .and_then(|b| b.model.cabinets.iter().find(|m| m.id == c.id))
                    .map_or((0, 0, 0), |m| {
                        (
                            m.parts.len(),
                            m.parts.iter().map(|p| p.operations.len()).sum::<usize>(),
                            m.hardware
                                .iter()
                                .filter(|h| matches!(h, Hardware::Hinge { .. }))
                                .count(),
                        )
                    });
                vec![
                    (i + 1).to_string().into(),
                    c.id.clone().into(),
                    type_name(c.cabinet_type).into(),
                    fmt(c.w).into(),
                    fmt(c.h).into(),
                    fmt(c.d).into(),
                    c.adjustable_shelves.to_string().into(),
                    c.fixed_shelves
                        .iter()
                        .map(|z| fmt(*z))
                        .collect::<Vec<_>>()
                        .join(", ")
                        .into(),
                    c.wall.clone().unwrap_or_else(|| "–".to_owned()).into(),
                    if c.wall.is_some() {
                        fmt(c.at)
                    } else {
                        "–".to_owned()
                    }
                    .into(),
                    fronts_text(&c.fronts).into(),
                    hinges.to_string().into(),
                    parts.to_string().into(),
                    ops.to_string().into(),
                ]
            })
            .collect();
        let selected = self.selected_index();
        let available = ui.available_height();
        // skrinky hore, súpis kovania pod nimi
        let bill_h = ((bill.len() as f32 + 1.0) * 24.0 + 30.0).min(available * 0.45);
        let r = ui
            .allocate_ui(
                egui::vec2(ui.available_width(), available - 22.0 - bill_h),
                |ui| {
                    grid::show(ui, "reports", &COLUMNS, rows.len(), Some(selected), |i| {
                        rows.get(i).cloned().unwrap_or_default()
                    })
                },
            )
            .inner;
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!("reports.bill.title")).strong());
        let bill_rows: Vec<Vec<Cell>> = bill
            .iter()
            .map(|l| vec![l.qty.to_string().into(), l.name.clone().into()])
            .collect();
        ui.allocate_ui(egui::vec2(ui.available_width(), bill_h - 24.0), |ui| {
            grid::show(
                ui,
                "hardware_bill",
                &BILL_COLUMNS,
                bill_rows.len(),
                None,
                |i| bill_rows.get(i).cloned().unwrap_or_default(),
            )
        });
        ui.label(
            egui::RichText::new(tr!("reports.hint"))
                .size(theme::SMALL)
                .color(theme::TEXT_WEAK),
        );
        if let Some(i) = r.clicked {
            self.select_index(i);
        }
        if r.double_clicked.is_some() {
            self.tab = Tab::View3d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BILL_COLUMNS, COLUMNS};

    /// Nadpisy stĺpcov sú kľúče mimo `tr!` (xtask ich nevidí): každý musí byť v sk aj en.
    #[test]
    fn column_titles_are_translated() {
        let catalogs: Vec<cc_i18n::Catalog> = cc_i18n::BUILTIN
            .iter()
            .map(|(code, text)| cc_i18n::Catalog::parse(code, text).unwrap())
            .collect();
        for c in COLUMNS.iter().chain(BILL_COLUMNS.iter()) {
            if !c.title.contains('.') {
                continue;
            }
            for catalog in &catalogs {
                assert!(
                    catalog.contains(c.title),
                    "{} chýba v {}",
                    c.title,
                    catalog.code
                );
            }
        }
    }
}
