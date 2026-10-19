//! 2D plátno v mm (pôdorys, pohľad na stenu): posun, zoom ku kurzoru, kóty.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};

/// Pohľad: stred v mm a mierka (body obrazovky na mm). Os y v mm smeruje nahor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View2d {
    pub center: [f64; 2],
    pub scale: f64,
    /// `true`: pohľad sa pri ďalšom kreslení prispôsobí obsahu.
    pub fit_pending: bool,
}

impl Default for View2d {
    fn default() -> Self {
        View2d {
            center: [0.0, 0.0],
            scale: 0.2,
            fit_pending: true,
        }
    }
}

/// Prepočet mm ↔ obrazovka pre jeden snímok.
#[derive(Debug, Clone, Copy)]
pub struct Xf {
    pub rect: Rect,
    center: [f64; 2],
    scale: f64,
}

impl Xf {
    pub fn to_screen(self, x: f64, y: f64) -> Pos2 {
        let c = self.rect.center();
        Pos2::new(
            c.x + ((x - self.center[0]) * self.scale) as f32,
            c.y - ((y - self.center[1]) * self.scale) as f32,
        )
    }

    pub fn to_mm(self, p: Pos2) -> [f64; 2] {
        let c = self.rect.center();
        [
            self.center[0] + (p.x - c.x) as f64 / self.scale,
            self.center[1] - (p.y - c.y) as f64 / self.scale,
        ]
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }
}

impl View2d {
    /// Spracuje posun (pravé/stredné tlačidlo) a zoom (koliesko) a vráti prepočet.
    /// `bounds` (min x, min y, max x, max y) sa použije pri prispôsobení.
    pub fn interact(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        bounds: Option<[f64; 4]>,
    ) -> Xf {
        let rect = response.rect;
        if self.fit_pending {
            if let Some(b) = bounds {
                self.fit(rect, b);
            }
            self.fit_pending = false;
        }
        if response.dragged_by(egui::PointerButton::Secondary)
            || response.dragged_by(egui::PointerButton::Middle)
        {
            let d = response.drag_delta();
            self.center[0] -= d.x as f64 / self.scale;
            self.center[1] += d.y as f64 / self.scale;
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                if let Some(pointer) = response.hover_pos() {
                    let before = self.xf(rect).to_mm(pointer);
                    self.scale = (self.scale * (1.0 + scroll as f64 * 0.0015)).clamp(0.005, 20.0);
                    let after = self.xf(rect).to_mm(pointer);
                    self.center[0] += before[0] - after[0];
                    self.center[1] += before[1] - after[1];
                }
            }
        }
        if response.double_clicked_by(egui::PointerButton::Secondary) {
            if let Some(b) = bounds {
                self.fit(rect, b);
            }
        }
        self.xf(rect)
    }

    pub fn xf(&self, rect: Rect) -> Xf {
        Xf {
            rect,
            center: self.center,
            scale: self.scale,
        }
    }

    pub fn fit(&mut self, rect: Rect, b: [f64; 4]) {
        let w = (b[2] - b[0]).max(100.0);
        let h = (b[3] - b[1]).max(100.0);
        self.center = [(b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0];
        self.scale = ((rect.width() as f64 - 80.0) / w)
            .min((rect.height() as f64 - 80.0) / h)
            .max(0.005);
    }
}

/// Kóta medzi bodmi `a` a `b` (mm), odsadená o `offset` bodov obrazovky na ľavú stranu smeru a → b.
pub fn dimension(
    painter: &egui::Painter,
    xf: &Xf,
    a: [f64; 2],
    b: [f64; 2],
    offset: f32,
    text: &str,
    color: Color32,
) {
    let (pa, pb) = (xf.to_screen(a[0], a[1]), xf.to_screen(b[0], b[1]));
    let dir = (pb - pa).normalized();
    if !dir.x.is_finite() {
        return;
    }
    let n = Vec2::new(dir.y, -dir.x) * offset;
    let stroke = Stroke::new(1.0f32, color);
    let (qa, qb) = (pa + n, pb + n);
    painter.line_segment([pa + n * 0.15, qa + n.normalized() * 4.0], stroke);
    painter.line_segment([pb + n * 0.15, qb + n.normalized() * 4.0], stroke);
    painter.line_segment([qa, qb], stroke);
    for (p, d) in [(qa, dir), (qb, -dir)] {
        let t = Vec2::new(-d.y, d.x) * 3.0;
        painter.line_segment([p + d * 6.0 + t, p], stroke);
        painter.line_segment([p + d * 6.0 - t, p], stroke);
    }
    let mid = qa + (qb - qa) * 0.5 + n.normalized() * 8.0;
    let mut angle = dir.y.atan2(dir.x);
    if angle > std::f32::consts::FRAC_PI_2 || angle < -std::f32::consts::FRAC_PI_2 {
        angle += std::f32::consts::PI;
    }
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(12.0), color);
    let size = galley.size();
    // text otočený pozdĺž kóty, vycentrovaný
    let (s, c) = angle.sin_cos();
    let offset_vec = Vec2::new(
        -size.x / 2.0 * c + size.y / 2.0 * s,
        -size.x / 2.0 * s - size.y / 2.0 * c,
    );
    let mut shape = egui::epaint::TextShape::new(mid + offset_vec, galley, color);
    shape.angle = angle;
    painter.add(shape);
}
