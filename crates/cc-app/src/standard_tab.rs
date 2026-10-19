//! Záložka Štandard: hodnoty konštrukčného štandardu.
//!
//! Widgety menia kópiu štandardu; každá zmenená hodnota ide ako `standard.set`.

use cc_i18n::tr;
use cc_model::{DowelLayout, LineBoreLayout, TopStyle};
use eframe::egui::{self, Color32, RichText};
use serde_json::{json, Value};

use crate::app::App;
use crate::part_view::fmt;

impl App {
    pub(crate) fn tab_standard(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let original = doc.standard.clone();
        let mut std = original.clone();
        let construction = doc.project.defaults.construction.clone();
        let path = doc.standard_path();
        let dirty = doc.standard_dirty;
        let machine = doc.machine.clone();
        let mut action: Option<&'static str> = None;

        ui.horizontal(|ui| {
            ui.label(RichText::new(tr!("standard_tab.title", name = &construction)).strong());
            ui.label(
                RichText::new(
                    path.file_name()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
                .weak(),
            )
            .on_hover_text(path.display().to_string());
            if dirty {
                ui.colored_label(Color32::from_rgb(190, 130, 0), tr!("standard_tab.unsaved"));
            }
            if ui
                .add_enabled(dirty, egui::Button::new(tr!("standard_tab.save")))
                .clicked()
            {
                action = Some("standard.save");
            }
            if ui.button(tr!("standard_tab.reload")).clicked() {
                action = Some("standard.reload");
            }
        });
        ui.label(RichText::new(tr!("standard_tab.hint")).weak());
        ui.separator();
        let s = &mut std;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    let ui = &mut cols[0];
                    section(ui, "carcass", &tr!("standard_tab.carcass.title"), |ui| {
                        ui.label(tr!("standard_tab.carcass.top_style"));
                        egui::ComboBox::from_id_salt("top_style")
                            .selected_text(match s.carcass.top_style {
                                TopStyle::Stretchers => tr!("standard_tab.carcass.stretchers"),
                                TopStyle::Panel => tr!("standard_tab.carcass.panel"),
                                TopStyle::Sink => tr!("standard_tab.carcass.sink"),
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut s.carcass.top_style,
                                    TopStyle::Stretchers,
                                    tr!("standard_tab.carcass.stretchers"),
                                );
                                ui.selectable_value(
                                    &mut s.carcass.top_style,
                                    TopStyle::Panel,
                                    tr!("standard_tab.carcass.panel"),
                                );
                            });
                        ui.end_row();
                        num(
                            ui,
                            &tr!("standard_tab.carcass.stretcher_width"),
                            &mut s.carcass.stretcher_width,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.carcass.shelf_side_clearance"),
                            &mut s.carcass.shelf_side_clearance,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.carcass.shelf_front_setback"),
                            &mut s.carcass.shelf_front_setback,
                        );
                    });
                    section(ui, "back", &tr!("standard_tab.back.title"), |ui| {
                        num(
                            ui,
                            &tr!("standard_tab.back.groove_offset"),
                            &mut s.back.groove_offset_from_rear,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.back.groove_width_clearance"),
                            &mut s.back.groove_width_clearance,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.back.groove_depth"),
                            &mut s.back.groove_depth,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.back.fit_clearance"),
                            &mut s.back.fit_clearance,
                        );
                        ui.label(tr!("standard_tab.back.groove_through"));
                        ui.checkbox(&mut s.back.groove_through, "");
                        ui.end_row();
                    });
                    let ui = &mut cols[1];
                    section(ui, "dowel", &tr!("standard_tab.dowel.title"), |ui| {
                        num(
                            ui,
                            &tr!("standard_tab.dowel.diameter"),
                            &mut s.dowel.diameter,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.dowel.face_depth"),
                            &mut s.dowel.face_depth,
                        );
                        num(
                            ui,
                            &tr!("standard_tab.dowel.edge_depth"),
                            &mut s.dowel.edge_depth,
                        );
                        ui.label(tr!("standard_tab.dowel.layout"));
                        egui::ComboBox::from_id_salt("dowel_layout")
                            .selected_text(match s.dowel.layout {
                                DowelLayout::Cv333 => tr!("standard_tab.dowel.cv333"),
                                DowelLayout::Pairs => tr!("standard_tab.dowel.pairs"),
                                DowelLayout::Even => tr!("standard_tab.dowel.even"),
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut s.dowel.layout,
                                    DowelLayout::Cv333,
                                    tr!("standard_tab.dowel.cv333"),
                                );
                                ui.selectable_value(
                                    &mut s.dowel.layout,
                                    DowelLayout::Pairs,
                                    tr!("standard_tab.dowel.pairs"),
                                );
                                ui.selectable_value(
                                    &mut s.dowel.layout,
                                    DowelLayout::Even,
                                    tr!("standard_tab.dowel.even"),
                                );
                            });
                        ui.end_row();
                        num(
                            ui,
                            &tr!("standard_tab.dowel.end_offset"),
                            &mut s.dowel.end_offset,
                        );
                        match s.dowel.layout {
                            DowelLayout::Cv333 => {
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.confirmat_pitch"),
                                    &mut s.dowel.pair_pitch,
                                );
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.short_end_offset"),
                                    &mut s.dowel.short_end_offset,
                                );
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.group_spacing"),
                                    &mut s.dowel.group_spacing[0],
                                );
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.group_spacing_long"),
                                    &mut s.dowel.group_spacing[1],
                                );
                            }
                            DowelLayout::Pairs => {
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.pair_pitch"),
                                    &mut s.dowel.pair_pitch,
                                );
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.max_pair_spacing"),
                                    &mut s.dowel.max_pair_spacing,
                                );
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.short_end_offset"),
                                    &mut s.dowel.short_end_offset,
                                );
                            }
                            DowelLayout::Even => {
                                num(
                                    ui,
                                    &tr!("standard_tab.dowel.max_spacing"),
                                    &mut s.dowel.max_spacing,
                                );
                                ui.label(tr!("standard_tab.dowel.snap_32"));
                                ui.checkbox(&mut s.dowel.snap_32, "");
                                ui.end_row();
                            }
                        }
                        ui.label(tr!("standard_tab.dowel.stop_at_back"));
                        ui.checkbox(&mut s.dowel.stop_at_back, "");
                        ui.end_row();
                    });
                    section(
                        ui,
                        "line_bore",
                        &tr!("standard_tab.line_bore.title"),
                        |ui| {
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.diameter"),
                                &mut s.line_bore.diameter,
                            );
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.depth"),
                                &mut s.line_bore.depth,
                            );
                            ui.label(tr!("standard_tab.line_bore.layout"));
                            egui::ComboBox::from_id_salt("bore_layout")
                                .selected_text(match s.line_bore.layout {
                                    LineBoreLayout::PerShelf => {
                                        tr!("standard_tab.line_bore.per_shelf")
                                    }
                                    LineBoreLayout::Full => tr!("standard_tab.line_bore.full"),
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut s.line_bore.layout,
                                        LineBoreLayout::PerShelf,
                                        tr!("standard_tab.line_bore.per_shelf"),
                                    );
                                    ui.selectable_value(
                                        &mut s.line_bore.layout,
                                        LineBoreLayout::Full,
                                        tr!("standard_tab.line_bore.full"),
                                    );
                                });
                            ui.end_row();
                            if s.line_bore.layout == LineBoreLayout::PerShelf {
                                ui.label(tr!("standard_tab.line_bore.holes_per_shelf"));
                                ui.add(
                                    egui::DragValue::new(&mut s.line_bore.holes_per_shelf)
                                        .range(1..=9),
                                );
                                ui.end_row();
                            }
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.pitch"),
                                &mut s.line_bore.pitch,
                            );
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.front_offset"),
                                &mut s.line_bore.front_offset,
                            );
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.rear_offset"),
                                &mut s.line_bore.rear_offset,
                            );
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.start_above_bottom"),
                                &mut s.line_bore.start_above_bottom,
                            );
                            num(
                                ui,
                                &tr!("standard_tab.line_bore.end_below_top"),
                                &mut s.line_bore.end_below_top,
                            );
                        },
                    );
                    if let Some(m) = &machine {
                        section(ui, "machine", &tr!("standard_tab.machine.title"), |ui| {
                            ui.label(tr!("standard_tab.machine.machine"));
                            ui.label(&m.machine.name);
                            ui.end_row();
                            ui.label(tr!("standard_tab.machine.drills"));
                            ui.label(
                                m.drills
                                    .diameters
                                    .iter()
                                    .map(|d| format!("Ø{}", fmt(*d)))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            );
                            ui.end_row();
                            if let Some(x) = &m.xcs {
                                ui.label(tr!("standard_tab.machine.saw"));
                                ui.label(tr!(
                                    "standard_tab.machine.kerf",
                                    kerf = fmt(x.slot.saw_kerf),
                                    step = fmt(x.slot.max_step)
                                ));
                                ui.end_row();
                            }
                            ui.label(tr!("standard_tab.machine.work_area"));
                            ui.label(format!(
                                "{} × {} mm",
                                fmt(m.work_area.max_length),
                                fmt(m.work_area.max_width)
                            ));
                            ui.end_row();
                        });
                    }
                });
            });

        if std != original {
            let (Ok(a), Ok(b)) = (serde_json::to_value(&original), serde_json::to_value(&std))
            else {
                return;
            };
            let mut changes = Vec::new();
            json_diff(&a, &b, String::new(), &mut changes);
            for (path, value) in changes {
                self.exec(
                    "standard.set",
                    json!({ "path": path, "value": value, "coalesce": true }),
                );
            }
        }
        if let Some(id) = action {
            if self.exec(id, json!({})).is_some() {
                self.set_status(
                    if id == "standard.save" {
                        tr!("standard_tab.status.saved")
                    } else {
                        tr!("standard_tab.status.reloaded")
                    },
                    false,
                );
            }
        }
    }
}

