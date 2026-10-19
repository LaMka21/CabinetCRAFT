//! Ľavý panel: katalógy skriniek ako v CV („Select Catalog…“). Výber katalógu, položky
//! po skupinách, pridanie do zákazky, uloženie vybranej skrinky do katalógu, nový katalóg.

use cc_i18n::tr;
use eframe::egui::{self, RichText};
use serde_json::json;

use crate::app::App;
use crate::part_view::fmt;
use crate::theme;

#[derive(Default)]
pub struct CatalogUi {
    /// Otvorený katalóg (meno).
    pub current: Option<String>,
    /// Vybraná položka (ID).
    pub item: Option<String>,
    /// Rozpísané meno nového katalógu (`Some` = pole je otvorené).
    pub new_catalog: Option<String>,
    /// Uloženie vybranej skrinky: ID a meno položky.
    pub save_id: String,
    pub save_name: String,
    pub save_group: String,
    /// Skrinka, pre ktorú je vyplnený formulár uloženia (pri zmene výberu sa vyprázdni).
    pub save_for: Option<String>,
}

/// Položka katalógu ťahaná myšou do pôdorysu alebo pohľadu na stenu (vloženie ako v CV).
#[derive(Clone, Debug)]
pub struct CatalogDrag {
    pub id: String,
    pub catalog: Option<String>,
    pub w: f64,
    pub h: f64,
    pub d: f64,
    pub cabinet_type: cc_model::CabinetType,
}

impl CatalogDrag {
    /// `cabinet.add` na stenu `wall`, ľavá hrana `at`; pri prekrytí k susedovi.
    pub fn add_params(&self, wall: &str, at: f64) -> serde_json::Value {
        let mut p = json!({ "catalog": self.id, "wall": wall, "at": at.round(), "snap": true });
        if let Some(c) = &self.catalog {
            p["from"] = json!(c);
        }
        p
    }
}

