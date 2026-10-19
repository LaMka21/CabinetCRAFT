//! Štítky: tabule nestingu s dielmi a tlač štítkov ako program +Label v CV (findings 14 §8).
//! Klik na diel vytlačí jeho štítok, tlačidlá tabuľu alebo všetky štítky (PDF sa otvorí
//! v prehliadači PDF na tlač). Údaje dávajú príkazy `labels.sheets` a `labels.print`.

use std::collections::BTreeSet;
use std::path::PathBuf;

use cc_i18n::tr;
use cc_model::LabelOrder;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use serde_json::{json, Value};

use crate::app::App;
use crate::theme;

/// Popis poradia štítkov v zvolenom jazyku (`LabelOrder::label` v cc-model je iba po
/// slovensky a cc-model nesmie závisieť od cc-i18n).
fn order_label(order: LabelOrder) -> String {
    cc_i18n::tr(order_key(order))
}

/// Kľúč prekladu pre poradie štítkov (test kontroluje, že sú v oboch jazykoch).
fn order_key(order: LabelOrder) -> &'static str {
    match order {
        LabelOrder::Machining => "labels_view.order.machining",
        LabelOrder::RowsTopLeft => "labels_view.order.rows_top_left",
        LabelOrder::ColumnsLeftTop => "labels_view.order.columns_left_top",
        LabelOrder::RowsTopRight => "labels_view.order.rows_top_right",
        LabelOrder::ColumnsRightTop => "labels_view.order.columns_right_top",
        LabelOrder::RowsBottomLeft => "labels_view.order.rows_bottom_left",
        LabelOrder::ColumnsLeftBottom => "labels_view.order.columns_left_bottom",
        LabelOrder::RowsBottomRight => "labels_view.order.rows_bottom_right",
        LabelOrder::ColumnsRightBottom => "labels_view.order.columns_right_bottom",
    }
}

/// Stav záložky (výsledok `labels.sheets` sa počíta iba pri zmene modelu alebo režimu).
#[derive(Default)]
pub(crate) struct LabelsState {
    key: usize,
    mode: Option<bool>,
    data: Option<Result<Value, String>>,
    pub(crate) sheet: usize,
    /// Diely, ktorých štítok sa v tomto okne už tlačil.
    pub(crate) printed: BTreeSet<String>,
    pub(crate) a4: bool,
    /// Tlačiť hneď na tlačiareň (bez otvorenia PDF).
    pub(crate) direct: bool,
    printer: Option<String>,
}

