//! Vlastnosti skrinky ako „Assembly Properties“ v CV: záložky Všeobecné, Komentár, Nožičky,
//! Prepísania a Parametre. Mení sa koncept; OK pošle zmenené polia príkazom `cabinet.set`.

use cc_i18n::tr;
use cc_model::{CabinetSpec, CabinetType};
use eframe::egui::{self, Align2};
use serde_json::{json, Map, Value};

use crate::app::App;
use crate::part_view::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PropsTab {
    General,
    Comment,
    Legs,
    Overrides,
    Parameters,
}

pub struct PropsDialog {
    /// ID skrinky pri otvorení (premenovanie ide cez koncept).
    pub cabinet: String,
    pub tab: PropsTab,
    pub draft: CabinetSpec,
    /// Rozpísané hodnoty v záložke Parametre: (kľúč, text).
    edit: Option<(String, String)>,
    error: Option<String>,
}

impl PropsDialog {
    pub fn new(c: &CabinetSpec) -> PropsDialog {
        PropsDialog {
            cabinet: c.id.clone(),
            tab: PropsTab::General,
            draft: c.clone(),
            edit: None,
            error: None,
        }
    }
}

/// Polia, ktoré pri skrinke prepisujú štandard (záložka Prepísania, ako Overrides v CV):
/// pole konceptu, kľúč textu (`cc_i18n::tr`).
const OVERRIDES: [(&str, &str); 10] = [
    ("reveals", "props.overrides.reveals"),
    ("front_mount", "props.overrides.front_mount"),
    ("hinge", "props.overrides.hinge"),
    ("drawer", "props.overrides.drawer"),
    ("drawer_length", "props.overrides.drawer_length"),
    ("top_style", "props.overrides.top_style"),
    ("back", "props.overrides.back"),
    ("carcass", "props.overrides.carcass"),
    ("elevation", "props.overrides.elevation"),
    ("options", "props.overrides.options"),
];

fn to_object(c: &CabinetSpec) -> Map<String, Value> {
    match serde_json::to_value(c) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

fn from_object(m: Map<String, Value>) -> Result<CabinetSpec, String> {
    serde_json::from_value(Value::Object(m)).map_err(|e| e.to_string())
}

/// Zmenené polia konceptu voči originálu pre `cabinet.set` (chýbajúce = null).
fn diff(original: &CabinetSpec, draft: &CabinetSpec) -> Map<String, Value> {
    let (a, b) = (to_object(original), to_object(draft));
    let mut out = Map::new();
    for (k, v) in &b {
        if a.get(k) != Some(v) {
            out.insert(k.clone(), v.clone());
        }
    }
    for k in a.keys() {
        if !b.contains_key(k) {
            out.insert(k.clone(), Value::Null);
        }
    }
    out
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), fmt),
        other => other.to_string(),
    }
}

impl App {
    /// Otvorí vlastnosti skrinky `id`.
    pub(crate) fn open_props(&mut self, id: &str) {
        let spec = self
            .session
            .doc()
            .and_then(|d| d.project.cabinets.iter().find(|c| c.id == id).cloned());
        if let Some(c) = spec {
            self.props = Some(PropsDialog::new(&c));
        }
    }

    pub(crate) fn props_window(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.props.take() else {
            return;
        };
        let Some(doc) = self.session.doc() else {
            return;
        };
        let Some((index, original)) = doc
            .project
            .cabinets
            .iter()
            .enumerate()
            .find(|(_, c)| c.id == dlg.cabinet)
            .map(|(i, c)| (i, c.clone()))
        else {
            return;
        };
        let std_opts = doc.standard.cabinet_options;
        let mut close = false;
        let mut apply = false;
        egui::Window::new(tr!("props.title", id = &original.id))
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (PropsTab::General, tr!("props.tab.general")),
                        (PropsTab::Comment, tr!("props.tab.comment")),
                        (PropsTab::Legs, tr!("props.tab.legs")),
                        (PropsTab::Overrides, tr!("props.tab.overrides")),
                        (PropsTab::Parameters, tr!("props.tab.parameters")),
                    ] {
                        ui.selectable_value(&mut dlg.tab, tab, label);
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| match dlg.tab {
                        PropsTab::General => general(ui, &mut dlg.draft, index),
                        PropsTab::Comment => {
                            let mut text = dlg.draft.comment.clone().unwrap_or_default();
                            if ui
                                .add(
                                    egui::TextEdit::multiline(&mut text)
                                        .desired_rows(10)
                                        .desired_width(f32::INFINITY),
                                )
                                .changed()
                            {
                                dlg.draft.comment = (!text.trim().is_empty()).then_some(text);
                            }
                        }
                        PropsTab::Legs => legs(ui, &mut dlg.draft, &std_opts),
                        PropsTab::Overrides => overrides(ui, &mut dlg),
                        PropsTab::Parameters => parameters(ui, &mut dlg),
                    });
                if let Some(e) = &dlg.error {
                    ui.colored_label(egui::Color32::from_rgb(190, 40, 40), e);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr!("props.ok")).clicked() {
                        apply = true;
                    }
                    if ui.button(tr!("props.cancel")).clicked() {
                        close = true;
                    }
                });
            });
        if apply {
            let set = diff(&original, &dlg.draft);
            if !set.is_empty() {
                self.exec(
                    "cabinet.set",
                    json!({ "id": original.id, "set": Value::Object(set) }),
                );
            }
            return;
        }
        if !close {
            self.props = Some(dlg);
        }
    }
}

