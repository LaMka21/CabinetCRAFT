//! Výkresy na papieri (A4 na šírku, rám a popisové pole):
//! - 3D celej zákazky so stenami alebo 3D jednej skrinky,
//! - výkres dielu (pohľad na plochu F s obrábaním a malý 3D náhľad dielu), pod ním
//!   tabuľka operácií a program pre stroj.

use std::hash::{Hash, Hasher};

use cc_i18n::tr;
use cc_model::{Model, Operation};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke};

use crate::app::App;
use crate::grid::{self, col, num, Cell};
use crate::part_view::{self, face_name, fmt};
use crate::tabs::role_name;
use crate::theme;
use cc_scene::{Camera, Scene, SceneOptions};

/// Výška popisového poľa na papieri.
const TITLE_H: f32 = 98.0;
/// Okraj pre kóty okolo pravouhlého pohľadu.
const DIM_MARGIN: f32 = 28.0;
/// Rovnaké vyplnenie ako pri pravouhlom pohľade v `render3d::Projection`.
const ORTHO_FIT: f32 = 0.86;
/// Veľkosť 3D náhľadu dielu na výkrese dielu.
const THUMB: egui::Vec2 = egui::Vec2::new(220.0, 150.0);

/// Čo je na papieri.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) enum Sheet {
    /// Výkres vybraného dielu.
    #[default]
    Part,
    /// 3D celej zákazky so stenami.
    Job,
    /// 3D jednej skrinky.
    Cabinet(String),
}

impl Sheet {
    /// Zápis pre riadiaci kanál: `part`, `job`, `cabinet:ID`.
    pub(crate) fn parse(s: &str) -> Option<Sheet> {
        match s {
            "part" => Some(Sheet::Part),
            "job" => Some(Sheet::Job),
            _ => s
                .strip_prefix("cabinet:")
                .map(|id| Sheet::Cabinet(id.to_owned())),
        }
    }

    pub(crate) fn key(&self) -> String {
        match self {
            Sheet::Part => "part".to_owned(),
            Sheet::Job => "job".to_owned(),
            Sheet::Cabinet(id) => format!("cabinet:{id}"),
        }
    }
}

/// Vykreslený 3D obraz na papieri (prestaví sa pri zmene modelu, výkresu alebo veľkosti).
struct Render {
    key: u64,
    texture: egui::TextureHandle,
    /// Popisy skriniek: poloha v obraze (0…1) a text.
    labels: Vec<(egui::Pos2, String)>,
}

#[derive(Default)]
pub(crate) struct DrawingState {
    pub sheet: Sheet,
    /// 3D otvorenej dosky.
    part3d: Option<Render>,
    /// Obrazy pohľadov na papieri (najviac štyri).
    views: [Option<Render>; 4],
    thumb_img: Option<Render>,
}

/// Pohľad na výkrese.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum View {
    /// 3D (izometria s perspektívou)
    Iso,
    /// spredu (pravouhlo)
    Front,
    /// zhora = pôdorys (pravouhlo)
    Top,
    /// zľava (pravouhlo)
    Left,
}

impl View {
    fn caption(self) -> String {
        match self {
            View::Iso => tr!("drawings_view.view.iso"),
            View::Front => tr!("drawings_view.view.front"),
            View::Top => tr!("drawings_view.view.top"),
            View::Left => tr!("drawings_view.view.left"),
        }
    }

    /// Šírka a výška obrysu skrinky `[w, h, d]` v tomto pohľade (iba pravouhlé pohľady).
    fn extents(self, [w, h, d]: [f32; 3]) -> Option<[f32; 2]> {
        let e = match self {
            View::Iso => return None,
            View::Front => [w, h],
            View::Top => [w, d],
            View::Left => [d, h],
        };
        (e[0] > 0.0 && e[1] > 0.0).then_some(e)
    }

