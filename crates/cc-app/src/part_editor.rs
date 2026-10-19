//! Jedna doska samostatne ako v CV (dvojklik na dosku v editore skrinky): záložky CAM
//! (pracovná rovina a obrábanie na nej), Čelo (plocha F s kótami polôh dier), Hrana
//! (pohľad na hranu Y1), Koniec (pohľad na koniec X1), 3D a Zostavy (operácie dielu).
//! Ľavý panel a pás sa menia podľa záložky; Návrat vráti editor skrinky. Rozloženie
//! zodpovedá CV, texty a ikony sú vlastné. Obrábanie sa tu needituje (vzniká zo štandardu).

use cc_i18n::tr;
use cc_model::{Face, Operation, Part};
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Vec2};
use glam::DVec3;

use crate::app::{App, Tab};
use crate::cabinet_editor::big_dimension;
use crate::icons::Icon;
use crate::part_view::{face_name, fmt};
use crate::side_panel::{prop_row, task_row};
use crate::tabs::role_name;
use crate::theme;

/// Záložka otvorenej dosky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PartTab {
    Cam,
    #[default]
    Front,
    Top,
    End,
    View3d,
    Reports,
}

/// Záložky dosky: záložka, ikona, kľúč textu (`cc_i18n::tr`), kľúč pre riadiaci kanál.
pub(crate) const PART_TABS: [(PartTab, Icon, &str, &str); 6] = [
    (PartTab::Cam, Icon::Cad, "part_editor.tab.cam", "cam"),
    (
        PartTab::Front,
        Icon::Drawing,
        "part_editor.tab.face",
        "front",
    ),
    (PartTab::Top, Icon::Plan, "part_editor.tab.edge", "top"),
    (
        PartTab::End,
        Icon::CabinetTall,
        "part_editor.tab.end",
        "end",
    ),
    (PartTab::View3d, Icon::Cube, "part_editor.tab.view3d", "3d"),
    (
        PartTab::Reports,
        Icon::Parts,
        "part_editor.tab.reports",
        "reports",
    ),
];

impl PartTab {
    pub(crate) fn key(self) -> &'static str {
        PART_TABS
            .iter()
            .find(|t| t.0 == self)
            .map_or("front", |t| t.3)
    }

    pub(crate) fn from_key(key: &str) -> Option<PartTab> {
        PART_TABS.iter().find(|t| t.3 == key).map(|t| t.0)
    }
}

/// Pracovné roviny v poradí zoznamu (ako v CV).
const PLANES: [Face; 6] = [Face::F, Face::B, Face::X0, Face::X1, Face::Y1, Face::Y0];

/// Názov pracovnej roviny v zozname (kód plochy ostáva v zátvorke).
fn plane_label(face: Face) -> String {
    match face {
        Face::F => tr!("part_editor.plane.f"),
        Face::B => tr!("part_editor.plane.b"),
        Face::X0 => tr!("part_editor.plane.x0"),
        Face::X1 => tr!("part_editor.plane.x1"),
        Face::Y1 => tr!("part_editor.plane.y1"),
        Face::Y0 => tr!("part_editor.plane.y0"),
    }
}

impl App {
    /// Otvorí jednu dosku samostatne; Návrat vráti záložku, z ktorej sa otvorila.
    pub(crate) fn open_part_view(&mut self, id: &str) {
        self.select_part(Some(id.to_owned()));
        if self.tab != Tab::PartEdit {
            self.part_back = Some(self.tab);
        }
        self.tab = Tab::PartEdit;
    }

    /// Otvorená doska (vybraný diel) v aktuálnom modeli.
    fn open_part(&self) -> Option<Part> {
        let id = self.selected_part()?;
        let built = self.model_built()?;
        let part = built.model.parts().find(|p| p.id == id).cloned();
        part
    }

