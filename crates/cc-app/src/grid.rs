//! Tabuľka v štýle CAD programov: sivá hlavička, tenké čiary, výber riadku,
//! farebné bunky (stav). Riadky sa kreslia priamo, takže aj tisíce dielov idú plynulo
//! (kreslia sa iba viditeľné riadky).

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, Sense, Stroke};

use crate::theme;

pub const ROW_H: f32 = 21.0;
const HEADER_H: f32 = 22.0;

/// Stĺpec: nadpis a šírka v bodoch.
#[derive(Debug, Clone, Copy)]
pub struct Col {
    /// Kľúč prekladu nadpisu (`cc_i18n::tr` v hlavičke); text, ktorý nie je kľúčom
    /// (`#`, `ID`), sa zobrazí bez zmeny.
    pub title: &'static str,
    pub width: f32,
    /// Čísla zarovnať doprava.
    pub right: bool,
}

pub const fn col(title: &'static str, width: f32) -> Col {
    Col {
        title,
        width,
        right: false,
    }
}

pub const fn num(title: &'static str, width: f32) -> Col {
    Col {
        title,
        width,
        right: true,
    }
}

/// Bunka: text a voliteľná výplň a farba textu.
#[derive(Debug, Clone, Default)]
pub struct Cell {
    pub text: String,
    pub fill: Option<Color32>,
    pub color: Option<Color32>,
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell {
            text,
            ..Cell::default()
        }
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Cell::from(text.to_owned())
    }
}

impl Cell {
    pub fn fill(mut self, fill: Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

/// Čo sa s tabuľkou stalo v tejto snímke.
#[derive(Debug, Default)]
pub struct GridResponse {
    pub clicked: Option<usize>,
    pub double_clicked: Option<usize>,
    pub hovered: Option<usize>,
}

/// Nakreslí tabuľku s posúvaním. `rows(i)` vráti bunky riadku `i`.
pub fn show(
    ui: &mut egui::Ui,
    id: &str,
    cols: &[Col],
    count: usize,
    selected: Option<usize>,
    mut rows: impl FnMut(usize) -> Vec<Cell>,
) -> GridResponse {
    let mut out = GridResponse::default();
    let total: f32 = cols.iter().map(|c| c.width).sum();
    egui::ScrollArea::horizontal()
        .id_salt(format!("{id}_h"))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let width = total.max(ui.available_width());
            header(ui, cols, width);
            egui::ScrollArea::vertical()
                .id_salt(format!("{id}_v"))
                .auto_shrink([false, false])
                .show_rows(ui, ROW_H, count, |ui, range| {
                    for i in range {
                        let cells = rows(i);
                        let r = row(ui, cols, width, &cells, selected == Some(i));
                        if r.clicked() {
                            out.clicked = Some(i);
                        }
                        if r.double_clicked() {
                            out.double_clicked = Some(i);
                        }
                        if r.hovered() {
                            out.hovered = Some(i);
                        }
                    }
                });
        });
    out
}

fn header(ui: &mut egui::Ui, cols: &[Col], width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEADER_H), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, theme::HEADER_BG);
    let mut x = rect.min.x;
    for c in cols {
        let cell = Rect::from_min_size(pos2(x, rect.min.y), vec2(c.width, HEADER_H));
        let title = cc_i18n::tr(c.title);
        text(p, cell, &title, c.right, theme::TEXT, theme::SMALL + 0.5);
        x += c.width;
        p.line_segment(
            [pos2(x, rect.min.y), pos2(x, rect.max.y)],
            Stroke::new(1.0_f32, theme::BORDER),
        );
    }
    p.line_segment(
        [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
        Stroke::new(1.0_f32, theme::BORDER),
    );
}

fn row(
    ui: &mut egui::Ui,
    cols: &[Col],
    width: f32,
    cells: &[Cell],
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 0.0, theme::SELECT_BG);
    } else if response.hovered() {
        p.rect_filled(rect, 0.0, theme::HOVER_BG);
    }
    let mut x = rect.min.x;
    for (c, cell) in cols.iter().zip(cells) {
        let r = Rect::from_min_size(pos2(x, rect.min.y), vec2(c.width, ROW_H));
        if let Some(fill) = cell.fill {
            p.rect_filled(r.shrink2(vec2(0.5, 0.5)), 0.0, fill);
        }
        text(
            p,
            r,
            &cell.text,
            c.right,
            cell.color.unwrap_or(theme::TEXT),
            theme::BODY - 0.5,
        );
        x += c.width;
        p.line_segment(
            [pos2(x, rect.min.y), pos2(x, rect.max.y)],
            Stroke::new(1.0_f32, theme::GRID_LINE),
        );
    }
    p.line_segment(
        [pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)],
        Stroke::new(1.0_f32, theme::GRID_LINE),
    );
    response
}

/// Text v bunke, orezaný na šírku bunky.
fn text(p: &egui::Painter, cell: Rect, s: &str, right: bool, color: Color32, size: f32) {
    let clip = p.with_clip_rect(cell.shrink2(vec2(3.0, 0.0)).intersect(p.clip_rect()));
    let (pos, align) = if right {
        (
            pos2(cell.max.x - 4.0, cell.center().y),
            Align2::RIGHT_CENTER,
        )
    } else {
        (pos2(cell.min.x + 4.0, cell.center().y), Align2::LEFT_CENTER)
    };
    clip.text(pos, align, s, FontId::proportional(size), color);
}