    /// Kamera: os z nahor, predok skriniek na y = 0.
    fn camera(self) -> Camera {
        use std::f64::consts::FRAC_PI_2;
        let ortho = |yaw: f64, pitch: f64| Camera {
            yaw,
            pitch,
            zoom: 1.0,
            pan: [0.0, 0.0],
            perspective: false,
            fit: None,
        };
        match self {
            View::Iso => Camera::default(),
            View::Front => ortho(0.0, 0.0),
            View::Top => ortho(0.0, FRAC_PI_2),
            View::Left => ortho(FRAC_PI_2, 0.0),
        }
    }
}

// Hlavičky stĺpcov s bodkou sú kľúče prekladu (preloží ich `grid::show`).
const OP_COLUMNS: [grid::Col; 8] = [
    num("#", 34.0),
    col("drawings_view.ops.operation", 70.0),
    col("drawings_view.ops.face", 96.0),
    num("X", 90.0),
    num("Y", 90.0),
    num("Z", 50.0),
    col("drawings_view.ops.size", 70.0),
    num("drawings_view.ops.depth", 56.0),
];

impl App {
    pub(crate) fn tab_part(&mut self, ui: &mut egui::Ui) {
        match self.drawing.sheet.clone() {
            Sheet::Part => self.sheet_part(ui),
            Sheet::Job => self.sheet_3d(ui, None),
            Sheet::Cabinet(id) => self.sheet_3d(ui, Some(id)),
        }
    }

