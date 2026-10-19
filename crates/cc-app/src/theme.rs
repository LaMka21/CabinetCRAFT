//! Vzhľad okna: farby, písmo a rozostupy v štýle bežných CAD programov pre Windows
//! (svetlosivé panely, biele pracovné plochy, pás nástrojov). Všetky farby okna
//! sú tu, ostatné moduly ich berú odtiaľto.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

/// Pozadie okna a panelov.
pub const BG: Color32 = Color32::from_rgb(240, 240, 240);
/// Pás nástrojov (ribbon).
pub const RIBBON: Color32 = Color32::from_rgb(248, 248, 248);
/// Pracovná plocha (mriežka, výkres).
pub const SHEET: Color32 = Color32::WHITE;
/// Okraje panelov a buniek.
pub const BORDER: Color32 = Color32::from_rgb(208, 208, 208);
/// Jemné čiary mriežky.
pub const GRID_LINE: Color32 = Color32::from_rgb(228, 228, 228);
pub const TEXT: Color32 = Color32::from_rgb(30, 30, 30);
pub const TEXT_WEAK: Color32 = Color32::from_rgb(110, 110, 110);
/// Bunka s menom vlastnosti.
pub const LABEL_BG: Color32 = Color32::from_rgb(244, 244, 244);
/// Hlavička skupiny vlastností a stĺpcov.
pub const HEADER_BG: Color32 = Color32::from_rgb(232, 232, 232);
/// Výber (riadok, položka).
pub const SELECT_BG: Color32 = Color32::from_rgb(204, 228, 247);
pub const SELECT_STROKE: Color32 = Color32::from_rgb(0, 120, 215);
/// Prechod myšou nad tlačidlom.
pub const HOVER_BG: Color32 = Color32::from_rgb(229, 241, 251);
pub const HOVER_STROKE: Color32 = Color32::from_rgb(160, 200, 236);
/// Stlačené / aktívne tlačidlo pásu.
pub const ACTIVE_BG: Color32 = Color32::from_rgb(214, 214, 214);
/// Zvýraznený riadok vlastností (rozpísaná hodnota).
pub const EDIT_ROW: Color32 = Color32::from_rgb(247, 242, 214);

// farby ikon
pub const GOLD: Color32 = Color32::from_rgb(232, 168, 40);
pub const GOLD_DARK: Color32 = Color32::from_rgb(170, 112, 18);
pub const BLUE: Color32 = Color32::from_rgb(43, 110, 180);
pub const BLUE_LIGHT: Color32 = Color32::from_rgb(120, 170, 220);
pub const GREEN: Color32 = Color32::from_rgb(52, 150, 60);
pub const RED: Color32 = Color32::from_rgb(200, 52, 40);
pub const GREY: Color32 = Color32::from_rgb(105, 110, 118);
pub const WOOD: Color32 = Color32::from_rgb(222, 196, 150);

pub const ERROR: Color32 = Color32::from_rgb(200, 40, 40);
pub const WARNING: Color32 = Color32::from_rgb(190, 130, 0);
pub const OK: Color32 = Color32::from_rgb(40, 140, 60);

/// Veľkosti písma (Segoe UI): o niečo menšie, ako má egui, aby sa zmestilo viac údajov.
pub const BODY: f32 = 13.0;
pub const SMALL: f32 = 11.5;
pub const HEADING: f32 = 15.0;

/// Nastaví vzhľad celého okna.
pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.style_mut_of(egui::Theme::Light, |s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
        s.spacing.interact_size.y = 20.0;
        s.spacing.combo_width = 160.0;
        for (style, size) in [
            (egui::TextStyle::Body, BODY),
            (egui::TextStyle::Button, BODY),
            (egui::TextStyle::Heading, HEADING),
            (egui::TextStyle::Monospace, 12.0),
            (egui::TextStyle::Small, SMALL),
        ] {
            if let Some(f) = s.text_styles.get_mut(&style) {
                f.size = size;
            }
        }

        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = BG;
        v.extreme_bg_color = SHEET;
        v.faint_bg_color = Color32::from_rgb(248, 248, 248);
        v.window_stroke = Stroke::new(1.0_f32, BORDER);
        v.window_corner_radius = CornerRadius::same(2);
        v.menu_corner_radius = CornerRadius::same(2);
        v.override_text_color = None;
        v.selection.bg_fill = SELECT_BG;
        v.selection.stroke = Stroke::new(1.0_f32, SELECT_STROKE);
        v.hyperlink_color = BLUE;
        v.striped = true;

        let radius = CornerRadius::same(2);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = BG;
        w.noninteractive.weak_bg_fill = BG;
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.noninteractive.corner_radius = radius;

        w.inactive.bg_fill = Color32::WHITE;
        w.inactive.weak_bg_fill = Color32::from_rgb(250, 250, 250);
        w.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.inactive.corner_radius = radius;

        w.hovered.bg_fill = HOVER_BG;
        w.hovered.weak_bg_fill = HOVER_BG;
        w.hovered.bg_stroke = Stroke::new(1.0_f32, HOVER_STROKE);
        w.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.hovered.corner_radius = radius;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = SELECT_BG;
        w.active.weak_bg_fill = SELECT_BG;
        w.active.bg_stroke = Stroke::new(1.0_f32, SELECT_STROKE);
        w.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.active.corner_radius = radius;
        w.active.expansion = 0.0;

        w.open.bg_fill = Color32::WHITE;
        w.open.weak_bg_fill = HOVER_BG;
        w.open.bg_stroke = Stroke::new(1.0_f32, HOVER_STROKE);
        w.open.fg_stroke = Stroke::new(1.0_f32, TEXT);
        w.open.corner_radius = radius;
    });
}
