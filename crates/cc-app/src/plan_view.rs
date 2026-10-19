//! Pôdorys miestnosti (kap. 13): steny, otvory, skrinky, kóty; kreslenie stien,
//! posúvanie skriniek pozdĺž steny, okná a dvere, vyplnenie steny skrinkami.

use cc_i18n::tr;
use cc_model::{CabinetType, OpeningKind, ProjectFile, RoomSpec, WallSpec};
use cc_room::{Room, Row};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Sense, Shape, Stroke, Vec2};
use glam::DVec2;

use crate::canvas2d::{self, View2d, Xf};
use crate::part_view::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    /// Steny čiarou (reťaz).
    DrawWall,
    /// Obdĺžniková miestnosť ťahaním rámu.
    RectRoom,
    /// Výrez okna (zoom rámom).
    ZoomWindow,
}

/// Stav pôdorysu medzi snímkami.
pub struct PlanState {
    pub view: View2d,
    pub tool: Tool,
    /// začiatok rozkreslenej steny
    drawing: Option<DVec2>,
    /// posúvaná skrinka: (index, posun uchopenia od ľavej hrany pozdĺž steny)
    drag: Option<(usize, f64)>,
    pub selected_wall: Option<String>,
    /// Položka katalógu pustená do pôdorysu: `cabinet.add` pre aplikáciu.
    pub dropped: Option<serde_json::Value>,
    /// Roh pod pravým klikom: stena, na ktorej konci je.
    corner: Option<String>,
    /// Príkazy z ponuky rohu pre aplikáciu.
    pub commands: Vec<(&'static str, serde_json::Value)>,
    /// Dvojklik na skrinku: ID pre editor skrinky.
    pub open_cabinet: Option<String>,
}

impl PlanState {
    /// Prepne nástroj; rozkreslená stena alebo rám sa zahodí.
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.drawing = None;
    }
}

impl Default for PlanState {
    fn default() -> Self {
        PlanState {
            view: View2d::default(),
            tool: Tool::Select,
            drawing: None,
            drag: None,
            selected_wall: None,
            dropped: None,
            corner: None,
            commands: Vec::new(),
            open_cabinet: None,
        }
    }
}

const BASE_FILL: Color32 = Color32::from_rgb(236, 228, 214);
const TALL_FILL: Color32 = Color32::from_rgb(214, 204, 188);
const UPPER_FILL: Color32 = Color32::from_rgba_premultiplied(70, 110, 170, 40);
const WALL_FILL: Color32 = Color32::from_rgb(150, 150, 155);
const SELECT: Color32 = Color32::from_rgb(235, 120, 20);
const DIM: Color32 = Color32::from_rgb(60, 90, 140);

