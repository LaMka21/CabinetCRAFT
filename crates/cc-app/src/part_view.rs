//! Výkres dielu zhora (plocha F) s vŕtaním a drážkami.

use cc_i18n::tr;
use cc_model::{Edge, Face, Operation, Part};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

const HOLE_DOWEL: Color32 = Color32::from_rgb(55, 55, 62);
const HOLE_BORE: Color32 = Color32::from_rgb(50, 90, 180);
const EDGE_HOLE: Color32 = Color32::from_rgb(70, 150, 90);
const GROOVE: Color32 = Color32::from_rgb(150, 105, 60);
const HIGHLIGHT: Color32 = Color32::from_rgb(235, 120, 20);
const BAND: Color32 = Color32::from_rgb(205, 130, 50);

pub fn face_name(face: Face) -> String {
    match face {
        Face::F => tr!("part_view.face.f"),
        Face::B => tr!("part_view.face.b"),
        Face::X0 => tr!("part_view.face.x0"),
        Face::X1 => tr!("part_view.face.x1"),
        Face::Y0 => tr!("part_view.face.y0"),
        Face::Y1 => tr!("part_view.face.y1"),
    }
}

/// Obdĺžnik operácie v súradniciach dielu (x0, y0, x1, y1).
fn footprint(op: &Operation, part: &Part) -> (f64, f64, f64, f64) {
    let (l, w) = (part.length, part.width);
    match *op {
        Operation::Hole {
            face,
            x,
            y,
            diameter,
            depth,
            ..
        } => {
            let r = diameter / 2.0;
            match face {
                Face::F | Face::B => (x - r, y - r, x + r, y + r),
                Face::X0 => (0.0, y - r, depth, y + r),
                Face::X1 => (l - depth, y - r, l, y + r),
                Face::Y0 => (x - r, 0.0, x + r, depth),
                Face::Y1 => (x - r, w - depth, x + r, w),
            }
        }
        Operation::Groove {
            start, end, width, ..
        } => {
            let h = width / 2.0;
            let (x0, x1) = (start.x.min(end.x), start.x.max(end.x));
            let (y0, y1) = (start.y.min(end.y), start.y.max(end.y));
            if (start.y - end.y).abs() < 1e-9 {
                (x0.max(0.0), y0 - h, x1.min(l), y1 + h)
            } else {
                (x0 - h, y0.max(0.0), x1 + h, y1.min(w))
            }
        }
    }
}

