//! Ľavý panel: úlohy pre aktuálny pohľad (veľké ikony) a pod nimi mriežka
//! vlastností so zbaliteľnými skupinami (projekt, skrinky, vybraná skrinka, stroj).
//!
//! Widgety menia kópiu zadania; zmena sa na konci pošle ako príkaz.

use cc_i18n::tr;
use cc_model::{
    CabinetSpec, CabinetType, FrontKind, FrontMount, FrontSpec, HingeSide, JointType, ShelfSupport,
    TipOn, TipOnColor, TipOnKind,
};
use eframe::egui::{self, pos2, vec2, Align2, FontId, Rect, RichText, Sense, Stroke};
use serde_json::{json, Map, Value};

use crate::app::{App, Tab};
use crate::icons::{self, Icon};
use crate::part_view::fmt;
use crate::theme;
use cc_scene::Camera;

/// Šírka stĺpca s menom vlastnosti.
const LABEL_W: f32 = 140.0;
const ROW_H: f32 = 22.0;

/// Akcia úlohy v ľavom paneli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
    AddBase,
    AddWall,
    AddTall,
    Check,
    Export,
    ExportTo,
    OpenExport,
    PartsList,
    ResetCamera,
    Explode,
    PrevPart,
    NextPart,
    SaveStandard,
    ReloadStandard,
    PrintAll,
    PrintSheet,
    Labels,
}

impl App {
    pub(crate) fn side_panel(&mut self, ui: &mut egui::Ui) {
        if self.session.doc().is_none() {
            ui.add_space(8.0);
            ui.label(RichText::new(tr!("side_panel.general.no_project")).size(theme::HEADING));
            if let Some(e) = &self.open_error {
                ui.colored_label(theme::ERROR, e);
            }
            ui.label(tr!("side_panel.general.open_hint"));
            return;
        }
        if matches!(self.tab, Tab::Plan | Tab::Elevation) {
            // pôdorys a pohľad: zoznam režimov ako v CV (plan_modes.rs)
            self.plan_panel(ui);
        } else if self.tab == Tab::Cabinet {
            // editor skrinky: panel podľa jeho záložky (cabinet_editor.rs)
            self.cabinet_panel(ui);
        } else if self.tab == Tab::PartEdit {
            // otvorená doska: panel podľa jej záložky (part_editor.rs)
            self.part_panel(ui);
        } else {
            self.task_list(ui);
            ui.add_space(10.0);
        }
        match self.tab {
            Tab::Plan | Tab::Elevation => {}
            Tab::View3d | Tab::Reports => {
                self.catalog_section(ui);
                self.cabinet_sections(ui);
                self.project_section(ui);
            }
            Tab::Part => {
                self.parts_picker(ui);
            }
            Tab::Parts | Tab::Issues | Tab::Labels => {
                self.machine_section(ui);
                self.project_section(ui);
            }
            Tab::Standard => self.project_section(ui),
            Tab::Cabinet | Tab::PartEdit => {}
        }
        // problém zadania
        if let Some(e) = self.build_error() {
            ui.add_space(8.0);
            ui.colored_label(
                theme::ERROR,
                tr!("side_panel.general.build_error", error = e),
            );
        }
    }

    // ----- úlohy -----

    fn task_list(&mut self, ui: &mut egui::Ui) {
        let can_export = matches!(self.session.built(), Some(Ok(b)) if !b.has_errors());
        let tasks: Vec<(Icon, String, Task, bool)> = match self.tab {
            Tab::Cabinet | Tab::PartEdit => vec![
                (
                    Icon::Parts,
                    tr!("side_panel.task.parts_list"),
                    Task::PartsList,
                    true,
                ),
                (Icon::Check, tr!("side_panel.task.check"), Task::Check, true),
            ],
            Tab::Plan | Tab::Elevation | Tab::Reports => {
                let tasks = vec![
                    (
                        Icon::CabinetBase,
                        tr!("side_panel.task.add_base"),
                        Task::AddBase,
                        true,
                    ),
                    (
                        Icon::CabinetWall,
                        tr!("side_panel.task.add_wall"),
                        Task::AddWall,
                        true,
                    ),
                    (
                        Icon::CabinetTall,
                        tr!("side_panel.task.add_tall"),
                        Task::AddTall,
                        true,
                    ),
                    (
                        Icon::Parts,
                        tr!("side_panel.task.parts_list"),
                        Task::PartsList,
                        true,
                    ),
                    (Icon::Check, tr!("side_panel.task.check"), Task::Check, true),
                ];
                tasks
            }
            Tab::View3d => vec![
                (
                    Icon::Cube,
                    tr!("side_panel.task.reset_camera"),
                    Task::ResetCamera,
                    true,
                ),
                (
                    Icon::Duplicate,
                    tr!("side_panel.task.explode"),
                    Task::Explode,
                    true,
                ),
                (Icon::Check, tr!("side_panel.task.check"), Task::Check, true),
            ],
            Tab::Part => {
                let can_print = self.session.model().is_some();
                vec![
                    (
                        Icon::Drawing,
                        tr!("side_panel.task.print_all"),
                        Task::PrintAll,
                        can_print,
                    ),
                    (
                        Icon::Drawing,
                        tr!("side_panel.task.print_sheet"),
                        Task::PrintSheet,
                        can_print,
                    ),
                    (
                        Icon::MoveUp,
                        tr!("side_panel.task.prev_part"),
                        Task::PrevPart,
                        true,
                    ),
                    (
                        Icon::MoveDown,
                        tr!("side_panel.task.next_part"),
                        Task::NextPart,
                        true,
                    ),
                ]
            }
            Tab::Parts | Tab::Issues | Tab::Labels => vec![
                (
                    Icon::Parts,
                    tr!("side_panel.task.parts_list"),
                    Task::PartsList,
                    true,
                ),
                (Icon::Check, tr!("side_panel.task.check"), Task::Check, true),
                (
                    Icon::Drawing,
                    tr!("side_panel.task.labels"),
                    Task::Labels,
                    true,
                ),
                (
                    Icon::Export,
                    tr!("side_panel.task.export"),
                    Task::Export,
                    can_export,
                ),
                (
                    Icon::Open,
                    tr!("side_panel.task.export_to"),
                    Task::ExportTo,
                    can_export,
                ),
                (
                    Icon::Open,
                    tr!("side_panel.task.open_export"),
                    Task::OpenExport,
                    self.last_export.is_some(),
                ),
            ],
            Tab::Standard => {
                let dirty = self.session.doc().is_some_and(|d| d.standard_dirty);
                vec![
                    (
                        Icon::Save,
                        tr!("side_panel.task.save_standard"),
                        Task::SaveStandard,
                        dirty,
                    ),
                    (
                        Icon::Reload,
                        tr!("side_panel.task.reload_standard"),
                        Task::ReloadStandard,
                        true,
                    ),
                ]
            }
        };
        for (icon, label, task, enabled) in tasks {
            let label = label.as_str();
            let active = match task {
                Task::PartsList => self.tab == Tab::Parts,
                Task::Check => self.tab == Tab::Issues,
                Task::Labels => self.tab == Tab::Labels,
                Task::Explode => self.scene_opts.explode > 0.0,
                _ => false,
            };
            if task_row(ui, icon, label, enabled, active).clicked() {
                self.run_task(task);
            }
        }
    }