/// Nakreslí pôdorys a spracuje prácu myšou. Vráti `true`, ak sa projekt zmenil.
pub fn show(
    ui: &mut egui::Ui,
    st: &mut PlanState,
    project: &mut ProjectFile,
    selected_cabinet: &mut usize,
) -> bool {
    let mut changed = false;
    changed |= notices(ui, st, project, selected_cabinet);

    let room = Room::new(project);
    let layout = cc_room::layout(project);
    let size = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let bounds = scene_bounds(&room, project, &layout);
    let xf = st.view.interact(ui, &response, bounds);
    let painter = ui.painter_at(rect);
    // čisté biele plátno bez mriežky (ako v CV); os x/y vľavo dole
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    draw_axes(&painter, rect);

    draw_walls(&painter, &xf, &room, project, st.selected_wall.as_deref());
    draw_cabinets(&painter, &xf, &room, project, &layout, *selected_cabinet);
    draw_dimensions(&painter, &xf, &room, project);

    let pointer = response.hover_pos().map(|p| to_d(xf.to_mm(p)));
    match st.tool {
        Tool::Select => {
            changed |= select_tool(
                &response,
                st,
                project,
                &room,
                &xf,
                selected_cabinet,
                pointer,
            );
        }
        Tool::DrawWall => {
            changed |= draw_tool(ui, &painter, &response, st, project, &room, &xf, pointer);
        }
        Tool::RectRoom | Tool::ZoomWindow => {
            let zoom = st.tool == Tool::ZoomWindow;
            changed |= rect_tool(ui, &painter, &response, st, project, &xf, pointer, zoom);
        }
    }
    // položka katalógu ťahaná do pôdorysu: prichytí sa k najbližšej stene (ako v CV),
    // stred pod kurzorom; počas ťahania duch pôdorysu, prekrytie so susedom rieši cabinet.add
    let nearest_wall = |p: DVec2| {
        room.walls
            .iter()
            .map(|w| {
                let t = (p - w.start).dot(w.dir).clamp(0.0, w.length);
                (w, t, (w.start + w.dir * t).distance(p))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
    };
    if let (Some(item), Some(p)) = (
        response.dnd_hover_payload::<crate::catalog_panel::CatalogDrag>(),
        pointer,
    ) {
        if let Some((w, t, _)) = nearest_wall(p) {
            let a = t - item.w / 2.0;
            let pts = [
                w.point(a, 0.0),
                w.point(a + item.w, 0.0),
                w.point(a + item.w, item.d),
                w.point(a, item.d),
            ]
            .map(|q| scr(&xf, q));
            painter.add(egui::Shape::convex_polygon(
                pts.to_vec(),
                Color32::from_rgba_unmultiplied(235, 120, 20, 40),
                Stroke::new(1.5f32, SELECT),
            ));
        }
    }
    if let Some(item) = response.dnd_release_payload::<crate::catalog_panel::CatalogDrag>() {
        if let Some((w, t, _)) = pointer.and_then(nearest_wall) {
            st.selected_wall = Some(w.id.clone());
            st.dropped = Some(item.add_params(&w.id, t - item.w / 2.0));
        }
    }
    // pravý klik do prázdneho rohu ako v CV (findings 06 §4.4): predĺžiť skrinku do rohu
    if st.tool == Tool::Select {
        corner_menu(&response, st, project, &room, &layout, pointer);
    }
    if room.walls.is_empty() && st.tool == Tool::Select {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr!("plan_view.no_walls"),
            FontId::proportional(16.0),
            ui.visuals().weak_text_color(),
        );
    }
    changed
}

fn to_d(p: [f64; 2]) -> DVec2 {
    DVec2::new(p[0], p[1])
}

fn scr(xf: &Xf, p: DVec2) -> Pos2 {
    xf.to_screen(p.x, p.y)
}

fn scene_bounds(
    room: &Room,
    project: &ProjectFile,
    layout: &[cc_model::Transform],
) -> Option<[f64; 4]> {
    let mut pts: Vec<DVec2> = (0..room.walls.len())
        .flat_map(|i| room.wall_outline(i))
        .collect();
    for (c, t) in project.cabinets.iter().zip(layout) {
        if c.wall.is_some() {
            pts.extend(Room::footprint(t, c));
        }
    }
    let first = *pts.first()?;
    let (lo, hi) = pts
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    Some([lo.x - 600.0, lo.y - 600.0, hi.x + 600.0, hi.y + 600.0])
}

/// Upozornenia nad pôdorysom, iba keď treba: steny proti smeru hodín, skrinka bez steny.
fn notices(
    ui: &mut egui::Ui,
    st: &mut PlanState,
    project: &mut ProjectFile,
    selected_cabinet: &mut usize,
) -> bool {
    let mut changed = false;
    let reversed = !cc_room::reversed_loops(project).is_empty();
    let room = Room::new(project);
    let loose = match (
        project.cabinets.get(*selected_cabinet),
        st.selected_wall.clone(),
    ) {
        (Some(c), Some(wall)) if c.wall.is_none() && room.wall_index(&wall).is_some() => {
            Some((c.id.clone(), c.cabinet_type, c.w, wall))
        }
        _ => None,
    };
    if !reversed && loose.is_none() {
        return false;
    }
    ui.horizontal_wrapped(|ui| {
        // miestnosť nakreslená proti smeru hodín: skrinky by stáli zvonku
        if reversed {
            ui.colored_label(
                Color32::from_rgb(190, 130, 0),
                tr!("plan_view.notice.reversed"),
            );
            if ui
                .button(tr!("plan_view.notice.walls_inward"))
                .on_hover_text(tr!("plan_view.notice.walls_inward_hint"))
                .clicked()
            {
                cc_room::orient_rooms(project);
                changed = true;
            }
        }
        // vybraná skrinka bez steny: umiestnenie ku stene
        if let Some((id, t, w, wall)) = loose {
            let row = if t == CabinetType::Wall {
                Row::Upper
            } else {
                Row::Base
            };
            ui.label(tr!("plan_view.notice.loose", cabinet = &id));
            if ui
                .button(tr!("plan_view.notice.place_at_wall", wall = &wall))
                .clicked()
            {
                let at = cc_room::free_gaps(project, &wall, row, w)
                    .first()
                    .map_or(0.0, |g| g.0);
                if let Some(c) = project.cabinets.get_mut(*selected_cabinet) {
                    c.wall = Some(wall.clone());
                    c.at = at.round();
                    changed = true;
                }
            }
        }
    });
    ui.separator();
    changed
}

pub(crate) fn opening_name(k: OpeningKind) -> String {
    match k {
        OpeningKind::Window => tr!("plan_view.opening.window"),
        OpeningKind::Door => tr!("plan_view.opening.door"),
        OpeningKind::Opening => tr!("plan_view.opening.opening"),
    }
}

fn next_wall_id(project: &ProjectFile) -> String {
    cc_room::next_wall_id(project)
}

/// Os x a y v ľavom dolnom rohu plátna (ako v CAD programoch).
fn draw_axes(painter: &egui::Painter, rect: egui::Rect) {
    let o = rect.left_bottom() + Vec2::new(36.0, -36.0);
    let font = FontId::proportional(12.0);
    for (dir, color, name) in [
        (Vec2::new(40.0, 0.0), Color32::from_rgb(220, 40, 40), "X"),
        (Vec2::new(0.0, -40.0), Color32::from_rgb(30, 160, 40), "Y"),
    ] {
        let tip = o + dir;
        painter.line_segment([o, tip], Stroke::new(1.5f32, color));
        let n = dir.normalized();
        let side = Vec2::new(-n.y, n.x) * 4.0;
        painter.add(Shape::convex_polygon(
            vec![tip + n * 6.0, tip + side, tip - side],
            color,
            Stroke::NONE,
        ));
        painter.text(
            tip + n * 14.0,
            Align2::CENTER_CENTER,
            name,
            font.clone(),
            color,
        );
    }
    painter.circle_filled(o, 2.5, Color32::from_rgb(30, 160, 40));
}

/// Rám ťahaný myšou: obdĺžniková miestnosť (`zoom = false`) alebo výrez okna (`zoom = true`).
#[allow(clippy::too_many_arguments)]
fn rect_tool(
    ui: &egui::Ui,
    painter: &egui::Painter,
    response: &egui::Response,
    st: &mut PlanState,
    project: &mut ProjectFile,
    xf: &Xf,
    pointer: Option<DVec2>,
    zoom: bool,
) -> bool {
    let mut changed = false;
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
        st.drawing = None;
        st.tool = Tool::Select;
        return false;
    }
    let snap = |p: DVec2| DVec2::new((p.x / 10.0).round() * 10.0, (p.y / 10.0).round() * 10.0);
    if response.drag_started_by(egui::PointerButton::Primary) {
        st.drawing = pointer.map(snap);
    }
    if let (Some(a), Some(p)) = (st.drawing, pointer) {
        let b = snap(p);
        let r = egui::Rect::from_two_pos(scr(xf, a), scr(xf, b));
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.5f32, SELECT),
            egui::StrokeKind::Middle,
        );
        if !zoom {
            painter.text(
                r.center_top() + Vec2::new(0.0, -6.0),
                Align2::CENTER_BOTTOM,
                format!("{} × {} mm", fmt((b.x - a.x).abs()), fmt((b.y - a.y).abs())),
                FontId::proportional(13.0),
                SELECT,
            );
        }
    }
    if response.drag_stopped() {
        if let (Some(a), Some(p)) = (st.drawing, pointer) {
            let b = snap(p);
            let (lo, hi) = (a.min(b), a.max(b));
            let size = hi - lo;
            if zoom {
                if size.x > 50.0 && size.y > 50.0 {
                    st.view.fit(response.rect, [lo.x, lo.y, hi.x, hi.y]);
                }
                st.tool = Tool::Select;
            } else if size.x >= 500.0 && size.y >= 500.0 {
                if project.room.is_none() {
                    project.room = Some(RoomSpec::default());
                }
                let walls = cc_room::rect_walls(project, lo, size.x, size.y);
                st.selected_wall = walls.first().map(|w| w.id.clone());
                project.walls.extend(walls);
                changed = true;
            }
        }
        st.drawing = None;
    }
    painter.text(
        response.rect.left_top() + Vec2::new(10.0, 10.0),
        Align2::LEFT_TOP,
        if zoom {
            tr!("plan_view.hint.zoom_window")
        } else {
            tr!("plan_view.hint.rect_room")
        },
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    changed
}

