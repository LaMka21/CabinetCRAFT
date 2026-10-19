//! Pôdorys a pohľad na stenu ako v CV: ľavý panel so zoznamom režimov (Steny, Objekty,
//! Podlahy, … v pôdoryse; Objekty, Vyplnenie, Lišty, Zástena, … v pohľade) a pod nimi
//! parametre miestnosti. Klik na režim (alebo F2–F7) ukáže jeho obsah, Návrat v páse vráti
//! zoznam. V režime Steny má pás nástroje kreslenia. Rozloženie zodpovedá CV, ikony a texty
//! sú vlastné; zmeny idú príkazmi `room.*`, `wall.*`, `opening.*`, `cabinet.fill`.

use cc_i18n::tr;
use cc_model::{OpeningKind, ProjectFile};
use cc_room::FillParams;
use eframe::egui::{self, RichText};
use serde_json::{json, Value};

use crate::app::{App, Tab};
use crate::icons::Icon;
use crate::part_view::fmt;
use crate::plan_view::{opening_name, Tool};
use crate::side_panel::{parse_relative, prop_row, task_row};
use crate::theme;

/// Režim ľavého panela v pôdoryse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PlanMode {
    /// Zoznam režimov a parametre miestnosti.
    #[default]
    Home,
    Walls,
    Objects,
    Floors,
    Ceilings,
    Tops,
    Molding,
    AutoFill,
    Section,
    Cad,
    /// Zástena nad pracovnou doskou (pohľad na stenu).
    Splash,
    /// Skupiny kresby dreva cez čelá (pohľad na stenu).
    GrainMatch,
}

impl PlanMode {
    /// Režimy v poradí zoznamu.
    pub(crate) const LIST: [PlanMode; 9] = [
        PlanMode::Walls,
        PlanMode::Objects,
        PlanMode::Floors,
        PlanMode::Ceilings,
        PlanMode::Tops,
        PlanMode::Molding,
        PlanMode::AutoFill,
        PlanMode::Section,
        PlanMode::Cad,
    ];

    /// Režimy pohľadu na stenu v poradí zoznamu (ako v CV).
    pub(crate) const ELEV_LIST: [PlanMode; 7] = [
        PlanMode::Objects,
        PlanMode::AutoFill,
        PlanMode::Molding,
        PlanMode::Splash,
        PlanMode::GrainMatch,
        PlanMode::Section,
        PlanMode::Cad,
    ];

    pub(crate) fn label(self) -> String {
        match self {
            PlanMode::Home => tr!("plan_modes.mode.home"),
            PlanMode::Walls => tr!("plan_modes.mode.walls"),
            PlanMode::Objects => tr!("plan_modes.mode.objects"),
            PlanMode::Floors => tr!("plan_modes.mode.floors"),
            PlanMode::Ceilings => tr!("plan_modes.mode.ceilings"),
            PlanMode::Tops => tr!("plan_modes.mode.tops"),
            PlanMode::Molding => tr!("plan_modes.mode.molding"),
            PlanMode::AutoFill => tr!("plan_modes.mode.autofill"),
            PlanMode::Section => tr!("plan_modes.mode.section"),
            PlanMode::Cad => tr!("plan_modes.mode.cad"),
            PlanMode::Splash => tr!("plan_modes.mode.splash"),
            PlanMode::GrainMatch => tr!("plan_modes.mode.grain"),
        }
    }

    fn icon(self) -> Icon {
        match self {
            PlanMode::Home => Icon::Plan,
            PlanMode::Walls => Icon::Walls,
            PlanMode::Objects => Icon::Catalog,
            PlanMode::Floors => Icon::Floor,
            PlanMode::Ceilings => Icon::Ceiling,
            PlanMode::Tops => Icon::Worktop,
            PlanMode::Molding => Icon::Molding,
            PlanMode::AutoFill => Icon::AutoFill,
            PlanMode::Section => Icon::Section,
            PlanMode::Cad => Icon::Cad,
            PlanMode::Splash => Icon::Splash,
            PlanMode::GrainMatch => Icon::Grain,
        }
    }