    fn run_task(&mut self, task: Task) {
        match task {
            Task::AddBase | Task::AddWall | Task::AddTall => {
                let t = match task {
                    Task::AddBase => "base",
                    Task::AddWall => "wall",
                    _ => "tall",
                };
                self.exec("cabinet.add", json!({ "type": t }));
            }
            Task::Check => self.tab = Tab::Issues,
            Task::PartsList => self.tab = Tab::Parts,
            Task::Labels => self.tab = Tab::Labels,
            Task::Export => {
                if let Some(dir) = self.default_export_dir() {
                    self.export(dir);
                }
            }
            Task::ExportTo => {
                let mut dialog = rfd::FileDialog::new();
                if let Some(dir) = self.default_export_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    dialog = dialog.set_directory(dir);
                }
                if let Some(dir) = dialog.pick_folder() {
                    self.export(dir);
                }
            }
            Task::OpenExport => {
                if let Some(dir) = &self.last_export {
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                }
            }
            Task::ResetCamera => self.camera = Camera::default(),
            Task::Explode => {
                self.scene_opts.explode = if self.scene_opts.explode > 0.0 {
                    0.0
                } else {
                    0.6
                };
            }
            Task::PrevPart | Task::NextPart => {
                let Some(built) = self.model_built() else {
                    return;
                };
                let ids: Vec<String> = built.model.parts().map(|p| p.id.clone()).collect();
                let current = self
                    .selected_part()
                    .and_then(|id| ids.iter().position(|x| *x == id));
                let next = match (current, task == Task::NextPart) {
                    (Some(i), true) => (i + 1).min(ids.len().saturating_sub(1)),
                    (Some(i), false) => i.saturating_sub(1),
                    (None, _) => 0,
                };
                if let Some(id) = ids.get(next) {
                    self.select_part(Some(id.clone()));
                }
            }
            Task::PrintAll => self.print_drawings(None, false),
            Task::PrintSheet => {
                let key = match self.drawing.sheet.clone() {
                    crate::drawings::Sheet::Part => match self.selected_part() {
                        Some(id) => format!("part:{id}"),
                        None => return,
                    },
                    other => other.key(),
                };
                self.print_drawings(Some(vec![key]), false);
            }
            Task::SaveStandard => {
                if self.exec("standard.save", json!({})).is_some() {
                    self.set_status(tr!("side_panel.status.standard_saved"), false);
                }
            }
            Task::ReloadStandard => {
                if self.exec("standard.reload", json!({})).is_some() {
                    self.set_status(tr!("side_panel.status.standard_reloaded"), false);
                }
            }
        }
    }

    // ----- skupiny vlastností -----

    /// Hlavička skupiny (text v strede, šípka vpravo). Vráti `true`, ak je rozbalená.
    /// Zbalenie sa pamätá podľa zobrazeného textu; nezávisle od jazyka [Self::section_id].
    pub(crate) fn section(&mut self, ui: &mut egui::Ui, title: &str) -> bool {
        self.section_id(ui, title, title)
    }

    /// Hlavička skupiny so stálym `id` pre zbalenie (nezávislým od jazyka) a zobrazeným
    /// textom `title`. Vráti `true`, ak je rozbalená.
    pub(crate) fn section_id(&mut self, ui: &mut egui::Ui, id: &str, title: &str) -> bool {
        let open = !self.layout.collapsed.contains(id);
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
        if response.hovered() {
            ui.painter().rect_filled(rect, 0.0, theme::HOVER_BG);
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            title,
            FontId::proportional(theme::SMALL + 0.5),
            theme::TEXT,
        );
        let c = pos2(rect.max.x - 10.0, rect.center().y);
        let (a, b, d) = if open {
            (vec2(-4.0, 2.0), vec2(4.0, 2.0), vec2(0.0, -3.0))
        } else {
            (vec2(-4.0, -2.0), vec2(4.0, -2.0), vec2(0.0, 3.0))
        };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + a, c + b, c + d],
            theme::TEXT_WEAK,
            Stroke::NONE,
        ));
        if response.clicked() {
            if open {
                self.layout.collapsed.insert(id.to_owned());
            } else {
                self.layout.collapsed.remove(id);
            }
        }
        open
    }

    pub(crate) fn project_section(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let project = doc.project.clone();
        let materials = doc.materials.materials.clone();
        let standards = doc.standards_available();
        let machine = doc.machine.as_ref().map(|m| m.machine.name.clone());
        if !self.section_id(ui, "project", &tr!("side_panel.project.title")) {
            return;
        }
        let mut cmds: Vec<(&'static str, Value)> = Vec::new();
        prop_row(ui, &tr!("side_panel.project.name"), false, |ui| {
            if let Some(name) = self.buffered_text(ui, "project.name", &project.project.name) {
                cmds.push(("project.rename", json!({ "name": name })));
            }
        });
        let info = &project.project;
        for (label, key, current) in [
            (tr!("side_panel.project.number"), "number", &info.number),
            (
                tr!("side_panel.project.customer"),
                "customer",
                &info.customer,
            ),
            (
                tr!("side_panel.project.designer"),
                "designer",
                &info.designer,
            ),
            (tr!("side_panel.project.company"), "company", &info.company),
        ] {
            prop_row(ui, &label, false, |ui| {
                let current = current.as_deref().unwrap_or("");
                if let Some(v) = self.buffered_field(ui, &format!("project.{key}"), current) {
                    cmds.push(("project.info", json!({ key: v })));
                }
            });
        }
        // logo firmy: cesta (relatívna k projektu) a výber súboru
        prop_row(ui, &tr!("side_panel.project.logo"), false, |ui| {
            let current = info.logo.as_deref().unwrap_or("");
            ui.horizontal(|ui| {
                let pick = ui
                    .small_button("…")
                    .on_hover_text(tr!("side_panel.project.logo_pick_tip"))
                    .clicked();
                if !current.is_empty()
                    && ui
                        .small_button("×")
                        .on_hover_text(tr!("side_panel.project.logo_none_tip"))
                        .clicked()
                {
                    cmds.push(("project.info", json!({ "logo": null })));
                }
                if let Some(v) = self.buffered_field(ui, "project.logo", current) {
                    cmds.push(("project.info", json!({ "logo": v })));
                }
                if pick {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter(
                            tr!("side_panel.project.image_filter"),
                            &["png", "jpg", "jpeg"],
                        )
                        .pick_file()
                    {
                        cmds.push(("project.info", json!({ "logo": file.to_string_lossy() })));
                    }
                }
            });
        });
        for (label, key, current) in [
            (
                tr!("side_panel.project.panel_material"),
                "panel_material",
                &project.defaults.panel_material,
            ),
            (
                tr!("side_panel.project.back_material"),
                "back_material",
                &project.defaults.back_material,
            ),
        ] {
            prop_row(ui, &label, false, |ui| {
                let mut value = current.clone();
                egui::ComboBox::from_id_salt(key)
                    .selected_text(value.as_str())
                    .width(ui.available_width() - 4.0)
                    .show_ui(ui, |ui| {
                        for m in &materials {
                            let text = format!("{} ({} mm)", m.id, fmt(m.thickness));
                            ui.selectable_value(&mut value, m.id.clone(), text);
                        }
                    });
                if &value != current {
                    cmds.push(("defaults.set", json!({ key: value })));
                }
            });
        }
        prop_row(ui, &tr!("side_panel.project.standard"), false, |ui| {
            let current = &project.defaults.construction;
            egui::ComboBox::from_id_salt("construction")
                .selected_text(current)
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    for name in &standards {
                        if ui.selectable_label(name == current, name).clicked() && name != current {
                            cmds.push(("standard.use", json!({ "name": name })));
                        }
                    }
                });
        });
        prop_row(ui, &tr!("side_panel.project.machine"), false, |ui| {
            match machine.as_deref() {
                Some(m) => ui.label(m),
                None => ui.label(tr!("side_panel.project.no_machine")),
            };
        });
        for (id, params) in cmds {
            self.exec(id, params);
        }
    }

    pub(crate) fn cabinet_sections(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let project = doc.project.clone();
        let selected = self.selected_index();
        let mut cmds: Vec<(&'static str, Value)> = Vec::new();

        // editor skrinky má jednu skrinku (výber skrinky je v páse)
        if self.tab != Tab::Cabinet
            && self.section_id(ui, "cabinets", &tr!("side_panel.cabinets.title"))
        {
            let mut select = None;
            egui::Frame::new()
                .fill(theme::SHEET)
                .stroke(Stroke::new(1.0_f32, theme::BORDER))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    table_header(
                        ui,
                        &[
                            (tr!("side_panel.cabinets.col_id").as_str(), 52.0),
                            (tr!("side_panel.cabinets.col_type").as_str(), 62.0),
                            (tr!("side_panel.cabinets.col_size").as_str(), 0.0),
                        ],
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("cabinet_list")
                        .max_height(190.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for (i, c) in project.cabinets.iter().enumerate() {
                                let cells = [
                                    c.id.clone(),
                                    type_name(c.cabinet_type),
                                    format!("{} × {} × {}", fmt(c.w), fmt(c.h), fmt(c.d)),
                                ];
                                if table_row(ui, &cells, &[52.0, 62.0, 0.0], selected == i)
                                    .clicked()
                                {
                                    select = Some(i);
                                }
                            }
                        });
                });
            if let Some(i) = select {
                self.select_index(i);
            }
            if let Some(c) = project.cabinets.get(selected) {
                let id = c.id.clone();
                let count = project.cabinets.len();
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (icon, tip, enabled, cmd) in [
                        (
                            Icon::MoveUp,
                            tr!("side_panel.cabinets.move_up"),
                            selected > 0,
                            Some(-1),
                        ),
                        (
                            Icon::MoveDown,
                            tr!("side_panel.cabinets.move_down"),
                            selected + 1 < count,
                            Some(1),
                        ),
                        (
                            Icon::Duplicate,
                            tr!("side_panel.cabinets.duplicate"),
                            true,
                            None,
                        ),
                        (
                            Icon::Delete,
                            tr!("side_panel.cabinets.remove"),
                            count > 1,
                            None,
                        ),
                    ] {
                        if crate::ribbon::small_button(ui, Some(icon), "", enabled, false)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            match (icon, cmd) {
                                (_, Some(by)) => {
                                    cmds.push(("cabinet.move", json!({ "id": id, "by": by })))
                                }
                                (Icon::Duplicate, _) => {
                                    cmds.push(("cabinet.duplicate", json!({ "id": id })))
                                }
                                _ => cmds.push(("cabinet.remove", json!({ "id": id }))),
                            }
                        }
                    }
                });
            }
            ui.add_space(6.0);
        }

        // vybraný otvor čela (editor sekcií): panel ako „Face Size“ v CV
        if let (Some(original), Some((oid, row, col))) = (
            project.cabinets.get(selected),
            self.wall_view.opening.clone(),
        ) {
            if oid == original.id {
                let name = crate::wall_view::opening_name(row, col);
                let title = tr!("side_panel.opening.title", name = name);
                if self.section_id(ui, &format!("opening:{row}:{col:?}"), &title) {
                    self.opening_props(ui, original, row, col, &mut cmds);
                }
            }
        }
        if let Some(original) = project.cabinets.get(selected) {
            let title = tr!("side_panel.cabinet.title", id = &original.id);
            if self.section_id(ui, &format!("cabinet:{}", original.id), &title) {
                if ui
                    .button(tr!("side_panel.cabinet.properties"))
                    .on_hover_text(tr!("side_panel.cabinet.properties_tip"))
                    .clicked()
                {
                    let id = original.id.clone();
                    self.open_props(&id);
                }
                if self.tab != Tab::Cabinet
                    && ui
                        .button(tr!("side_panel.cabinet.open"))
                        .on_hover_text(tr!("side_panel.cabinet.open_tip"))
                        .clicked()
                {
                    let id = original.id.clone();
                    self.open_cabinet_editor(&id);
                }
                self.cabinet_props(ui, original, &project, &mut cmds);
            }
        }
        for (id, params) in cmds {
            self.exec(id, params);
        }
    }

    /// Otvor čela ako „Face Size“ v CV: rozmery a zámky, typ, pánt, úpravy rozmeru.
    fn opening_props(
        &mut self,
        ui: &mut egui::Ui,
        cab: &CabinetSpec,
        row: usize,
        col: Option<usize>,
        cmds: &mut Vec<(&'static str, Value)>,
    ) {
        let Some(r) = cab.fronts.get(row) else {
            return;
        };
        let Some(f) = (match col {
            Some(k) => r.columns.get(k),
            None => Some(r),
        }) else {
            return;
        };
        let size = self.session.model().and_then(|b| {
            b.model
                .cabinets
                .iter()
                .find(|c| c.id == cab.id)
                .and_then(|c| c.openings.iter().find(|o| o.row == row && o.column == col))
                .map(|o| (o.w, o.h))
        });
        let target = |extra: Value| {
            let mut p = json!({ "cabinet": cab.id, "row": row });
            if let Some(k) = col {
                p["column"] = json!(k);
            }
            if let (Some(o), Value::Object(e)) = (p.as_object_mut(), extra) {
                o.extend(e);
            }
            p
        };
        let (w, h) = size.unwrap_or((0.0, 0.0));
        // Rozmery: zadaná hodnota zamkne rozmer (Lock Width/Height)
        for (label, key, value, enabled) in [
            (
                tr!("side_panel.opening.width"),
                "width",
                f.width.unwrap_or(w),
                col.is_some(),
            ),
            (
                tr!("side_panel.opening.height"),
                "height",
                r.height.unwrap_or(h),
                col.is_none(),
            ),
        ] {
            let k = format!("opening:{}:{row}:{col:?}:{key}", cab.id);
            let editing = matches!(&self.text_edit, Some((x, _)) if *x == k);
            prop_row(ui, &label, editing, |ui| {
                if enabled {
                    if let Some(t) = self.buffered_text(ui, &k, &fmt(value)) {
                        if let Some(v) = parse_relative(&t, value) {
                            cmds.push(("front.set", target(json!({ key: v }))));
                        }
                    }
                } else {
                    ui.label(fmt(value));
                }
            });
        }
        for (label, locked, enabled) in [
            (
                tr!("side_panel.opening.lock_width"),
                f.width.is_some() && col.is_some(),
                col.is_some(),
            ),
            (
                tr!("side_panel.opening.lock_height"),
                r.height.is_some(),
                col.is_none(),
            ),
        ] {
            prop_row(ui, &label, false, |ui| {
                let mut on = locked;
                if ui
                    .add_enabled(enabled, egui::Checkbox::new(&mut on, ""))
                    .changed()
                {
                    cmds.push(("front.set", target(json!({ "lock": on }))));
                }
            });
        }
        // Typ (prepínače ako v CV)
        let mut kinds = vec![(FrontKind::Door, tr!("side_panel.opening.kind_door"))];
        if col.is_none() {
            kinds.push((FrontKind::Pair, tr!("side_panel.opening.kind_pair")));
        }
        kinds.extend([
            (FrontKind::Drawer, tr!("side_panel.opening.kind_drawer")),
            (
                FrontKind::FalseFront,
                tr!("side_panel.opening.kind_false_front"),
            ),
            (FrontKind::Open, tr!("side_panel.opening.kind_open")),
            (FrontKind::Panel, tr!("side_panel.opening.kind_panel")),
        ]);
        for (kind, label) in kinds {
            prop_row(ui, &label, false, |ui| {
                if ui.radio(f.kind == kind, "").clicked() && f.kind != kind {
                    cmds.push(("front.set", target(json!({ "kind": kind }))));
                }
            });
        }
        if f.kind == FrontKind::Door {
            prop_row(ui, &tr!("side_panel.opening.hinge"), false, |ui| {
                let right = f.hinge_side == Some(HingeSide::Right);
                for (side, label, on) in [
                    ("left", tr!("side_panel.opening.hinge_left"), !right),
                    ("right", tr!("side_panel.opening.hinge_right"), right),
                ] {
                    if ui.selectable_label(on, label).clicked() && !on {
                        cmds.push(("front.set", target(json!({ "hinge_side": side }))));
                    }
                }
            });
        }
        // Úpravy rozmeru (Size Adjustments)
        let a = f.adjust.clone();
        for (label, key, value) in [
            (tr!("side_panel.opening.adjust_left"), "left", a.left),
            (tr!("side_panel.opening.adjust_right"), "right", a.right),
            (tr!("side_panel.opening.adjust_top"), "top", a.top),
            (tr!("side_panel.opening.adjust_bottom"), "bottom", a.bottom),
        ] {
            let k = format!("opening:{}:{row}:{col:?}:adj:{key}", cab.id);
            let editing = matches!(&self.text_edit, Some((x, _)) if *x == k);
            prop_row(ui, &label, editing, |ui| {
                if let Some(t) = self.buffered_text(ui, &k, &fmt(value)) {
                    if let Some(v) = t
                        .trim()
                        .replace(',', ".")
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite())
                    {
                        let mut adj = a.clone();
                        match key {
                            "left" => adj.left = v,
                            "right" => adj.right = v,
                            "top" => adj.top = v,
                            _ => adj.bottom = v,
                        }
                        cmds.push(("front.set", target(json!({ "adjust": adj }))));
                    }
                }
            });
        }
    }

    fn cabinet_props(
        &mut self,
        ui: &mut egui::Ui,
        original: &CabinetSpec,
        project: &cc_model::ProjectFile,
        cmds: &mut Vec<(&'static str, Value)>,
    ) {
        let id = original.id.clone();
        let wall_ids: Vec<String> = project.walls.iter().map(|w| w.id.clone()).collect();
        let room_spec = project.room.clone().unwrap_or_default();
        let mut c = original.clone();
        let editing_id =
            matches!(&self.text_edit, Some((k, _)) if *k == format!("cabinet.id:{id}"));
        prop_row(ui, &tr!("side_panel.cabinet.id"), editing_id, |ui| {
            if let Some(new_id) = self.buffered_text(ui, &format!("cabinet.id:{id}"), &id) {
                cmds.push(("cabinet.set", json!({ "id": id, "set": { "id": new_id } })));
            }
        });
        prop_row(ui, &tr!("side_panel.cabinet.type"), false, |ui| {
            egui::ComboBox::from_id_salt("cabinet_type")
                .selected_text(type_name(c.cabinet_type))
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    for t in [CabinetType::Base, CabinetType::Wall, CabinetType::Tall] {
                        ui.selectable_value(&mut c.cabinet_type, t, type_name(t));
                    }
                });
        });
        // umiestnenie v miestnosti (kap. 13)
        prop_row(ui, &tr!("side_panel.cabinet.wall"), false, |ui| {
            egui::ComboBox::from_id_salt("cabinet_wall")
                .selected_text(
                    c.wall
                        .clone()
                        .unwrap_or_else(|| tr!("side_panel.cabinet.no_wall")),
                )
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.wall, None, tr!("side_panel.cabinet.no_wall"));
                    for w in &wall_ids {
                        ui.selectable_value(&mut c.wall, Some(w.clone()), w);
                    }
                });
        });
        if c.wall.is_some() {
            // polohy ako v CV (findings 06 §4.2): každé pole posunie skrinku; „+50“ / „-50“ = o koľko
            let index = project.cabinets.iter().position(|x| x.id == id);
            let limits = index.and_then(|i| cc_room::row_limits(project, i));
            let (x, w) = (c.at, c.w);
            // (stály kľúč poľa, meno, hodnota, nápoveda, prepočet na polohu)
            type Field = (&'static str, String, f64, String, Box<dyn Fn(f64) -> f64>);
            let mut fields: Vec<Field> = Vec::new();
            if let Some((left, right, len)) = limits {
                fields.push((
                    "clearance_left",
                    tr!("side_panel.cabinet.clearance_left"),
                    x - left,
                    tr!("side_panel.cabinet.clearance_left_tip"),
                    Box::new(move |v| left + v),
                ));
                fields.push((
                    "clearance_right",
                    tr!("side_panel.cabinet.clearance_right"),
                    right - (x + w),
                    tr!("side_panel.cabinet.clearance_right_tip"),
                    Box::new(move |v| right - v - w),
                ));
                fields.push((
                    "end_left",
                    tr!("side_panel.cabinet.end_left"),
                    x,
                    tr!("side_panel.cabinet.end_left_tip"),
                    Box::new(|v| v),
                ));
                fields.push((
                    "end_right",
                    tr!("side_panel.cabinet.end_right"),
                    len - (x + w),
                    tr!("side_panel.cabinet.end_right_tip"),
                    Box::new(move |v| len - v - w),
                ));
                fields.push((
                    "center_left",
                    tr!("side_panel.cabinet.center_left"),
                    x + w / 2.0,
                    tr!("side_panel.cabinet.center_left_tip"),
                    Box::new(move |v| v - w / 2.0),
                ));
                fields.push((
                    "center_right",
                    tr!("side_panel.cabinet.center_right"),
                    len - (x + w / 2.0),
                    tr!("side_panel.cabinet.center_right_tip"),
                    Box::new(move |v| len - v - w / 2.0),
                ));
            } else {
                fields.push((
                    "end_left",
                    tr!("side_panel.cabinet.end_left"),
                    x,
                    tr!("side_panel.cabinet.end_left_tip"),
                    Box::new(|v| v),
                ));
            }
            for (field, label, value, hint, to_at) in fields {
                let key = format!("cabinet.pos:{id}:{field}");
                let editing = matches!(&self.text_edit, Some((k, _)) if *k == key);
                prop_row(ui, &label, editing, |ui| {
                    if let Some(t) = self.buffered_text(ui, &key, &fmt(value)) {
                        if let Some(v) = parse_relative(&t, value) {
                            let at = to_at(v);
                            if at.is_finite() {
                                c.at = (at * 10.0).round() / 10.0;
                            }
                        }
                    }
                    ui.response().on_hover_text(hint);
                });
            }
        }
        for (label, v) in [
            (tr!("side_panel.cabinet.width"), &mut c.w),
            (tr!("side_panel.cabinet.height"), &mut c.h),
            (tr!("side_panel.cabinet.depth"), &mut c.d),
        ] {
            prop_row(ui, &label, false, |ui| {
                ui.add(
                    egui::DragValue::new(v)
                        .speed(1.0)
                        .range(50.0..=3000.0)
                        .suffix(" mm"),
                );
            });
        }
        prop_row(ui, &tr!("side_panel.cabinet.lock_width"), false, |ui| {
            ui.checkbox(&mut c.lock_width, "")
                .on_hover_text(tr!("side_panel.cabinet.lock_width_tip"));
        });
        // Poloha (ako Position v CV): výška nad podlahou, odsadenie od steny
        if c.wall.is_some() {
            // Umiestnenie ako Placement v CV: pravidlo voči miestnosti, alebo voľne (výška)
            let levels = cc_model::PlacementLevels::of(project);
            prop_row(ui, &tr!("side_panel.cabinet.placement"), false, |ui| {
                use cc_model::Placement as P;
                let current = c
                    .elevation
                    .map_or(c.placement.map(placement_key), |_| Some("float"));
                let label = |k: Option<&str>| match k {
                    Some(key) if PLACEMENTS.contains(&key) => placement_label(key),
                    _ => tr!("side_panel.cabinet.by_type"),
                };
                egui::ComboBox::from_id_salt("cabinet_placement")
                    .selected_text(label(current))
                    .width(ui.available_width() - 4.0)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), tr!("side_panel.cabinet.by_type"))
                            .clicked()
                        {
                            c.placement = None;
                            c.elevation = None;
                        }
                        for key in PLACEMENTS {
                            if ui
                                .selectable_label(current == Some(key), placement_label(key))
                                .clicked()
                            {
                                let here = c.elevation_in(&room_spec, &levels);
                                c.elevation = None;
                                c.placement = match key {
                                    "floor" => Some(P::Floor),
                                    "toe_height" => Some(P::ToeHeight),
                                    "under_counter" => Some(P::UnderCounter),
                                    "counter_top" => Some(P::CounterTop),
                                    "under_uppers" => Some(P::UnderUppers),
                                    "under_soffit" => Some(P::UnderSoffit),
                                    "ceiling" => Some(P::Ceiling),
                                    "above_soffit" => Some(P::AboveSoffit),
                                    "above_elevation" => Some(P::AboveElevation(here)),
                                    "below_elevation" => Some(P::BelowElevation(here + c.h)),
                                    _ => {
                                        c.elevation = Some(here);
                                        c.placement
                                    }
                                };
                            }
                        }
                    });
            });
            // výška: pri „spodok/vrch vo výške“ sa mení hodnota pravidla, inak sa skrinka uvoľní
            prop_row(ui, &tr!("side_panel.cabinet.elevation"), false, |ui| {
                let mut e = c.elevation_in(&room_spec, &levels);
                let r = ui.add(
                    egui::DragValue::new(&mut e)
                        .speed(1.0)
                        .range(0.0..=5000.0)
                        .suffix(" mm"),
                );
                if r.changed() {
                    match &mut c.placement {
                        Some(cc_model::Placement::AboveElevation(elevation))
                            if c.elevation.is_none() =>
                        {
                            *elevation = e;
                        }
                        Some(cc_model::Placement::BelowElevation(elevation))
                            if c.elevation.is_none() =>
                        {
                            *elevation = e + c.h;
                        }
                        _ => c.elevation = Some(e),
                    }
                }
                r.on_hover_text(tr!("side_panel.cabinet.elevation_tip"));
            });
            prop_row(ui, &tr!("side_panel.cabinet.outset"), false, |ui| {
                ui.add(
                    egui::DragValue::new(&mut c.outset)
                        .speed(1.0)
                        .range(0.0..=2000.0)
                        .suffix(" mm"),
                );
            });
        }
        let shelf_opening = c.has_shelf_opening();
        prop_row(ui, &tr!("side_panel.cabinet.shelves"), false, |ui| {
            let r = ui.add_enabled(
                shelf_opening,
                egui::DragValue::new(&mut c.adjustable_shelves).range(0..=12),
            );
            if shelf_opening {
                r.on_hover_text(tr!("side_panel.cabinet.shelves_tip"));
            } else {
                r.on_disabled_hover_text(tr!("side_panel.cabinet.shelves_disabled_tip"));
            }
        });
        self.front_props(ui, &mut c);
        if c.cabinet_type == CabinetType::Base {
            let std_top = self
                .session
                .doc()
                .map_or(cc_model::TopStyle::Stretchers, |d| {
                    d.standard.carcass.top_style
                });
            opt_combo(
                ui,
                &tr!("side_panel.cabinet.top"),
                "opt_top",
                &mut c.top_style,
                std_top,
                &[
                    (
                        cc_model::TopStyle::Stretchers,
                        tr!("side_panel.cabinet.top_stretchers"),
                    ),
                    (
                        cc_model::TopStyle::Panel,
                        tr!("side_panel.cabinet.top_panel"),
                    ),
                    (cc_model::TopStyle::Sink, tr!("side_panel.cabinet.top_sink")),
                ],
            );
        }
        prop_row(ui, &tr!("side_panel.cabinet.back"), false, |ui| {
            ui.checkbox(&mut c.back, "")
                .on_hover_text(tr!("side_panel.cabinet.back_tip"));
        });
        prop_row(ui, &tr!("side_panel.cabinet.carcass"), false, |ui| {
            ui.checkbox(&mut c.carcass, "")
                .on_hover_text(tr!("side_panel.cabinet.carcass_tip"));
        });
        self.attribute_props(ui, &mut c);
        if c.cabinet_type == CabinetType::Tall {
            let mut remove = None;
            let count = c.fixed_shelves.len();
            for (k, z) in c.fixed_shelves.iter_mut().enumerate() {
                let label = tr!("side_panel.cabinet.fixed_shelf", n = k + 1);
                prop_row(ui, &label, false, |ui| {
                    ui.add(
                        egui::DragValue::new(z)
                            .speed(1.0)
                            .range(50.0..=3000.0)
                            .suffix(" mm"),
                    )
                    .on_hover_text(tr!("side_panel.cabinet.fixed_shelf_tip"));
                    if ui.small_button("✖").clicked() {
                        remove = Some(k);
                    }
                });
            }
            if let Some(k) = remove {
                c.fixed_shelves.remove(k);
            }
            prop_row(ui, "", false, |ui| {
                if ui
                    .small_button(if count == 0 {
                        tr!("side_panel.cabinet.add_fixed_shelf")
                    } else {
                        tr!("side_panel.cabinet.add_another_shelf")
                    })
                    .clicked()
                {
                    let z = c
                        .fixed_shelves
                        .last()
                        .map_or(c.h / 2.0, |z| (z + 300.0).min(c.h - 100.0));
                    c.fixed_shelves.push(z.round());
                }
            });
        } else {
            c.fixed_shelves.clear();
        }
        if let Some(set) = cabinet_diff(original, &c) {
            cmds.push((
                "cabinet.set",
                json!({ "id": id, "set": set, "coalesce": true }),
            ));
        }
    }

    /// Čelá skrinky: rozloženie (predvoľby), montáž a pánt z knižnice štandardu.
    fn front_props(&mut self, ui: &mut egui::Ui, c: &mut CabinetSpec) {
        let (hinges, std_mount, std_hinge, drawers, std_drawer) = match self.session.doc() {
            Some(d) => (
                d.standard
                    .hinges
                    .iter()
                    .map(|h| (h.id.clone(), h.name.clone()))
                    .collect::<Vec<_>>(),
                d.standard.fronts.mount,
                d.standard.fronts.hinge.clone(),
                d.standard
                    .drawers
                    .iter()
                    .map(|h| (h.id.clone(), h.name.clone()))
                    .collect::<Vec<_>>(),
                d.standard.fronts.drawer.clone(),
            ),
            None => (Vec::new(), FrontMount::Overlay, None, Vec::new(), None),
        };
        prop_row(ui, &tr!("side_panel.fronts.fronts"), false, |ui| {
            egui::ComboBox::from_id_salt("cabinet_fronts")
                .selected_text(fronts_text(&c.fronts))
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    for (label, preset) in front_presets() {
                        if ui.selectable_label(c.fronts == preset, label).clicked() {
                            c.fronts = preset;
                        }
                    }
                });
        });
        if c.fronts.is_empty() {
            return;
        }
        // jedny dvierka: strana pántov
        let doors: Vec<usize> = c
            .fronts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == FrontKind::Door)
            .map(|(i, _)| i)
            .collect();
        if !doors.is_empty() {
            prop_row(ui, &tr!("side_panel.fronts.hinges"), false, |ui| {
                let current = c
                    .fronts
                    .get(doors[0])
                    .and_then(|f| f.hinge_side)
                    .unwrap_or_default();
                for (side, label) in [
                    (HingeSide::Left, tr!("side_panel.fronts.hinges_left")),
                    (HingeSide::Right, tr!("side_panel.fronts.hinges_right")),
                ] {
                    if ui.selectable_label(current == side, label).clicked() {
                        for &i in &doors {
                            if let Some(f) = c.fronts.get_mut(i) {
                                f.hinge_side = (side == HingeSide::Right).then_some(side);
                            }
                        }
                    }
                }
            });
        }
        // škáry čiel (ako Reveals v CV): auto = podľa štandardu
        if let Some(d) = self.session.doc() {
            let fs = &d.standard.fronts;
            let rows = fs.rows(c.cabinet_type);
            let defaults = [
                (tr!("side_panel.fronts.reveal_top"), rows.top),
                (tr!("side_panel.fronts.reveal_bottom"), rows.bottom),
                (tr!("side_panel.fronts.reveal_left"), fs.reveal_side),
                (tr!("side_panel.fronts.reveal_right"), fs.reveal_side),
                (tr!("side_panel.fronts.gap_rows"), rows.gap),
                (tr!("side_panel.fronts.gap_columns"), fs.pair_gap),
            ];
            let r = &mut c.reveals;
            let slots = [
                &mut r.top,
                &mut r.bottom,
                &mut r.left,
                &mut r.right,
                &mut r.horizontal,
                &mut r.vertical,
            ];
            for ((label, std), slot) in defaults.into_iter().zip(slots) {
                opt_mm(ui, &label, slot, std);
            }
        }
        // úprava rozmeru jednotlivého čela pri hranách (ako Size Adjustments v CV)
        for (i, f) in c.fronts.iter_mut().enumerate() {
            let a = &mut f.adjust;
            let changed = !a.is_empty();
            let label = tr!("side_panel.fronts.adjust", n = i + 1);
            prop_row(ui, &label, changed, |ui| {
                for (slot, tip) in [
                    (&mut a.left, tr!("side_panel.fronts.adjust_left_tip")),
                    (&mut a.right, tr!("side_panel.fronts.adjust_right_tip")),
                    (&mut a.top, tr!("side_panel.fronts.adjust_top_tip")),
                    (&mut a.bottom, tr!("side_panel.fronts.adjust_bottom_tip")),
                ] {
                    ui.add(egui::DragValue::new(slot).speed(0.5).range(-300.0..=300.0))
                        .on_hover_text(tip);
                }
            });
        }
        let mount_name = |m: FrontMount| match m {
            FrontMount::Overlay => tr!("side_panel.fronts.mount_overlay"),
            FrontMount::Inset => tr!("side_panel.fronts.mount_inset"),
        };
        let std_mount_text = standard_text(&mount_name(std_mount));
        prop_row(ui, &tr!("side_panel.fronts.mount"), false, |ui| {
            egui::ComboBox::from_id_salt("cabinet_mount")
                .selected_text(match c.front_mount {
                    Some(m) => mount_name(m),
                    None => std_mount_text.clone(),
                })
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut c.front_mount, None, std_mount_text.clone());
                    for m in [FrontMount::Overlay, FrontMount::Inset] {
                        ui.selectable_value(&mut c.front_mount, Some(m), mount_name(m));
                    }
                });
        });
        if c.fronts
            .iter()
            .any(|f| matches!(f.kind, FrontKind::Door | FrontKind::Pair))
        {
            let name_of = |id: &str| {
                hinges
                    .iter()
                    .find(|(h, _)| h == id)
                    .map_or_else(|| id.to_owned(), |(_, n)| n.clone())
            };
            let std_text = match &std_hinge {
                Some(id) => standard_text(&name_of(id)),
                None => tr!("side_panel.general.standard"),
            };
            prop_row(ui, &tr!("side_panel.fronts.hinge"), false, |ui| {
                egui::ComboBox::from_id_salt("cabinet_hinge")
                    .selected_text(match &c.hinge {
                        Some(id) => name_of(id),
                        None => std_text.clone(),
                    })
                    .width(ui.available_width() - 4.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut c.hinge, None, std_text.clone());
                        for (id, name) in &hinges {
                            ui.selectable_value(&mut c.hinge, Some(id.clone()), name);
                        }
                    });
            });
        }
        if c.fronts.iter().any(|f| f.kind == FrontKind::Drawer) && !drawers.is_empty() {
            let name_of = |id: &str| {
                drawers
                    .iter()
                    .find(|(h, _)| h == id)
                    .map_or_else(|| id.to_owned(), |(_, n)| n.clone())
            };
            let std_text = match &std_drawer {
                Some(id) => standard_text(&name_of(id)),
                None => tr!("side_panel.general.standard"),
            };
            prop_row(ui, &tr!("side_panel.fronts.drawers"), false, |ui| {
                egui::ComboBox::from_id_salt("cabinet_drawer")
                    .selected_text(match &c.drawer {
                        Some(id) => name_of(id),
                        None => std_text.clone(),
                    })
                    .width(ui.available_width() - 4.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut c.drawer, None, std_text.clone());
                        for (id, name) in &drawers {
                            ui.selectable_value(&mut c.drawer, Some(id.clone()), name);
                        }
                    });
            });
        }
    }

    /// Atribúty skrinky ako v CV: spoje, typ spojov, policové kovanie, čočky, nožičky, TIP-ON.
    /// Prvá voľba „štandard (…)“ = hodnota z `[cabinet_options]` štandardu.
    fn attribute_props(&mut self, ui: &mut egui::Ui, c: &mut CabinetSpec) {
        let Some(d) = self.session.doc().map(|d| d.standard.cabinet_options) else {
            return;
        };
        let o = &mut c.options;
        ui.label(
            RichText::new(tr!("side_panel.attributes.title"))
                .size(theme::SMALL)
                .color(theme::TEXT_WEAK),
        );
        let yes_no = [
            (true, tr!("side_panel.general.yes")),
            (false, tr!("side_panel.general.no")),
        ];
        opt_combo(
            ui,
            &tr!("side_panel.attributes.joints"),
            "opt_joints",
            &mut o.joints,
            d.joints,
            &yes_no,
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.joint_type"),
            "opt_joint_type",
            &mut o.joint_type,
            d.joint_type,
            &[
                (JointType::Dowels, tr!("side_panel.attributes.dowels")),
                (JointType::Confirmat, tr!("side_panel.attributes.confirmat")),
                (JointType::Screw, tr!("side_panel.attributes.screw")),
            ],
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.shelf_support"),
            "opt_shelf",
            &mut o.shelf_support,
            d.shelf_support,
            &[
                // obchodný názov kovania sa neprekladá
                (ShelfSupport::Sekura7, "Sekura 7".to_owned()),
                (ShelfSupport::None, tr!("side_panel.attributes.shelf_none")),
            ],
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.bumpers"),
            "opt_bumpers",
            &mut o.bumpers,
            d.bumpers,
            &yes_no,
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.legs"),
            "opt_legs",
            &mut o.legs,
            d.legs,
            &yes_no,
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.plinth"),
            "opt_plinth",
            &mut o.plinth,
            d.plinth,
            &yes_no,
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.link_left"),
            "opt_link_left",
            &mut o.link_left,
            d.link_left,
            &yes_no,
        );
        opt_combo(
            ui,
            &tr!("side_panel.attributes.tip_on"),
            "opt_tip_on",
            &mut o.tip_on,
            d.tip_on,
            &[
                (TipOn::Off, tr!("side_panel.attributes.tip_on_off")),
                (TipOn::Drilled, tr!("side_panel.attributes.tip_on_drilled")),
                (TipOn::Adapter, tr!("side_panel.attributes.tip_on_adapter")),
            ],
        );
        if o.tip_on.unwrap_or(d.tip_on) != TipOn::Off {
            opt_combo(
                ui,
                &tr!("side_panel.attributes.tip_on_color"),
                "opt_tip_color",
                &mut o.tip_on_color,
                d.tip_on_color,
                &[
                    (TipOnColor::White, tr!("side_panel.attributes.white")),
                    (TipOnColor::Grey, tr!("side_panel.attributes.grey")),
                    (TipOnColor::Black, tr!("side_panel.attributes.black")),
                ],
            );
            opt_combo(
                ui,
                &tr!("side_panel.attributes.tip_on_kind"),
                "opt_tip_kind",
                &mut o.tip_on_kind,
                d.tip_on_kind,
                &[
                    (TipOnKind::Stop, tr!("side_panel.attributes.tip_on_stop")),
                    (
                        TipOnKind::Magnet,
                        tr!("side_panel.attributes.tip_on_magnet"),
                    ),
                ],
            );
        }
    }

    fn machine_section(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let machine = doc.machine.clone();
        let machine_path = doc.machine_path.clone();
        if self.section_id(ui, "machine", &tr!("side_panel.machine.title")) {
            match &machine {
                Some(m) => {
                    prop_row(ui, &tr!("side_panel.machine.machine"), false, |ui| {
                        ui.label(&m.machine.name);
                    });
                    prop_row(ui, &tr!("side_panel.machine.drills"), false, |ui| {
                        ui.label(
                            m.drills
                                .diameters
                                .iter()
                                .map(|d| format!("Ø{}", fmt(*d)))
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                    });
                    if let Some(x) = &m.xcs {
                        prop_row(ui, &tr!("side_panel.machine.saw"), false, |ui| {
                            let (kerf, step) = (fmt(x.slot.saw_kerf), fmt(x.slot.max_step));
                            let text =
                                tr!("side_panel.machine.saw_value", kerf = kerf, step = step);
                            ui.label(text);
                        });
                    }
                    prop_row(ui, &tr!("side_panel.machine.work_area"), false, |ui| {
                        ui.label(format!(
                            "{} × {} mm",
                            fmt(m.work_area.max_length),
                            fmt(m.work_area.max_width)
                        ));
                    });
                    prop_row(ui, &tr!("side_panel.machine.profile"), false, |ui| {
                        let text = machine_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .map(|f| f.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        ui.label(text);
                    });
                }
                None => {
                    ui.label(tr!("side_panel.machine.no_profile"));
                }
            }
            ui.add_space(6.0);
        }
        if self.section_id(ui, "parts_list", &tr!("side_panel.parts_list.title")) {
            prop_row(ui, &tr!("side_panel.parts_list.merge"), false, |ui| {
                ui.checkbox(&mut self.csv.merge, "");
            });
            prop_row(
                ui,
                &tr!("side_panel.parts_list.decimal_comma"),
                false,
                |ui| {
                    ui.checkbox(&mut self.csv.decimal_comma, "");
                },
            );
            prop_row(ui, &tr!("side_panel.parts_list.folder"), false, |ui| {
                let dir = self
                    .default_export_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default();
                ui.label(RichText::new(dir).size(theme::SMALL));
            });
            ui.add_space(6.0);
        }
    }

    /// Výkresy: zoznam výkresov (zákazka, skrinky) a dielov na výber.
    fn parts_picker(&mut self, ui: &mut egui::Ui) {
        use crate::drawings::Sheet;
        let Some(built) = self.model_built() else {
            return;
        };
        let sheet = self.drawing.sheet.clone();
        let current = self.selected_part();
        let mut pick_sheet = None;
        let mut pick_part = None;
        if self.section_id(ui, "drawings", &tr!("side_panel.drawings.title")) {
            egui::Frame::new()
                .fill(theme::SHEET)
                .stroke(Stroke::new(1.0_f32, theme::BORDER))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    table_header(ui, &[(tr!("side_panel.drawings.col_sheet").as_str(), 0.0)]);
                    egui::ScrollArea::vertical()
                        .id_salt("sheets_picker")
                        .max_height(200.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let job = [tr!("side_panel.drawings.job")];
                            if table_row(ui, &job, &[0.0], sheet == Sheet::Job).clicked() {
                                pick_sheet = Some(Sheet::Job);
                            }
                            for c in &built.model.cabinets {
                                let s = Sheet::Cabinet(c.id.clone());
                                let text = [tr!("side_panel.drawings.cabinet", id = &c.id)];
                                if table_row(ui, &text, &[0.0], sheet == s).clicked() {
                                    pick_sheet = Some(s);
                                }
                            }
                        });
                });
            ui.add_space(6.0);
        }
        if self.section_id(ui, "parts", &tr!("side_panel.drawings.parts")) {
            egui::Frame::new()
                .fill(theme::SHEET)
                .stroke(Stroke::new(1.0_f32, theme::BORDER))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    table_header(
                        ui,
                        &[
                            (tr!("side_panel.drawings.col_part").as_str(), 120.0),
                            (tr!("side_panel.drawings.col_operations").as_str(), 0.0),
                        ],
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("parts_picker")
                        .max_height(ui.available_height().max(200.0) - 40.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for p in built.model.parts() {
                                let sel = sheet == Sheet::Part
                                    && current.as_deref() == Some(p.id.as_str());
                                let cells = [p.id.clone(), p.operations.len().to_string()];
                                if table_row(ui, &cells, &[120.0, 0.0], sel).clicked() {
                                    pick_part = Some(p.id.clone());
                                }
                            }
                        });
                });
        }
        if let Some(s) = pick_sheet {
            self.drawing.sheet = s;
        }
        if let Some(id) = pick_part {
            self.drawing.sheet = Sheet::Part;
            self.select_part(Some(id));
        }
    }

    /// Ako [Self::buffered_text], ale prázdny text je platná hodnota (zmaže pole).
    pub(crate) fn buffered_field(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        current: &str,
    ) -> Option<String> {
        let mut text = match &self.text_edit {
            Some((k, t)) if k == key => t.clone(),
            _ => current.to_owned(),
        };
        let r =
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(ui.available_width() - 4.0));
        if r.changed() || r.has_focus() {
            self.text_edit = Some((key.to_owned(), text.clone()));
        }
        if r.lost_focus() {
            if matches!(&self.text_edit, Some((k, _)) if k == key) {
                self.text_edit = None;
            }
            if text.trim() != current {
                return Some(text.trim().to_owned());
            }
        }
        None
    }

    /// Textové pole, ktoré potvrdí hodnotu až pri opustení (Enter, klik inam).
    /// Vráti novú neprázdnu hodnotu, ak sa zmenila.
    pub(crate) fn buffered_text(
        &mut self,
        ui: &mut egui::Ui,
        key: &str,
        current: &str,
    ) -> Option<String> {
        let mut text = match &self.text_edit {
            Some((k, t)) if k == key => t.clone(),
            _ => current.to_owned(),
        };
        let r =
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(ui.available_width() - 4.0));
        if r.changed() || r.has_focus() {
            self.text_edit = Some((key.to_owned(), text.clone()));
        }
        if r.lost_focus() {
            if matches!(&self.text_edit, Some((k, _)) if k == key) {
                self.text_edit = None;
            }
            let t = text.trim();
            if !t.is_empty() && t != current {
                return Some(t.to_owned());
            }
        }
        None
    }
}