fn draw_walls(
    painter: &egui::Painter,
    xf: &Xf,
    room: &Room,
    project: &ProjectFile,
    selected: Option<&str>,
) {
    for (i, w) in room.walls.iter().enumerate() {
        let outline = room.wall_outline(i);
        let pts: Vec<Pos2> = outline.iter().map(|p| scr(xf, *p)).collect();
        let sel = selected == Some(w.id.as_str());
        let stroke = Stroke::new(
            if sel { 2.5f32 } else { 1.0f32 },
            if sel { SELECT } else { Color32::from_gray(70) },
        );
        painter.add(Shape::convex_polygon(pts, WALL_FILL, stroke));
        // otvory: biela medzera cez hrúbku steny, okno s dvojitou čiarou, dvere s oblúkom
        for o in project.openings.iter().filter(|o| o.wall == w.id) {
            let a = w.point(o.at, 0.0);
            let b = w.point(o.at + o.width, 0.0);
            let out = -w.inward * w.thickness;
            let quad = [a, b, b + out, a + out].map(|p| scr(xf, p)).to_vec();
            painter.add(Shape::convex_polygon(
                quad,
                Color32::WHITE,
                Stroke::new(1.0f32, Color32::from_gray(70)),
            ));
            match o.kind {
                OpeningKind::Window => {
                    for f in [0.35, 0.65] {
                        painter.line_segment(
                            [scr(xf, a + out * f), scr(xf, b + out * f)],
                            Stroke::new(1.0f32, Color32::from_rgb(70, 120, 200)),
                        );
                    }
                }
                OpeningKind::Door => {
                    // krídlo otvorené do miestnosti, pánt na ľavej strane otvoru
                    let r = o.width;
                    let leaf = a + w.inward * r;
                    painter.line_segment(
                        [scr(xf, a), scr(xf, leaf)],
                        Stroke::new(1.5f32, Color32::from_gray(60)),
                    );
                    let arc: Vec<Pos2> = (0..=24)
                        .map(|k| {
                            let t = k as f64 / 24.0 * std::f64::consts::FRAC_PI_2;
                            scr(xf, a + (w.dir * t.cos() + w.inward * t.sin()) * r)
                        })
                        .collect();
                    painter.add(Shape::dashed_line(
                        &arc,
                        Stroke::new(1.0f32, Color32::from_gray(90)),
                        6.0,
                        4.0,
                    ));
                }
                OpeningKind::Opening => {}
            }
        }
        // meno steny v tele steny: pri strede, ale mimo okien a dverí
        let free = |a: f64| {
            !project
                .openings
                .iter()
                .any(|o| o.wall == w.id && a >= o.at - 150.0 && a <= o.at + o.width + 150.0)
        };
        let along = [0.5, 0.3, 0.7, 0.15, 0.85]
            .into_iter()
            .map(|f| w.length * f)
            .find(|a| free(*a))
            .unwrap_or(w.length / 2.0);
        let mid = w.point(along, -w.thickness / 2.0);
        painter.text(
            scr(xf, mid),
            Align2::CENTER_CENTER,
            &w.id,
            FontId::proportional(12.0),
            Color32::WHITE,
        );
    }
}

