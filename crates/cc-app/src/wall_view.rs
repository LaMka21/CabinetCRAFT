//! Pohľad na stenu (kap. 13): plocha steny z miestnosti, otvory, skrinky pri stene
//! a kóty (šírky v radoch, dĺžka steny, výšky). Skrinky sa dajú vybrať a posunúť.

use cc_i18n::tr;
use cc_model::{CabinetType, Hardware, Model, OpeningKind, Part, ProjectFile};
use cc_room::{Room, Row};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use glam::DVec3;

use crate::canvas2d::{self, View2d};
use crate::part_view::fmt;

#[derive(Default)]
pub struct WallViewState {
    pub view: View2d,
    pub wall: Option<String>,
    drag: Option<Drag>,
    last_wall: Option<String>,
    /// Položka katalógu pustená na stenu: `cabinet.add` pre aplikáciu.
    pub dropped: Option<serde_json::Value>,
    /// Vybraný otvor čela (editor sekcií): skrinka (ID), riadok, stĺpec.
    pub opening: Option<(String, usize, Option<usize>)>,
    /// Príkazy z ponuky otvoru pre aplikáciu.
    pub commands: Vec<(&'static str, serde_json::Value)>,
    /// Ponuka skrinky → Vlastnosti: ID skrinky pre aplikáciu.
    pub props: Option<String>,
    /// Dvojklik na skrinku alebo ponuka → Otvoriť skrinku: ID pre editor skrinky.
    pub open_cabinet: Option<String>,
}

/// Ťahanie v pohľade: posun skrinky (úchop od jej ľavej hrany) alebo zmena šírky hranou.
#[derive(Clone, Copy)]
enum Drag {
    Move(usize, f64),
    Left(usize),
    Right(usize),
}

/// Najmenšia šírka skrinky pri ťahaní hrany (mm).
const MIN_WIDTH: f64 = 100.0;

const DIM: Color32 = Color32::from_rgb(60, 90, 140);
const SELECT: Color32 = Color32::from_rgb(235, 120, 20);
/// Vybraný otvor čela.
const OPENING: Color32 = Color32::from_rgb(30, 120, 220);
/// Žltý úchyt vybranej skrinky (CV: žlté body posunu a rozmeru).
const HANDLE: Color32 = Color32::from_rgb(250, 210, 40);

/// Vráti `true`, ak sa projekt zmenil. `prefer_wall` = stena vybraná v pôdoryse.
pub fn show(
    ui: &mut egui::Ui,
    st: &mut WallViewState,
    project: &mut ProjectFile,
    model: Option<&Model>,
    selected_cabinet: &mut usize,
    prefer_wall: Option<&str>,
) -> bool {
    let room = Room::new(project);
    if room.walls.is_empty() {
        ui.label(tr!("wall_view.no_walls"));
        return false;
    }
    if st
        .wall
        .as_ref()
        .is_none_or(|w| room.wall_index(w).is_none())
    {
        st.wall = prefer_wall
            .filter(|w| room.wall_index(w).is_some())
            .map(str::to_owned)
            .or_else(|| room.walls.first().map(|w| w.id.clone()));
    }
    // stena sa vyberá v páse (Stena ◀ W1 ▶ ako v CV)
    if st.wall != st.last_wall {
        st.view.fit_pending = true;
        st.last_wall = st.wall.clone();
    }
    let Some(wall) = st
        .wall
        .clone()
        .and_then(|id| room.wall_index(&id))
        .and_then(|i| room.walls.get(i))
        .cloned()
    else {
        return false;
    };
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let bounds = [-700.0, -500.0, wall.length + 500.0, wall.height + 500.0];
    let xf = st.view.interact(ui, &response, Some(bounds));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    let text = ui.visuals().text_color();
    let to = |x: f64, z: f64| xf.to_screen(x, z);
    let rect_mm = |x0: f64, z0: f64, x1: f64, z1: f64| Rect::from_two_pos(to(x0, z0), to(x1, z1));

    // plocha steny, podlaha, soffit
    painter.rect_filled(
        rect_mm(0.0, 0.0, wall.length, wall.height),
        0.0,
        Color32::from_rgb(246, 244, 240),
    );
    painter.rect_stroke(
        rect_mm(0.0, 0.0, wall.length, wall.height),
        0.0,
        Stroke::new(1.5f32, Color32::from_gray(90)),
        StrokeKind::Inside,
    );
    painter.line_segment(
        [to(-300.0, 0.0), to(wall.length + 300.0, 0.0)],
        Stroke::new(2.0f32, Color32::from_gray(60)),
    );
    if room.spec.soffit > 0.0 {
        let z = wall.height - room.spec.soffit;
        painter.rect_filled(
            rect_mm(0.0, z, wall.length, wall.height),
            0.0,
            Color32::from_rgb(225, 222, 216),
        );
    }

    // otvory
    for o in project.openings.iter().filter(|o| o.wall == wall.id) {
        let r = rect_mm(o.at, o.elevation, o.at + o.width, o.elevation + o.height);
        let fill = match o.kind {
            OpeningKind::Window => Color32::from_rgb(214, 232, 248),
            OpeningKind::Door => Color32::from_rgb(232, 226, 216),
            OpeningKind::Opening => Color32::from_rgb(240, 240, 240),
        };
        let label = crate::plan_view::opening_name(o.kind);
        painter.rect_filled(r, 0.0, fill);
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(1.0f32, Color32::from_rgb(90, 120, 160)),
            StrokeKind::Inside,
        );
        if o.kind == OpeningKind::Window {
            painter.line_segment(
                [r.center_top(), r.center_bottom()],
                Stroke::new(1.0f32, Color32::from_rgb(90, 120, 160)),
            );
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(12.0),
            Color32::from_rgb(60, 90, 130),
        );
    }