/// Riadok úlohy: ikona 32 bodov a text.
pub(crate) fn task_row(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    active: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), sense);
    if active {
        ui.painter().rect_filled(rect, 2.0, theme::ACTIVE_BG);
    } else if enabled && response.hovered() {
        ui.painter().rect_filled(rect, 2.0, theme::HOVER_BG);
        ui.painter().rect_stroke(
            rect,
            2.0,
            Stroke::new(1.0_f32, theme::HOVER_STROKE),
            egui::StrokeKind::Inside,
        );
    }
    icons::paint(
        ui.painter(),
        Rect::from_min_size(rect.min + vec2(8.0, 6.0), vec2(32.0, 32.0)),
        icon,
        enabled,
    );
    ui.painter().text(
        pos2(rect.min.x + 50.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.5),
        if enabled {
            theme::TEXT
        } else {
            theme::TEXT_WEAK
        },
    );
    response
}

/// Riadok vlastnosti: meno v sivom stĺpci, hodnota vpravo, čiara pod riadkom.
pub(crate) fn prop_row(
    ui: &mut egui::Ui,
    label: &str,
    highlight: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    let width = ui.available_width();
    let top = ui.cursor().min;
    let label_rect = Rect::from_min_size(top, vec2(LABEL_W, ROW_H));
    let r = ui
        .allocate_ui_with_layout(
            vec2(width, ROW_H),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_height(ROW_H);
                ui.add_space(LABEL_W + 4.0);
                let value_w = (width - LABEL_W - 6.0).max(40.0);
                ui.allocate_ui_with_layout(
                    vec2(value_w, ROW_H),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_max_width(value_w);
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        add(ui);
                    },
                );
            },
        )
        .response;
    let row = Rect::from_min_max(top, pos2(top.x + width, r.rect.max.y.max(top.y + ROW_H)));
    let painter = ui.painter();
    let fill = if highlight {
        theme::EDIT_ROW
    } else {
        theme::LABEL_BG
    };
    painter.rect_filled(
        Rect::from_min_max(label_rect.min, pos2(label_rect.max.x, row.max.y)),
        0.0,
        fill,
    );
    painter.text(
        pos2(top.x + 4.0, (top.y + row.max.y) / 2.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(theme::SMALL + 0.5),
        theme::TEXT,
    );
    painter.line_segment(
        [pos2(row.min.x, row.max.y), pos2(row.max.x, row.max.y)],
        Stroke::new(1.0_f32, theme::GRID_LINE),
    );
    painter.line_segment(
        [
            pos2(label_rect.max.x, row.min.y),
            pos2(label_rect.max.x, row.max.y),
        ],
        Stroke::new(1.0_f32, theme::GRID_LINE),
    );
    ui.add_space(1.0);
}