fn draw_cabinets(
    painter: &egui::Painter,
    xf: &Xf,
    room: &Room,
    project: &ProjectFile,
    layout: &[cc_model::Transform],
    selected: usize,
) {
    // najprv spodné a vysoké, potom horné (priesvitne navrchu)
    for pass in [false, true] {
        for (i, (c, t)) in project.cabinets.iter().zip(layout).enumerate() {
            if c.wall.is_none() || (c.cabinet_type == CabinetType::Wall) != pass {
                continue;
            }
            let fp: Vec<Pos2> = Room::footprint(t, c).iter().map(|p| scr(xf, *p)).collect();
            let fill = match c.cabinet_type {
                CabinetType::Base => BASE_FILL,
                CabinetType::Tall => TALL_FILL,
                CabinetType::Wall => UPPER_FILL,
            };
            let sel = i == selected;
            let stroke = Stroke::new(
                if sel { 2.5f32 } else { 1.0f32 },
                if sel { SELECT } else { Color32::from_gray(80) },
            );
            if pass {
                painter.add(Shape::convex_polygon(fp.clone(), fill, Stroke::NONE));
                let mut closed = fp.clone();
                if let Some(first) = fp.first() {
                    closed.push(*first);
                }
                painter.extend(Shape::dashed_line(&closed, stroke, 6.0, 4.0));
            } else {
                painter.add(Shape::convex_polygon(fp.clone(), fill, stroke));
                // predná hrana hrubšie
                if let (Some(a), Some(b)) = (fp.first(), fp.get(1)) {
                    painter.line_segment([*a, *b], Stroke::new(2.0f32, Color32::from_gray(60)));
                }
            }
            let center =
                fp.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / fp.len().max(1) as f32;
            let label_pos = Pos2::new(center.x, center.y)
                + if pass {
                    Vec2::new(0.0, -8.0)
                } else {
                    Vec2::new(0.0, 8.0)
                };
            painter.text(
                label_pos,
                Align2::CENTER_CENTER,
                &c.id,
                FontId::proportional(12.0),
                if pass {
                    Color32::from_rgb(40, 70, 130)
                } else {
                    Color32::from_gray(40)
                },
            );
            let _ = room;
        }
    }
}

