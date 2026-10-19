//! Ikony kreslené v kóde (žiadne cudzie obrázky). Každá ikona sa kreslí do štvorca
//! v jednotkových súradniciach 0…1 a prispôsobí sa veľkosti (16 aj 32 bodov).

use eframe::egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};

use crate::theme::{BLUE, BLUE_LIGHT, GOLD, GOLD_DARK, GREEN, GREY, RED, WOOD};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Open,
    New,
    Save,
    SaveAs,
    Undo,
    Redo,
    CabinetBase,
    CabinetWall,
    CabinetTall,
    Duplicate,
    Delete,
    MoveUp,
    MoveDown,
    Plan,
    Elevation,
    Cube,
    Parts,
    Drawing,
    Check,
    Standard,
    Export,
    Materials,
    Reload,
    Catalog,
    /// Návrat z režimu (pôdorys) alebo z editora skrinky.
    Return,
    /// Režimy pôdorysu.
    Walls,
    Floor,
    Ceiling,
    Worktop,
    Molding,
    AutoFill,
    Section,
    Cad,
    /// Zoom: výrez oknom, oddialiť, všetko.
    ZoomWindow,
    ZoomOut,
    ZoomAll,
    /// Kreslenie stien: čiara (reťaz), obdĺžnik; výber.
    Line,
    Rectangle,
    Select,
    Turn,
    /// Zástena nad pracovnou doskou.
    Splash,
    /// Kresba dreva cez čelá.
    Grain,
}

/// Kreslenie v jednotkových súradniciach ikony.
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    dim: bool,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.rect.min.x + x * self.rect.width(),
            self.rect.min.y + y * self.rect.height(),
        )
    }

    fn c(&self, color: Color32) -> Color32 {
        if self.dim {
            let g = ((color.r() as u16 + color.g() as u16 + color.b() as u16) / 3) as u8;
            Color32::from_rgb(g, g, g).gamma_multiply(0.45)
        } else {
            color
        }
    }

    fn w(&self, units: f32) -> f32 {
        (units * self.rect.width() / 32.0).max(1.0)
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.p(x0, y0), self.p(x1, y1))
    }

    fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) {
        self.painter
            .rect_filled(self.rect(x0, y0, x1, y1), 1.0, self.c(color));
    }

    fn frame(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color32, width: f32) {
        self.painter.rect_stroke(
            self.rect(x0, y0, x1, y1),
            1.0,
            Stroke::new(self.w(width), self.c(color)),
            StrokeKind::Inside,
        );
    }

    fn boxed(&self, x0: f32, y0: f32, x1: f32, y1: f32, fill: Color32, edge: Color32) {
        self.fill(x0, y0, x1, y1, fill);
        self.frame(x0, y0, x1, y1, edge, 1.5);
    }

    fn line(&self, pts: &[(f32, f32)], color: Color32, width: f32) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::line(pts, Stroke::new(self.w(width), self.c(color))));
    }

    fn poly(&self, pts: &[(f32, f32)], fill: Color32, edge: Color32) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter.add(Shape::convex_polygon(
            pts,
            self.c(fill),
            Stroke::new(self.w(1.2), self.c(edge)),
        ));
    }

    fn circle(&self, x: f32, y: f32, r: f32, fill: Color32, edge: Color32) {
        self.painter.circle(
            self.p(x, y),
            r * self.rect.width(),
            self.c(fill),
            Stroke::new(self.w(1.5), self.c(edge)),
        );
    }

    /// Oblúk so šípkou (späť / znova).
    fn curved_arrow(&self, mirror: bool, color: Color32) {
        let fx = |x: f32| if mirror { 1.0 - x } else { x };
        let mut pts = Vec::new();
        for k in 0..=12 {
            let a = std::f32::consts::PI * (k as f32 / 12.0);
            pts.push((fx(0.52 + 0.28 * a.cos()), 0.56 - 0.26 * a.sin()));
        }
        pts.push((fx(0.80), 0.80));
        self.line(&pts, color, 3.0);
        self.poly(
            &[(fx(0.12), 0.56), (fx(0.36), 0.40), (fx(0.36), 0.72)],
            color,
            color,
        );
    }

    /// Korpus skrinky s dvierkami.
    fn cabinet(&self, x0: f32, y0: f32, x1: f32, y1: f32, doors: u8) {
        self.boxed(x0, y0, x1, y1, WOOD, GOLD_DARK);
        let m = 0.05;
        let n = doors.max(1) as f32;
        let w = (x1 - x0 - 2.0 * m) / n;
        for i in 0..doors.max(1) {
            let a = x0 + m + w * i as f32;
            self.boxed(a + 0.01, y0 + m, a + w - 0.01, y1 - m, GOLD, GOLD_DARK);
            let hx = if i % 2 == 0 { a + w - 0.06 } else { a + 0.06 };
            self.line(
                &[(hx, (y0 + y1) / 2.0 - 0.06), (hx, (y0 + y1) / 2.0 + 0.06)],
                GOLD_DARK,
                2.0,
            );
        }
    }
}