/// Hlavička malej tabuľky; šírka 0 = zvyšok.
pub(crate) fn table_header(ui: &mut egui::Ui, cols: &[(&str, f32)]) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme::HEADER_BG);
    let mut x = rect.min.x;
    for (title, w) in cols {
        ui.painter().text(
            pos2(x + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            *title,
            FontId::proportional(theme::SMALL + 0.5),
            theme::TEXT,
        );
        if *w > 0.0 {
            x += w;
            ui.painter().line_segment(
                [pos2(x, rect.min.y), pos2(x, rect.max.y)],
                Stroke::new(1.0_f32, theme::BORDER),
            );
        }
    }
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
        Stroke::new(1.0_f32, theme::BORDER),
    );
}

/// Riadok malej tabuľky; šírka 0 = zvyšok.
pub(crate) fn table_row(
    ui: &mut egui::Ui,
    cells: &[String],
    widths: &[f32],
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 0.0, theme::SELECT_BG);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, theme::HOVER_BG);
    }
    let mut x = rect.min.x;
    for (cell, w) in cells.iter().zip(widths) {
        ui.painter().text(
            pos2(x + 4.0, rect.center().y),
            Align2::LEFT_CENTER,
            cell,
            FontId::proportional(theme::BODY - 0.5),
            theme::TEXT,
        );
        x += w;
    }
    ui.painter().line_segment(
        [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
        Stroke::new(1.0_f32, theme::GRID_LINE),
    );
    response
}