    /// Zákazka (3D so stenami, pôdorys, pohľad spredu) alebo skrinka (3D, spredu,
    /// zhora, zľava) na celom papieri.
    fn sheet_3d(&mut self, ui: &mut egui::Ui, cabinet: Option<String>) {
        let Some(built) = self.model_built() else {
            return;
        };
        let Some(doc) = self.session.doc() else {
            return;
        };
        let project = doc.project.clone();
        let model = filtered(&built.model, cabinet.as_deref(), None);
        if model.cabinets.is_empty() {
            ui.label(tr!("drawings_view.cabinet_missing"));
            return;
        }
        let (_, drawing, title) = paper(ui, ui.available_height());
        let opts = SceneOptions {
            walls: cabinet.is_none(),
            operations: cabinet.is_some(),
            ..SceneOptions::default()
        };
        // rozvrhnutie pohľadov na papieri
        let gap = 10.0;
        let half_w = (drawing.width() - gap) / 2.0;
        let half_h = (drawing.height() - gap) / 2.0;
        let cell = |x: f32, y: f32, w: f32, h: f32| {
            Rect::from_min_size(drawing.min + vec2(x, y), vec2(w, h))
        };
        let views: Vec<(View, Rect)> = match &cabinet {
            None => {
                let big_w = drawing.width() * 0.6;
                let side_w = drawing.width() - big_w - gap;
                vec![
                    (View::Iso, cell(0.0, 0.0, big_w, drawing.height())),
                    (View::Top, cell(big_w + gap, 0.0, side_w, half_h)),
                    (View::Front, cell(big_w + gap, half_h + gap, side_w, half_h)),
                ]
            }
            Some(_) => vec![
                (View::Iso, cell(0.0, 0.0, half_w, half_h)),
                (View::Front, cell(half_w + gap, 0.0, half_w, half_h)),
                (View::Top, cell(0.0, half_h + gap, half_w, half_h)),
                (View::Left, cell(half_w + gap, half_h + gap, half_w, half_h)),
            ],
        };
        let ppp = ui.ctx().pixels_per_point();
        let model_ptr = std::sync::Arc::as_ptr(&built) as usize;
        let painter = ui.painter().clone();
        // miesto na popis pohľadu
        let image_of = |rect: &Rect| Rect::from_min_max(rect.min + vec2(0.0, 16.0), rect.max);
        // Pravouhlé pohľady skrinky majú spoločnú mierku (body na mm), aby sa dali
        // porovnať a okótovať.
        let size = cabinet
            .as_ref()
            .and_then(|_| model.cabinets.first())
            .map(|c| [c.w as f32, c.h as f32, c.d as f32]);
        let scale_in = |view: View, r: Rect| -> Option<(f32, [f32; 2])> {
            let ext = view.extents(size?)?;
            Some((
                (r.width() / ext[0]).min(r.height() / ext[1]) * ORTHO_FIT,
                ext,
            ))
        };
        let common = views
            .iter()
            .filter_map(|(v, r)| scale_in(*v, image_of(r).shrink(DIM_MARGIN)).map(|(s, _)| s))
            .fold(f32::INFINITY, f32::min);
        for (slot, (view, rect)) in views.iter().enumerate() {
            let image_rect = image_of(rect);
            let mut camera = view.camera();
            let ortho = scale_in(*view, image_rect).filter(|_| common.is_finite());
            if let Some((full, _)) = ortho {
                camera.zoom = f64::from(common / full);
            }
            let key = hash_key(&(
                model_ptr,
                self.drawing.sheet.key(),
                *view,
                (image_rect.width() * ppp) as u32,
                (image_rect.height() * ppp) as u32,
                camera.zoom.to_bits(),
            ));
            let Some(target) = self.drawing.views.get_mut(slot) else {
                continue;
            };
            let img = render_cached(
                ui.ctx(),
                target,
                key,
                &model,
                &project,
                &opts,
                &camera,
                image_rect.size() * ppp,
                ortho.map(|_| common * ppp),
            );
            painter.text(
                rect.min,
                Align2::LEFT_TOP,
                view.caption(),
                FontId::proportional(theme::SMALL + 0.5),
                theme::TEXT,
            );
            painter.rect_stroke(
                *rect,
                0.0,
                Stroke::new(1.0_f32, theme::GRID_LINE),
                egui::StrokeKind::Outside,
            );
            let Some(img) = img else {
                continue;
            };
            painter.image(
                img.texture.id(),
                image_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            // kóty: obrys skrinky je v strede obrazu v spoločnej mierke
            if let (Some((_, ext)), true) = (ortho, common.is_finite()) {
                let b = Rect::from_center_size(
                    image_rect.center(),
                    vec2(ext[0] * common, ext[1] * common),
                );
                dimension(
                    &painter,
                    b.left_bottom(),
                    b.right_bottom(),
                    vec2(0.0, 14.0),
                    &fmt(f64::from(ext[0])),
                );
                dimension(
                    &painter,
                    b.left_bottom(),
                    b.left_top(),
                    vec2(-14.0, 0.0),
                    &fmt(f64::from(ext[1])),
                );
            }
            if ortho.is_none() {
                let clip = painter.with_clip_rect(image_rect);
                for (pos, label) in &img.labels {
                    clip.text(
                        image_rect.min
                            + vec2(pos.x * image_rect.width(), pos.y * image_rect.height()),
                        Align2::CENTER_BOTTOM,
                        label,
                        FontId::proportional(theme::SMALL),
                        theme::TEXT,
                    );
                }
            }
        }
        let parts: usize = model.cabinets.iter().map(|c| c.parts.len()).sum();
        let (name, detail) = match &cabinet {
            None => (
                tr!("drawings_view.sheet.job"),
                tr!(
                    "drawings_view.sheet.job_detail",
                    cabinets = model.cabinets.len(),
                    parts = parts
                ),
            ),
            Some(id) => {
                let c = model.cabinets.first();
                (
                    tr!("drawings_view.sheet.cabinet", id = id),
                    c.map_or(String::new(), |c| {
                        tr!(
                            "drawings_view.sheet.cabinet_detail",
                            w = fmt(c.w),
                            h = fmt(c.h),
                            d = fmt(c.d),
                            parts = parts
                        )
                    }),
                )
            }
        };
        title_block(
            &painter,
            title,
            &[
                job_rows(&project.project),
                vec![
                    (tr!("drawings_view.title.drawing"), name),
                    (tr!("drawings_view.title.content"), detail),
                    (
                        tr!("drawings_view.title.material"),
                        project.defaults.panel_material.clone(),
                    ),
                ],
            ]
            .concat(),
        );
    }

    /// Výkres dielu: pohľad na plochu F, 3D náhľad, operácie a program.
    fn sheet_part(&mut self, ui: &mut egui::Ui) {
        let Some(built) = self.model_built() else {
            return;
        };
        let ids: Vec<String> = built.model.parts().map(|p| p.id.clone()).collect();
        let selected = match self.selected_part() {
            Some(id) if ids.contains(&id) => id,
            _ => built
                .model
                .parts()
                .find(|p| !p.operations.is_empty())
                .map(|p| p.id.clone())
                .unwrap_or_default(),
        };
        if !selected.is_empty() && self.selected_part().as_deref() != Some(selected.as_str()) {
            self.select_part(Some(selected.clone()));
        }
        let Some(part) = built.model.parts().find(|p| p.id == selected).cloned() else {
            return;
        };
        let info = self
            .session
            .doc()
            .map(|d| d.project.project.clone())
            .unwrap_or_else(|| cc_model::ProjectInfo::named(""));
        let program = self.programs.get(&part.id).cloned();

        // papier
        let paper_h = (ui.available_height() * 0.62).max(280.0);
        let (paper, drawing, title) = paper(ui, paper_h);
        let hovered = {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(drawing));
            part_view::draw(&mut child, &part, self.hovered_op, drawing.height())
        };
        // 3D náhľad dielu v pravom dolnom rohu papiera
        if let Some(doc) = self.session.doc() {
            let project = doc.project.clone();
            let model = filtered(&built.model, None, Some(&part.id));
            let thumb = Rect::from_min_size(
                pos2(paper.max.x - 14.0 - THUMB.x, paper.max.y - 10.0 - THUMB.y),
                THUMB,
            );
            let ppp = ui.ctx().pixels_per_point();
            let key = hash_key(&(
                std::sync::Arc::as_ptr(&built) as usize,
                part.id.as_str(),
                (thumb.width() * ppp) as u32,
            ));
            let opts = SceneOptions {
                walls: false,
                ..SceneOptions::default()
            };
            let img = render_cached(
                ui.ctx(),
                &mut self.drawing.thumb_img,
                key,
                &model,
                &project,
                &opts,
                &Camera::default(),
                thumb.size() * ppp,
                None,
            );
            if let Some(img) = img {
                let p = ui.painter();
                p.image(
                    img.texture.id(),
                    thumb,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                p.rect_stroke(
                    thumb,
                    0.0,
                    Stroke::new(1.0_f32, theme::BORDER),
                    egui::StrokeKind::Inside,
                );
                p.text(
                    thumb.min + vec2(4.0, 3.0),
                    Align2::LEFT_TOP,
                    tr!("drawings_view.view.iso"),
                    FontId::proportional(theme::SMALL),
                    theme::TEXT_WEAK,
                );
            }
        }
        title_block(
            ui.painter(),
            title,
            &[
                job_rows(&info),
                vec![
                    (
                        tr!("drawings_view.title.part"),
                        format!("{} – {}", part.id, role_name(part.role)),
                    ),
                    (
                        tr!("drawings_view.title.size"),
                        format!(
                            "{} × {} × {} mm · {}",
                            fmt(part.length),
                            fmt(part.width),
                            fmt(part.thickness),
                            part.material
                        ),
                    ),
                    (
                        tr!("drawings_view.title.program"),
                        program.unwrap_or_else(|| tr!("drawings_view.title.no_program")),
                    ),
                ],
            ]
            .concat(),
        );

        // operácie a program
        ui.add_space(6.0);
        let mut table_hover = None;
        let profile = self
            .session
            .doc()
            .and_then(|d| d.machine.as_ref())
            .and_then(|m| m.xcs.clone());
        let rows: Vec<Vec<Cell>> = part
            .operations
            .iter()
            .enumerate()
            .map(|(i, op)| {
                let mut cells: Vec<Cell> = vec![(i + 1).to_string().into()];
                match *op {
                    Operation::Hole {
                        face,
                        x,
                        y,
                        z,
                        diameter,
                        depth,
                        dowel,
                        ..
                    } => cells.extend([
                        if dowel {
                            tr!("drawings_view.ops.dowel")
                        } else {
                            tr!("drawings_view.ops.hole")
                        }
                        .into(),
                        face_name(face).into(),
                        fmt(x).into(),
                        fmt(y).into(),
                        fmt(z).into(),
                        format!("Ø{}", fmt(diameter)).into(),
                        fmt(depth).into(),
                    ]),
                    Operation::Groove {
                        face,
                        start,
                        end,
                        width,
                        depth,
                    } => cells.extend([
                        tr!("drawings_view.ops.groove").into(),
                        face_name(face).into(),
                        format!("{} → {}", fmt(start.x), fmt(end.x)).into(),
                        format!("{} → {}", fmt(start.y), fmt(end.y)).into(),
                        "–".into(),
                        fmt(width).into(),
                        fmt(depth).into(),
                    ]),
                }
                if hovered == Some(i) {
                    for c in &mut cells {
                        c.fill = Some(theme::EDIT_ROW);
                    }
                }
                cells
            })
            .collect();
        ui.columns(2, |cols| {
            let r = grid::show(&mut cols[0], "ops", &OP_COLUMNS, rows.len(), None, |i| {
                rows.get(i).cloned().unwrap_or_default()
            });
            table_hover = r.hovered;
            let ui = &mut cols[1];
            let program = profile.as_ref().map(|profile| {
                let out = cc_out_xcs::render(&built.model, profile);
                out.files
                    .into_iter()
                    .filter(|f| f.part_id == part.id)
                    .map(|f| {
                        (
                            f.name,
                            String::from_utf8_lossy(&f.content).replace("\r\n", "\n"),
                        )
                    })
                    .collect::<Vec<_>>()
            });
            match program {
                Some(files) if !files.is_empty() => {
                    egui::ScrollArea::both()
                        .id_salt("xcs")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (name, text) in files {
                                ui.label(
                                    RichText::new(tr!("drawings_view.program_file", name = &name))
                                        .strong(),
                                );
                                ui.add(egui::Label::new(RichText::new(text).monospace()).extend());
                                ui.add_space(8.0);
                            }
                        });
                }
                Some(_) => {
                    ui.label(tr!("drawings_view.no_machining"));
                }
                None => {
                    ui.label(tr!("drawings_view.no_xcs"));
                }
            }
        });
        self.hovered_op = hovered.or(table_hover);
    }
}