/// Nakreslí ikonu do `rect`. `enabled = false` ju vykreslí sivo.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, enabled: bool) {
    let pen = Pen {
        painter,
        rect,
        dim: !enabled,
    };
    let white = Color32::WHITE;
    match icon {
        Icon::Open => {
            pen.poly(
                &[
                    (0.06, 0.22),
                    (0.38, 0.22),
                    (0.46, 0.32),
                    (0.88, 0.32),
                    (0.88, 0.82),
                    (0.06, 0.82),
                ],
                GOLD_DARK,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.18, 0.44), (0.98, 0.44), (0.86, 0.82), (0.06, 0.82)],
                GOLD,
                GOLD_DARK,
            );
        }
        Icon::New => {
            pen.poly(
                &[
                    (0.2, 0.08),
                    (0.62, 0.08),
                    (0.82, 0.28),
                    (0.82, 0.92),
                    (0.2, 0.92),
                ],
                white,
                GREY,
            );
            pen.line(&[(0.62, 0.08), (0.62, 0.28), (0.82, 0.28)], GREY, 1.5);
            pen.circle(0.78, 0.78, 0.13, GOLD, GOLD_DARK);
        }
        Icon::Save | Icon::SaveAs => {
            pen.boxed(0.1, 0.1, 0.9, 0.9, BLUE, BLUE);
            pen.fill(0.24, 0.12, 0.72, 0.4, white);
            pen.fill(0.56, 0.16, 0.66, 0.36, BLUE);
            pen.fill(0.22, 0.56, 0.78, 0.88, BLUE_LIGHT);
            if icon == Icon::SaveAs {
                pen.line(&[(0.5, 0.95), (0.95, 0.5)], GOLD_DARK, 4.0);
            }
        }
        Icon::Undo => pen.curved_arrow(false, BLUE),
        Icon::Redo => pen.curved_arrow(true, BLUE),
        Icon::CabinetBase => {
            pen.fill(0.08, 0.86, 0.92, 0.92, GREY);
            pen.cabinet(0.12, 0.4, 0.88, 0.86, 2);
        }
        Icon::CabinetWall => {
            pen.fill(0.06, 0.06, 0.94, 0.1, GREY);
            pen.cabinet(0.12, 0.1, 0.88, 0.56, 2);
        }
        Icon::CabinetTall => {
            pen.cabinet(0.26, 0.06, 0.74, 0.94, 1);
        }
        Icon::Duplicate => {
            pen.boxed(0.1, 0.1, 0.62, 0.62, white, GREY);
            pen.boxed(0.38, 0.38, 0.9, 0.9, WOOD, GOLD_DARK);
        }
        Icon::Delete => {
            pen.line(&[(0.2, 0.2), (0.8, 0.8)], RED, 4.5);
            pen.line(&[(0.8, 0.2), (0.2, 0.8)], RED, 4.5);
        }
        Icon::MoveUp => pen.poly(&[(0.5, 0.18), (0.86, 0.72), (0.14, 0.72)], BLUE, BLUE),
        Icon::MoveDown => pen.poly(&[(0.14, 0.28), (0.86, 0.28), (0.5, 0.82)], BLUE, BLUE),
        Icon::Plan => {
            pen.fill(0.08, 0.08, 0.92, 0.92, white);
            pen.line(
                &[(0.1, 0.9), (0.1, 0.1), (0.9, 0.1), (0.9, 0.9), (0.6, 0.9)],
                GREY,
                3.0,
            );
            pen.boxed(0.14, 0.14, 0.62, 0.34, GOLD, GOLD_DARK);
            pen.boxed(0.66, 0.14, 0.86, 0.6, GOLD, GOLD_DARK);
        }
        Icon::Elevation => {
            pen.fill(0.06, 0.06, 0.94, 0.94, Color32::from_rgb(232, 232, 232));
            pen.cabinet(0.1, 0.1, 0.9, 0.36, 3);
            pen.cabinet(0.1, 0.56, 0.9, 0.9, 3);
        }
        Icon::Cube => {
            pen.poly(
                &[(0.5, 0.08), (0.9, 0.28), (0.5, 0.48), (0.1, 0.28)],
                WOOD,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.1, 0.28), (0.5, 0.48), (0.5, 0.92), (0.1, 0.72)],
                GOLD,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.5, 0.48), (0.9, 0.28), (0.9, 0.72), (0.5, 0.92)],
                GOLD_DARK,
                GOLD_DARK,
            );
        }
        Icon::Catalog => {
            // tri skrinky vedľa seba ako v katalógu
            pen.cabinet(0.04, 0.42, 0.34, 0.92, 1);
            pen.cabinet(0.36, 0.30, 0.66, 0.92, 2);
            pen.cabinet(0.68, 0.08, 0.96, 0.92, 1);
        }
        Icon::Parts => {
            pen.boxed(0.08, 0.12, 0.92, 0.88, white, GREY);
            pen.fill(0.08, 0.12, 0.92, 0.3, Color32::from_rgb(220, 220, 220));
            for y in [0.44, 0.58, 0.72] {
                pen.line(&[(0.12, y), (0.88, y)], GREY, 1.0);
            }
            pen.line(&[(0.36, 0.12), (0.36, 0.88)], GREY, 1.0);
        }
        Icon::Drawing => {
            pen.boxed(0.08, 0.24, 0.92, 0.76, WOOD, GOLD_DARK);
            for x in [0.22, 0.4, 0.6, 0.78] {
                pen.circle(x, 0.5, 0.05, white, BLUE);
            }
            pen.line(&[(0.08, 0.88), (0.92, 0.88)], BLUE, 1.5);
        }
        Icon::Check => {
            pen.circle(0.5, 0.5, 0.42, GREEN, GREEN);
            pen.line(&[(0.28, 0.52), (0.44, 0.68), (0.74, 0.34)], white, 4.0);
        }
        Icon::Standard => {
            pen.circle(0.5, 0.5, 0.3, GREY, GREY);
            for k in 0..8 {
                let a = std::f32::consts::TAU * k as f32 / 8.0;
                let (s, c) = a.sin_cos();
                pen.line(
                    &[
                        (0.5 + 0.26 * c, 0.5 + 0.26 * s),
                        (0.5 + 0.44 * c, 0.5 + 0.44 * s),
                    ],
                    GREY,
                    5.0,
                );
            }
            pen.circle(0.5, 0.5, 0.12, white, GREY);
        }
        Icon::Export => {
            pen.boxed(0.06, 0.4, 0.66, 0.9, WOOD, GOLD_DARK);
            for x in [0.2, 0.36, 0.52] {
                pen.circle(x, 0.65, 0.045, white, GOLD_DARK);
            }
            pen.poly(&[(0.56, 0.1), (0.96, 0.32), (0.56, 0.54)], GREEN, GREEN);
        }
        Icon::Materials => {
            pen.poly(
                &[(0.1, 0.62), (0.6, 0.62), (0.9, 0.46), (0.4, 0.46)],
                WOOD,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.1, 0.46), (0.6, 0.46), (0.9, 0.3), (0.4, 0.3)],
                GOLD,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.1, 0.3), (0.6, 0.3), (0.9, 0.14), (0.4, 0.14)],
                white,
                GREY,
            );
            pen.fill(0.1, 0.62, 0.6, 0.72, GOLD_DARK);
        }
        Icon::Reload => {
            pen.curved_arrow(true, GREEN);
        }
        Icon::Return => {
            pen.circle(0.5, 0.5, 0.42, BLUE, BLUE);
            pen.line(&[(0.66, 0.68), (0.36, 0.38)], white, 4.0);
            pen.poly(&[(0.28, 0.3), (0.56, 0.32), (0.3, 0.58)], white, white);
        }
        Icon::Walls => {
            pen.fill(0.06, 0.06, 0.94, 0.94, white);
            pen.fill(0.08, 0.12, 0.9, 0.26, GREY);
            pen.fill(0.76, 0.12, 0.9, 0.9, GREY);
            pen.boxed(0.14, 0.3, 0.42, 0.52, WOOD, GOLD_DARK);
            pen.line(&[(0.08, 0.9), (0.6, 0.9)], BLUE, 2.0);
        }
        Icon::Floor => {
            for i in 0..4 {
                for j in 0..4 {
                    let color = if (i + j) % 2 == 0 { GREY } else { white };
                    let (x, y) = (0.1 + i as f32 * 0.2, 0.1 + j as f32 * 0.2);
                    pen.fill(x, y, x + 0.2, y + 0.2, color);
                }
            }
            pen.frame(0.1, 0.1, 0.9, 0.9, GREY, 1.0);
        }
        Icon::Ceiling => {
            pen.fill(0.06, 0.1, 0.94, 0.24, Color32::from_rgb(236, 226, 196));
            pen.line(&[(0.06, 0.24), (0.94, 0.24)], GOLD_DARK, 1.5);
            pen.line(&[(0.5, 0.24), (0.5, 0.48)], GREY, 1.5);
            pen.poly(
                &[(0.36, 0.48), (0.64, 0.48), (0.58, 0.62), (0.42, 0.62)],
                GOLD,
                GOLD_DARK,
            );
        }
        Icon::Worktop => {
            pen.boxed(
                0.06,
                0.3,
                0.94,
                0.44,
                Color32::from_rgb(200, 200, 205),
                GREY,
            );
            pen.cabinet(0.14, 0.44, 0.86, 0.9, 2);
        }
        Icon::Molding => {
            pen.poly(
                &[(0.12, 0.18), (0.88, 0.18), (0.88, 0.34), (0.12, 0.34)],
                WOOD,
                GOLD_DARK,
            );
            pen.poly(
                &[(0.2, 0.34), (0.8, 0.34), (0.7, 0.48), (0.3, 0.48)],
                GOLD,
                GOLD_DARK,
            );
            pen.cabinet(0.2, 0.48, 0.8, 0.9, 1);
        }
        Icon::AutoFill => {
            pen.fill(0.06, 0.08, 0.94, 0.16, GREY);
            pen.cabinet(0.08, 0.5, 0.36, 0.9, 1);
            pen.cabinet(0.38, 0.5, 0.64, 0.9, 1);
            pen.frame(0.66, 0.5, 0.92, 0.9, GOLD_DARK, 1.0);
            pen.line(&[(0.7, 0.36), (0.88, 0.36)], BLUE, 2.0);
            pen.poly(&[(0.88, 0.28), (0.96, 0.36), (0.88, 0.44)], BLUE, BLUE);
        }
        Icon::Section => {
            pen.cabinet(0.3, 0.1, 0.7, 0.42, 1);
            pen.cabinet(0.3, 0.52, 0.7, 0.9, 1);
            pen.line(&[(0.1, 0.06), (0.1, 0.94)], RED, 2.0);
            pen.line(&[(0.1, 0.06), (0.22, 0.06)], RED, 2.0);
            pen.line(&[(0.1, 0.94), (0.22, 0.94)], RED, 2.0);
        }
        Icon::Cad => {
            pen.poly(&[(0.08, 0.92), (0.08, 0.3), (0.7, 0.92)], white, BLUE);
            pen.line(&[(0.36, 0.64), (0.88, 0.12)], RED, 3.0);
            pen.poly(&[(0.84, 0.08), (0.94, 0.18), (0.88, 0.2)], GOLD, GOLD_DARK);
        }
        Icon::ZoomWindow | Icon::ZoomOut | Icon::ZoomAll => {
            if icon == Icon::ZoomWindow {
                pen.frame(0.04, 0.04, 0.6, 0.6, BLUE, 1.0);
            }
            pen.circle(0.42, 0.42, 0.24, white, GREY);
            pen.line(&[(0.6, 0.6), (0.9, 0.9)], GREY, 4.0);
            match icon {
                Icon::ZoomOut => pen.line(&[(0.3, 0.42), (0.54, 0.42)], GREY, 2.5),
                Icon::ZoomAll => pen.frame(0.32, 0.32, 0.52, 0.52, GREY, 1.0),
                _ => {}
            }
        }
        Icon::Line => {
            pen.line(&[(0.14, 0.86), (0.86, 0.14)], GREY, 2.5);
            pen.circle(0.14, 0.86, 0.07, BLUE, BLUE);
            pen.circle(0.86, 0.14, 0.07, BLUE, BLUE);
        }
        Icon::Rectangle => {
            pen.frame(0.12, 0.2, 0.88, 0.8, GREY, 2.5);
            for (x, y) in [(0.12, 0.2), (0.88, 0.2), (0.88, 0.8), (0.12, 0.8)] {
                pen.circle(x, y, 0.06, BLUE, BLUE);
            }
        }
        Icon::Select => {
            pen.poly(
                &[(0.24, 0.1), (0.24, 0.78), (0.4, 0.62), (0.62, 0.62)],
                white,
                Color32::from_rgb(30, 30, 30),
            );
            pen.line(
                &[(0.38, 0.6), (0.52, 0.88)],
                Color32::from_rgb(30, 30, 30),
                2.5,
            );
        }
        Icon::Splash => {
            pen.cabinet(0.1, 0.06, 0.9, 0.32, 2);
            for y in [0.4, 0.5] {
                pen.line(&[(0.1, y), (0.9, y)], GREY, 1.0);
            }
            for x in [0.3, 0.5, 0.7] {
                pen.line(&[(x, 0.36), (x, 0.56)], GREY, 1.0);
            }
            pen.fill(0.06, 0.56, 0.94, 0.64, Color32::from_rgb(200, 200, 205));
            pen.cabinet(0.1, 0.64, 0.9, 0.94, 2);
        }
        Icon::Grain => {
            pen.boxed(0.18, 0.06, 0.82, 0.94, WOOD, GOLD_DARK);
            for k in 0..4 {
                let x = 0.3 + k as f32 * 0.12;
                pen.line(
                    &[(x, 0.1), (x + 0.04, 0.35), (x - 0.02, 0.6), (x + 0.03, 0.9)],
                    GOLD_DARK,
                    1.2,
                );
            }
        }
        Icon::Turn => {
            pen.frame(0.14, 0.14, 0.86, 0.86, GREY, 3.0);
            pen.curved_arrow(false, GREEN);
        }
    }
}