    pub(crate) fn tab_part_edit(&mut self, ui: &mut egui::Ui) {
        let Some(part) = self.open_part() else {
            ui.label(tr!("part_editor.no_part"));
            return;
        };
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(tr!(
                    "part_editor.title",
                    id = &part.id,
                    name = role_name(part.role).to_string(),
                    length = fmt(part.length),
                    width = fmt(part.width),
                    thickness = fmt(part.thickness),
                    material = &part.material
                ))
                .size(theme::SMALL + 0.5),
            );
        });
        match self.part_tab {
            PartTab::Cam => part_lines(ui, &part, self.part_plane, false),
            PartTab::Front => part_lines(ui, &part, Face::F, true),
            PartTab::Top => part_lines(ui, &part, Face::Y1, true),
            PartTab::End => part_lines(ui, &part, Face::X1, true),
            PartTab::View3d => {
                let cam = self.part_camera;
                self.part_render(ui, &part.id, cam);
            }
            PartTab::Reports => self.part_operations(ui, &part),
        }
    }

    /// Ľavý panel otvorenej dosky.
    pub(crate) fn part_panel(&mut self, ui: &mut egui::Ui) {
        let Some(part) = self.open_part() else {
            return;
        };
        match self.part_tab {
            PartTab::Cam => {
                plane_box(ui, self.part_plane);
                ui.label(RichText::new(tr!("part_editor.panel.plane")).size(theme::SMALL + 0.5));
                egui::Frame::new()
                    .fill(theme::SHEET)
                    .stroke(Stroke::new(1.0_f32, theme::BORDER))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for face in PLANES {
                            let label = plane_label(face);
                            let n = part
                                .operations
                                .iter()
                                .filter(|op| op_face(op) == face)
                                .count();
                            let text = if n > 0 {
                                format!("{label}   · {n}")
                            } else {
                                label
                            };
                            if ui.selectable_label(self.part_plane == face, text).clicked() {
                                self.part_plane = face;
                            }
                        }
                    });
            }
            PartTab::Front | PartTab::Top | PartTab::End => {
                for (icon, label) in [
                    (Icon::Cad, tr!("part_editor.panel.edit_shape")),
                    (Icon::Line, tr!("part_editor.panel.edge_joint")),
                    (Icon::Molding, tr!("part_editor.panel.edge_banding")),
                    (Icon::Rectangle, tr!("part_editor.panel.split")),
                    (Icon::Cad, tr!("part_editor.panel.cad")),
                ] {
                    task_row(ui, icon, &label, false, false)
                        .on_hover_text(tr!("part_editor.not_available"));
                }
            }
            PartTab::View3d => {
                if task_row(
                    ui,
                    Icon::Cube,
                    &tr!("part_editor.panel.camera"),
                    true,
                    false,
                )
                .on_hover_text(tr!("part_editor.panel.camera_hint"))
                .clicked()
                {
                    self.part_camera = cc_scene::Camera::default();
                }
                task_row(
                    ui,
                    Icon::Standard,
                    &tr!("part_editor.panel.lights"),
                    false,
                    false,
                )
                .on_hover_text(tr!("part_editor.not_available"));
                task_row(ui, Icon::Cad, &tr!("part_editor.panel.cad"), false, false)
                    .on_hover_text(tr!("part_editor.not_available"));
            }
            PartTab::Reports => {
                task_row(
                    ui,
                    Icon::New,
                    &tr!("part_editor.panel.new_operation"),
                    false,
                    false,
                )
                .on_hover_text(tr!("part_editor.panel.new_operation_hint"));
            }
        }
        ui.add_space(6.0);
        if self.section(ui, &tr!("part_editor.props.section")) {
            let edges = |e: &Option<String>| e.clone().unwrap_or_else(|| "–".to_owned());
            for (label, value) in [
                (tr!("part_editor.props.id"), part.id.clone()),
                (
                    tr!("part_editor.props.name"),
                    role_name(part.role).to_string(),
                ),
                (tr!("part_editor.props.cabinet"), part.cabinet.clone()),
                (tr!("part_editor.props.length"), fmt(part.length)),
                (tr!("part_editor.props.width"), fmt(part.width)),
                (tr!("part_editor.props.thickness"), fmt(part.thickness)),
                (tr!("part_editor.props.material"), part.material.clone()),
                (tr!("part_editor.props.edge_x0"), edges(&part.edges.x0)),
                (tr!("part_editor.props.edge_x1"), edges(&part.edges.x1)),
                (tr!("part_editor.props.edge_y0"), edges(&part.edges.y0)),
                (tr!("part_editor.props.edge_y1"), edges(&part.edges.y1)),
                (
                    tr!("part_editor.props.operations"),
                    part.operations.len().to_string(),
                ),
                (
                    tr!("part_editor.props.program"),
                    self.programs
                        .get(&part.id)
                        .cloned()
                        .unwrap_or_else(|| "–".to_owned()),
                ),
            ] {
                prop_row(ui, &label, false, |ui| {
                    ui.label(value);
                });
            }
        }
    }

    /// Pás otvorenej dosky (po skupinách Domov a Úpravy).
    pub(crate) fn ribbon_part(&mut self, ui: &mut egui::Ui) {
        use crate::ribbon::{big_button, column, group, small_button};
        let Some(part) = self.open_part() else {
            return;
        };
        // ostatné dosky tej istej skrinky: ◀ ▶ ako výber skrinky v editore
        let siblings: Vec<(String, String)> = self
            .model_built()
            .and_then(|b| {
                b.model
                    .cabinets
                    .iter()
                    .find(|c| c.id == part.cabinet)
                    .map(|c| {
                        c.parts
                            .iter()
                            .map(|p| (p.id.clone(), role_name(p.role).to_string()))
                            .collect()
                    })
            })
            .unwrap_or_default();
        let index = siblings.iter().position(|p| p.0 == part.id);
        let mut open: Option<String> = None;
        group(ui, &tr!("part_editor.ribbon.view_group"), |ui| {
            column(ui, |ui| {
                ui.horizontal(|ui| {
                    let prev = index.and_then(|i| i.checked_sub(1));
                    let next = index.map(|i| i + 1).filter(|&i| i < siblings.len());
                    if ui
                        .add_enabled(prev.is_some(), egui::Button::new("◀").small())
                        .clicked()
                    {
                        open = prev.and_then(|i| siblings.get(i)).map(|p| p.0.clone());
                    }
                    egui::ComboBox::from_id_salt("part_editor_pick")
                        .selected_text(&part.id)
                        .width(150.0)
                        .show_ui(ui, |ui| {
                            for (id, name) in &siblings {
                                if ui
                                    .selectable_label(*id == part.id, format!("{id}  {name}"))
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
                        open = next.and_then(|i| siblings.get(i)).map(|p| p.0.clone());
                    }
                });
            });
        });
        if let Some(id) = open {
            self.select_part(Some(id));
        }
        if self.part_tab == PartTab::Cam {
            group(ui, &tr!("part_editor.ribbon.machining_group"), |ui| {
                column(ui, |ui| {
                    for (icon, label) in [
                        (Icon::Parts, tr!("part_editor.ribbon.hole_row")),
                        (Icon::Standard, tr!("part_editor.ribbon.hole")),
                        (Icon::Line, tr!("part_editor.ribbon.groove")),
                    ] {
                        small_button(ui, Some(icon), &label, false, false)
                            .on_hover_text(tr!("part_editor.ribbon.machining_hint"));
                    }
                });
            });
        }
        if self.part_tab == PartTab::View3d {
            group(ui, &tr!("part_editor.ribbon.camera_group"), |ui| {
                for (key, icon, label) in [
                    ("front", Icon::Elevation, tr!("view3d.view.front")),
                    ("top", Icon::Plan, tr!("view3d.view.top")),
                    ("iso", Icon::Cube, tr!("view3d.view.iso")),
                    ("default", Icon::Reload, tr!("view3d.view.default")),
                ] {
                    if big_button(ui, icon, &label, true, false).clicked() {
                        let cam = self.part_camera;
                        self.part_camera = match key {
                            "front" => cam.front(),
                            "top" => cam.top(),
                            "iso" => cam.iso(),
                            _ => cc_scene::Camera::default(),
                        };
                    }
                }
            });
        }
        group(ui, &tr!("part_editor.ribbon.part_group"), |ui| {
            if big_button(
                ui,
                Icon::Drawing,
                &tr!("part_editor.ribbon.print_drawing"),
                true,
                false,
            )
            .on_hover_text(tr!("part_editor.ribbon.print_drawing_hint"))
            .clicked()
            {
                self.print_drawings(Some(vec![format!("part:{}", part.id)]), false);
            }
        });
    }

    /// Operácie dosky ako zostava v CV a pod nimi program pre stroj.
    fn part_operations(&mut self, ui: &mut egui::Ui, part: &Part) {
        egui::ScrollArea::vertical()
            .id_salt("part_ops")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("part_ops_grid")
                    .striped(true)
                    .num_columns(9)
                    .spacing([16.0, 4.0])
                    .show(ui, |ui| {
                        for h in [
                            "#".to_owned(),
                            tr!("part_editor.ops.operation"),
                            tr!("part_editor.ops.face"),
                            "X".to_owned(),
                            "Y".to_owned(),
                            "Z".to_owned(),
                            tr!("part_editor.ops.diameter_width"),
                            tr!("part_editor.ops.length"),
                            tr!("part_editor.ops.depth"),
                        ] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (i, op) in part.operations.iter().enumerate() {
                            ui.label((i + 1).to_string());
                            match *op {
                                Operation::Hole {
                                    face,
                                    x,
                                    y,
                                    z,
                                    diameter,
                                    depth,
                                    through,
                                    dowel,
                                } => {
                                    ui.label(if dowel {
                                        tr!("part_editor.ops.dowel")
                                    } else if through {
                                        tr!("part_editor.ops.through_hole")
                                    } else {
                                        tr!("part_editor.ops.hole")
                                    });
                                    ui.label(face_name(face));
                                    ui.label(fmt(x));
                                    ui.label(fmt(y));
                                    ui.label(fmt(z));
                                    ui.label(format!("Ø{}", fmt(diameter)));
                                    ui.label("");
                                    ui.label(fmt(depth));
                                }
                                Operation::Groove {
                                    face,
                                    start,
                                    end,
                                    width,
                                    depth,
                                } => {
                                    ui.label(tr!("part_editor.ops.groove"));
                                    ui.label(face_name(face));
                                    ui.label(fmt(start.x));
                                    ui.label(fmt(start.y));
                                    ui.label("");
                                    ui.label(fmt(width));
                                    let len = ((end.x - start.x).powi(2)
                                        + (end.y - start.y).powi(2))
                                    .sqrt();
                                    ui.label(fmt(len));
                                    ui.label(fmt(depth));
                                }
                            }
                            ui.end_row();
                        }
                    });
                if part.operations.is_empty() {
                    ui.label(
                        RichText::new(tr!("part_editor.ops.no_machining")).color(theme::TEXT_WEAK),
                    );
                }
            });
    }
}

