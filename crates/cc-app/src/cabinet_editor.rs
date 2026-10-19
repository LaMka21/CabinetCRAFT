//! Editor skrinky ako v CV (dvojklik na skrinku): samotná skrinka s vlastnými záložkami
//! Rez (sekcie čiel – otvory 1A, 1B…), Čelo, Pôdorys, Bok (pravouhlé pohľady s vŕtaním),
//! 3D a Zostavy (diely skrinky). Ľavý panel a pás sa menia podľa záložky; Návrat v páse
//! vráti predchádzajúcu záložku. Rozloženie zodpovedá CV, texty a ikony sú vlastné.

use cc_i18n::tr;
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Vec2};
use serde_json::json;

use glam::DVec3;

use crate::app::{App, Tab};
use crate::icons::Icon;
use crate::part_view::fmt;
use crate::side_panel::{prop_row, task_row};
use crate::tabs::role_name;
use crate::theme;

const OPENING: Color32 = Color32::from_rgb(30, 120, 220);

/// Záložka editora skrinky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CabTab {
    #[default]
    Section,
    Face,
    Plan,
    End,
    View3d,
    Reports,
}

/// Záložky editora: záložka, ikona, kľúč textu (`cc_i18n::tr`), kľúč pre riadiaci kanál.
pub(crate) const CAB_TABS: [(CabTab, Icon, &str, &str); 6] = [
    (
        CabTab::Section,
        Icon::Section,
        "cabinet_editor.tab.section",
        "section",
    ),
    (
        CabTab::Face,
        Icon::Elevation,
        "cabinet_editor.tab.face",
        "face",
    ),
    (CabTab::Plan, Icon::Plan, "cabinet_editor.tab.plan", "plan"),
    (
        CabTab::End,
        Icon::CabinetTall,
        "cabinet_editor.tab.end",
        "end",
    ),
    (
        CabTab::View3d,
        Icon::Cube,
        "cabinet_editor.tab.view3d",
        "3d",
    ),
    (
        CabTab::Reports,
        Icon::Parts,
        "cabinet_editor.tab.reports",
        "reports",
    ),
];

impl CabTab {
    pub(crate) fn key(self) -> &'static str {
        CAB_TABS
            .iter()
            .find(|t| t.0 == self)
            .map_or("section", |t| t.3)
    }

    pub(crate) fn from_key(key: &str) -> Option<CabTab> {
        CAB_TABS.iter().find(|t| t.3 == key).map(|t| t.0)
    }
}

impl App {
    /// Otvorí editor skrinky `id` (záložka Skrinka); Návrat vráti predchádzajúcu záložku.
    pub(crate) fn open_cabinet_editor(&mut self, id: &str) {
        let exists = self
            .session
            .doc()
            .is_some_and(|d| d.project.cabinets.iter().any(|c| c.id == id));
        if !exists {
            return;
        }
        if self.session.selection().cabinet.as_deref() != Some(id) {
            self.exec("selection.set", json!({ "cabinet": id }));
        }
        if self.cab_focus.as_deref() != Some(id) {
            self.cab_camera = cc_scene::Camera::default();
        }
        self.cab_focus = Some(id.to_owned());
        if self.tab != Tab::Cabinet {
            self.cab_return = self.tab;
        }
        self.tab = Tab::Cabinet;
    }

    /// Skrinka v editore: otvorená, inak vybraná.
    fn cab_focus_id(&mut self) -> Option<String> {
        let selected = self.session.selection().cabinet.clone();
        let exists = |f: &String| {
            self.session
                .doc()
                .is_some_and(|d| d.project.cabinets.iter().any(|c| &c.id == f))
        };
        let focus = match (&self.cab_focus, selected) {
            (Some(f), _) if exists(f) => f.clone(),
            (_, Some(s)) => s,
            _ => return None,
        };
        self.cab_focus = Some(focus.clone());
        Some(focus)
    }