    /// Kláves režimu (ako v CV: F2 steny … F7 CAD).
    pub(crate) fn key(self) -> Option<egui::Key> {
        match self {
            PlanMode::Walls => Some(egui::Key::F2),
            PlanMode::Objects => Some(egui::Key::F3),
            PlanMode::Tops => Some(egui::Key::F4),
            PlanMode::Molding => Some(egui::Key::F5),
            PlanMode::AutoFill => Some(egui::Key::F6),
            PlanMode::Cad => Some(egui::Key::F7),
            _ => None,
        }
    }

    /// Kľúč pre riadiaci kanál (`ui.set {"plan_mode": …}`).
    pub(crate) fn id(self) -> &'static str {
        match self {
            PlanMode::Home => "home",
            PlanMode::Walls => "walls",
            PlanMode::Objects => "objects",
            PlanMode::Floors => "floors",
            PlanMode::Ceilings => "ceilings",
            PlanMode::Tops => "tops",
            PlanMode::Molding => "molding",
            PlanMode::AutoFill => "autofill",
            PlanMode::Section => "section",
            PlanMode::Cad => "cad",
            PlanMode::Splash => "splash",
            PlanMode::GrainMatch => "grain",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<PlanMode> {
        std::iter::once(PlanMode::Home)
            .chain(PlanMode::LIST)
            .chain([PlanMode::Splash, PlanMode::GrainMatch])
            .find(|m| m.id() == id)
    }
}

/// Rozsah automatického vyplnenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillScope {
    SelectedWall,
    AllWalls,
}

/// Rady skriniek pri automatickom vyplnení.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FillRows {
    Base,
    Upper,
    Both,
}

/// Stav ľavého panela pôdorysu.
pub(crate) struct PlanUi {
    pub mode: PlanMode,
    /// Režim ľavého panela v pohľade na stenu.
    pub elev_mode: PlanMode,
    /// Objekty: otvorený katalóg (`false` = zoznam katalógov).
    catalog_open: bool,
    fill: FillParams,
    fill_scope: FillScope,
    fill_rows: FillRows,
}

impl Default for PlanUi {
    fn default() -> Self {
        PlanUi {
            mode: PlanMode::Home,
            elev_mode: PlanMode::Home,
            catalog_open: false,
            fill: FillParams::default(),
            fill_scope: FillScope::SelectedWall,
            fill_rows: FillRows::Base,
        }
    }
}

impl App {
    /// Režim ľavého panela aktuálnej záložky (pôdorys alebo pohľad na stenu).
    pub(crate) fn plan_mode(&self) -> PlanMode {
        if self.tab == Tab::Elevation {
            self.plan_ui.elev_mode
        } else {
            self.plan_ui.mode
        }
    }