/// Polia skrinky, ktoré sa zmenili (bez `id`); `null` = pole odstrániť.
fn cabinet_diff(old: &CabinetSpec, new: &CabinetSpec) -> Option<Map<String, Value>> {
    if old == new {
        return None;
    }
    let (Ok(Value::Object(a)), Ok(Value::Object(b))) =
        (serde_json::to_value(old), serde_json::to_value(new))
    else {
        return None;
    };
    let mut set = Map::new();
    for (k, v) in &b {
        if k != "id" && a.get(k) != Some(v) {
            set.insert(k.clone(), v.clone());
        }
    }
    for k in a.keys() {
        if !b.contains_key(k) {
            set.insert(k.clone(), Value::Null);
        }
    }
    (!set.is_empty()).then_some(set)
}

/// Rozmer s voľbou „auto“ (= `None`, hodnota zo štandardu).
fn opt_mm(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>, standard: f64) {
    prop_row(ui, label, false, |ui| {
        let mut auto = value.is_none();
        if ui
            .checkbox(&mut auto, tr!("side_panel.general.auto"))
            .on_hover_text(tr!("side_panel.general.auto_tip", value = fmt(standard)))
            .changed()
        {
            *value = if auto { None } else { Some(standard) };
        }
        let mut v = value.unwrap_or(standard);
        if ui
            .add_enabled(
                !auto,
                egui::DragValue::new(&mut v)
                    .speed(0.5)
                    .range(-100.0..=200.0)
                    .suffix(" mm"),
            )
            .changed()
        {
            *value = Some(v);
        }
    });
}