    pub(crate) fn tab_cabinet(&mut self, ui: &mut egui::Ui) {
        let Some(focus) = self.cab_focus_id() else {
            ui.label(tr!("cabinet_editor.no_cabinet"));
            return;
        };
        let Some(doc) = self.session.doc() else {
            return;
        };
        let Some(spec) = doc.project.cabinets.iter().find(|c| c.id == focus).cloned() else {
            return;
        };
        let built = self.model_built();
        let model = built
            .as_ref()
            .and_then(|b| b.model.cabinets.iter().find(|c| c.id == focus).cloned());

        // názov skrinky nad plochou (ako v CV)
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr!(
                    "cabinet_editor.title",
                    id = &spec.id,
                    catalog = spec.catalog.as_deref().unwrap_or("")
                ))
                .size(theme::SMALL + 0.5)
                .color(theme::TEXT),
            );
        });
        match self.cab_tab {
            CabTab::Section => {
                let size = ui.available_size();
                self.cabinet_front(ui, &spec, model.as_ref(), size);
            }
            CabTab::Face | CabTab::Plan | CabTab::End => match &model {
                Some(m) => {
                    let selected = self.selected_part();
                    let pick = cabinet_lines(ui, m, self.cab_tab, selected.as_deref());
                    match pick {
                        // dvojklik na dosku: diel samostatne (ako v CV)
                        Some((id, true)) => self.open_part_view(&id),
                        Some((id, false)) => self.select_part(Some(id)),
                        None => {}
                    }
                }
                None => {
                    ui.label(tr!("cabinet_editor.cannot_build"));
                }
            },
            CabTab::View3d => {
                // 3D iba tejto skrinky: vlastná scéna a kamera (výber, rozloženie ako v 3D)
                std::mem::swap(&mut self.view, &mut self.cab_view);
                std::mem::swap(&mut self.camera, &mut self.cab_camera);
                self.view.focus = Some(focus.clone());
                self.tab_3d(ui);
                std::mem::swap(&mut self.view, &mut self.cab_view);
                std::mem::swap(&mut self.camera, &mut self.cab_camera);
                // dvojklik na dosku v 3D skrinky
                if let Some(id) = self.cab_view.open_part.take() {
                    self.open_part_view(&id);
                }
            }
            CabTab::Reports => self.cabinet_parts(ui, model.as_ref()),
        }
    }

    /// Ľavý panel editora skrinky: položky záložky, otvor a vlastnosti skrinky.
    pub(crate) fn cabinet_panel(&mut self, ui: &mut egui::Ui) {
        let Some(focus) = self.cab_focus_id() else {
            return;
        };
        let spec = self
            .session
            .doc()
            .and_then(|d| d.project.cabinets.iter().find(|c| c.id == focus).cloned());
        match self.cab_tab {
            CabTab::Section => {
                if let Some(spec) = &spec {
                    ui.label(
                        RichText::new(tr!("cabinet_editor.panel.view")).size(theme::SMALL + 0.5),
                    );
                    mini_plan(ui, spec.w, spec.d);
                    prop_row(ui, &tr!("cabinet_editor.panel.side"), false, |ui| {
                        let _ = ui.radio(true, tr!("cabinet_editor.panel.side_front"));
                    });
                }
            }
            CabTab::Face | CabTab::Plan | CabTab::End => {
                for (icon, label) in [
                    (Icon::Cad, tr!("cabinet_editor.panel.edit_shape")),
                    (Icon::Catalog, tr!("cabinet_editor.panel.objects")),
                    (Icon::New, tr!("cabinet_editor.panel.new_part")),
                    (Icon::Parts, tr!("cabinet_editor.panel.library_part")),
                    (Icon::Molding, tr!("cabinet_editor.panel.moldings")),
                    (Icon::Cad, tr!("cabinet_editor.panel.cad")),
                ] {
                    task_row(ui, icon, &label, false, false)
                        .on_hover_text(tr!("cabinet_editor.not_available"));
                }
            }
            CabTab::View3d => {
                if task_row(
                    ui,
                    Icon::Cube,
                    &tr!("cabinet_editor.panel.camera"),
                    true,
                    false,
                )
                .on_hover_text(tr!("cabinet_editor.panel.camera_hint"))
                .clicked()
                {
                    self.cab_camera = cc_scene::Camera::default();
                }
                task_row(
                    ui,
                    Icon::Standard,
                    &tr!("cabinet_editor.panel.lights"),
                    false,
                    false,
                )
                .on_hover_text(tr!("cabinet_editor.not_available"));
                task_row(
                    ui,
                    Icon::Cad,
                    &tr!("cabinet_editor.panel.cad"),
                    false,
                    false,
                )
                .on_hover_text(tr!("cabinet_editor.not_available"));
            }
            CabTab::Reports => {
                if task_row(
                    ui,
                    Icon::Parts,
                    &tr!("cabinet_editor.panel.parts_list"),
                    true,
                    false,
                )
                .clicked()
                {
                    self.tab = Tab::Parts;
                }
                if task_row(
                    ui,
                    Icon::Drawing,
                    &tr!("cabinet_editor.panel.cabinet_drawing"),
                    true,
                    false,
                )
                .clicked()
                {
                    self.drawing.sheet = crate::drawings::Sheet::Cabinet(focus.clone());
                    self.tab = Tab::Part;
                }
                if task_row(
                    ui,
                    Icon::Drawing,
                    &tr!("cabinet_editor.panel.part_drawings"),
                    true,
                    false,
                )
                .clicked()
                {
                    self.drawing.sheet = crate::drawings::Sheet::Part;
                    self.tab = Tab::Part;
                }
                if task_row(
                    ui,
                    Icon::Check,
                    &tr!("cabinet_editor.panel.validation"),
                    true,
                    false,
                )
                .clicked()
                {
                    self.tab = Tab::Issues;
                }
            }
        }
        ui.add_space(6.0);
        if self.cab_tab == CabTab::View3d && self.section(ui, &tr!("cabinet_editor.panel.display"))
        {
            prop_row(ui, &tr!("cabinet_editor.panel.explode"), false, |ui| {
                ui.add(
                    egui::Slider::new(&mut self.scene_opts.explode, 0.0..=1.0).show_value(false),
                );
            });
            prop_row(ui, &tr!("cabinet_editor.panel.fronts"), false, |ui| {
                ui.checkbox(&mut self.scene_opts.fronts, "");
            });
        }
        self.cabinet_sections(ui);
    }

    /// Pás editora skrinky (po skupinách Domov a Úpravy).
    pub(crate) fn ribbon_cabinet(&mut self, ui: &mut egui::Ui) {
        use crate::ribbon::{big_button, column, group, small_button};
        let Some(focus) = self.cab_focus_id() else {
            return;
        };
        let cabinets: Vec<(String, String)> = self
            .session
            .doc()
            .map(|d| {
                d.project
                    .cabinets
                    .iter()
                    .map(|c| {
                        let name = c.catalog.clone().unwrap_or_default();
                        (c.id.clone(), name)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let index = cabinets.iter().position(|c| c.0 == focus);
        let mut open: Option<String> = None;
        group(ui, &tr!("cabinet_editor.ribbon.view_group"), |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    self.standard_combo(ui);
                });
                // skrinka v editore: predchádzajúca / ďalšia ako v CV
                ui.horizontal(|ui| {
                    let prev = index.and_then(|i| i.checked_sub(1));
                    let next = index.map(|i| i + 1).filter(|&i| i < cabinets.len());
                    if ui
                        .add_enabled(prev.is_some(), egui::Button::new("◀").small())
                        .clicked()
                    {
                        open = prev.and_then(|i| cabinets.get(i)).map(|c| c.0.clone());
                    }
                    let text = cabinets
                        .iter()
                        .find(|c| c.0 == focus)
                        .map_or(focus.clone(), |c| format!("{} {}", c.0, c.1));
                    egui::ComboBox::from_id_salt("cab_editor_pick")
                        .selected_text(text)
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for (id, name) in &cabinets {
                                if ui
                                    .selectable_label(*id == focus, format!("{id} {name}"))
                                    .clicked()
                                {
                                    open = Some(id.clone());
                                }
                            }
                        });
                    if ui
                        .add_enabled(next.is_some(), egui::Button::new("▶").small())
                        .clicked()
                    {
                        open = next.and_then(|i| cabinets.get(i)).map(|c| c.0.clone());
                    }
                });
            });
        });
        if let Some(id) = open {
            self.open_cabinet_editor(&id);
        }

        if self.cab_tab == CabTab::Section {
            // delenie vybraného otvoru (ako Section v CV)
            let target = self
                .wall_view
                .opening
                .clone()
                .filter(|(id, _, _)| *id == focus);
            let base = |(id, row, col): &(String, usize, Option<usize>)| {
                let mut p = json!({ "cabinet": id, "row": row });
                if let Some(k) = col {
                    p["column"] = json!(k);
                }
                p
            };
            let mut cmd: Option<(&'static str, serde_json::Value)> = None;
            group(ui, &tr!("cabinet_editor.ribbon.section_group"), |ui| {
                column(ui, |ui| {
                    let row_only = target.as_ref().is_some_and(|t| t.2.is_none());
                    if small_button(
                        ui,
                        Some(Icon::Rectangle),
                        &tr!("cabinet_editor.ribbon.split_horizontal"),
                        row_only,
                        false,
                    )
                    .on_hover_text(tr!("cabinet_editor.ribbon.split_horizontal_hint"))
                    .clicked()
                    {
                        if let Some(t) = &target {
                            let mut p = base(t);
                            p["direction"] = json!("horizontal");
                            cmd = Some(("front.split", p));
                        }
                    }
                    if small_button(
                        ui,
                        Some(Icon::Line),
                        &tr!("cabinet_editor.ribbon.split_vertical"),
                        target.is_some(),
                        false,
                    )
                    .on_hover_text(tr!("cabinet_editor.ribbon.split_vertical_hint"))
                    .clicked()
                    {
                        if let Some(t) = &target {
                            let mut p = base(t);
                            p["direction"] = json!("vertical");
                            cmd = Some(("front.split", p));
                        }
                    }
                    if small_button(
                        ui,
                        Some(Icon::Delete),
                        &tr!("cabinet_editor.ribbon.delete_split"),
                        target.is_some(),
                        false,
                    )
                    .clicked()
                    {
                        if let Some(t) = &target {
                            cmd = Some(("front.remove", base(t)));
                        }
                    }
                });
            });
            if let Some((id, params)) = cmd {
                self.exec(id, params);
            }
        }

        group(ui, &tr!("cabinet_editor.ribbon.cabinet_group"), |ui| {
            if big_button(
                ui,
                Icon::Standard,
                &tr!("cabinet_editor.ribbon.properties"),
                true,
                false,
            )
            .on_hover_text(tr!("cabinet_editor.ribbon.properties_hint"))
            .clicked()
            {
                self.open_props(&focus);
            }
        });

        if self.cab_tab == CabTab::View3d {
            group(ui, &tr!("cabinet_editor.ribbon.camera_group"), |ui| {
                for (key, icon, label) in [
                    ("front", Icon::Elevation, tr!("view3d.view.front")),
                    ("top", Icon::Plan, tr!("view3d.view.top")),
                    ("iso", Icon::Cube, tr!("view3d.view.iso")),
                    ("default", Icon::Reload, tr!("view3d.view.default")),
                ] {
                    if big_button(ui, icon, &label, true, false).clicked() {
                        let cam = self.cab_camera;
                        self.cab_camera = match key {
                            "front" => cam.front(),
                            "top" => cam.top(),
                            "iso" => cam.iso(),
                            _ => cc_scene::Camera::default(),
                        };
                    }
                }
                column(ui, |ui| {
                    ui.checkbox(
                        &mut self.cab_camera.perspective,
                        tr!("cabinet_editor.ribbon.perspective"),
                    );
                    ui.checkbox(
                        &mut self.scene_opts.fronts,
                        tr!("cabinet_editor.ribbon.fronts"),
                    );
                });
            });
        }
    }

    /// Čelo skrinky s otvormi (sekciami) cez celú plochu; klik vyberie otvor, pravý klik
    /// ukáže ponuku delenia.
    fn cabinet_front(
        &mut self,
        ui: &mut egui::Ui,
        spec: &cc_model::CabinetSpec,
        model: Option<&cc_model::CabinetModel>,
        size: Vec2,
    ) {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::SHEET);
        let (w, h) = (spec.w.max(1.0), spec.h.max(1.0));
        let margin = 56.0;
        let s = ((rect.width() - 2.0 * margin) as f64 / w)
            .min((rect.height() - 2.0 * margin) as f64 / h)
            .max(0.01);
        let ox = rect.center().x - (w * s) as f32 / 2.0;
        let oy = rect.center().y + (h * s) as f32 / 2.0;
        let to = |x: f64, z: f64| egui::pos2(ox + (x * s) as f32, oy - (z * s) as f32);
        let r_mm = |x0: f64, z0: f64, x1: f64, z1: f64| Rect::from_two_pos(to(x0, z0), to(x1, z1));
        painter.rect_stroke(
            r_mm(0.0, 0.0, w, h),
            0.0,
            Stroke::new(1.5_f32, theme::TEXT),
            egui::StrokeKind::Inside,
        );
        // celkové kóty: šírka dole, výška vľavo, hĺbka šikmo vpravo hore (ako v CV)
        big_dimension(
            &painter,
            to(0.0, 0.0),
            to(w, 0.0),
            Vec2::new(0.0, 34.0),
            &fmt(w),
        );
        big_dimension(
            &painter,
            to(0.0, 0.0),
            to(0.0, h),
            Vec2::new(-34.0, 0.0),
            &fmt(h),
        );
        let corner = to(w, h);
        painter.line_segment(
            [corner, corner + Vec2::new(30.0, -30.0)],
            Stroke::new(1.0_f32, theme::TEXT),
        );
        painter.text(
            corner + Vec2::new(32.0, -32.0),
            Align2::LEFT_BOTTOM,
            fmt(spec.d),
            FontId::proportional(20.0),
            theme::TEXT,
        );
        let small = FontId::proportional(12.0);
        let Some(m) = model else {
            return;
        };
        let selected = self
            .wall_view
            .opening
            .clone()
            .filter(|(id, _, _)| *id == spec.id);
        let mut hit = None;
        let pointer = response.interact_pointer_pos();
        for o in &m.openings {
            let rr = r_mm(o.x, o.z, o.x + o.w, o.z + o.h);
            let kind = spec
                .fronts
                .get(o.row)
                .and_then(|f| match o.column {
                    Some(k) => f.columns.get(k),
                    None => Some(f),
                })
                .map_or_else(
                    || tr!("cabinet_editor.front.opening"),
                    |f| front_kind(f.kind),
                );
            let is_sel = selected
                .as_ref()
                .is_some_and(|(_, r, c)| *r == o.row && *c == o.column);
            painter.rect_filled(
                rr.shrink(1.0),
                0.0,
                if is_sel {
                    theme::SELECT_BG
                } else {
                    Color32::from_rgb(246, 244, 240)
                },
            );
            painter.rect_stroke(
                rr,
                0.0,
                Stroke::new(
                    if is_sel { 2.5_f32 } else { 1.0 },
                    if is_sel { OPENING } else { theme::GREY },
                ),
                egui::StrokeKind::Inside,
            );
            let ink = Color32::from_rgb(30, 90, 170);
            painter.text(
                rr.left_top() + Vec2::new(4.0, 3.0),
                Align2::LEFT_TOP,
                crate::wall_view::opening_name(o.row, o.column),
                small.clone(),
                ink,
            );
            painter.text(
                rr.center(),
                Align2::CENTER_CENTER,
                &kind,
                FontId::proportional(13.0),
                theme::TEXT,
            );
            painter.text(
                rr.right_center() - Vec2::new(4.0, 0.0),
                Align2::RIGHT_CENTER,
                fmt(o.h),
                small.clone(),
                ink,
            );
            painter.text(
                rr.center_bottom() - Vec2::new(0.0, 3.0),
                Align2::CENTER_BOTTOM,
                fmt(o.w),
                small.clone(),
                ink,
            );
            if pointer.is_some_and(|p| rr.contains(p)) {
                hit = Some((o.row, o.column));
            }
        }
        if response.clicked() || response.secondary_clicked() {
            self.wall_view.opening = hit.map(|(r, c)| (spec.id.clone(), r, c));
        }
        if m.openings.is_empty() {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                tr!("cabinet_editor.front.no_fronts"),
                FontId::proportional(13.0),
                theme::TEXT_WEAK,
            );
        }
        // pravý klik na otvor: ponuka delenia a zámkov ako v pohľade na stenu
        let mut cmds = Vec::new();
        if let Some((_, row, col)) = self
            .wall_view
            .opening
            .clone()
            .filter(|(id, _, _)| *id == spec.id)
        {
            response.context_menu(|ui| {
                crate::wall_view::opening_menu(ui, spec, &spec.id, row, col, &mut cmds);
            });
        }
        for (id, params) in cmds {
            self.exec(id, params);
        }
    }

    /// Diely skrinky ako zostava v CV (rovnaké diely spolu); klik vyberie diel, dvojklik
    /// otvorí výkres dielu.
    fn cabinet_parts(&mut self, ui: &mut egui::Ui, model: Option<&cc_model::CabinetModel>) {
        let Some(m) = model else {
            ui.label(tr!("cabinet_editor.cannot_build"));
            return;
        };
        // rovnaké diely (názov, rozmery, materiál) v jednom riadku
        let mut rows: Vec<(usize, String, f64, f64, f64, String, Vec<String>)> = Vec::new();
        for p in &m.parts {
            let name = role_name(p.role).to_string();
            let same = rows.iter_mut().find(|r| {
                r.1 == name
                    && (r.2 - p.width).abs() < 0.01
                    && (r.3 - p.length).abs() < 0.01
                    && (r.4 - p.thickness).abs() < 0.01
                    && r.5 == p.material
            });
            match same {
                Some(r) => {
                    r.0 += 1;
                    r.6.push(p.id.clone());
                }
                None => rows.push((
                    1,
                    name,
                    p.width,
                    p.length,
                    p.thickness,
                    p.material.clone(),
                    vec![p.id.clone()],
                )),
            }
        }
        let current = self.selected_part();
        let mut pick: Option<(String, bool)> = None;
        egui::ScrollArea::vertical()
            .id_salt("cabinet_parts")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("cab_parts_grid")
                    .striped(true)
                    .num_columns(7)
                    .spacing([18.0, 4.0])
                    .show(ui, |ui| {
                        for h in [
                            tr!("cabinet_editor.parts.qty"),
                            tr!("cabinet_editor.parts.name"),
                            tr!("cabinet_editor.parts.width"),
                            tr!("cabinet_editor.parts.length"),
                            tr!("cabinet_editor.parts.thickness"),
                            tr!("cabinet_editor.parts.material"),
                            tr!("cabinet_editor.parts.parts"),
                        ] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (qty, name, w, l, t, mat, ids) in &rows {
                            ui.label(qty.to_string());
                            let sel = current.as_ref().is_some_and(|c| ids.contains(c));
                            let r = ui.selectable_label(sel, name.as_str());
                            if let Some(first) = ids.first() {
                                if r.double_clicked() {
                                    pick = Some((first.clone(), true));
                                } else if r.clicked() {
                                    pick = Some((first.clone(), false));
                                }
                            }
                            ui.label(fmt(*w));
                            ui.label(fmt(*l));
                            ui.label(fmt(*t));
                            ui.label(mat);
                            ui.label(RichText::new(ids.join(", ")).color(theme::TEXT_WEAK));
                            ui.end_row();
                        }
                    });
            });
        if let Some((id, drawing)) = pick {
            if drawing {
                self.open_part_view(&id);
            } else {
                self.select_part(Some(id));
            }
        }
    }
}