/// Plocha, na ktorej operácia začína.
fn op_face(op: &Operation) -> Face {
    match *op {
        Operation::Hole { face, .. } | Operation::Groove { face, .. } => face,
    }
}

/// Normála plochy dielu v jeho súradniciach (plocha F je na z = hrúbka).
fn face_normal(face: Face) -> DVec3 {
    match face {
        Face::F => DVec3::Z,
        Face::B => DVec3::NEG_Z,
        Face::X0 => DVec3::NEG_X,
        Face::X1 => DVec3::X,
        Face::Y0 => DVec3::NEG_Y,
        Face::Y1 => DVec3::Y,
    }
}

/// Doska pri pohľade na plochu `plane` ako čiarový výkres: obrys, olepené hrany, diery
/// (krúžok kolmo na pohľad, inak pás v smere vŕtania), drážky; pri `dims` celkové kóty
/// a reťaze polôh dier (ako Front v CV).
fn part_lines(ui: &mut egui::Ui, part: &Part, plane: Face, dims: bool) {
    let (l, w, t) = (part.length, part.width, part.thickness);
    // časť → rovina pohľadu (u doprava, v nahor) a rozmer obrysu
    let proj = |p: DVec3| -> (f64, f64) {
        match plane {
            Face::F => (p.x, p.y),
            Face::B => (l - p.x, p.y),
            Face::Y0 => (p.x, p.z),
            Face::Y1 => (l - p.x, p.z),
            Face::X0 => (p.y, p.z),
            // koniec dosky: hrúbka vodorovne, šírka zvislo (dlhý rozmer nastojato ako v CV)
            Face::X1 => (p.z, p.y),
        }
    };
    let (ew, eh) = match plane {
        Face::F | Face::B => (l, w),
        Face::Y0 | Face::Y1 => (l, t),
        Face::X0 => (w, t),
        Face::X1 => (t, w),
    };
    let (ew, eh) = (ew.max(1.0), eh.max(1.0));
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    let margin = if dims { 90.0 } else { 50.0 };
    let s = ((rect.width() - 2.0 * margin) as f64 / ew)
        .min((rect.height() - 2.0 * margin) as f64 / eh)
        .max(0.01);
    let ox = rect.center().x - (ew * s) as f32 / 2.0;
    let oy = rect.center().y + (eh * s) as f32 / 2.0;
    let to = |u: f64, v: f64| egui::pos2(ox + (u * s) as f32, oy - (v * s) as f32);
    let outline = Rect::from_two_pos(to(0.0, 0.0), to(ew, eh));
    painter.rect_filled(outline, 0.0, Color32::from_rgb(252, 250, 246));
    painter.rect_stroke(
        outline,
        0.0,
        Stroke::new(1.2_f32, Color32::from_gray(40)),
        egui::StrokeKind::Middle,
    );
    // olepené hrany (oranžovo) pri pohľade na plochu F alebo B
    if matches!(plane, Face::F | Face::B) {
        let band = Stroke::new(2.5_f32, Color32::from_rgb(205, 130, 50));
        for (on, a, b) in [
            (part.edges.x0.is_some(), (0.0, 0.0, 0.0), (0.0, w, 0.0)),
            (part.edges.x1.is_some(), (l, 0.0, 0.0), (l, w, 0.0)),
            (part.edges.y0.is_some(), (0.0, 0.0, 0.0), (l, 0.0, 0.0)),
            (part.edges.y1.is_some(), (0.0, w, 0.0), (l, w, 0.0)),
        ] {
            if on {
                let (ua, va) = proj(DVec3::new(a.0, a.1, a.2));
                let (ub, vb) = proj(DVec3::new(b.0, b.1, b.2));
                painter.line_segment([to(ua, va), to(ub, vb)], band);
            }
        }
    }
    let hole_blue = Color32::from_rgb(40, 40, 46);
    let dowel = Color32::from_rgb(190, 110, 20);
    let groove = Color32::from_rgb(150, 100, 50);
    let mut us: Vec<f64> = vec![0.0, ew];
    let mut vs: Vec<f64> = vec![0.0, eh];
    for op in &part.operations {
        match *op {
            Operation::Hole {
                face,
                x,
                y,
                z,
                diameter,
                depth,
                dowel: is_dowel,
                ..
            } => {
                let entry = DVec3::new(x, y, z);
                let axis = -face_normal(face);
                let (u0, v0) = proj(entry);
                let (u1, v1) = proj(entry + axis * depth);
                let color = if is_dowel { dowel } else { hole_blue };
                let along = ((u1 - u0).powi(2) + (v1 - v0).powi(2)).sqrt();
                if along < 0.5 {
                    painter.circle_stroke(
                        to(u0, v0),
                        ((diameter / 2.0) * s).max(2.0) as f32,
                        Stroke::new(1.5_f32, color),
                    );
                    us.push(u0);
                    vs.push(v0);
                } else {
                    let wpx = ((diameter * s) as f32).clamp(2.0, 10.0);
                    painter.line_segment([to(u0, v0), to(u1, v1)], Stroke::new(wpx, color));
                    if (u1 - u0).abs() < 0.5 {
                        us.push(u0);
                    } else {
                        vs.push(v0);
                    }
                }
            }
            Operation::Groove {
                face,
                start,
                end,
                width,
                ..
            } => {
                let z = if face == Face::B { 0.0 } else { t };
                let (ua, va) = proj(DVec3::new(start.x, start.y, z));
                let (ub, vb) = proj(DVec3::new(end.x, end.y, z));
                let wpx = ((width * s) as f32).clamp(1.5, 12.0);
                painter.line_segment(
                    [to(ua, va), to(ub, vb)],
                    Stroke::new(wpx, groove.gamma_multiply(0.6)),
                );
            }
        }
    }
    if !dims {
        return;
    }
    // celkové kóty: šírka dole, výška vľavo
    big_dimension(
        &painter,
        to(0.0, 0.0),
        to(ew, 0.0),
        Vec2::new(0.0, 58.0),
        &fmt(ew),
    );
    big_dimension(
        &painter,
        to(0.0, 0.0),
        to(0.0, eh),
        Vec2::new(-58.0, 0.0),
        &fmt(eh),
    );
    // reťaze polôh dier: hore (vodorovne) a vpravo (zvislo), ako v CV
    let chain = |mut vals: Vec<f64>, max: f64| -> Vec<f64> {
        vals.retain(|v| v.is_finite() && *v >= -0.01 && *v <= max + 0.01);
        vals.sort_by(f64::total_cmp);
        vals.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        vals
    };
    let small = FontId::proportional(12.0);
    let ink = Color32::from_rgb(20, 60, 140);
    let stroke = Stroke::new(1.0_f32, ink);
    let us = chain(us, ew);
    if us.len() > 2 {
        let y = to(0.0, eh).y - 26.0;
        painter.line_segment(
            [egui::pos2(to(0.0, 0.0).x, y), egui::pos2(to(ew, 0.0).x, y)],
            stroke,
        );
        for pair in us.windows(2) {
            let (a, b) = (to(pair[0], eh).x, to(pair[1], eh).x);
            painter.line_segment([egui::pos2(a, y - 4.0), egui::pos2(a, y + 6.0)], stroke);
            painter.text(
                egui::pos2((a + b) / 2.0, y - 3.0),
                Align2::CENTER_BOTTOM,
                fmt(pair[1] - pair[0]),
                small.clone(),
                ink,
            );
        }
        let end = to(ew, eh).x;
        painter.line_segment([egui::pos2(end, y - 4.0), egui::pos2(end, y + 6.0)], stroke);
    }
    let vs = chain(vs, eh);
    if vs.len() > 2 {
        let x = to(ew, 0.0).x + 26.0;
        painter.line_segment(
            [egui::pos2(x, to(0.0, 0.0).y), egui::pos2(x, to(0.0, eh).y)],
            stroke,
        );
        for pair in vs.windows(2) {
            let (a, b) = (to(0.0, pair[0]).y, to(0.0, pair[1]).y);
            painter.line_segment([egui::pos2(x - 6.0, a), egui::pos2(x + 4.0, a)], stroke);
            painter.text(
                egui::pos2(x + 4.0, (a + b) / 2.0),
                Align2::LEFT_CENTER,
                fmt(pair[1] - pair[0]),
                small.clone(),
                ink,
            );
        }
        let top = to(0.0, eh).y;
        painter.line_segment([egui::pos2(x - 6.0, top), egui::pos2(x + 4.0, top)], stroke);
    }
}