fn draw_dimensions(painter: &egui::Painter, xf: &Xf, room: &Room, project: &ProjectFile) {
    for w in &room.walls {
        let gap = 18.0f32;
        let base = (w.thickness * xf.scale()) as f32 + 14.0;
        let mut level = 0;
        for row in [Some(Row::Base), Some(Row::Upper), None] {
            let pts = match row {
                Some(r) => cc_room::dimension_points(project, &w.id, r),
                None => vec![0.0, w.length],
            };
            // rad bez skriniek a otvorov (iba konce steny) sa nekreslí, dĺžka steny vždy
            if row.is_some() && pts.len() <= 2 {
                continue;
            }
            let offset = base + gap * level as f32;
            for pair in pts.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if b - a < 0.5 {
                    continue;
                }
                let pa = w.point(a, 0.0);
                let pb = w.point(b, 0.0);
                canvas2d::dimension(
                    painter,
                    xf,
                    [pa.x, pa.y],
                    [pb.x, pb.y],
                    offset,
                    &fmt(b - a),
                    DIM,
                );
            }
            level += 1;
        }
    }
}

fn point_in_poly(p: DVec2, poly: &[DVec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

/// Výber skrinky alebo steny, posun skrinky pozdĺž steny s prichytením k susedom a koncom.
fn select_tool(
    response: &egui::Response,
    st: &mut PlanState,
    project: &mut ProjectFile,
    room: &Room,
    xf: &Xf,
    selected_cabinet: &mut usize,
    pointer: Option<DVec2>,
) -> bool {
    let layout = cc_room::layout(project);
    let hit_wall = |p: DVec2| -> Option<String> {
        (0..room.walls.len())
            .find(|&i| point_in_poly(p, &room.wall_outline(i)))
            .and_then(|i| room.walls.get(i).map(|w| w.id.clone()))
    };

    if response.drag_started_by(egui::PointerButton::Primary) {
        if let Some(p) = response.interact_pointer_pos().map(|p| to_d(xf.to_mm(p))) {
            if let Some(i) = hit_cabinet(project, &layout, p) {
                *selected_cabinet = i;
                let grab = project.cabinets.get(i).and_then(|c| {
                    let w = room.walls.get(room.wall_index(c.wall.as_deref()?)?)?;
                    Some((p - w.start).dot(w.dir) - c.at)
                });
                st.drag = grab.map(|g| (i, g));
            }
        }
    }
    let mut changed = false;
    if let (Some((i, grab)), Some(p)) = (
        st.drag,
        pointer.or_else(|| response.interact_pointer_pos().map(|p| to_d(xf.to_mm(p)))),
    ) {
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(c) = project.cabinets.get(i) {
                if let Some(w) = c
                    .wall
                    .as_deref()
                    .and_then(|id| room.wall_index(id))
                    .and_then(|wi| room.walls.get(wi))
                {
                    let raw = (p - w.start).dot(w.dir) - grab;
                    let snapped = snap_along(project, i, w, raw, 30.0 / xf.scale());
                    if let Some(c) = project.cabinets.get_mut(i) {
                        if (c.at - snapped).abs() > 1e-6 {
                            c.at = snapped;
                            changed = true;
                        }
                    }
                }
            }
        }
    }
    if response.drag_stopped() {
        st.drag = None;
    }
    if response.double_clicked() {
        if let Some(p) = response.interact_pointer_pos().map(|p| to_d(xf.to_mm(p))) {
            if let Some(i) = hit_cabinet(project, &layout, p) {
                st.open_cabinet = project.cabinets.get(i).map(|c| c.id.clone());
            }
        }
    }
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos().map(|p| to_d(xf.to_mm(p))) {
            if let Some(i) = hit_cabinet(project, &layout, p) {
                *selected_cabinet = i;
                if let Some(wall) = project.cabinets.get(i).and_then(|c| c.wall.clone()) {
                    st.selected_wall = Some(wall);
                }
            } else {
                st.selected_wall = hit_wall(p);
            }
        }
    }
    changed
}