/// Kóta ako v CV: čiara s lomkami na koncoch a veľké číslo v strede.
pub(crate) fn big_dimension(
    p: &egui::Painter,
    a: egui::Pos2,
    b: egui::Pos2,
    offset: Vec2,
    text: &str,
) {
    let stroke = Stroke::new(1.0_f32, theme::TEXT);
    let (a2, b2) = (a + offset, b + offset);
    let n = offset.normalized();
    p.line_segment([a + n * 3.0, a2 + n * 4.0], stroke);
    p.line_segment([b + n * 3.0, b2 + n * 4.0], stroke);
    let d = (b2 - a2).normalized();
    let slash = (d + Vec2::new(-d.y, d.x)) * 6.0;
    for q in [a2, b2] {
        p.line_segment([q - slash, q + slash], Stroke::new(1.5_f32, theme::TEXT));
    }
    let galley = p.layout_no_wrap(text.to_owned(), FontId::proportional(20.0), theme::TEXT);
    let size = galley.size();
    let mid = a2 + (b2 - a2) / 2.0;
    // biely podklad pod číslom preruší čiaru kóty
    let (pos, bg) = if d.x.abs() < 0.5 {
        let pos = mid - Vec2::new(size.x / 2.0, size.y / 2.0);
        (pos, Rect::from_min_size(pos, size).expand(2.0))
    } else {
        let pos = mid - size / 2.0;
        (pos, Rect::from_min_size(pos, size).expand(2.0))
    };
    p.line_segment([a2, b2], stroke);
    p.rect_filled(bg, 0.0, theme::SHEET);
    p.galley(pos, galley, theme::TEXT);
}