/// Voľba „štandard (hodnota)“ v ponukách.
fn standard_text(value: &str) -> String {
    tr!("side_panel.general.standard_value", value = value)
}

/// Výber atribútu: „štandard (…)“ (= `None`) alebo vlastná hodnota.
fn opt_combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: &mut Option<T>,
    standard: T,
    choices: &[(T, String)],
) {
    let name = |v: T| {
        choices
            .iter()
            .find(|(c, _)| *c == v)
            .map_or("?", |(_, n)| n.as_str())
    };
    let std_text = standard_text(name(standard));
    prop_row(ui, label, false, |ui| {
        egui::ComboBox::from_id_salt(id)
            .selected_text(match value {
                Some(v) => name(*v).to_owned(),
                None => std_text.clone(),
            })
            .width(ui.available_width() - 4.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(value, None, std_text.clone());
                for (v, n) in choices {
                    ui.selectable_value(value, Some(*v), n.as_str());
                }
            });
    });
}

/// Predvoľby rozloženia čiel (zhora nadol): zobrazený názov a čelá.
pub(crate) fn front_presets() -> Vec<(String, Vec<FrontSpec>)> {
    let f = FrontSpec::new;
    let sized = |kind, h: f64| FrontSpec {
        height: Some(h),
        ..FrontSpec::new(kind)
    };
    let right = FrontSpec {
        hinge_side: Some(HingeSide::Right),
        ..FrontSpec::new(FrontKind::Door)
    };
    vec![
        (tr!("side_panel.front_preset.none"), Vec::new()),
        (
            tr!("side_panel.front_preset.door"),
            vec![f(FrontKind::Door)],
        ),
        (tr!("side_panel.front_preset.door_right"), vec![right]),
        (
            tr!("side_panel.front_preset.pair"),
            vec![f(FrontKind::Pair)],
        ),
        (
            tr!("side_panel.front_preset.two_doors"),
            vec![f(FrontKind::Door), f(FrontKind::Door)],
        ),
        (
            tr!("side_panel.front_preset.drawer_door"),
            vec![sized(FrontKind::Drawer, 180.0), f(FrontKind::Door)],
        ),
        (
            tr!("side_panel.front_preset.drawer_pair"),
            vec![sized(FrontKind::Drawer, 180.0), f(FrontKind::Pair)],
        ),
        (
            tr!("side_panel.front_preset.sink"),
            vec![sized(FrontKind::FalseFront, 140.0), f(FrontKind::Pair)],
        ),
        (
            tr!("side_panel.front_preset.blind_left"),
            vec![FrontSpec {
                columns: vec![
                    FrontSpec {
                        width: Some(500.0),
                        ..f(FrontKind::Panel)
                    },
                    f(FrontKind::Door),
                ],
                ..f(FrontKind::Door)
            }],
        ),
        (
            tr!("side_panel.front_preset.blind_right"),
            vec![FrontSpec {
                columns: vec![
                    FrontSpec {
                        width: Some(500.0),
                        ..f(FrontKind::Door)
                    },
                    f(FrontKind::Panel),
                ],
                ..f(FrontKind::Door)
            }],
        ),
        (
            tr!("side_panel.front_preset.drawers_2"),
            vec![f(FrontKind::Drawer); 2],
        ),
        (
            tr!("side_panel.front_preset.drawers_3"),
            vec![f(FrontKind::Drawer); 3],
        ),
        (
            tr!("side_panel.front_preset.drawers_4"),
            vec![
                sized(FrontKind::Drawer, 140.0),
                sized(FrontKind::Drawer, 140.0),
                f(FrontKind::Drawer),
                f(FrontKind::Drawer),
            ],
        ),
    ]
}