/// Najväčší obdĺžnik s pomerom strán `aspect` (šírka / výška) v strede `outer`.
fn fit(outer: Rect, aspect: f32) -> Rect {
    let (w, h) = if outer.width() / outer.height().max(1.0) > aspect {
        (outer.height() * aspect, outer.height())
    } else {
        (outer.width(), outer.width() / aspect)
    };
    Rect::from_center_size(outer.center(), vec2(w.max(10.0), h.max(10.0)))
}

/// Riadky popisového poľa o zákazke: firma, číslo a meno, zákazník, kreslil.
fn job_rows(info: &cc_model::ProjectInfo) -> Vec<(String, String)> {
    let job = match info.number.as_deref().map(str::trim) {
        Some(n) if !n.is_empty() => format!("{n} – {}", info.name),
        _ => info.name.clone(),
    };
    let mut rows = vec![
        (
            tr!("drawings_view.title.company"),
            info.company
                .clone()
                .unwrap_or_else(|| "CabinetCRAFT".to_owned()),
        ),
        (tr!("drawings_view.title.job"), job),
    ];
    for (label, value) in [
        (tr!("drawings_view.title.customer"), &info.customer),
        (tr!("drawings_view.title.designer"), &info.designer),
    ] {
        if let Some(v) = value.as_ref().filter(|v| !v.trim().is_empty()) {
            rows.push((label, v.clone()));
        }
    }
    rows
}