/// Nakreslí diel; vráti index operácie pod kurzorom.
pub fn draw(
    ui: &mut egui::Ui,
    part: &Part,
    highlight: Option<usize>,
    height: f32,
) -> Option<usize> {
    let size = Vec2::new(ui.available_width(), height.max(160.0));
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let dark = ui.visuals().dark_mode;
    let text_color = ui.visuals().text_color();
    let margin = 56.0;
    let (l, w) = (part.length, part.width);
    let s = (((rect.width() - 2.0 * margin) as f64 / l)
        .min((rect.height() - 2.0 * margin) as f64 / w))
    .max(0.01);
    let origin = Pos2::new(
        rect.center().x - (l * s / 2.0) as f32,
        rect.center().y + (w * s / 2.0) as f32,
    );
    let to = |x: f64, y: f64| Pos2::new(origin.x + (x * s) as f32, origin.y - (y * s) as f32);
    let to_rect =
        |(x0, y0, x1, y1): (f64, f64, f64, f64)| Rect::from_two_pos(to(x0, y0), to(x1, y1));

    let board = to_rect((0.0, 0.0, l, w));
    let board_fill = if dark {
        Color32::from_rgb(78, 74, 68)
    } else {
        Color32::from_rgb(242, 238, 230)
    };
    painter.rect_filled(board, 1.0, board_fill);
    painter.rect_stroke(
        board,
        1.0,
        Stroke::new(1.5f32, Color32::from_gray(110)),
        StrokeKind::Inside,
    );

    // olepené hrany
    for edge in Edge::ALL {
        if part.edges.get(edge).is_some() {
            let (a, b) = match edge {
                Edge::X0 => (to(0.0, 0.0), to(0.0, w)),
                Edge::X1 => (to(l, 0.0), to(l, w)),
                Edge::Y0 => (to(0.0, 0.0), to(l, 0.0)),
                Edge::Y1 => (to(0.0, w), to(l, w)),
            };
            painter.line_segment([a, b], Stroke::new(4.0f32, BAND));
        }
    }

    // popisy hrán a rozmerov
    let font = FontId::proportional(13.0);
    painter.text(
        to(l / 2.0, 0.0) + Vec2::new(0.0, 8.0),
        Align2::CENTER_TOP,
        tr!("part_view.edge_y0", length = fmt(l)),
        font.clone(),
        text_color,
    );
    painter.text(
        to(l / 2.0, w) - Vec2::new(0.0, 8.0),
        Align2::CENTER_BOTTOM,
        "Y1",
        font.clone(),
        text_color,
    );
    painter.text(
        to(0.0, w / 2.0) - Vec2::new(8.0, 0.0),
        Align2::RIGHT_CENTER,
        format!("X0\n{}", fmt(w)),
        font.clone(),
        text_color,
    );
    painter.text(
        to(l, w / 2.0) + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        "X1",
        font.clone(),
        text_color,
    );
    painter.text(
        to(0.0, 0.0) + Vec2::new(-4.0, 4.0),
        Align2::RIGHT_TOP,
        "0,0",
        FontId::proportional(11.0),
        text_color,
    );

    let pointer = response.hover_pos();
    let mut hovered: Option<(usize, f32)> = None;
    for (i, op) in part.operations.iter().enumerate() {
        let r = to_rect(footprint(op, part));
        match *op {
            Operation::Hole {
                face,
                x,
                y,
                diameter,
                dowel,
                ..
            } => {
                let radius = ((diameter / 2.0) * s).max(2.5) as f32;
                let color = if dowel { HOLE_DOWEL } else { HOLE_BORE };
                match face {
                    Face::F => {
                        painter.circle_filled(to(x, y), radius, color);
                    }
                    Face::B => {
                        painter.circle_stroke(to(x, y), radius, Stroke::new(1.5f32, color));
                    }
                    _ => {
                        let r = r.expand(if r.width().min(r.height()) < 3.0 {
                            1.0
                        } else {
                            0.0
                        });
                        painter.rect_filled(r, 0.0, EDGE_HOLE.gamma_multiply(0.55));
                        painter.rect_stroke(
                            r,
                            0.0,
                            Stroke::new(1.0f32, EDGE_HOLE),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            Operation::Groove { face, .. } => {
                if face == Face::F {
                    painter.rect_filled(r, 0.0, GROOVE.gamma_multiply(0.6));
                }
                painter.rect_stroke(r, 0.0, Stroke::new(1.0f32, GROOVE), StrokeKind::Inside);
            }
        }
        if highlight == Some(i) {
            painter.rect_stroke(
                r.expand(3.0),
                2.0,
                Stroke::new(2.0f32, HIGHLIGHT),
                StrokeKind::Outside,
            );
        }
        if let Some(p) = pointer {
            if r.expand(3.0).contains(p) {
                let area = r.area();
                if hovered.is_none_or(|(_, a)| area < a) {
                    hovered = Some((i, area));
                }
            }
        }
    }

    // legenda
    let mut y = rect.top() + 6.0;
    for (color, text, filled) in [
        (HOLE_DOWEL, tr!("part_view.legend.dowel"), true),
        (HOLE_BORE, tr!("part_view.legend.hole_row"), true),
        (EDGE_HOLE, tr!("part_view.legend.edge_drilling"), true),
        (GROOVE, tr!("part_view.legend.back_groove"), true),
        (BAND, tr!("part_view.legend.edge_band"), true),
    ] {
        let dot = Pos2::new(rect.left() + 12.0, y + 7.0);
        if filled {
            painter.circle_filled(dot, 5.0, color);
        }
        painter.text(
            dot + Vec2::new(10.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            text_color,
        );
        y += 17.0;
    }
    hovered.map(|(i, _)| i)
}

/// Číslo bez zbytočných núl (`564`, `311.5`).
pub fn fmt(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