impl App {
    pub(crate) fn catalog_section(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let names: Vec<String> = doc.catalogs.iter().map(|c| c.name.clone()).collect();
        if self
            .catalog_ui
            .current
            .as_ref()
            .is_none_or(|c| !names.contains(c))
        {
            self.catalog_ui.current = names.first().cloned();
        }
        let current = self.catalog_ui.current.clone();
        #[allow(clippy::type_complexity)]
        let items: Vec<(
            String,
            String,
            String,
            String,
            [f64; 3],
            cc_model::CabinetType,
        )> = current
            .as_ref()
            .and_then(|name| doc.catalogs.iter().find(|c| &c.name == name))
            .map(|c| {
                c.catalog
                    .items
                    .iter()
                    .map(|i| {
                        (
                            i.group.clone(),
                            i.id.clone(),
                            i.name.clone(),
                            format!("{} × {} × {}", fmt(i.w), fmt(i.h), fmt(i.d)),
                            [i.w, i.h, i.d],
                            i.cabinet_type,
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let selected_cabinet = self.session.selection().cabinet.clone();
        if !self.section(ui, &tr!("catalog.title")) {
            return;
        }
        let mut cmds: Vec<(&'static str, serde_json::Value)> = Vec::new();

        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("catalog_current")
                .selected_text(current.clone().unwrap_or_else(|| tr!("catalog.none")))
                .width(ui.available_width() - 70.0)
                .show_ui(ui, |ui| {
                    for n in &names {
                        if ui
                            .selectable_label(current.as_ref() == Some(n), n)
                            .clicked()
                        {
                            self.catalog_ui.current = Some(n.clone());
                            self.catalog_ui.item = None;
                        }
                    }
                });
            if ui
                .small_button(tr!("catalog.new"))
                .on_hover_text(tr!("catalog.new_hint"))
                .clicked()
            {
                self.catalog_ui.new_catalog = Some(String::new());
            }
        });
        if let Some(name) = &mut self.catalog_ui.new_catalog {
            let mut create = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text(tr!("catalog.name_hint"))
                        .desired_width(ui.available_width() - 90.0),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    create = true;
                }
                create |= ui.small_button(tr!("catalog.create")).clicked();
                cancel = ui.small_button("✖").clicked();
            });
            if create && !name.trim().is_empty() {
                cmds.push(("catalog.create", json!({ "name": name.trim() })));
                self.catalog_ui.current = Some(name.trim().to_owned());
                self.catalog_ui.new_catalog = None;
            } else if cancel {
                self.catalog_ui.new_catalog = None;
            }
        }

        // položky po skupinách
        egui::ScrollArea::vertical()
            .id_salt("catalog_items")
            .max_height(230.0)
            .show(ui, |ui| {
                if items.is_empty() {
                    ui.label(RichText::new(tr!("catalog.empty")).color(theme::TEXT_WEAK));
                }
                let mut last: Option<&str> = None;
                for (group, id, name, size, [w, h, d], t) in &items {
                    if last != Some(group.as_str()) && !group.is_empty() {
                        ui.label(RichText::new(group).strong().size(theme::SMALL));
                    }
                    last = Some(group.as_str());
                    let on = self.catalog_ui.item.as_ref() == Some(id);
                    let r = ui
                        .selectable_label(on, format!("{name}   {size}"))
                        .interact(egui::Sense::click_and_drag())
                        .on_hover_text(tr!("catalog.item_hint", id = id));
                    r.dnd_set_drag_payload(CatalogDrag {
                        id: id.clone(),
                        catalog: current.clone(),
                        w: *w,
                        h: *h,
                        d: *d,
                        cabinet_type: *t,
                    });
                    if r.clicked() {
                        self.catalog_ui.item = Some(id.clone());
                        self.catalog_ui.save_id = id.clone();
                        self.catalog_ui.save_name = name.clone();
                        self.catalog_ui.save_group = group.clone();
                    }
                    if r.double_clicked() {
                        cmds.push(add_params(id, current.as_deref(), &selected_cabinet));
                    }
                }
            });

        let item = self.catalog_ui.item.clone();
        ui.horizontal(|ui| {
            let can = item.is_some();
            if ui
                .add_enabled(can, egui::Button::new(tr!("catalog.add_to_job")))
                .clicked()
            {
                if let Some(id) = &item {
                    cmds.push(add_params(id, current.as_deref(), &selected_cabinet));
                }
            }
            if ui
                .add_enabled(can, egui::Button::new(tr!("catalog.remove_item")))
                .clicked()
            {
                if let (Some(id), Some(cat)) = (&item, &current) {
                    cmds.push(("catalog.remove_item", json!({ "catalog": cat, "id": id })));
                    self.catalog_ui.item = None;
                }
            }
        });

        // uloženie vybranej skrinky
        if let (Some(cab), Some(cat)) = (&selected_cabinet, &current) {
            ui.add_space(4.0);
            ui.label(
                RichText::new(tr!("catalog.save.title", cabinet = cab, catalog = cat))
                    .size(theme::SMALL)
                    .color(theme::TEXT_WEAK),
            );
            let ui_state = &mut self.catalog_ui;
            if ui_state.save_for.as_ref() != Some(cab) {
                ui_state.save_for = Some(cab.clone());
                ui_state.save_id.clear();
                ui_state.save_name.clear();
                ui_state.save_group.clear();
            }
            if ui_state.save_id.is_empty() {
                ui_state.save_id = cab.clone();
            }
            for (label, value, hint) in [
                (
                    tr!("catalog.save.id"),
                    &mut ui_state.save_id,
                    tr!("catalog.save.id_hint"),
                ),
                (
                    tr!("catalog.save.name"),
                    &mut ui_state.save_name,
                    tr!("catalog.save.name_hint"),
                ),
                (
                    tr!("catalog.save.group"),
                    &mut ui_state.save_group,
                    tr!("catalog.save.group_hint"),
                ),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized([56.0, 18.0], egui::Label::new(label));
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .hint_text(hint)
                            .desired_width(ui.available_width() - 4.0),
                    );
                });
            }
            let exists = items.iter().any(|i| i.1 == ui_state.save_id.trim());
            let text = if exists {
                tr!("catalog.save.overwrite")
            } else {
                tr!("catalog.save.save")
            };
            if ui
                .add_enabled(!ui_state.save_id.trim().is_empty(), egui::Button::new(text))
                .clicked()
            {
                let mut params = json!({
                    "catalog": cat,
                    "cabinet": cab,
                    "id": ui_state.save_id.trim(),
                    "group": ui_state.save_group.trim(),
                });
                if !ui_state.save_name.trim().is_empty() {
                    params["name"] = json!(ui_state.save_name.trim());
                }
                cmds.push(("catalog.save_item", params));
                ui_state.item = Some(ui_state.save_id.trim().to_owned());
            }
        }

        for (id, params) in cmds {
            if self.exec(id, params).is_some() && id.starts_with("catalog.") {
                self.set_status(
                    match id {
                        "catalog.create" => tr!("catalog.status.created"),
                        "catalog.save_item" => tr!("catalog.status.saved"),
                        _ => tr!("catalog.status.removed"),
                    },
                    false,
                );
            }
        }
    }
}

fn add_params(
    id: &str,
    catalog: Option<&str>,
    after: &Option<String>,
) -> (&'static str, serde_json::Value) {
    let mut p = json!({ "catalog": id });
    if let Some(c) = catalog {
        p["from"] = json!(c);
    }
    if let Some(a) = after {
        p["after"] = json!(a);
    }
    ("cabinet.add", p)
}