/// Krátky popis čiel: „zásuvka 180 · dvierka“.
pub(crate) fn fronts_text(fronts: &[FrontSpec]) -> String {
    if fronts.is_empty() {
        return tr!("side_panel.fronts_text.none");
    }
    // `side`: pri dvierkach uviesť stranu pántov (v stĺpcoch vedľa seba sa neuvádza)
    let kind_name = |f: &FrontSpec, side: bool| match f.kind {
        FrontKind::Door if !side => tr!("side_panel.fronts_text.door"),
        FrontKind::Door => match f.hinge_side {
            Some(HingeSide::Right) => tr!("side_panel.fronts_text.door_right"),
            _ => tr!("side_panel.fronts_text.door_left"),
        },
        FrontKind::Pair => tr!("side_panel.fronts_text.pair"),
        FrontKind::Drawer => tr!("side_panel.fronts_text.drawer"),
        FrontKind::FalseFront => tr!("side_panel.fronts_text.false_front"),
        FrontKind::Open => tr!("side_panel.fronts_text.open"),
        FrontKind::Panel => tr!("side_panel.fronts_text.panel"),
    };
    let mut parts: Vec<String> = Vec::new();
    for f in fronts {
        let name = if f.columns.is_empty() {
            kind_name(f, true)
        } else {
            // stĺpce vedľa seba: „panel | dvierka“
            f.columns
                .iter()
                .map(|c| kind_name(c, false))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        parts.push(match f.height {
            Some(h) => format!("{name} {}", fmt(h)),
            None => name,
        });
    }
    // rovnaké za sebou: „3 × zásuvka“
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let mut n = 1;
        while parts.get(i + n) == parts.get(i) {
            n += 1;
        }
        out.push(if n > 1 {
            format!("{n} × {}", parts[i])
        } else {
            parts[i].clone()
        });
        i += n;
    }
    out.join(" · ")
}