/// Cesty (`a.b.0.c`) k hodnotám, ktoré sa v `b` líšia od `a`.
fn json_diff(a: &Value, b: &Value, path: String, out: &mut Vec<(String, Value)>) {
    if a == b {
        return;
    }
    let join = |k: &str| {
        if path.is_empty() {
            k.to_owned()
        } else {
            format!("{path}.{k}")
        }
    };
    match (a, b) {
        (Value::Object(x), Value::Object(y)) if x.len() == y.len() => {
            for (k, vb) in y {
                match x.get(k) {
                    Some(va) => json_diff(va, vb, join(k), out),
                    None => out.push((join(k), vb.clone())),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                json_diff(va, vb, join(&i.to_string()), out);
            }
        }
        _ if !path.is_empty() => out.push((path, b.clone())),
        _ => {}
    }
}

/// Sekcia s nadpisom `title`; `id` je stály (nezávisí od jazyka).
fn section(ui: &mut egui::Ui, id: &str, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).strong().size(15.0));
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new(("standard_section", id))
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, body);
    });
    ui.add_space(6.0);
}

fn num(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    ui.label(label);
    ui.add(
        egui::DragValue::new(v)
            .speed(0.5)
            .max_decimals(2)
            .range(0.0..=5000.0)
            .suffix(" mm"),
    );
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_finds_leaf_paths() {
        let a = json!({"dowel": {"d": 8, "x": [1, 2]}, "b": true});
        let b = json!({"dowel": {"d": 10, "x": [1, 3]}, "b": true});
        let mut out = Vec::new();
        json_diff(&a, &b, String::new(), &mut out);
        assert_eq!(
            out,
            vec![
                ("dowel.d".into(), json!(10)),
                ("dowel.x.1".into(), json!(3))
            ]
        );
    }
}