    /// Režimy v zozname aktuálnej záložky.
    pub(crate) fn mode_list(&self) -> &'static [PlanMode] {
        if self.tab == Tab::Elevation {
            &PlanMode::ELEV_LIST
        } else {
            &PlanMode::LIST
        }
    }

    /// Prepne režim ľavého panela aktuálnej záložky; mimo režimu Steny sa kreslenie ukončí.
    /// Režim, ktorý záložka nemá, sa ignoruje.
    pub(crate) fn set_plan_mode(&mut self, mode: PlanMode) {
        if mode != PlanMode::Home && !self.mode_list().contains(&mode) {
            return;
        }
        if mode == PlanMode::Objects && self.plan_mode() != PlanMode::Objects {
            self.plan_ui.catalog_open = false;
        }
        if self.tab == Tab::Elevation {
            self.plan_ui.elev_mode = mode;
            return;
        }
        if mode != PlanMode::Walls && matches!(self.plan.tool, Tool::DrawWall | Tool::RectRoom) {
            self.plan.tool = Tool::Select;
        }
        self.plan_ui.mode = mode;
    }

    /// Ľavý panel pôdorysu.
    pub(crate) fn plan_panel(&mut self, ui: &mut egui::Ui) {
        let mode = self.plan_mode();
        if mode != PlanMode::Home {
            // hlavička režimu: ikona a meno (Návrat je v páse)
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
                crate::icons::paint(ui.painter(), rect, mode.icon(), true);
                ui.label(RichText::new(mode.label()).size(theme::HEADING));
            });
            ui.add_space(4.0);
        }
        match mode {
            PlanMode::Home => self.plan_home(ui),
            PlanMode::Walls => self.plan_walls(ui),
            PlanMode::Objects => self.plan_objects(ui),
            PlanMode::Floors => not_yet(
                ui,
                &[
                    (Icon::Floor, tr!("plan_modes.floors.auto")),
                    (Icon::Floor, tr!("plan_modes.floors.new")),
                ],
                &tr!("plan_modes.floors.note"),
            ),
            PlanMode::Ceilings => not_yet(
                ui,
                &[
                    (Icon::Ceiling, tr!("plan_modes.ceilings.auto")),
                    (Icon::Ceiling, tr!("plan_modes.ceilings.new")),
                ],
                &tr!("plan_modes.ceilings.note"),
            ),
            PlanMode::Tops => not_yet(
                ui,
                &[
                    (Icon::Worktop, tr!("plan_modes.tops.auto")),
                    (Icon::Worktop, tr!("plan_modes.tops.new")),
                ],
                &tr!("plan_modes.tops.note"),
            ),
            PlanMode::Molding => not_yet(
                ui,
                &[
                    (Icon::Molding, tr!("plan_modes.molding.auto")),
                    (Icon::Molding, tr!("plan_modes.molding.new")),
                ],
                &tr!("plan_modes.molding.note"),
            ),
            PlanMode::AutoFill => self.plan_autofill(ui),
            PlanMode::Section => self.plan_section(ui),
            PlanMode::Cad => not_yet(ui, &[], &tr!("plan_modes.cad.note")),
            PlanMode::Splash => not_yet(
                ui,
                &[(Icon::Splash, tr!("plan_modes.splash.new"))],
                &tr!("plan_modes.splash.note"),
            ),
            PlanMode::GrainMatch => {
                ui.label(RichText::new(tr!("plan_modes.grain.groups")).size(theme::SMALL + 0.5));
                egui::Frame::new()
                    .fill(theme::SHEET)
                    .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(ui.available_width(), 220.0));
                    });
                ui.add_enabled(false, egui::Button::new(tr!("plan_modes.grain.ungroup")));
                ui.label(RichText::new(tr!("plan_modes.grain.note")).color(theme::TEXT_WEAK));
            }
        }
    }

    // ----- zoznam režimov -----

    fn plan_home(&mut self, ui: &mut egui::Ui) {
        for &mode in self.mode_list() {
            if task_row(ui, mode.icon(), &mode.label(), true, false).clicked() {
                self.set_plan_mode(mode);
            }
        }
        ui.add_space(6.0);
        self.room_params(ui);
        // vybraná skrinka: zoznam a vlastnosti ako doteraz
        if self.session.selection().cabinet.is_some() {
            ui.add_space(6.0);
            self.cabinet_sections(ui);
        }
    }

    /// Parametre miestnosti (výšky platné pre celú miestnosť).
    fn room_params(&mut self, ui: &mut egui::Ui) {
        if !self.section(ui, &tr!("plan_modes.room.title")) {
            return;
        }
        let Some(doc) = self.session.doc() else {
            return;
        };
        let room = doc.project.room.clone().unwrap_or_default();
        let upper_top = room.upper_top.unwrap_or(room.wall_height - room.soffit);
        let mut set = serde_json::Map::new();
        for (label, key, value) in [
            (
                tr!("plan_modes.room.wall_height"),
                "wall_height",
                room.wall_height,
            ),
            (
                tr!("plan_modes.room.wall_thickness"),
                "wall_thickness",
                room.wall_thickness,
            ),
            (tr!("plan_modes.room.soffit"), "soffit", room.soffit),
            (
                tr!("plan_modes.room.toe_height"),
                "toe_height",
                room.toe_height,
            ),
            (tr!("plan_modes.room.upper_top"), "upper_top", upper_top),
        ] {
            if let Some(v) = self.mm_row(ui, &label, &format!("room.{key}"), value) {
                if (0.0..=100_000.0).contains(&v) {
                    set.insert(key.to_owned(), json!(v));
                }
            }
        }
        if !set.is_empty() {
            self.exec("room.set", json!({ "set": Value::Object(set) }));
        }
    }

    /// Riadok s rozmerom v mm; vráti novú hodnotu po potvrdení („+50“ = o 50 viac).
    fn mm_row(&mut self, ui: &mut egui::Ui, label: &str, key: &str, value: f64) -> Option<f64> {
        let mut out = None;
        prop_row(ui, label, false, |ui| {
            if let Some(t) = self.buffered_text(ui, key, &fmt(value)) {
                out = parse_relative(&t, value);
            }
        });
        out
    }

    // ----- Steny -----

    fn plan_walls(&mut self, ui: &mut egui::Ui) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let project = doc.project.clone();
        let room = project.room.clone().unwrap_or_default();
        let wall = self
            .plan
            .selected_wall
            .clone()
            .and_then(|id| project.walls.iter().find(|w| w.id == id).cloned());
        let Some(w) = wall else {
            ui.label(RichText::new(tr!("plan_modes.walls.hint")).color(theme::TEXT_WEAK));
            ui.add_space(6.0);
            if self.section(ui, &tr!("plan_modes.walls.new_walls")) {
                let mut set = serde_json::Map::new();
                for (label, key, value) in [
                    (
                        tr!("plan_modes.walls.height"),
                        "wall_height",
                        room.wall_height,
                    ),
                    (
                        tr!("plan_modes.walls.thickness"),
                        "wall_thickness",
                        room.wall_thickness,
                    ),
                    (tr!("plan_modes.walls.soffit"), "soffit", room.soffit),
                ] {
                    if let Some(v) = self.mm_row(ui, &label, &format!("room.{key}"), value) {
                        if (0.0..=100_000.0).contains(&v) {
                            set.insert(key.to_owned(), json!(v));
                        }
                    }
                }
                if !set.is_empty() {
                    self.exec("room.set", json!({ "set": Value::Object(set) }));
                }
            }
            return;
        };

        let id = w.id.clone();
        let mut set = serde_json::Map::new();
        if self.section(ui, &tr!("plan_modes.walls.wall", wall = &id)) {
            for (label, key, value) in [
                (tr!("plan_modes.walls.start_x"), "x", w.x),
                (tr!("plan_modes.walls.start_y"), "y", w.y),
                (tr!("plan_modes.walls.length"), "length", w.length),
                (tr!("plan_modes.walls.angle"), "angle", w.angle),
            ] {
                if let Some(v) = self.mm_row(ui, &label, &format!("wall.{id}.{key}"), value) {
                    let ok = match key {
                        "length" => (100.0..=50_000.0).contains(&v),
                        "angle" => (-360.0..=360.0).contains(&v),
                        _ => v.abs() <= 100_000.0,
                    };
                    if ok {
                        set.insert(key.to_owned(), json!(v));
                    }
                }
            }
        }
        if self.section(ui, &tr!("plan_modes.walls.dimensions")) {
            for (label, key, value) in [
                (
                    tr!("plan_modes.walls.height"),
                    "height",
                    w.height.unwrap_or(room.wall_height),
                ),
                (
                    tr!("plan_modes.walls.thickness"),
                    "thickness",
                    w.thickness.unwrap_or(room.wall_thickness),
                ),
            ] {
                if let Some(v) = self.mm_row(ui, &label, &format!("wall.{id}.{key}"), value) {
                    if (10.0..=10_000.0).contains(&v) {
                        set.insert(key.to_owned(), json!(v));
                    }
                }
            }
            let soffit = tr!("plan_modes.walls.room_soffit");
            if let Some(v) = self.mm_row(ui, &soffit, "room.soffit", room.soffit) {
                if (0.0..=5_000.0).contains(&v) {
                    self.exec("room.set", json!({ "set": { "soffit": v } }));
                }
            }
        }
        if !set.is_empty() {
            self.exec("wall.set", json!({ "id": id, "set": Value::Object(set) }));
        }
        self.wall_openings(ui, &project, &id);
    }

    /// Okná, dvere a otvory vybranej steny.
    fn wall_openings(&mut self, ui: &mut egui::Ui, project: &ProjectFile, wall: &str) {
        if !self.section(ui, &tr!("plan_modes.openings.title")) {
            return;
        }
        let mut cmds: Vec<(&'static str, Value)> = Vec::new();
        for (index, o) in project
            .openings
            .iter()
            .enumerate()
            .filter(|(_, o)| o.wall == wall)
        {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{}.", index + 1)).strong());
                let mut kind = o.kind;
                egui::ComboBox::from_id_salt(("plan_opening_kind", index))
                    .selected_text(opening_name(kind))
                    .width(80.0)
                    .show_ui(ui, |ui| {
                        for k in [OpeningKind::Window, OpeningKind::Door, OpeningKind::Opening] {
                            ui.selectable_value(&mut kind, k, opening_name(k));
                        }
                    });
                if kind != o.kind {
                    cmds.push((
                        "opening.set",
                        json!({ "index": index, "set": { "kind": kind.as_str() } }),
                    ));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("✖")
                        .on_hover_text(tr!("plan_modes.openings.delete"))
                        .clicked()
                    {
                        cmds.push(("opening.remove", json!({ "index": index })));
                    }
                });
            });
            for (label, key, value) in [
                (tr!("plan_modes.openings.at"), "at", o.at),
                (tr!("plan_modes.openings.width"), "width", o.width),
                (tr!("plan_modes.openings.height"), "height", o.height),
                (tr!("plan_modes.openings.sill"), "elevation", o.elevation),
            ] {
                if let Some(v) = self.mm_row(ui, &label, &format!("opening.{index}.{key}"), value) {
                    if (0.0..=50_000.0).contains(&v) {
                        cmds.push(("opening.set", json!({ "index": index, "set": { key: v } })));
                    }
                }
            }
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            for (kind, label) in [
                (OpeningKind::Window, tr!("plan_modes.openings.add_window")),
                (OpeningKind::Door, tr!("plan_modes.openings.add_door")),
                (OpeningKind::Opening, tr!("plan_modes.openings.add_opening")),
            ] {
                if ui.button(label).clicked() {
                    cmds.push((
                        "opening.add",
                        json!({ "wall": wall, "kind": kind.as_str() }),
                    ));
                }
            }
        });
        for (id, params) in cmds {
            if self.exec(id, params).is_none() {
                break;
            }
        }
    }

    // ----- Objekty -----

    fn plan_objects(&mut self, ui: &mut egui::Ui) {
        let names: Vec<String> = self
            .session
            .doc()
            .map(|d| d.catalogs.iter().map(|c| c.name.clone()).collect())
            .unwrap_or_default();
        if !self.plan_ui.catalog_open {
            // zoznam katalógov (ako „vybrať katalóg“ v CV)
            ui.label(RichText::new(tr!("plan_modes.objects.select_catalog")).strong());
            ui.add_space(2.0);
            egui::Frame::new()
                .fill(theme::SHEET)
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER))
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if names.is_empty() {
                        ui.label(
                            RichText::new(tr!("plan_modes.objects.no_catalogs"))
                                .color(theme::TEXT_WEAK),
                        );
                    }
                    for n in &names {
                        let r = ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                            crate::icons::paint(ui.painter(), rect, Icon::Parts, true);
                            ui.selectable_label(false, n)
                        });
                        if r.inner.clicked() {
                            self.catalog_ui.current = Some(n.clone());
                            self.catalog_ui.item = None;
                            self.plan_ui.catalog_open = true;
                        }
                    }
                });
            ui.add_space(8.0);
            // samostatná skrinka bez katalógu
            for (icon, t, label) in [
                (Icon::CabinetBase, "base", tr!("plan_modes.objects.base")),
                (Icon::CabinetWall, "wall", tr!("plan_modes.objects.wall")),
                (Icon::CabinetTall, "tall", tr!("plan_modes.objects.tall")),
            ] {
                if task_row(ui, icon, &label, true, false).clicked() {
                    self.exec("cabinet.add", json!({ "type": t }));
                }
            }
        } else {
            if ui
                .button(tr!("plan_modes.objects.back"))
                .on_hover_text(tr!("plan_modes.objects.back_hint"))
                .clicked()
            {
                self.plan_ui.catalog_open = false;
            }
            ui.add_space(4.0);
            self.catalog_section(ui);
        }
        if self.session.selection().cabinet.is_some() {
            ui.add_space(6.0);
            self.cabinet_sections(ui);
        }
    }

    // ----- Automatické vyplnenie -----

    fn plan_autofill(&mut self, ui: &mut egui::Ui) {
        let wall = if self.tab == Tab::Elevation {
            self.wall_view.wall.clone()
        } else {
            self.plan.selected_wall.clone()
        };
        let walls: Vec<String> = self
            .session
            .doc()
            .map(|d| d.project.walls.iter().map(|w| w.id.clone()).collect())
            .unwrap_or_default();
        let st = &mut self.plan_ui;
        let scope_name = |s: FillScope| match s {
            FillScope::SelectedWall => tr!("plan_modes.autofill.selected_wall"),
            FillScope::AllWalls => tr!("plan_modes.autofill.all_walls"),
        };
        let rows_name = |r: FillRows| match r {
            FillRows::Base => tr!("plan_modes.autofill.base_row"),
            FillRows::Upper => tr!("plan_modes.autofill.upper_row"),
            FillRows::Both => tr!("plan_modes.autofill.both_rows"),
        };
        prop_row(ui, &tr!("plan_modes.autofill.scope"), false, |ui| {
            egui::ComboBox::from_id_salt("fill_scope")
                .selected_text(scope_name(st.fill_scope))
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    for s in [FillScope::SelectedWall, FillScope::AllWalls] {
                        ui.selectable_value(&mut st.fill_scope, s, scope_name(s));
                    }
                });
        });
        prop_row(ui, &tr!("plan_modes.autofill.rows"), false, |ui| {
            egui::ComboBox::from_id_salt("fill_rows")
                .selected_text(rows_name(st.fill_rows))
                .width(ui.available_width() - 4.0)
                .show_ui(ui, |ui| {
                    for r in [FillRows::Base, FillRows::Upper, FillRows::Both] {
                        ui.selectable_value(&mut st.fill_rows, r, rows_name(r));
                    }
                });
        });
        prop_row(ui, &tr!("plan_modes.autofill.source"), false, |ui| {
            ui.label(RichText::new(tr!("plan_modes.autofill.source_last")).color(theme::TEXT_WEAK));
        });
        if self.section(ui, &tr!("plan_modes.autofill.cabinets")) {
            let p = &mut self.plan_ui.fill;
            for (label, v) in [
                (tr!("plan_modes.autofill.nominal"), &mut p.nominal),
                (tr!("plan_modes.autofill.max"), &mut p.max),
                (tr!("plan_modes.autofill.increment"), &mut p.increment),
                (tr!("plan_modes.autofill.min"), &mut p.min),
            ] {
                prop_row(ui, &label, false, |ui| {
                    ui.add(
                        egui::DragValue::new(v)
                            .speed(5.0)
                            .range(10.0..=3000.0)
                            .suffix(" mm"),
                    );
                });
            }
        }
        ui.add_space(6.0);
        let targets: Vec<String> = match self.plan_ui.fill_scope {
            FillScope::SelectedWall => wall.clone().into_iter().collect(),
            FillScope::AllWalls => walls,
        };
        let text = match (self.plan_ui.fill_scope, &wall) {
            (FillScope::SelectedWall, Some(w)) => tr!("plan_modes.autofill.fill_wall", wall = w),
            (FillScope::SelectedWall, None) => tr!("plan_modes.autofill.pick_wall"),
            (FillScope::AllWalls, _) => tr!("plan_modes.autofill.fill_all"),
        };
        if ui
            .add_enabled(!targets.is_empty(), egui::Button::new(text))
            .on_hover_text(tr!("plan_modes.autofill.fill_hint"))
            .clicked()
        {
            let rows: &[&str] = match self.plan_ui.fill_rows {
                FillRows::Base => &["base"],
                FillRows::Upper => &["upper"],
                FillRows::Both => &["base", "upper"],
            };
            let p = self.plan_ui.fill;
            'fill: for w in &targets {
                for row in rows {
                    let params = json!({
                        "wall": w, "row": row,
                        "nominal": p.nominal, "max": p.max,
                        "increment": p.increment, "min": p.min,
                    });
                    if self.exec("cabinet.fill", params).is_none() {
                        break 'fill;
                    }
                }
            }
        }
    }

    // ----- Rez -----

    fn plan_section(&mut self, ui: &mut egui::Ui) {
        let walls: Vec<(String, f64)> = self
            .session
            .doc()
            .map(|d| {
                d.project
                    .walls
                    .iter()
                    .map(|w| (w.id.clone(), w.length))
                    .collect()
            })
            .unwrap_or_default();
        let elevation = self.tab == Tab::Elevation;
        if elevation {
            // pohľad na stenu: strana steny a druh pohľadu (ako v CV; zatiaľ iba čelo steny)
            if self.section(ui, &tr!("plan_modes.section.wall_side")) {
                for (label, on) in [
                    (tr!("plan_modes.section.front"), true),
                    (tr!("plan_modes.section.back"), false),
                    (tr!("plan_modes.section.both"), false),
                ] {
                    prop_row(ui, &label, false, |ui| {
                        ui.add_enabled(on, egui::RadioButton::new(on, ""));
                    });
                }
            }
            if self.section(ui, &tr!("plan_modes.section.views")) {
                for (label, on) in [
                    (tr!("plan_modes.section.view_wall"), true),
                    (tr!("plan_modes.section.view_end"), false),
                ] {
                    prop_row(ui, &label, false, |ui| {
                        ui.add_enabled(on, egui::RadioButton::new(on, ""));
                    });
                }
            }
            ui.add_space(4.0);
        } else {
            ui.label(RichText::new(tr!("plan_modes.section.hint")).color(theme::TEXT_WEAK));
            ui.add_space(4.0);
        }
        let selected = if elevation {
            self.wall_view.wall.clone()
        } else {
            self.plan.selected_wall.clone()
        };
        let mut open = None;
        for (id, length) in &walls {
            let on = selected.as_ref() == Some(id);
            let len = fmt(*length);
            let text = tr!("plan_modes.section.wall_row", wall = id, length = len);
            let r = ui.selectable_label(on, text);
            if r.clicked() {
                self.plan.selected_wall = Some(id.clone());
                if elevation {
                    open = Some(id.clone());
                }
            }
            if r.double_clicked() {
                open = Some(id.clone());
            }
        }
        ui.add_space(6.0);
        if !elevation
            && ui
                .add_enabled(
                    selected.is_some(),
                    egui::Button::new(tr!("plan_modes.section.open_elevation")),
                )
                .clicked()
        {
            open = selected;
        }
        if let Some(id) = open {
            self.wall_view.wall = Some(id);
            self.tab = Tab::Elevation;
        }
    }
}

/// Režim, ktorý zatiaľ nie je hotový: položky ako v CV (sivé) a vysvetlenie.
fn not_yet(ui: &mut egui::Ui, items: &[(Icon, String)], note: &str) {
    for (icon, label) in items {
        task_row(ui, *icon, label, false, false).on_hover_text(tr!("plan_modes.not_available"));
    }
    ui.add_space(6.0);
    ui.label(RichText::new(note).color(theme::TEXT_WEAK));
}