/// Popisové pole: riadky „meno: hodnota“ oddelené čiarami.
fn title_block(p: &egui::Painter, rect: Rect, rows: &[(String, String)]) {
    let n = rows.len().max(1) as f32;
    let h = rect.height() / n;
    let clip = p.with_clip_rect(rect.intersect(p.clip_rect()));
    for (i, (label, value)) in rows.iter().enumerate() {
        let y = rect.min.y + h * i as f32;
        clip.text(
            pos2(rect.min.x + 2.0, y + h / 2.0),
            Align2::LEFT_CENTER,
            format!("{label}:"),
            FontId::proportional(theme::SMALL),
            theme::TEXT,
        );
        clip.text(
            pos2(rect.min.x + 58.0, y + h / 2.0),
            Align2::LEFT_CENTER,
            value,
            FontId::proportional(theme::SMALL),
            theme::BLUE,
        );
        p.line_segment(
            [pos2(rect.min.x, y + h), pos2(rect.max.x, y + h)],
            Stroke::new(1.0_f32, Color32::BLACK),
        );
    }
}

/// Papier A4 na šírku v strede plochy výšky `height`: (papier, plocha na kreslenie,
/// popisové pole). Plocha na kreslenie končí nad popisovým poľom.
fn paper(ui: &mut egui::Ui, height: f32) -> (Rect, Rect, Rect) {
    let (outer, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let paper = fit(outer.shrink(10.0), 297.0 / 210.0);
    let p = ui.painter();
    p.rect_filled(
        paper.translate(vec2(4.0, 4.0)),
        0.0,
        Color32::from_gray(150),
    );
    p.rect_filled(paper, 0.0, Color32::WHITE);
    p.rect_stroke(
        paper,
        0.0,
        Stroke::new(1.0_f32, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
    let drawing = Rect::from_min_max(
        paper.min + vec2(14.0, 10.0),
        paper.max - vec2(14.0, 14.0 + TITLE_H),
    );
    let title = Rect::from_min_size(
        pos2(paper.min.x + 14.0, paper.max.y - 10.0 - TITLE_H),
        vec2(380.0_f32.min(paper.width() - 28.0), TITLE_H),
    );
    (paper, drawing, title)
}

impl App {
    /// Samotná doska v 3D cez celú plochu (otvorená doska, záložka 3D).
    pub(crate) fn part_render(&mut self, ui: &mut egui::Ui, id: &str, camera: Camera) {
        let Some(built) = self.model_built() else {
            return;
        };
        let Some(doc) = self.session.doc() else {
            return;
        };
        let project = doc.project.clone();
        let model = filtered(&built.model, None, Some(id));
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let ppp = ui.ctx().pixels_per_point();
        let key = hash_key(&(
            std::sync::Arc::as_ptr(&built) as usize,
            id,
            (rect.width() * ppp) as u32,
            (rect.height() * ppp) as u32,
            camera.yaw.to_bits(),
            camera.pitch.to_bits(),
        ));
        let opts = SceneOptions {
            walls: false,
            ..SceneOptions::default()
        };
        let painter = ui.painter().clone();
        let img = render_cached(
            ui.ctx(),
            &mut self.drawing.part3d,
            key,
            &model,
            &project,
            &opts,
            &camera,
            rect.size() * ppp,
            None,
        );
        if let Some(img) = img {
            painter.image(
                img.texture.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

/// Kópia modelu iba so skrinkou `cabinet` alebo iba s dielom `part`.
fn filtered(model: &Model, cabinet: Option<&str>, part: Option<&str>) -> Model {
    let mut m = model.clone();
    if let Some(id) = cabinet {
        m.cabinets.retain(|c| c.id == id);
    }
    if let Some(id) = part {
        for c in &mut m.cabinets {
            c.parts.retain(|p| p.id == id);
        }
        m.cabinets.retain(|c| !c.parts.is_empty());
    }
    m
}

fn hash_key(value: &impl Hash) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// Vykreslí scénu do textúry, ak sa kľúč zmenil; inak vráti uložený obraz.
/// `ortho_px`: pravouhlý pohľad v presnej mierke (pixely obrazu na mm), aby sedeli kóty;
/// kamera inak vmestí guľovú obálku scény (nezávislú od natočenia).
#[allow(clippy::too_many_arguments)]
fn render_cached<'a>(
    ctx: &egui::Context,
    slot: &'a mut Option<Render>,
    key: u64,
    model: &Model,
    project: &cc_model::ProjectFile,
    opts: &SceneOptions,
    camera: &Camera,
    size: egui::Vec2,
    ortho_px: Option<f32>,
) -> Option<&'a Render> {
    let width = size.x.round().clamp(16.0, 4000.0) as usize;
    let height = size.y.round().clamp(16.0, 4000.0) as usize;
    if slot.as_ref().map(|r| r.key) != Some(key) {
        let scene = Scene::build(model, project, opts);
        let mut camera = *camera;
        if let Some(px) = ortho_px.filter(|px| px.is_finite() && *px > 0.0) {
            // mierka projekcie = min(š, v) / uhlopriečka obálky × zoom
            let (lo, hi) = scene.bounds();
            let diagonal = (hi - lo).length().max(1.0);
            camera.zoom = f64::from(px) * diagonal / width.min(height).max(1) as f64;
        }
        let img = cc_scene::render(
            &scene,
            &camera,
            width,
            height,
            2,
            1.0,
            cc_scene::Selection::default(),
            [255, 255, 255],
        );
        let labels = scene
            .labels
            .iter()
            .map(|(p, text)| {
                let s = img.projection.project(*p);
                (
                    pos2(
                        (s.x / img.width.max(1) as f64) as f32,
                        (s.y / img.height.max(1) as f64) as f32,
                    ),
                    text.clone(),
                )
            })
            .collect();
        let color = egui::ColorImage::from_rgb([img.width, img.height], &img.rgb);
        let texture = match slot.take() {
            Some(mut r) => {
                r.texture.set(color, egui::TextureOptions::LINEAR);
                r.texture
            }
            None => ctx.load_texture("drawing", color, egui::TextureOptions::LINEAR),
        };
        *slot = Some(Render {
            key,
            texture,
            labels,
        });
    }
    slot.as_ref()
}

/// Kóta medzi bodmi `a` a `b` posunutá o `offset` (body), text v strede.
fn dimension(p: &egui::Painter, a: egui::Pos2, b: egui::Pos2, offset: egui::Vec2, text: &str) {
    let stroke = Stroke::new(1.0_f32, theme::BLUE);
    let (a2, b2) = (a + offset, b + offset);
    let ext = offset.normalized() * 3.0;
    p.line_segment([a + offset.normalized() * 2.0, a2 + ext], stroke);
    p.line_segment([b + offset.normalized() * 2.0, b2 + ext], stroke);
    p.line_segment([a2, b2], stroke);
    let d = (b2 - a2).normalized();
    let n = vec2(-d.y, d.x) * 3.0;
    for q in [a2, b2] {
        p.line_segment([q - d * 3.0 - n, q + d * 3.0 + n], stroke);
    }
    let mid = a2 + (b2 - a2) / 2.0 + offset.normalized() * 8.0;
    let angle = if d.x.abs() < 0.5 {
        -std::f32::consts::FRAC_PI_2
    } else {
        0.0
    };
    let galley = p.layout_no_wrap(
        text.to_owned(),
        FontId::proportional(theme::SMALL),
        theme::BLUE,
    );
    let size = galley.size();
    let pos = if angle == 0.0 {
        mid - size / 2.0
    } else {
        mid + vec2(-size.y / 2.0, size.x / 2.0)
    };
    p.add(egui::epaint::TextShape::new(pos, galley, theme::BLUE).with_angle(angle));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hlavičky stĺpcov s bodkou sú kľúče, ktoré `cargo xtask i18n` nevidí.
    #[test]
    fn column_titles_are_translated() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for c in OP_COLUMNS.iter().filter(|c| c.title.contains('.')) {
                assert!(catalog.contains(c.title), "{code}.toml: chýba {}", c.title);
            }
        }
    }
}
