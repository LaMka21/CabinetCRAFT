//! Záložka 3D: ovládanie pohľadu, pohľady spredu/zhora/izo, priblíženie na výber,
//! výber dielu lúčom a kreslenie scény: na GPU (`gpu3d.rs`), inak CPU raster z `cc-scene`.

use std::sync::Arc;

use cc_i18n::tr;
use cc_scene::{Camera, Projection, Scene, Selection};
use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, Vec2};

use crate::app::{App, Tab};
use crate::gpu3d::{Frame3d, Mesh};
use crate::part_view::fmt;
use crate::tabs::role_name;

pub(crate) struct View3d {
    pub scene: Scene,
    pub scene_key: (usize, [u64; 4]),
    /// pribúda pri každej prestavbe scény (kľúč nahratých bufferov na GPU)
    pub generation: u64,
    /// vrcholy pre GPU (vytvoria sa až pri prvom kreslení na GPU)
    pub mesh: Option<Arc<Mesh>>,
    pub image_key: Option<u64>,
    pub texture: Option<egui::TextureHandle>,
    pub projection: Option<Projection>,
    /// Posledný 3D výrez okna (pre `ui.render` s `window: true` a `ui.inspect`).
    pub viewport: Option<Viewport>,
    /// Iba táto skrinka (editor skrinky); `None` = celá zákazka.
    pub focus: Option<String>,
    /// Editor skrinky: dvojklik na dosku (ID dielu na otvorenie).
    pub open_part: Option<String>,
    pub scene_focus: Option<String>,
}