/// Skrinka pod bodom `p`; horné skrinky majú prednosť (sú navrchu).
/// Dosah rohu pre pravý klik (mm od vnútorného rohu po oboch stenách).
const CORNER_REACH: f64 = 800.0;

fn corner_menu(
    response: &egui::Response,
    st: &mut PlanState,
    project: &ProjectFile,
    room: &Room,
    layout: &[cc_model::Transform],
    pointer: Option<DVec2>,
) {
    if response.secondary_clicked() {
        st.corner = pointer.and_then(|p| {
            if hit_cabinet(project, layout, p).is_some() {
                return None;
            }
            room.walls.iter().find_map(|w| {
                w.next?;
                let d = p - w.start;
                let along = d.dot(w.dir);
                let inward = d.dot(w.inward);
                (along <= w.length + 1.0
                    && along >= w.length - CORNER_REACH
                    && (0.0..=CORNER_REACH).contains(&inward))
                .then(|| w.id.clone())
            })
        });
    }
    let Some(wall) = st.corner.clone() else {
        return;
    };
    let mut cmds = Vec::new();
    response.context_menu(|ui| {
        let next = room
            .wall_index(&wall)
            .and_then(|i| room.walls.get(i))
            .and_then(|w| w.next)
            .and_then(|n| room.walls.get(n))
            .map_or_else(String::new, |n| n.id.clone());
        ui.label(
            egui::RichText::new(tr!("plan_view.corner.title", wall = &wall, next = next)).strong(),
        );
        let mut any = false;
        for (row, row_key, row_name) in [
            (cc_room::Row::Base, "base", tr!("plan_view.corner.base")),
            (cc_room::Row::Upper, "upper", tr!("plan_view.corner.upper")),
        ] {
            let Some((a, b, next_id)) = cc_room::corner_cabinets(project, &wall, row) else {
                continue;
            };
            any = true;
            ui.label(egui::RichText::new(row_name).weak());
            for (idx, ext) in [(a, wall.clone()), (b, next_id)] {
                let Some(c) = project.cabinets.get(idx) else {
                    continue;
                };
                let label = tr!("plan_view.corner.extend", cabinet = &c.id);
                let drawers = c.fronts.iter().any(|f| {
                    f.kind == cc_model::FrontKind::Drawer
                        || f.columns
                            .iter()
                            .any(|k| k.kind == cc_model::FrontKind::Drawer)
                });
                let why = if c.lock_width {
                    Some(tr!("plan_view.corner.locked_width"))
                } else if drawers {
                    Some(tr!("plan_view.corner.has_drawers"))
                } else {
                    None
                };
                let r = ui.add_enabled(why.is_none(), egui::Button::new(label));
                let r = match why {
                    Some(t) => r.on_disabled_hover_text(t),
                    None => r.on_hover_text(tr!("plan_view.corner.extend_hint")),
                };
                if r.clicked() {
                    cmds.push((
                        "room.corner",
                        serde_json::json!({ "wall": wall, "row": row_key, "extend": ext }),
                    ));
                    ui.close_menu();
                }
            }
        }
        if !any {
            ui.label(tr!("plan_view.corner.no_cabinets"));
        }
        ui.separator();
        // v CV ešte Make Void, Miter a Corner (tvarovanie): náš model ich zatiaľ nemá
        for label in [
            tr!("plan_view.corner.void"),
            tr!("plan_view.corner.miter"),
            tr!("plan_view.corner.shape"),
        ] {
            ui.add_enabled(false, egui::Button::new(label))
                .on_disabled_hover_text(tr!("plan_view.corner.not_yet"));
        }
    });
    st.commands.extend(cmds);
}