    // skrinky steny: obdĺžnik korpusu, sokel, police čiarkovane
    let on_wall: Vec<usize> = project
        .cabinets
        .iter()
        .enumerate()
        .filter(|(_, c)| c.wall.as_deref() == Some(wall.id.as_str()))
        .map(|(i, _)| i)
        .collect();
    for &i in &on_wall {
        let Some(c) = project.cabinets.get(i) else {
            continue;
        };
        let z0 = room.elevation(c);
        let r = rect_mm(c.at, z0, c.at + c.w, z0 + c.h);
        let fill = match c.cabinet_type {
            CabinetType::Base => Color32::from_rgb(236, 228, 214),
            CabinetType::Tall => Color32::from_rgb(222, 212, 196),
            CabinetType::Wall => Color32::from_rgb(222, 230, 242),
        };
        painter.rect_filled(r, 0.0, fill);
        let sel = i == *selected_cabinet;
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(
                if sel { 2.5f32 } else { 1.2f32 },
                if sel { SELECT } else { Color32::from_gray(70) },
            ),
            StrokeKind::Inside,
        );
        if z0 > 1.0 && c.cabinet_type != CabinetType::Wall {
            // sokel / nožičky
            let leg = rect_mm(c.at + 50.0, 0.0, c.at + c.w - 50.0, z0);
            painter.rect_filled(leg, 0.0, Color32::from_gray(200));
        }
        // čelá z vygenerovaného modelu (rám skrinky: x pozdĺž steny, z nahor)
        let cab_model = model.and_then(|m| m.cabinets.iter().find(|m| m.id == c.id));
        if let Some(cm) = cab_model {
            for p in cm.parts.iter().filter(|p| p.role.is_front()) {
                let (lo, hi) = cabinet_box(p);
                let fr = rect_mm(c.at + lo.x, z0 + lo.z, c.at + hi.x, z0 + hi.z);
                painter.rect_filled(fr, 0.0, Color32::from_rgb(250, 248, 244));
                painter.rect_stroke(
                    fr,
                    0.0,
                    Stroke::new(1.0f32, Color32::from_gray(90)),
                    StrokeKind::Inside,
                );
                // dvierka: trojuholník otvárania so špičkou na strane pántov
                let hinge_x = cm.hardware.iter().find_map(|h| match h {
                    Hardware::Hinge { door, side, .. } if *door == p.id => {
                        cm.parts.iter().find(|s| s.id == *side).map(|s| {
                            let (a, b) = cabinet_box(s);
                            (a.x + b.x) / 2.0
                        })
                    }
                    _ => None,
                });
                if let Some(hx) = hinge_x {
                    let left = (hx - lo.x).abs() < (hx - hi.x).abs();
                    let (apex, far) = if left { (lo.x, hi.x) } else { (hi.x, lo.x) };
                    let mid = (lo.z + hi.z) / 2.0;
                    let pts = [
                        to(c.at + far, z0 + lo.z),
                        to(c.at + apex, z0 + mid),
                        to(c.at + far, z0 + hi.z),
                    ];
                    painter.extend(egui::Shape::dashed_line(
                        &pts,
                        Stroke::new(0.8f32, Color32::from_gray(140)),
                        6.0,
                        4.0,
                    ));
                }
            }
        }
        if sel {
            // výber nad čelami a úchyty ako v CV: stred = posun, bočné hrany = šírka
            painter.rect_stroke(r, 0.0, Stroke::new(2.5f32, SELECT), StrokeKind::Inside);
            let handle = |p: Pos2| {
                painter.circle(p, 5.0, HANDLE, Stroke::new(1.0f32, Color32::from_gray(60)));
            };
            handle(r.center());
            if !c.lock_width {
                handle(r.left_center());
                handle(r.right_center());
            }
        }
        let shelves = if cab_model.is_some_and(|m| m.parts.iter().any(|p| p.role.is_front())) {
            0
        } else {
            c.adjustable_shelves as usize + c.fixed_shelves.len()
        };
        for k in 1..=shelves {
            let z = z0 + c.h * k as f64 / (shelves + 1) as f64;
            painter.extend(egui::Shape::dashed_line(
                &[to(c.at + 20.0, z), to(c.at + c.w - 20.0, z)],
                Stroke::new(1.0f32, Color32::from_gray(150)),
                5.0,
                4.0,
            ));
        }
        painter.text(
            r.center(),
            Align2::CENTER_CENTER,
            &c.id,
            FontId::proportional(14.0),
            Color32::from_gray(40),
        );
    }

    // kóty: spodný rad pod podlahou, horný nad stenou, dĺžka steny úplne dole
    let below = |level: f32| level * 20.0 + 22.0;
    let chain = |pts: &[f64], z: f64, offset: f32| {
        for pair in pts.windows(2) {
            if pair[1] - pair[0] > 0.5 {
                // a → b doprava: kladný offset ide nad čiaru (do steny), preto opačne
                canvas2d::dimension(
                    &painter,
                    &xf,
                    [pair[1], z],
                    [pair[0], z],
                    offset,
                    &fmt(pair[1] - pair[0]),
                    DIM,
                );
            }
        }
    };
    let base_pts = cc_room::dimension_points(project, &wall.id, Row::Base);
    let mut level = 0.0;
    if base_pts.len() > 2 {
        chain(&base_pts, 0.0, below(level));
        level += 1.0;
    }
    chain(&[0.0, wall.length], 0.0, below(level));
    let upper_pts = cc_room::dimension_points(project, &wall.id, Row::Upper);
    if upper_pts.len() > 2 {
        // zľava doprava s kladným offsetom = nad hornou hranou steny
        for pair in upper_pts.windows(2) {
            if pair[1] - pair[0] > 0.5 {
                canvas2d::dimension(
                    &painter,
                    &xf,
                    [pair[0], wall.height],
                    [pair[1], wall.height],
                    22.0,
                    &fmt(pair[1] - pair[0]),
                    DIM,
                );
            }
        }
    }
    // výšky vľavo (ako v CV): podlaha, spodky a vrchy skriniek, strop; celá výška vpravo
    let mut zs = vec![0.0, wall.height];
    for &i in &on_wall {
        if let Some(c) = project.cabinets.get(i) {
            let z0 = room.elevation(c);
            zs.extend([z0, z0 + c.h]);
        }
    }
    zs.sort_by(f64::total_cmp);
    zs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    for pair in zs.windows(2) {
        // zdola nahor: kladný offset = doľava od steny
        canvas2d::dimension(
            &painter,
            &xf,
            [0.0, pair[0]],
            [0.0, pair[1]],
            24.0,
            &fmt(pair[1] - pair[0]),
            DIM,
        );
    }
    canvas2d::dimension(
        &painter,
        &xf,
        [wall.length, 0.0],
        [wall.length, wall.height],
        -30.0,
        &fmt(wall.height),
        DIM,
    );
    painter.text(
        to(0.0, wall.height) + Vec2::new(0.0, -6.0),
        Align2::LEFT_BOTTOM,
        tr!("wall_view.left_end", wall = &wall.id),
        FontId::proportional(12.0),
        text,
    );

    // výber a posun skriniek
    // obdĺžniky skriniek (index, x0, z0, x1, z1) pre výber myšou
    let boxes: Vec<(usize, f64, f64, f64, f64)> = on_wall
        .iter()
        .filter_map(|&i| {
            let c = project.cabinets.get(i)?;
            let z0 = room.elevation(c);
            Some((i, c.at, z0, c.at + c.w, z0 + c.h))
        })
        .collect();
    let hit = |p: Pos2| -> Option<usize> {
        let [x, z] = xf.to_mm(p);
        boxes
            .iter()
            .find(|b| x >= b.1 && x <= b.3 && z >= b.2 && z <= b.4)
            .map(|b| b.0)
    };
    // hrana skrinky pod kurzorom (do 6 bodov): ťahaním sa mení šírka
    let edge_tol = 6.0 / xf.scale();
    let edge_at = |p: Pos2| -> Option<Drag> {
        let [x, z] = xf.to_mm(p);
        boxes.iter().find_map(|b| {
            let locked = project.cabinets.get(b.0).is_some_and(|c| c.lock_width);
            if locked || z < b.2 || z > b.4 {
                None
            } else if (x - b.1).abs() <= edge_tol {
                Some(Drag::Left(b.0))
            } else if (x - b.3).abs() <= edge_tol {
                Some(Drag::Right(b.0))
            } else {
                None
            }
        })
    };
    if let Some(p) = response.hover_pos() {
        if st.drag.is_none() && edge_at(p).is_some()
            || matches!(st.drag, Some(Drag::Left(_) | Drag::Right(_)))
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    let mut changed = false;
    if response.drag_started_by(egui::PointerButton::Primary) {
        // miesto stlačenia, nie poloha po rozpoznaní ťahania (tá je už o pár bodov ďalej)
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .or(response.interact_pointer_pos());
        if let Some(p) = origin {
            if let Some(edge) = edge_at(p) {
                let (Drag::Left(i) | Drag::Right(i) | Drag::Move(i, _)) = edge;
                *selected_cabinet = i;
                st.drag = Some(edge);
            } else if let Some(i) = hit(p) {
                *selected_cabinet = i;
                let x = xf.to_mm(p)[0];
                st.drag = project.cabinets.get(i).map(|c| Drag::Move(i, x - c.at));
            }
        }
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let (Some(drag), Some(p)) = (st.drag, response.interact_pointer_pos()) {
            let x = xf.to_mm(p)[0];
            let tol = 30.0 / xf.scale();
            match drag {
                Drag::Move(i, grab) => {
                    let new_at = snap(project, i, &wall, x - grab, tol);
                    if let Some(c) = project.cabinets.get_mut(i) {
                        if (c.at - new_at).abs() > 1e-6 {
                            c.at = new_at;
                            changed = true;
                        }
                    }
                }
                Drag::Left(i) | Drag::Right(i) => {
                    let edge = snap_edge(project, i, &wall, x, tol);
                    if let Some(c) = project.cabinets.get_mut(i) {
                        let (at, w) = if matches!(drag, Drag::Right(_)) {
                            (c.at, (edge - c.at).max(MIN_WIDTH))
                        } else {
                            let right = c.at + c.w;
                            let left = edge.min(right - MIN_WIDTH);
                            (left, right - left)
                        };
                        if (c.at - at).abs() > 1e-6 || (c.w - w).abs() > 1e-6 {
                            c.at = at;
                            c.w = w;
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
    // otvory čiel skriniek steny (editor sekcií): (skrinka, riadok, stĺpec, obdĺžnik mm)
    let openings: Vec<(usize, usize, Option<usize>, [f64; 4])> = on_wall
        .iter()
        .filter_map(|&i| {
            let c = project.cabinets.get(i)?;
            let cm = model.and_then(|m| m.cabinets.iter().find(|m| m.id == c.id))?;
            let z0 = room.elevation(c);
            Some(cm.openings.iter().map(move |o| {
                (
                    i,
                    o.row,
                    o.column,
                    [c.at + o.x, z0 + o.z, c.at + o.x + o.w, z0 + o.z + o.h],
                )
            }))
        })
        .flatten()
        .collect();
    let opening_at = |p: Pos2, cab: usize| {
        let [x, z] = xf.to_mm(p);
        openings
            .iter()
            .find(|o| o.0 == cab && x >= o.3[0] && x <= o.3[2] && z >= o.3[1] && z <= o.3[3])
            .map(|o| (o.1, o.2))
    };
    let cab_id = |i: usize| project.cabinets.get(i).map(|c| c.id.clone());
    // dvojklik na skrinku: editor skrinky (ako v CV)
    if response.double_clicked() {
        if let Some(i) = response.interact_pointer_pos().and_then(hit) {
            st.open_cabinet = cab_id(i);
        }
    }
    // klik: skrinka; ďalší klik na vybranú skrinku: otvor čela (ako výber v CV)
    if response.clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            match hit(p) {
                Some(i) if i == *selected_cabinet => {
                    st.opening = opening_at(p, i).and_then(|(r, k)| cab_id(i).map(|id| (id, r, k)));
                }
                Some(i) => {
                    *selected_cabinet = i;
                    st.opening = None;
                }
                None => st.opening = None,
            }
        }
    }
    // otvory vybranej skrinky ako v CV: meno vľavo hore, výška vpravo, šírka dole
    for o in openings.iter().filter(|o| o.0 == *selected_cabinet) {
        let rr = rect_mm(o.3[0], o.3[1], o.3[2], o.3[3]);
        let small = FontId::proportional(11.0);
        let ink = Color32::from_rgb(30, 90, 170);
        painter.text(
            rr.left_top() + Vec2::new(3.0, 2.0),
            Align2::LEFT_TOP,
            opening_name(o.1, o.2),
            small.clone(),
            ink,
        );
        painter.text(
            rr.right_center() - Vec2::new(3.0, 0.0),
            Align2::RIGHT_CENTER,
            fmt(o.3[3] - o.3[1]),
            small.clone(),
            ink,
        );
        painter.text(
            rr.center_bottom() - Vec2::new(0.0, 2.0),
            Align2::CENTER_BOTTOM,
            fmt(o.3[2] - o.3[0]),
            small,
            ink,
        );
    }
    // vybraný otvor: modrý obrys
    if let Some((id, r, k)) = st.opening.clone() {
        if cab_id(*selected_cabinet).as_ref() != Some(&id) {
            st.opening = None;
        } else if let Some(o) = openings
            .iter()
            .find(|o| o.0 == *selected_cabinet && o.1 == r && o.2 == k)
        {
            let rr = rect_mm(o.3[0], o.3[1], o.3[2], o.3[3]);
            painter.rect_stroke(
                rr.shrink(2.0),
                0.0,
                Stroke::new(2.5f32, OPENING),
                StrokeKind::Inside,
            );
        }
    }
    // ťahaná položka katalógu: duch skrinky pod kurzorom v jej rade
    if let (Some(item), Some(p)) = (
        response.dnd_hover_payload::<crate::catalog_panel::CatalogDrag>(),
        response.hover_pos(),
    ) {
        let x = xf.to_mm(p)[0] - item.w / 2.0;
        let z0 = match item.cabinet_type {
            CabinetType::Wall => room.spec.upper_top() - item.h,
            CabinetType::Base | CabinetType::Tall => room.spec.toe_height,
        };
        let ghost = rect_mm(x, z0, x + item.w, z0 + item.h);
        painter.rect_filled(
            ghost,
            0.0,
            Color32::from_rgba_unmultiplied(235, 120, 20, 40),
        );
        painter.rect_stroke(ghost, 0.0, Stroke::new(1.5f32, SELECT), StrokeKind::Inside);
    }
    // položka katalógu pustená na stenu: stred pod kurzorom, prekrytie rieši cabinet.add
    if let Some(item) = response.dnd_release_payload::<crate::catalog_panel::CatalogDrag>() {
        if let Some(p) = response.hover_pos() {
            let x = xf.to_mm(p)[0];
            st.dropped = Some(item.add_params(&wall.id, x - item.w / 2.0));
        }
    }
    // pravý klik: vyberie skrinku a otvor pod kurzorom, ponuka otvoru a predvoľby čiel
    // ako v CV: na nevybranej skrinke ponuka skrinky, na vybranej skrinke ponuka otvoru
    if response.secondary_clicked() {
        if let Some(p) = response.interact_pointer_pos() {
            match hit(p) {
                Some(i) if i == *selected_cabinet => {
                    st.opening = opening_at(p, i).and_then(|(r, k)| cab_id(i).map(|id| (id, r, k)));
                }
                Some(i) => {
                    *selected_cabinet = i;
                    st.opening = None;
                }
                None => {}
            }
        }
    }
    let target = *selected_cabinet;
    if on_wall.contains(&target) {
        let opening = st.opening.clone();
        let mut cmds: Vec<(&'static str, serde_json::Value)> = Vec::new();
        let mut section_request: Option<String> = None;
        let mut props_request: Option<String> = None;
        let mut open_request: Option<String> = None;
        response.context_menu(|ui| {
            let Some(c) = project.cabinets.get_mut(target) else {
                ui.close_menu();
                return;
            };
            if let Some((id, row, col)) = &opening {
                opening_menu(ui, c, id, *row, *col, &mut cmds);
                return;
            }
            // ponuka skrinky ako v CV: Copy, Delete | Section | Lock Width | …
            ui.label(egui::RichText::new(tr!("wall_view.cabinet.title", cabinet = &c.id)).strong());
            if ui.button(tr!("wall_view.cabinet.copy")).clicked() {
                cmds.push(("cabinet.duplicate", serde_json::json!({ "id": c.id })));
                ui.close_menu();
            }
            if ui.button(tr!("wall_view.cabinet.delete")).clicked() {
                cmds.push(("cabinet.remove", serde_json::json!({ "id": c.id })));
                ui.close_menu();
            }
            ui.separator();
            if ui
                .add_enabled(
                    !c.fronts.is_empty(),
                    egui::Button::new(tr!("wall_view.cabinet.section")),
                )
                .on_hover_text(tr!("wall_view.cabinet.section_hint"))
                .clicked()
            {
                section_request = Some(c.id.clone());
                ui.close_menu();
            }
            ui.separator();
            let lock = check_label(c.lock_width, &tr!("wall_view.cabinet.lock_width"));
            if ui.button(lock).clicked() {
                cmds.push((
                    "cabinet.set",
                    serde_json::json!({ "id": c.id, "set": { "lock_width": !c.lock_width } }),
                ));
                ui.close_menu();
            }
            for (side, label) in [
                ("left", tr!("wall_view.cabinet.left_corner")),
                ("right", tr!("wall_view.cabinet.right_corner")),
            ] {
                if ui.button(label).clicked() {
                    cmds.push((
                        "cabinet.snap_corner",
                        serde_json::json!({ "id": c.id, "side": side }),
                    ));
                    ui.close_menu();
                }
            }
            ui.separator();
            if ui.button(tr!("wall_view.cabinet.open")).clicked() {
                open_request = Some(c.id.clone());
                ui.close_menu();
            }
            if ui.button(tr!("wall_view.cabinet.properties")).clicked() {
                props_request = Some(c.id.clone());
                ui.close_menu();
            }
            ui.separator();
            ui.label(egui::RichText::new(tr!("wall_view.cabinet.front_presets")).strong());
            for (label, preset) in crate::side_panel::front_presets() {
                if ui.selectable_label(c.fronts == preset, label).clicked() {
                    c.fronts = preset;
                    changed = true;
                    ui.close_menu();
                }
            }
            let doors = c.fronts.iter().any(|f| f.kind == cc_model::FrontKind::Door);
            if doors {
                ui.separator();
                for (side, label) in [
                    (
                        cc_model::HingeSide::Left,
                        tr!("wall_view.cabinet.hinges_left"),
                    ),
                    (
                        cc_model::HingeSide::Right,
                        tr!("wall_view.cabinet.hinges_right"),
                    ),
                ] {
                    if ui.button(label).clicked() {
                        for f in c
                            .fronts
                            .iter_mut()
                            .filter(|f| f.kind == cc_model::FrontKind::Door)
                        {
                            f.hinge_side = (side == cc_model::HingeSide::Right).then_some(side);
                        }
                        changed = true;
                        ui.close_menu();
                    }
                }
            }
        });
        st.commands.extend(cmds);
        if let Some(id) = section_request {
            st.opening = Some((id, 0, None));
        }
        if open_request.is_some() {
            st.open_cabinet = open_request;
        }
        if props_request.is_some() {
            st.props = props_request;
        }
    }
    changed
}

/// Ponuka otvoru čela ako v CV (pravý klik v Section Face): Split Vertically / Horizontally,
/// Lock / Equalize Width, Lock / Equalize Height. Typ a rozmery sú v bočnom paneli otvoru.
pub(crate) fn opening_menu(
    ui: &mut egui::Ui,
    c: &cc_model::CabinetSpec,
    id: &str,
    row: usize,
    col: Option<usize>,
    cmds: &mut Vec<(&'static str, serde_json::Value)>,
) {
    use serde_json::json;
    let Some(r) = c.fronts.get(row) else {
        return;
    };
    let cell = col.and_then(|k| r.columns.get(k));
    let target = |extra: serde_json::Value| {
        let mut p = json!({ "cabinet": id, "row": row });
        if let Some(k) = col {
            p["column"] = json!(k);
        }
        if let (Some(o), serde_json::Value::Object(e)) = (p.as_object_mut(), extra) {
            o.extend(e);
        }
        p
    };
    let name = opening_name(row, col);
    ui.label(egui::RichText::new(tr!("wall_view.opening.title", name = name)).strong());
    if ui.button(tr!("wall_view.opening.split_vertical")).clicked() {
        cmds.push(("front.split", target(json!({ "direction": "vertical" }))));
        ui.close_menu();
    }
    if ui
        .add_enabled(
            col.is_none(),
            egui::Button::new(tr!("wall_view.opening.split_horizontal")),
        )
        .clicked()
    {
        cmds.push(("front.split", target(json!({ "direction": "horizontal" }))));
        ui.close_menu();
    }
    ui.separator();
    // šírka sa zamyká iba pri stĺpci, výška pri riadku (ako v CV: druhá voľba je sivá)
    let width_locked = cell.is_some_and(|f| f.width.is_some());
    let height_locked = r.height.is_some();
    let row_only = |k: Option<usize>| k.is_none();
    let mut check = |ui: &mut egui::Ui, on: bool, enabled: bool, label: &str, lock: bool| {
        if ui
            .add_enabled(enabled, egui::Button::new(check_label(on, label)))
            .clicked()
        {
            cmds.push(("front.set", target(json!({ "lock": lock }))));
            ui.close_menu();
        }
    };
    check(
        ui,
        width_locked,
        col.is_some(),
        &tr!("wall_view.opening.lock_width"),
        true,
    );
    check(
        ui,
        !width_locked,
        col.is_some(),
        &tr!("wall_view.opening.equal_width"),
        false,
    );
    ui.separator();
    check(
        ui,
        height_locked,
        row_only(col),
        &tr!("wall_view.opening.lock_height"),
        true,
    );
    check(
        ui,
        !height_locked,
        row_only(col),
        &tr!("wall_view.opening.equal_height"),
        false,
    );
    ui.separator();
    if ui.button(tr!("wall_view.opening.remove_split")).clicked() {
        cmds.push(("front.remove", target(json!({}))));
        ui.close_menu();
    }
}

/// Položka ponuky so zaškrtnutím: „✔ text“, inak odsadený text.
fn check_label(on: bool, label: &str) -> String {
    if on {
        format!("✔ {label}")
    } else {
        format!("    {label}")
    }
}

/// Meno otvoru ako v CV: číslo stĺpca (1…) a písmeno riadku zhora (A…).
pub fn opening_name(row: usize, col: Option<usize>) -> String {
    let letter = char::from(b'A' + u8::try_from(row.min(25)).unwrap_or(25));
    format!("{}{letter}", col.map_or(1, |k| k + 1))
}

/// Obálka dielu v súradniciach skrinky (min, max).
fn cabinet_box(p: &Part) -> (DVec3, DVec3) {
    let a = p.transform.point_to_cabinet(DVec3::ZERO);
    let b = p
        .transform
        .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
    (a.min(b), a.max(b))
}

/// Poloha hrany pri zmene šírky: prichytí sa ku koncom steny a hranám skriniek
/// toho istého radu, inak celé mm.
fn snap_edge(project: &ProjectFile, index: usize, wall: &cc_room::Wall, raw: f64, tol: f64) -> f64 {
    let Some(c) = project.cabinets.get(index) else {
        return raw.round();
    };
    let mut targets = vec![0.0, wall.length];
    for (j, o) in project.cabinets.iter().enumerate() {
        if j != index
            && o.wall.as_deref() == Some(wall.id.as_str())
            && [Row::Base, Row::Upper]
                .into_iter()
                .any(|r| r.contains(c.cabinet_type) && r.contains(o.cabinet_type))
        {
            targets.extend([o.at, o.at + o.w]);
        }
    }
    targets
        .into_iter()
        .map(|t| (t, (t - raw).abs()))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(raw.round(), |(t, _)| t)
}

fn snap(project: &ProjectFile, index: usize, wall: &cc_room::Wall, raw: f64, tol: f64) -> f64 {
    let Some(c) = project.cabinets.get(index) else {
        return raw;
    };
    let rows: Vec<Row> = [Row::Base, Row::Upper]
        .into_iter()
        .filter(|r| r.contains(c.cabinet_type))
        .collect();
    let mut targets = vec![0.0, wall.length - c.w];
    for (j, o) in project.cabinets.iter().enumerate() {
        if j != index
            && o.wall.as_deref() == Some(wall.id.as_str())
            && rows.iter().any(|r| r.contains(o.cabinet_type))
        {
            targets.extend([o.at + o.w, o.at - c.w, o.at]);
        }
    }
    targets
        .into_iter()
        .map(|t| (t, (t - raw).abs()))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(raw.round(), |(t, _)| t)
}