/// Pravouhlý pohľad na skrinku ako čiarový výkres (Čelo, Pôdorys, Bok v CV): obrysy dielov
/// bez čiel, diery (modré, kolíky oranžové) a drážky; vidno aj zakryté obrábanie.
/// Klik vyberie dosku, dvojklik ju otvorí: vráti (ID dielu, dvojklik).
fn cabinet_lines(
    ui: &mut egui::Ui,
    m: &cc_model::CabinetModel,
    view: CabTab,
    selected: Option<&str>,
) -> Option<(String, bool)> {
    // súradnice skrinky (x doprava, y od predku dozadu, z nahor) → rovina pohľadu (u, v nahor)
    let proj = |p: DVec3| -> (f64, f64) {
        match view {
            CabTab::Plan => (p.x, p.y),
            CabTab::End => (p.y, p.z),
            _ => (p.x, p.z),
        }
    };
    let parts: Vec<&cc_model::Part> = m.parts.iter().filter(|p| !p.role.is_front()).collect();
    // obálka dielov v rovine pohľadu
    let mut lo = (f64::INFINITY, f64::INFINITY);
    let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut boxes = Vec::new();
    for p in &parts {
        let mut blo = (f64::INFINITY, f64::INFINITY);
        let mut bhi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for i in 0..8 {
            let c = DVec3::new(
                if i & 1 != 0 { p.length } else { 0.0 },
                if i & 2 != 0 { p.width } else { 0.0 },
                if i & 4 != 0 { p.thickness } else { 0.0 },
            );
            let (u, v) = proj(p.transform.point_to_cabinet(c));
            blo = (blo.0.min(u), blo.1.min(v));
            bhi = (bhi.0.max(u), bhi.1.max(v));
        }
        lo = (lo.0.min(blo.0), lo.1.min(blo.1));
        hi = (hi.0.max(bhi.0), hi.1.max(bhi.1));
        boxes.push((blo, bhi, p.id.as_str()));
    }
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    let painter = ui.painter_at(rect);
    if boxes.is_empty() || !lo.0.is_finite() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr!("cabinet_editor.lines.no_parts"),
            FontId::proportional(13.0),
            theme::TEXT_WEAK,
        );
        return None;
    }
    let (ew, eh) = ((hi.0 - lo.0).max(1.0), (hi.1 - lo.1).max(1.0));
    let margin = 70.0;
    let s = ((rect.width() - 2.0 * margin) as f64 / ew)
        .min((rect.height() - 2.0 * margin) as f64 / eh)
        .max(0.01);
    let ox = rect.center().x - (ew * s) as f32 / 2.0;
    let oy = rect.center().y + (eh * s) as f32 / 2.0;
    let to =
        |u: f64, v: f64| egui::pos2(ox + ((u - lo.0) * s) as f32, oy - ((v - lo.1) * s) as f32);
    let outline = Stroke::new(1.0_f32, Color32::from_gray(40));
    for (blo, bhi, _) in &boxes {
        painter.rect_stroke(
            Rect::from_two_pos(to(blo.0, blo.1), to(bhi.0, bhi.1)),
            0.0,
            outline,
            egui::StrokeKind::Middle,
        );
    }
    // doska pod kurzorom: najmenšia obálka, ktorá obsahuje bod (tenké diely majú prednosť)
    let tol = 4.0 / s;
    let under = |p: egui::Pos2| {
        let u = lo.0 + f64::from(p.x - ox) / s;
        let v = lo.1 + f64::from(oy - p.y) / s;
        boxes
            .iter()
            .filter(|(a, b, _)| {
                u >= a.0 - tol && u <= b.0 + tol && v >= a.1 - tol && v <= b.1 + tol
            })
            .min_by(|x, y| {
                let area = |(a, b, _): &&((f64, f64), (f64, f64), &str)| (b.0 - a.0) * (b.1 - a.1);
                area(x).total_cmp(&area(y))
            })
            .map(|b| b.2.to_owned())
    };
    let hovered = response.hover_pos().and_then(under);
    for (blo, bhi, id) in &boxes {
        let sel = selected == Some(*id);
        if sel || hovered.as_deref() == Some(*id) {
            painter.rect_stroke(
                Rect::from_two_pos(to(blo.0, blo.1), to(bhi.0, bhi.1)),
                0.0,
                Stroke::new(
                    if sel { 2.5_f32 } else { 1.5 },
                    if sel {
                        theme::SELECT_STROKE
                    } else {
                        theme::GOLD
                    },
                ),
                egui::StrokeKind::Middle,
            );
        }
    }
    // obrábanie: diera kolmo na pohľad = krúžok, inak pás v smere vŕtania
    let hole_blue = Color32::from_rgb(70, 160, 230);
    let dowel = Color32::from_rgb(190, 110, 20);
    let groove = Color32::from_rgb(150, 100, 50);
    for p in &parts {
        for op in &p.operations {
            match *op {
                cc_model::Operation::Hole {
                    face,
                    x,
                    y,
                    z,
                    diameter,
                    depth,
                    dowel: is_dowel,
                    ..
                } => {
                    let n = face_normal(face);
                    let entry = p.transform.point_to_cabinet(DVec3::new(x, y, z));
                    let axis = -p.transform.dir_to_cabinet(n);
                    let color = if is_dowel { dowel } else { hole_blue };
                    let (u0, v0) = proj(entry);
                    let (u1, v1) = proj(entry + axis * depth);
                    let along = ((u1 - u0).powi(2) + (v1 - v0).powi(2)).sqrt();
                    if along < 0.5 {
                        painter.circle_stroke(
                            to(u0, v0),
                            ((diameter / 2.0) * s).max(1.5) as f32,
                            Stroke::new(1.2_f32, color),
                        );
                    } else {
                        let w = ((diameter * s) as f32).clamp(1.5, 8.0);
                        painter.line_segment([to(u0, v0), to(u1, v1)], Stroke::new(w, color));
                    }
                }
                cc_model::Operation::Groove {
                    face,
                    start,
                    end,
                    width,
                    ..
                } => {
                    let z = if face == cc_model::Face::B {
                        0.0
                    } else {
                        p.thickness
                    };
                    let a = p
                        .transform
                        .point_to_cabinet(DVec3::new(start.x, start.y, z));
                    let b = p.transform.point_to_cabinet(DVec3::new(end.x, end.y, z));
                    let (ua, va) = proj(a);
                    let (ub, vb) = proj(b);
                    let w = ((width * s) as f32).clamp(1.0, 6.0);
                    painter.line_segment(
                        [to(ua, va), to(ub, vb)],
                        Stroke::new(w, groove.gamma_multiply(0.7)),
                    );
                }
            }
        }
    }
    big_dimension(
        &painter,
        to(lo.0, lo.1),
        to(hi.0, lo.1),
        Vec2::new(0.0, 40.0),
        &fmt(ew),
    );
    big_dimension(
        &painter,
        to(lo.0, lo.1),
        to(lo.0, hi.1),
        Vec2::new(-40.0, 0.0),
        &fmt(eh),
    );
    let response = response.on_hover_text(match &hovered {
        Some(id) => tr!("cabinet_editor.lines.hover_part", id = id),
        None => tr!("cabinet_editor.lines.hover"),
    });
    let pos = response.interact_pointer_pos()?;
    if response.double_clicked() {
        under(pos).map(|id| (id, true))
    } else if response.clicked() {
        under(pos).map(|id| (id, false))
    } else {
        None
    }
}