fn hit_cabinet(project: &ProjectFile, layout: &[cc_model::Transform], p: DVec2) -> Option<usize> {
    let mut hits: Vec<(usize, bool)> = project
        .cabinets
        .iter()
        .zip(layout)
        .enumerate()
        .filter(|(_, (c, _))| c.wall.is_some())
        .filter(|(_, (c, t))| point_in_poly(p, &Room::footprint(t, c)))
        .map(|(i, (c, _))| (i, c.cabinet_type == CabinetType::Wall))
        .collect();
    hits.sort_by_key(|(_, upper)| !*upper);
    hits.first().map(|h| h.0)
}

/// Poloha pozdĺž steny s prichytením ku koncom steny a k hranám susedných skriniek toho istého radu.
fn snap_along(
    project: &ProjectFile,
    index: usize,
    wall: &cc_room::Wall,
    raw: f64,
    tol: f64,
) -> f64 {
    let Some(c) = project.cabinets.get(index) else {
        return raw;
    };
    let rows: Vec<Row> = [Row::Base, Row::Upper]
        .into_iter()
        .filter(|r| r.contains(c.cabinet_type))
        .collect();
    let mut targets = vec![0.0, wall.length - c.w];
    for (j, o) in project.cabinets.iter().enumerate() {
        if j == index
            || o.wall.as_deref() != Some(wall.id.as_str())
            || !rows.iter().any(|r| r.contains(o.cabinet_type))
        {
            continue;
        }
        targets.push(o.at + o.w);
        targets.push(o.at - c.w);
        targets.push(o.at);
    }
    let best = targets
        .into_iter()
        .map(|t| (t, (t - raw).abs()))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    match best {
        Some((t, _)) => t,
        None => raw.round(),
    }
}