/// Malý kváder dosky v ľavom paneli s vyznačenou pracovnou rovinou (ako v CV).
fn plane_box(ui: &mut egui::Ui, plane: Face) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(200, 200, 200));
    // izometrický kváder: predná stena (F) a hĺbka doprava hore
    let c = rect.center();
    let (w, h, d) = (70.0, 100.0, Vec2::new(40.0, -28.0));
    let f0 = egui::pos2(c.x - w / 2.0 - 15.0, c.y + h / 2.0 + 8.0);
    let corners = |o: egui::Pos2| {
        [
            o,
            o + Vec2::new(w, 0.0),
            o + Vec2::new(w, -h),
            o + Vec2::new(0.0, -h),
        ]
    };
    let front = corners(f0);
    let back = corners(f0 + d);
    let line = Stroke::new(1.0_f32, Color32::from_gray(90));
    for i in 0..4 {
        painter.line_segment([front[i], front[(i + 1) % 4]], line);
        painter.line_segment([back[i], back[(i + 1) % 4]], line);
        painter.line_segment([front[i], back[i]], line);
    }
    let face: Vec<egui::Pos2> = match plane {
        Face::F => front.to_vec(),
        Face::B => back.to_vec(),
        Face::X0 => vec![front[0], front[3], back[3], back[0]],
        Face::X1 => vec![front[1], front[2], back[2], back[1]],
        Face::Y1 => vec![front[3], front[2], back[2], back[3]],
        Face::Y0 => vec![front[0], front[1], back[1], back[0]],
    };
    painter.add(egui::Shape::convex_polygon(
        face,
        Color32::from_rgba_unmultiplied(40, 180, 60, 90),
        Stroke::new(2.0_f32, Color32::from_rgb(30, 160, 40)),
    ));
}

#[cfg(test)]
mod tests {
    use super::PART_TABS;

    /// Kľúče textov záložiek sa skladajú za behu: každý musí byť v sk aj en.
    #[test]
    fn every_tab_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for (_, _, key, _) in PART_TABS {
                assert!(catalog.contains(key), "{code}.toml: chýba {key}");
            }
        }
    }
}