/// Normála plochy dielu v jeho súradniciach (plocha F je na z = hrúbka).
fn face_normal(face: cc_model::Face) -> DVec3 {
    match face {
        cc_model::Face::F => DVec3::Z,
        cc_model::Face::B => DVec3::NEG_Z,
        cc_model::Face::X0 => DVec3::NEG_X,
        cc_model::Face::X1 => DVec3::X,
        cc_model::Face::Y0 => DVec3::NEG_Y,
        cc_model::Face::Y1 => DVec3::Y,
    }
}

/// Malý pôdorys skrinky v ľavom paneli: obrys a červená predná hrana (strana rezu).
fn mini_plan(ui: &mut egui::Ui, w: f64, d: f64) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 130.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::SHEET);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let (w, d) = (w.max(1.0), d.max(1.0));
    let s = ((rect.width() - 60.0) as f64 / w).min((rect.height() - 40.0) as f64 / d);
    let size = Vec2::new((w * s) as f32, (d * s) as f32);
    let r = Rect::from_center_size(rect.center(), size);
    let dashed = Stroke::new(1.0_f32, theme::TEXT);
    for (a, b) in [
        (r.left_top(), r.right_top()),
        (r.left_top(), r.left_bottom()),
        (r.right_top(), r.right_bottom()),
    ] {
        painter.add(egui::Shape::dashed_line(&[a, b], dashed, 3.0, 3.0));
    }
    painter.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(2.0_f32, theme::RED),
    );
    for p in [
        r.left_top(),
        r.right_top(),
        r.left_bottom(),
        r.right_bottom(),
    ] {
        painter.circle(p, 3.0, theme::GOLD, Stroke::new(1.0_f32, theme::GOLD_DARK));
    }
}

fn front_kind(k: cc_model::FrontKind) -> String {
    match k {
        cc_model::FrontKind::Door => tr!("cabinet_editor.front.door"),
        cc_model::FrontKind::Pair => tr!("cabinet_editor.front.pair"),
        cc_model::FrontKind::Drawer => tr!("cabinet_editor.front.drawer"),
        cc_model::FrontKind::FalseFront => tr!("cabinet_editor.front.false_front"),
        cc_model::FrontKind::Panel => tr!("cabinet_editor.front.panel"),
        cc_model::FrontKind::Open => tr!("cabinet_editor.front.open"),
    }
}

#[cfg(test)]
mod tests {
    use super::CAB_TABS;

    /// Kľúče textov záložiek sa skladajú za behu: každý musí byť v sk aj en.
    #[test]
    fn every_tab_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for (_, _, key, _) in CAB_TABS {
                assert!(catalog.contains(key), "{code}.toml: chýba {key}");
            }
        }
    }
}