/// Kreslenie stien: klik = bod, steny sa reťazia; prichytenie ku koncom stien a k uhlu 45°.
#[allow(clippy::too_many_arguments)]
fn draw_tool(
    ui: &egui::Ui,
    painter: &egui::Painter,
    response: &egui::Response,
    st: &mut PlanState,
    project: &mut ProjectFile,
    room: &Room,
    xf: &Xf,
    pointer: Option<DVec2>,
) -> bool {
    let mut changed = false;
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) || response.secondary_clicked() {
        st.drawing = None;
        if st.drawing.is_none() && response.secondary_clicked() {
            st.tool = Tool::Select;
        }
    }
    let snap = |p: DVec2, from: Option<DVec2>| -> DVec2 {
        let tol = 12.0 / xf.scale();
        for w in &room.walls {
            for q in [w.start, w.end] {
                if q.distance(p) < tol {
                    return q;
                }
            }
        }
        match from {
            Some(f) => {
                let d = p - f;
                let len = (d.length() / 10.0).round() * 10.0;
                let ang = d.y.atan2(d.x);
                let step = std::f64::consts::FRAC_PI_4;
                let snapped = (ang / step).round() * step;
                let a = if (ang - snapped).abs() < 5f64.to_radians() {
                    snapped
                } else {
                    ang
                };
                f + DVec2::new(a.cos(), a.sin()) * len
            }
            None => DVec2::new((p.x / 10.0).round() * 10.0, (p.y / 10.0).round() * 10.0),
        }
    };
    if let Some(p) = pointer {
        let target = snap(p, st.drawing);
        painter.circle_stroke(scr(xf, target), 5.0, Stroke::new(1.5f32, SELECT));
        if let Some(start) = st.drawing {
            painter.line_segment(
                [scr(xf, start), scr(xf, target)],
                Stroke::new(2.0f32, SELECT),
            );
            let len = start.distance(target);
            let ang = (target - start).y.atan2((target - start).x).to_degrees();
            painter.text(
                scr(xf, (start + target) / 2.0) + Vec2::new(0.0, -14.0),
                Align2::CENTER_BOTTOM,
                format!("{} mm, {:.0}°", fmt(len), ang),
                FontId::proportional(13.0),
                SELECT,
            );
        }
        if response.clicked() {
            match st.drawing {
                None => st.drawing = Some(target),
                Some(start) => {
                    let d = target - start;
                    let len = d.length();
                    if len >= 100.0 {
                        if project.room.is_none() {
                            project.room = Some(RoomSpec::default());
                        }
                        let id = next_wall_id(project);
                        project.walls.push(WallSpec {
                            id: id.clone(),
                            x: (start.x * 10.0).round() / 10.0,
                            y: (start.y * 10.0).round() / 10.0,
                            length: (len * 10.0).round() / 10.0,
                            angle: ((d.y.atan2(d.x).to_degrees()) * 100.0).round() / 100.0,
                            height: None,
                            thickness: None,
                        });
                        st.selected_wall = Some(id);
                        changed = true;
                        st.drawing = Some(target);
                        // uzavretá miestnosť proti smeru hodín: otočiť steny dovnútra (ako CV)
                        if !cc_room::orient_rooms(project).is_empty() {
                            st.drawing = None;
                        }
                    }
                }
            }
        }
    }
    painter.text(
        response.rect.left_top() + Vec2::new(10.0, 10.0),
        Align2::LEFT_TOP,
        tr!("plan_view.hint.draw_wall"),
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    changed
}