impl Default for View3d {
    fn default() -> Self {
        View3d {
            scene: Scene::default(),
            scene_key: (usize::MAX, [0; 4]),
            generation: 0,
            mesh: None,
            image_key: None,
            texture: None,
            projection: None,
            viewport: None,
            focus: None,
            open_part: None,
            scene_focus: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Viewport {
    /// ľavý horný roh v bodoch okna
    pub min: [f32; 2],
    /// veľkosť v pixeloch
    pub size: [usize; 2],
    pub pixels_per_point: f64,
}

/// Pomenované pohľady (tlačidlá aj `ui.set {"view": …}`): meno, kľúč textu (`cc_i18n::tr`).
pub(crate) const VIEWS: [(&str, &str); 5] = [
    ("front", "view3d.view.front"),
    ("top", "view3d.view.top"),
    ("iso", "view3d.view.iso"),
    ("selection", "view3d.view.selection"),
    ("default", "view3d.view.default"),
];

impl App {
    /// Scéna pre aktuálny model a voľby zobrazenia (prestaví sa iba pri zmene).
    pub(crate) fn update_scene(&mut self) {
        let Some(built) = self.model_built() else {
            return;
        };
        let opts_key = [
            self.scene_opts.backs as u64,
            self.scene_opts.shelves as u64 | (self.scene_opts.fronts as u64) << 1,
            self.scene_opts.operations as u64 | (self.scene_opts.walls as u64) << 1,
            self.scene_opts.explode.to_bits(),
        ];
        let key = (std::sync::Arc::as_ptr(&built) as usize, opts_key);
        if self.view.scene_key != key || self.view.scene_focus != self.view.focus {
            if let Some(doc) = self.session.doc() {
                self.view.scene = match &self.view.focus {
                    // editor skrinky: samotná skrinka bez miestnosti
                    Some(id) => {
                        let model = cc_model::Model {
                            project: built.model.project.clone(),
                            cabinets: built
                                .model
                                .cabinets
                                .iter()
                                .filter(|c| &c.id == id)
                                .cloned()
                                .collect(),
                        };
                        let mut project = doc.project.clone();
                        project.room = None;
                        project.walls.clear();
                        project.openings.clear();
                        project.cabinets.retain(|c| &c.id == id);
                        for c in &mut project.cabinets {
                            c.wall = None;
                        }
                        Scene::build(&model, &project, &self.scene_opts)
                    }
                    None => Scene::build(&built.model, &doc.project, &self.scene_opts),
                };
            }
            self.view.scene_key = key;
            self.view.scene_focus = self.view.focus.clone();
            self.view.generation += 1;
            self.view.mesh = None;
            self.view.image_key = None;
        }
    }

    /// Výber v scéne: vybraný diel (plochy) a skrinka (drôtený obrys).
    pub(crate) fn scene_selection(&self) -> Selection {
        let sel = self.session.selection();
        self.view
            .scene
            .selection(sel.part.as_deref(), sel.cabinet.as_deref())
    }

    /// Klik v 3D: prvý klik vyberie skrinku, ďalší klik (alebo dvojklik) na tú istú
    /// skrinku vyberie diel; klik do prázdna výber zruší.
    fn click_pick(&mut self, hit: Option<u32>) {
        let Some(part) = hit else {
            if self.session.selection().cabinet.is_some() {
                self.exec("selection.set", serde_json::json!({ "cabinet": null }));
            }
            return;
        };
        let cabinet = self.view.scene.cabinet_of(part).map(str::to_owned);
        let part_id = (part as usize)
            .checked_sub(1)
            .and_then(|i| self.view.scene.parts.get(i))
            .cloned();
        if cabinet.is_some() && self.session.selection().cabinet == cabinet {
            self.select_part(part_id);
        } else if let Some(c) = cabinet {
            self.exec("selection.set", serde_json::json!({ "cabinet": c }));
        }
    }

    /// Nastaví pomenovaný pohľad; `Err` pri neznámom mene alebo prázdnom výbere.
    pub(crate) fn set_view(&mut self, name: &str) -> Result<(), String> {
        let cam = self.camera;
        self.camera = match name {
            "front" => cam.front(),
            "top" => cam.top(),
            "iso" => cam.iso(),
            "default" => Camera::default(),
            "selection" => {
                self.update_scene();
                let sel = self.session.selection();
                let bounds = match (&sel.part, &sel.cabinet) {
                    (Some(part), _) => {
                        let index = self.view.scene.part_index(part);
                        self.view.scene.bounds_of(|i| Some(i) == index)
                    }
                    (None, Some(cabinet)) => self.view.scene.cabinet_bounds(cabinet),
                    (None, None) => None,
                };
                let (lo, hi) = bounds.ok_or_else(|| tr!("view3d.nothing_selected"))?;
                cam.fit_to(lo, hi)
            }
            other => {
                return Err(format!(
                    "neznámy pohľad `{other}` (front, top, iso, selection, default)"
                ))
            }
        };
        Ok(())
    }

    pub(crate) fn tab_3d(&mut self, ui: &mut egui::Ui) {
        let Some(built) = self.model_built() else {
            return;
        };
        self.update_scene();

        let size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point() as f64;

        // ovládanie pohľadu
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            self.camera.yaw += d.x as f64 * 0.008;
            self.camera.pitch = (self.camera.pitch + d.y as f64 * 0.008)
                .clamp(-Camera::PITCH_LIMIT, Camera::PITCH_LIMIT);
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            self.camera.pan[0] += d.x as f64;
            self.camera.pan[1] += d.y as f64;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom =
                    (self.camera.zoom * (1.0 + scroll as f64 * 0.0015)).clamp(0.2, 20.0);
            }
        }
        let interacting = response.dragged();

        let width = (rect.width() as f64 * ppp).round().max(1.0) as usize;
        let height = (rect.height() as f64 * ppp).round().max(1.0) as usize;
        self.view.viewport = Some(Viewport {
            min: [rect.min.x, rect.min.y],
            size: [width, height],
            pixels_per_point: ppp,
        });
        let proj = Projection::new(
            &self.view.scene,
            &self.camera,
            width as f64,
            height as f64,
            ppp,
        );
        // výber dielu lúčom z kurzora
        let pick = |pos: egui::Pos2, scene: &Scene, proj: &Projection| -> Option<u32> {
            let x = (pos.x - rect.min.x) as f64 * ppp;
            let y = (pos.y - rect.min.y) as f64 * ppp;
            let (o, d) = proj.ray(x, y);
            scene.pick(o, d)
        };
        let part_id = |i: u32, scene: &Scene| -> Option<String> {
            scene.parts.get((i as usize).checked_sub(1)?).cloned()
        };
        // dvojklik na skrinku v 3D zákazky: editor skrinky (ako v CV)
        let mut open_editor = None;
        if response.double_clicked() && self.view.focus.is_none() {
            if let Some(pos) = response.interact_pointer_pos() {
                open_editor = pick(pos, &self.view.scene, &proj)
                    .and_then(|hit| self.view.scene.cabinet_of(hit).map(str::to_owned));
            }
        } else if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = pick(pos, &self.view.scene, &proj);
                self.click_pick(hit);
            }
        }
        if let Some(id) = open_editor {
            self.open_cabinet_editor(&id);
            return;
        }
        // editor skrinky: dvojklik na dosku ju otvorí samostatne (ako v CV)
        if response.double_clicked() && self.view.focus.is_some() {
            if let Some(pos) = response.interact_pointer_pos() {
                let hit = pick(pos, &self.view.scene, &proj);
                self.view.open_part = hit
                    .and_then(|h| (h as usize).checked_sub(1))
                    .and_then(|i| self.view.scene.parts.get(i))
                    .cloned();
            }
        }

        // kamera sa mohla zmeniť (dvojklik): kresliť a popisovať novou projekciou
        let proj = Projection::new(
            &self.view.scene,
            &self.camera,
            width as f64,
            height as f64,
            ppp,
        );
        let selection = self.scene_selection();
        let selected = selection.part;
        let dark = ui.visuals().dark_mode;
        let gpu = self.gpu.as_ref().filter(|g| !g.failed()).cloned();
        let key = self.image_key(width, height, 0, selected, dark);
        let on_gpu = gpu.is_some_and(|gpu| {
            let generation = self.view.generation;
            let scene = &self.view.scene;
            let mesh = self
                .view
                .mesh
                .get_or_insert_with(|| Arc::new(Mesh::build(scene, generation)))
                .clone();
            let frame = Frame3d::new(
                mesh,
                &proj,
                [width, height],
                selected,
                background(dark),
                key,
            );
            gpu.paint(ui.painter(), rect, [width, height], frame)
        });
        if !on_gpu {
            self.paint_cpu(ui, rect, width, height, ppp, selection, dark, interacting);
        }