fn general(ui: &mut egui::Ui, c: &mut CabinetSpec, index: usize) {
    egui::Grid::new("props_general")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label(tr!("props.general.id"));
            ui.text_edit_singleline(&mut c.id);
            ui.end_row();
            ui.label(tr!("props.general.type"));
            let type_name = |t: CabinetType| match t {
                CabinetType::Base => tr!("props.general.base"),
                CabinetType::Wall => tr!("props.general.wall"),
                CabinetType::Tall => tr!("props.general.tall"),
            };
            egui::ComboBox::from_id_salt("props_type")
                .selected_text(type_name(c.cabinet_type))
                .show_ui(ui, |ui| {
                    for t in [CabinetType::Base, CabinetType::Wall, CabinetType::Tall] {
                        ui.selectable_value(&mut c.cabinet_type, t, type_name(t));
                    }
                });
            ui.end_row();
            ui.label(tr!("props.general.number"));
            ui.label(format!("{}", index + 1));
            ui.end_row();
            for (label, v) in [
                (tr!("props.general.width"), &mut c.w),
                (tr!("props.general.height"), &mut c.h),
                (tr!("props.general.depth"), &mut c.d),
            ] {
                ui.label(label);
                ui.add(egui::DragValue::new(v).range(50.0..=3000.0).suffix(" mm"));
                ui.end_row();
            }
            ui.label(tr!("props.general.lock_width"));
            ui.checkbox(&mut c.lock_width, "");
            ui.end_row();
        });
}

fn legs(ui: &mut egui::Ui, c: &mut CabinetSpec, std: &cc_model::ResolvedOptions) {
    let yes_no = |b: bool| {
        if b {
            tr!("props.legs.yes")
        } else {
            tr!("props.legs.no")
        }
    };
    for (label, slot, standard) in [
        (tr!("props.legs.legs"), &mut c.options.legs, std.legs),
        (tr!("props.legs.plinth"), &mut c.options.plinth, std.plinth),
    ] {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.radio_value(
                slot,
                None,
                tr!("props.legs.by_standard", value = yes_no(standard)),
            );
            ui.radio_value(slot, Some(true), yes_no(true));
            ui.radio_value(slot, Some(false), yes_no(false));
        });
    }
}

fn overrides(ui: &mut egui::Ui, dlg: &mut PropsDialog) {
    let obj = to_object(&dlg.draft);
    let mut remove: Option<(String, Option<String>)> = None;
    let mut any = false;
    egui::Grid::new("props_overrides")
        .num_columns(3)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (key, label_key) in OVERRIDES {
                let Some(v) = obj.get(key) else {
                    continue;
                };
                let label = cc_i18n::tr(label_key);
                match v {
                    Value::Object(sub) => {
                        for (k, sv) in sub {
                            any = true;
                            ui.label(format!("{label}: {k}"));
                            ui.label(value_text(sv));
                            if ui.small_button(tr!("props.overrides.remove")).clicked() {
                                remove = Some((key.to_owned(), Some(k.clone())));
                            }
                            ui.end_row();
                        }
                    }
                    other => {
                        any = true;
                        ui.label(&label);
                        ui.label(value_text(other));
                        if ui.small_button(tr!("props.overrides.remove")).clicked() {
                            remove = Some((key.to_owned(), None));
                        }
                        ui.end_row();
                    }
                }
            }
        });
    if !any {
        ui.label(tr!("props.overrides.none"));
    }
    if let Some((key, sub)) = remove {
        let mut obj = obj;
        match sub {
            Some(k) => {
                if let Some(Value::Object(m)) = obj.get_mut(&key) {
                    m.remove(&k);
                }
            }
            None => {
                obj.remove(&key);
            }
        }
        match from_object(obj) {
            Ok(c) => {
                dlg.draft = c;
                dlg.error = None;
            }
            Err(e) => dlg.error = Some(e),
        }
    }
}

fn parameters(ui: &mut egui::Ui, dlg: &mut PropsDialog) {
    let obj = to_object(&dlg.draft);
    let mut commit: Option<(String, String)> = None;
    egui::Grid::new("props_params")
        .num_columns(2)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.strong(tr!("props.params.parameter"));
            ui.strong(tr!("props.params.value"));
            ui.end_row();
            for (k, v) in &obj {
                ui.label(k);
                let current = value_text(v);
                let mut text = match &dlg.edit {
                    Some((ek, t)) if ek == k => t.clone(),
                    _ => current.clone(),
                };
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(260.0));
                if r.changed() || r.has_focus() {
                    dlg.edit = Some((k.clone(), text.clone()));
                }
                if r.lost_focus() {
                    if text != current {
                        commit = Some((k.clone(), text));
                    }
                    dlg.edit = None;
                }
                ui.end_row();
            }
        });
    if let Some((k, text)) = commit {
        let mut obj = obj;
        // číslo, true/false, JSON (pole, objekt) alebo text
        let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        obj.insert(k, value);
        match from_object(obj) {
            Ok(c) => {
                dlg.draft = c;
                dlg.error = None;
            }
            Err(e) => dlg.error = Some(tr!("props.params.invalid_value", error = e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OVERRIDES;

    /// Kľúče textov prepísaní sa skladajú za behu: každý musí byť v sk aj en.
    #[test]
    fn every_override_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for (_, key) in OVERRIDES {
                assert!(catalog.contains(key), "{code}.toml: chýba {key}");
            }
        }
    }
}