/// Pošle PDF na tlačiareň cez Windows (sloveso PrintTo/Print aplikácie pre PDF).
fn print_pdf(file: &std::path::Path, printer: Option<&str>) -> Result<(), String> {
    let q = |s: &str| s.replace('\'', "''");
    let path = q(&file.to_string_lossy());
    let cmd = match printer {
        Some(p) => format!(
            "Start-Process -FilePath '{path}' -Verb PrintTo -ArgumentList '\"{}\"' -WindowStyle Hidden",
            q(p)
        ),
        None => format!("Start-Process -FilePath '{path}' -Verb Print -WindowStyle Hidden"),
    };
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

const PRINTED: Color32 = Color32::from_rgb(200, 232, 180);

impl App {
    fn labels_mode(&self) -> Option<&'static str> {
        self.export_nesting
            .map(|n| if n { "nesting" } else { "cx100" })
    }

    fn labels_refresh(&mut self) {
        if self.labels.data.is_some()
            && self.labels.key == self.model_key
            && self.labels.mode == self.export_nesting
        {
            return;
        }
        let mut params = json!({});
        if let Some(m) = self.labels_mode() {
            params["mode"] = json!(m);
        }
        self.labels.data = Some(
            self.session
                .run("labels.sheets", params)
                .map_err(|e| e.to_string()),
        );
        self.labels.key = self.model_key;
        self.labels.mode = self.export_nesting;
        self.labels.printer = self
            .labels
            .data
            .as_ref()
            .and_then(|d| d.as_ref().ok())
            .and_then(|d| d["printer"].as_str())
            .map(str::to_owned);
    }

    /// Vytlačí štítky (`params` = sheet / parts) a otvorí PDF.
    fn labels_print(&mut self, mut params: Value, what: &str) {
        if let Some(m) = self.labels_mode() {
            params["mode"] = json!(m);
        }
        params["a4"] = json!(self.labels.a4);
        let Some(r) = self.exec("labels.print", params) else {
            return;
        };
        if let Some(ids) = r["parts"].as_array() {
            for id in ids.iter().filter_map(|v| v.as_str()) {
                self.labels.printed.insert(id.to_owned());
            }
        }
        let file = r["path"].as_str().map(PathBuf::from);
        if self.labels.direct {
            if let Some(f) = &file {
                let printer = self.labels.printer.clone();
                match print_pdf(f, printer.as_deref()) {
                    Ok(()) => self.set_status(
                        tr!(
                            "labels_view.status.sent",
                            printer = printer
                                .clone()
                                .unwrap_or_else(|| tr!("labels_view.status.default_printer")),
                            what = what,
                            count = r["count"].to_string()
                        ),
                        false,
                    ),
                    Err(e) => self.set_status(
                        tr!(
                            "labels_view.status.direct_failed",
                            error = e,
                            path = f.display().to_string()
                        ),
                        true,
                    ),
                }
            }
            return;
        }
        self.set_status(
            tr!(
                "labels_view.status.ready",
                what = what,
                count = r["count"].to_string(),
                path = file
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            ),
            false,
        );
        if let Some(file) = file {
            let _ = std::process::Command::new("explorer").arg(file).spawn();
        }
    }

    pub(crate) fn tab_labels(&mut self, ui: &mut egui::Ui) {
        self.labels_refresh();
        let data = match &self.labels.data {
            Some(Ok(v)) => v.clone(),
            Some(Err(e)) => {
                ui.colored_label(theme::ERROR, tr!("labels_view.error", error = e));
                return;
            }
            None => return,
        };
        let sheets: Vec<Value> = data["sheets"].as_array().cloned().unwrap_or_default();
        let other = data["other"].as_array().map_or(0, Vec::len);
        let size = (
            data["size"][0].as_f64().unwrap_or(80.0),
            data["size"][1].as_f64().unwrap_or(80.0),
        );
        if self.labels.sheet >= sheets.len() {
            self.labels.sheet = 0;
        }

        let mut print: Option<(Value, String)> = None;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(tr!(
                    "labels_view.header",
                    width = crate::part_view::fmt(size.0),
                    height = crate::part_view::fmt(size.1),
                    order = serde_json::from_value::<LabelOrder>(data["order"].clone())
                        .map_or_else(|_| "?".to_owned(), order_label)
                ))
                .color(theme::TEXT_WEAK),
            );
            ui.separator();
            ui.checkbox(&mut self.labels.a4, tr!("labels_view.a4"));
            ui.checkbox(&mut self.labels.direct, tr!("labels_view.direct"))
                .on_hover_text(tr!(
                    "labels_view.direct_hint",
                    printer = self
                        .labels
                        .printer
                        .clone()
                        .unwrap_or_else(|| tr!("labels_view.direct_default_printer"))
                ));
            if ui
                .button(tr!(
                    "labels_view.print_all",
                    count = data["labels"].to_string()
                ))
                .on_hover_text(tr!("labels_view.print_all_hint"))
                .clicked()
            {
                print = Some((json!({}), tr!("labels_view.what.all")));
            }
        });
        ui.add_space(4.0);

        if sheets.is_empty() {
            ui.label(tr!("labels_view.no_sheets"));
            if let Some((p, what)) = print {
                self.labels_print(p, &what);
            }
            return;
        }

        let avail = ui.available_size();
        ui.horizontal_top(|ui| {
            // zoznam tabúľ
            let top_down = egui::Layout::top_down(egui::Align::Min);
            ui.allocate_ui_with_layout(Vec2::new(230.0, avail.y), top_down, |ui| {
                ui.set_width(230.0);
                egui::ScrollArea::vertical()
                    .id_salt("label_sheets")
                    .show(ui, |ui| {
                        for (i, s) in sheets.iter().enumerate() {
                            let parts = s["parts"].as_array().map_or(0, Vec::len);
                            let done = s["parts"].as_array().map_or(0, |a| {
                                a.iter()
                                    .filter(|p| {
                                        p["id"]
                                            .as_str()
                                            .is_some_and(|id| self.labels.printed.contains(id))
                                    })
                                    .count()
                            });
                            let text = format!(
                                "{}  {} {}/{}\n{}{}",
                                s["name"].as_str().unwrap_or(""),
                                s["material"].as_str().unwrap_or("").replace('_', " "),
                                s["index"],
                                s["count"],
                                tr!("labels_view.sheets.parts", count = parts),
                                if done > 0 {
                                    tr!("labels_view.sheets.printed", count = done)
                                } else {
                                    String::new()
                                }
                            );
                            if ui.selectable_label(self.labels.sheet == i, text).clicked() {
                                self.labels.sheet = i;
                            }
                        }
                        if other > 0 {
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(tr!("labels_view.sheets.other", count = other))
                                    .color(theme::TEXT_WEAK),
                            );
                        }
                    });
            });
            ui.separator();

            // tabuľa
            let rest = Vec2::new(ui.available_width(), avail.y);
            ui.allocate_ui_with_layout(rest, top_down, |ui| {
                let Some(sheet) = sheets.get(self.labels.sheet) else {
                    return;
                };
                let name = sheet["name"].as_str().unwrap_or("").to_owned();
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(tr!(
                            "labels_view.sheet.title",
                            name = &name,
                            material = sheet["material"].as_str().unwrap_or("").replace('_', " "),
                            thickness =
                                crate::part_view::fmt(sheet["thickness"].as_f64().unwrap_or(0.0)),
                            length = crate::part_view::fmt(sheet["length"].as_f64().unwrap_or(0.0)),
                            width = crate::part_view::fmt(sheet["width"].as_f64().unwrap_or(0.0)),
                            index = sheet["index"].to_string(),
                            total = sheet["count"].to_string()
                        ))
                        .strong(),
                    );
                    if ui.button(tr!("labels_view.sheet.print")).clicked() {
                        print = Some((
                            json!({ "sheet": name }),
                            tr!("labels_view.what.sheet", name = &name),
                        ));
                    }
                });
                ui.label(
                    egui::RichText::new(tr!("labels_view.sheet.hint")).color(theme::TEXT_WEAK),
                );
                if let Some(id) = self.sheet_canvas(ui, sheet) {
                    print = Some((
                        json!({ "parts": [id.clone()] }),
                        tr!("labels_view.what.part", id = &id),
                    ));
                }
            });
        });
        if let Some((p, what)) = print {
            self.labels_print(p, &what);
        }
    }

    /// Nakreslí tabuľu s dielmi; vráti ID dielu, na ktorý sa kliklo.
    fn sheet_canvas(&self, ui: &mut egui::Ui, sheet: &Value) -> Option<String> {
        let size = ui.available_size().max(Vec2::new(200.0, 150.0));
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::BG);
        let (len, wid) = (
            sheet["length"].as_f64().unwrap_or(1.0).max(1.0),
            sheet["width"].as_f64().unwrap_or(1.0).max(1.0),
        );
        let margin = 16.0;
        let scale = ((rect.width() - 2.0 * margin) as f64 / len)
            .min((rect.height() - 2.0 * margin) as f64 / wid)
            .max(0.01);
        let origin = Pos2::new(
            rect.center().x - (len * scale) as f32 / 2.0,
            rect.center().y + (wid * scale) as f32 / 2.0,
        );
        // tabuľa: X doprava, Y nahor (počiatok vľavo dole ako v programe)
        let to_screen = |x: f64, y: f64| {
            Pos2::new(origin.x + (x * scale) as f32, origin.y - (y * scale) as f32)
        };
        let board = Rect::from_two_pos(to_screen(0.0, 0.0), to_screen(len, wid));
        painter.rect_filled(board, 0.0, theme::WOOD);
        painter.rect_stroke(
            board,
            0.0,
            Stroke::new(1.0_f32, theme::GREY),
            egui::StrokeKind::Inside,
        );

        let pointer = response.hover_pos();
        let mut hovered: Option<&Value> = None;
        let parts = sheet["parts"].as_array().cloned().unwrap_or_default();
        for p in &parts {
            let (x, y, dx, dy) = (
                p["x"].as_f64().unwrap_or(0.0),
                p["y"].as_f64().unwrap_or(0.0),
                p["dx"].as_f64().unwrap_or(0.0),
                p["dy"].as_f64().unwrap_or(0.0),
            );
            let r = Rect::from_two_pos(to_screen(x, y), to_screen(x + dx, y + dy));
            let id = p["id"].as_str().unwrap_or("");
            let is_hover = pointer.is_some_and(|q| r.contains(q));
            let fill = if is_hover {
                theme::SELECT_BG
            } else if self.labels.printed.contains(id) {
                PRINTED
            } else {
                theme::SHEET
            };
            painter.rect_filled(r, 0.0, fill);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(
                    if is_hover { 2.0_f32 } else { 1.0 },
                    if is_hover {
                        theme::SELECT_STROKE
                    } else {
                        theme::TEXT
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let num_size = (r.height().min(r.width()) * 0.45).clamp(9.0, 26.0);
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                p["number"].to_string(),
                FontId::proportional(num_size),
                theme::TEXT,
            );
            if r.height() > 3.0 * num_size && r.width() > 90.0 {
                painter.text(
                    r.center() + Vec2::new(0.0, num_size * 0.9),
                    Align2::CENTER_TOP,
                    format!("{} · {}", id, p["name"].as_str().unwrap_or("")),
                    FontId::proportional(theme::SMALL),
                    theme::TEXT_WEAK,
                );
            }
            if is_hover {
                hovered = Some(p);
            }
        }
        let mut clicked = None;
        if let Some(p) = hovered {
            let id = p["id"].as_str().unwrap_or("").to_owned();
            let programs: Vec<&str> = p["programs"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                .unwrap_or_default();
            let tip = format!(
                "{}: {} ({})\n{} × {} mm{}\nCX100: {}{}",
                p["number"],
                p["name"].as_str().unwrap_or(""),
                id,
                crate::part_view::fmt(p["length"].as_f64().unwrap_or(0.0)),
                crate::part_view::fmt(p["width"].as_f64().unwrap_or(0.0)),
                if p["rotated"].as_bool() == Some(true) {
                    tr!("labels_view.part.rotated")
                } else {
                    String::new()
                },
                if programs.is_empty() {
                    "–".to_owned()
                } else {
                    programs.join(", ")
                },
                p["up_down"]
                    .as_str()
                    .map(|u| format!(" · {u}"))
                    .unwrap_or_default()
            );
            let response = response.on_hover_text(tip);
            if response.clicked() {
                clicked = Some(id);
            }
        }
        clicked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kľúče poradia štítkov sa vyberajú za behu, `cargo xtask i18n` ich nevidí.
    #[test]
    fn every_label_order_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for o in LabelOrder::ALL {
                let key = order_key(o);
                assert!(catalog.contains(key), "{code}.toml: chýba {key}");
            }
        }
    }
}