        // obrys vybranej skrinky (navrch, rovnako ako v CPU rastri)
        let painter = ui.painter_at(rect);
        if let Some(index) = selection.cabinet {
            let [r, g, b] = cc_scene::style::OUTLINE.map(|v| (v * 255.0).round() as u8);
            let stroke = egui::Stroke::new(
                cc_scene::style::OUTLINE_WIDTH as f32,
                Color32::from_rgb(r, g, b),
            );
            let to_screen = |p: glam::DVec3| {
                let s = proj.project(p);
                rect.min + Vec2::new((s.x / ppp) as f32, (s.y / ppp) as f32)
            };
            for [a, b] in self.view.scene.cabinet_outline(index) {
                painter.line_segment([to_screen(a), to_screen(b)], stroke);
            }
        }
        // popisy skriniek
        for (p, label) in &self.view.scene.labels {
            let s = proj.project(*p);
            let pos = rect.min + Vec2::new((s.x / ppp) as f32, (s.y / ppp) as f32);
            painter.text(
                pos,
                Align2::CENTER_BOTTOM,
                label,
                FontId::proportional(15.0),
                ui.visuals().strong_text_color(),
            );
        }
        self.view.projection = Some(proj);

        if let Some(pos) = response.hover_pos() {
            if !interacting {
                let hovered =
                    pick(pos, &self.view.scene, &proj).and_then(|i| part_id(i, &self.view.scene));
                if let Some(id) = hovered {
                    if let Some(part) = built.model.parts().find(|p| p.id == id) {
                        response.on_hover_text_at_pointer(tr!(
                            "view3d.part_hover",
                            id = &part.id,
                            name = role_name(part.role).to_string(),
                            length = fmt(part.length),
                            width = fmt(part.width),
                            thickness = fmt(part.thickness),
                            material = &part.material,
                            operations = part.operations.len()
                        ));
                    }
                }
            }
        }
        if let Some(id) = self.selected_part() {
            let r =
                egui::Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::new(260.0, 28.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tr!("view3d.selected", id = &id)).strong());
                    if ui.small_button(tr!("view3d.part_drawing")).clicked() {
                        self.tab = Tab::Part;
                    }
                });
            });
        }
    }

    /// Kľúč obrazu: zmení sa so scénou, kamerou, veľkosťou, výberom a vzhľadom.
    fn image_key(
        &self,
        width: usize,
        height: usize,
        ss: usize,
        selected: Option<u32>,
        dark: bool,
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.view.scene_key, self.view.generation).hash(&mut h);
        (
            self.camera.yaw.to_bits(),
            self.camera.pitch.to_bits(),
            self.camera.zoom.to_bits(),
            self.camera.pan.map(f64::to_bits),
            self.camera.perspective,
            self.camera
                .fit
                .map(|b| b.map(|v| v.to_array().map(f64::to_bits))),
            width,
            height,
            ss,
            selected,
            dark,
        )
            .hash(&mut h);
        h.finish()
    }

    /// Softvérový raster do textúry (prestaví sa iba pri zmene pohľadu, scény alebo výberu).
    #[allow(clippy::too_many_arguments)]
    fn paint_cpu(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        width: usize,
        height: usize,
        ppp: f64,
        selection: Selection,
        dark: bool,
        interacting: bool,
    ) {
        let ss = if interacting { 1 } else { 2 };
        // obrys skrinky kreslí okno navrch (painter), raster iba diel
        let selection = Selection {
            cabinet: None,
            ..selection
        };
        let key = self.image_key(width, height, ss, selection.part, dark);
        if self.view.image_key != Some(key) {
            let bg = background(dark);
            let img = cc_scene::render(
                &self.view.scene,
                &self.camera,
                width,
                height,
                ss,
                ppp,
                selection,
                bg,
            );
            let color = egui::ColorImage::from_rgb([img.width, img.height], &img.rgb);
            match &mut self.view.texture {
                Some(t) => t.set(color, egui::TextureOptions::LINEAR),
                None => {
                    self.view.texture = Some(ui.ctx().load_texture(
                        "view3d",
                        color,
                        egui::TextureOptions::LINEAR,
                    ))
                }
            }
            self.view.image_key = Some(key);
        }
        if let Some(t) = &self.view.texture {
            ui.painter().image(
                t.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }
}

pub(crate) fn background(dark: bool) -> [u8; 3] {
    if dark {
        cc_scene::style::BACKGROUND_DARK
    } else {
        cc_scene::style::BACKGROUND_LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::VIEWS;

    /// Kľúče textov pohľadov sa skladajú za behu: každý musí byť v sk aj en.
    #[test]
    fn every_view_has_translated_label() {
        for (code, text) in cc_i18n::BUILTIN {
            let catalog = cc_i18n::Catalog::parse(code, text).unwrap();
            for (_, key) in VIEWS {
                assert!(catalog.contains(key), "{code}.toml: chýba {key}");
            }
        }
    }
}