/// Predvolená výška spodku skrinky nad podlahou (kap. 13.3).
/// Hodnota poľa ako v CV: „+50“ / „-50“ (aj „−50“) = zmena o hodnotu, inak nová hodnota.
pub(crate) fn parse_relative(text: &str, current: f64) -> Option<f64> {
    let t = text.trim().replace(',', ".").replace('−', "-");
    let v = if let Some(d) = t.strip_prefix('+') {
        current + d.trim().parse::<f64>().ok()?
    } else if let Some(d) = t.strip_prefix('-') {
        current - d.trim().parse::<f64>().ok()?
    } else {
        t.parse::<f64>().ok()?
    };
    v.is_finite().then_some(v)
}

/// Výškové umiestnenia ako v CV (Placement) v poradí ponuky (stále kľúče).
const PLACEMENTS: [&str; 11] = [
    "floor",
    "toe_height",
    "under_counter",
    "counter_top",
    "under_uppers",
    "under_soffit",
    "ceiling",
    "above_soffit",
    "above_elevation",
    "below_elevation",
    "float",
];

/// Zobrazený názov umiestnenia z [PLACEMENTS].
fn placement_label(key: &str) -> String {
    match key {
        "floor" => tr!("side_panel.placement.floor"),
        "toe_height" => tr!("side_panel.placement.toe_height"),
        "under_counter" => tr!("side_panel.placement.under_counter"),
        "counter_top" => tr!("side_panel.placement.counter_top"),
        "under_uppers" => tr!("side_panel.placement.under_uppers"),
        "under_soffit" => tr!("side_panel.placement.under_soffit"),
        "ceiling" => tr!("side_panel.placement.ceiling"),
        "above_soffit" => tr!("side_panel.placement.above_soffit"),
        "above_elevation" => tr!("side_panel.placement.above_elevation"),
        "below_elevation" => tr!("side_panel.placement.below_elevation"),
        "float" => tr!("side_panel.placement.float"),
        other => other.to_owned(),
    }
}

fn placement_key(p: cc_model::Placement) -> &'static str {
    use cc_model::Placement as P;
    match p {
        P::Floor => "floor",
        P::ToeHeight => "toe_height",
        P::UnderCounter => "under_counter",
        P::CounterTop => "counter_top",
        P::UnderUppers => "under_uppers",
        P::UnderSoffit => "under_soffit",
        P::Ceiling => "ceiling",
        P::AboveSoffit => "above_soffit",
        P::AboveElevation(_) => "above_elevation",
        P::BelowElevation(_) => "below_elevation",
    }
}

/// Zobrazený názov typu skrinky („spodná“).
pub(crate) fn type_name(t: CabinetType) -> String {
    match t {
        CabinetType::Base => tr!("side_panel.cabinet_type.base"),
        CabinetType::Wall => tr!("side_panel.cabinet_type.wall"),
        CabinetType::Tall => tr!("side_panel.cabinet_type.tall"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Slovenský popis čiel ostáva rovnaký ako pred prekladom.
    #[test]
    fn fronts_text_slovak() {
        assert_eq!(fronts_text(&[]), "bez čiel");
        let presets = front_presets();
        let text = |i: usize| presets.get(i).map(|(_, f)| fronts_text(f));
        assert_eq!(text(5).as_deref(), Some("zásuvka 180 · dvierka Ľ"));
        assert_eq!(text(8).as_deref(), Some("panel | dvierka"));
        assert_eq!(text(11).as_deref(), Some("3 × zásuvka"));
    }
}
