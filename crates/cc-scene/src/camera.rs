//! Kamera a premietnutie: svet → obraz (CPU raster, popisy), matica pre GPU a lúč z obrazu
//! pre výber. Všetky tri cesty používajú tie isté vzorce, aby sa obraz okna zhodoval so
//! snímkou bez okna.

use glam::{DMat4, DVec3, DVec4};

use crate::Scene;

/// Pohľad: otočenie okolo zvislej osi, náklon, priblíženie a posun (v bodoch obrazovky).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub yaw: f64,
    pub pitch: f64,
    pub zoom: f64,
    pub pan: [f64; 2],
    /// `true`: perspektíva, `false`: ortogonálny pohľad (bez zbiehania hrán).
    pub perspective: bool,
    /// Obálka (min, max), ktorá sa vmestí do obrazu; `None` = celá scéna.
    pub fit: Option<[DVec3; 2]>,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            yaw: 32f64.to_radians(),
            pitch: 20f64.to_radians(),
            zoom: 1.0,
            pan: [0.0, 0.0],
            perspective: true,
            fit: None,
        }
    }
}

impl Camera {
    /// Najväčší náklon (pohľad zhora / zdola).
    pub const PITCH_LIMIT: f64 = std::f64::consts::FRAC_PI_2;

    /// Pohľad spredu (od `y < 0` k zadnej stene).
    pub fn front(self) -> Camera {
        Camera {
            yaw: 0.0,
            pitch: 0.0,
            ..self.reset_view()
        }
    }

    /// Pohľad zhora (zadná stena hore).
    pub fn top(self) -> Camera {
        Camera {
            yaw: 0.0,
            pitch: Self::PITCH_LIMIT,
            ..self.reset_view()
        }
    }

    /// Izometrický pohľad spredu zľava.
    pub fn iso(self) -> Camera {
        Camera {
            yaw: 45f64.to_radians(),
            pitch: (1.0 / 2f64.sqrt()).atan(),
            ..self.reset_view()
        }
    }

    /// Vmestí obálku do obrazu (priblíženie na výber).
    pub fn fit_to(self, lo: DVec3, hi: DVec3) -> Camera {
        Camera {
            fit: Some([lo, hi]),
            zoom: 1.0,
            pan: [0.0, 0.0],
            ..self
        }
    }

    fn reset_view(self) -> Camera {
        Camera {
            zoom: 1.0,
            pan: [0.0, 0.0],
            fit: None,
            ..self
        }
    }
}

/// Svet → obrazovka (pixely) a hĺbka (mm, väčšia = ďalej).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    yaw: (f64, f64),
    pitch: (f64, f64),
    center: DVec3,
    scale: f64,
    origin: [f64; 2],
    /// vzdialenosť oka od stredu scény (mm); `None` = ortogonálne
    dist: Option<f64>,
    /// rozsah hĺbky celej scény v súradniciach pohľadu (bez `center`)
    depth: (f64, f64),
    size: [f64; 2],
}

/// Rohy obálky (index: bit 0 = x, bit 1 = y, bit 2 = z).
fn box_corners(lo: DVec3, hi: DVec3) -> [DVec3; 8] {
    std::array::from_fn(|i| {
        DVec3::new(
            if i & 1 != 0 { hi.x } else { lo.x },
            if i & 2 != 0 { hi.y } else { lo.y },
            if i & 4 != 0 { hi.z } else { lo.z },
        )
    })
}

impl Projection {
    /// Otočenie sveta do súradníc pohľadu (vodorovne, nahor, hĺbka).
    pub fn rotate(&self, p: DVec3) -> DVec3 {
        let (sa, ca) = self.yaw;
        let x = p.x * ca - p.y * sa;
        let y = p.x * sa + p.y * ca;
        let (sp, cp) = self.pitch;
        DVec3::new(x, y * sp + p.z * cp, y * cp - p.z * sp)
    }

    /// Opačné otočenie (pohľad → svet); otočenie je ortonormálne.
    pub fn unrotate(&self, v: DVec3) -> DVec3 {
        let (sa, ca) = self.yaw;
        let (sp, cp) = self.pitch;
        DVec3::new(
            ca * v.x + sa * sp * v.y + sa * cp * v.z,
            -sa * v.x + ca * sp * v.y + ca * cp * v.z,
            cp * v.y - sp * v.z,
        )
    }

    pub fn new(scene: &Scene, cam: &Camera, width: f64, height: f64, pan_scale: f64) -> Self {
        let mut p = Projection {
            yaw: cam.yaw.sin_cos(),
            pitch: cam.pitch.sin_cos(),
            center: DVec3::ZERO,
            scale: 1.0,
            origin: [0.0, 0.0],
            dist: None,
            depth: (0.0, 1.0),
            size: [width.max(1.0), height.max(1.0)],
        };
        let (smin, smax) = scene.bounds();
        let [fmin, fmax] = cam.fit.unwrap_or([smin, smax]);
        let rot = p;
        let rotated_bounds = |lo: DVec3, hi: DVec3| {
            box_corners(lo, hi).iter().fold(
                (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
                |(a, b), c| {
                    let r = rot.rotate(*c);
                    (a.min(r), b.max(r))
                },
            )
        };
        // stred a mierka z guľovej obálky (nezávislé od natočenia): pri otáčaní sa model
        // nezväčšuje ani nezmenšuje a stred neuskakuje
        p.center = rot.rotate((fmin + fmax) / 2.0);
        let size = (fmax - fmin).length().max(1.0);
        // oko vo vzdialenosti ~2,2 × uhlopriečka scény: mierna perspektíva bez skreslenia
        let diagonal = (smax - smin).length().max(500.0);
        p.dist = cam.perspective.then_some(diagonal * 2.2);
        let fit = if cam.perspective { 0.92 } else { 1.0 };
        p.scale = width.min(height) / size * fit * cam.zoom;
        p.origin = [
            width / 2.0 + cam.pan[0] * pan_scale,
            height / 2.0 + cam.pan[1] * pan_scale,
        ];
        let (slo, shi) = rotated_bounds(smin, smax);
        p.depth = (slo.z - p.center.z, shi.z - p.center.z);
        p
    }

    /// (x, y, hĺbka) v pixeloch obrazu. Hĺbka je menšia bližšie k oku a dá sa
    /// lineárne interpolovať v obraze (pri perspektíve −1/w).
    pub fn project(&self, p: DVec3) -> DVec3 {
        let r = self.rotate(p) - self.center;
        match self.dist {
            None => DVec3::new(
                self.origin[0] + r.x * self.scale,
                self.origin[1] - r.y * self.scale,
                r.z,
            ),
            Some(d) => {
                let w = (d + r.z).max(1.0);
                let k = d / w;
                DVec3::new(
                    self.origin[0] + r.x * self.scale * k,
                    self.origin[1] - r.y * self.scale * k,
                    -1.0 / w,
                )
            }
        }
    }

    /// Posun `mm` (smerom k oku) v jednotkách hĺbky v mieste s hĺbkou `depth`.
    pub(crate) fn depth_bias(&self, mm: f64, depth: f64) -> f64 {
        match self.dist {
            None => mm,
            Some(_) => mm * depth * depth,
        }
    }

    /// Je plocha s normálou `n` v bode `p` (svet) natočená k oku?
    pub(crate) fn facing(&self, n: DVec3, p: DVec3) -> bool {
        let nr = self.rotate(n);
        match self.dist {
            None => nr.z <= 1e-9,
            Some(d) => {
                let r = self.rotate(p) - self.center;
                nr.dot(r - DVec3::new(0.0, 0.0, -d)) <= 1e-9
            }
        }
    }

    /// Je stena s normálou do miestnosti `n` medzi okom a miestnosťou (skryje sa)?
    pub fn wall_hidden(&self, n: DVec3) -> bool {
        self.rotate(n).z > crate::style::WALL_CULL
    }

    /// Oko vo svete (pri perspektíve).
    pub fn eye(&self) -> Option<DVec3> {
        self.dist
            .map(|d| self.unrotate(DVec3::new(0.0, 0.0, -d) + self.center))
    }

    /// Smer pohľadu vo svete (od oka do scény) pri ortogonálnom pohľade.
    pub fn view_dir(&self) -> DVec3 {
        self.unrotate(DVec3::Z)
    }

    /// Lúč z bodu obrazu (pixely) vo svete: (začiatok, jednotkový smer).
    pub fn ray(&self, x: f64, y: f64) -> (DVec3, DVec3) {
        let rx = (x - self.origin[0]) / self.scale;
        let ry = -(y - self.origin[1]) / self.scale;
        match self.dist {
            None => {
                let far = (self.depth.1 - self.depth.0).abs() + 1000.0;
                let o = DVec3::new(rx, ry, self.depth.0 - far) + self.center;
                (self.unrotate(o), self.view_dir())
            }
            Some(d) => {
                let eye = DVec3::new(0.0, 0.0, -d);
                let dir = DVec3::new(rx, ry, 0.0) - eye;
                (
                    self.unrotate(eye + self.center),
                    self.unrotate(dir).normalize_or_zero(),
                )
            }
        }
    }

    /// Matica svet → orezové súradnice pre GPU (stĺpce, hĺbka 0…1) pre obraz rovnakej
    /// veľkosti, s akou bola projekcia vytvorená. Zodpovedá [`Projection::project`].
    pub fn clip_matrix(&self) -> [[f32; 4]; 4] {
        let [w, h] = self.size;
        let s = self.scale;
        let (ox, oy) = (self.origin[0], self.origin[1]);
        let ax = 2.0 * ox / w - 1.0;
        let ay = 1.0 - 2.0 * oy / h;
        let margin = 10.0 + (self.depth.1 - self.depth.0).abs() * 0.01;
        let (zl, zh) = (self.depth.0 - margin, self.depth.1 + margin);
        let p = match self.dist {
            Some(d) => {
                let near = (d + zl).max(1.0);
                let far = (d + zh).max(near + 1.0);
                let f = far / (far - near);
                DMat4::from_cols(
                    DVec4::new(2.0 * s * d / w, 0.0, 0.0, 0.0),
                    DVec4::new(0.0, 2.0 * s * d / h, 0.0, 0.0),
                    DVec4::new(ax, ay, f, 1.0),
                    DVec4::new(ax * d, ay * d, f * (d - near), d),
                )
            }
            None => DMat4::from_cols(
                DVec4::new(2.0 * s / w, 0.0, 0.0, 0.0),
                DVec4::new(0.0, 2.0 * s / h, 0.0, 0.0),
                DVec4::new(0.0, 0.0, 1.0 / (zh - zl), 0.0),
                DVec4::new(ax, ay, -zl / (zh - zl), 1.0),
            ),
        };
        // svet → pohľad: otočenie a posun o stred
        let view = DMat4::from_cols(
            self.rotate(DVec3::X).extend(0.0),
            self.rotate(DVec3::Y).extend(0.0),
            self.rotate(DVec3::Z).extend(0.0),
            (-self.center).extend(1.0),
        );
        (p * view).to_cols_array_2d().map(|c| c.map(|v| v as f32))
    }

    /// Riadky otočenia svet → pohľad (pre tieňovanie na GPU).
    pub fn rotation_rows(&self) -> [[f64; 3]; 3] {
        let c = [
            self.rotate(DVec3::X),
            self.rotate(DVec3::Y),
            self.rotate(DVec3::Z),
        ];
        [0, 1, 2].map(|r| [c[0][r], c[1][r], c[2][r]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PartBox, SceneOptions};

    fn scene() -> Scene {
        let mut s = Scene {
            min: DVec3::new(0.0, 0.0, 0.0),
            max: DVec3::new(2400.0, 600.0, 900.0),
            ..Scene::default()
        };
        s.parts.push("A".into());
        s.part_cabinets.push("S1".into());
        s.boxes.push(PartBox {
            origin: DVec3::new(100.0, 0.0, 0.0),
            axes: [
                DVec3::new(600.0, 0.0, 0.0),
                DVec3::new(0.0, 560.0, 0.0),
                DVec3::new(0.0, 0.0, 720.0),
            ],
        });
        let _ = SceneOptions::default();
        s
    }

    /// Otočenie kamery nemení mierku ani stred obrazu (model sa pri otáčaní nenafukuje).
    #[test]
    fn scale_does_not_change_with_rotation() {
        let s = scene();
        let at = |yaw: f64, pitch: f64| {
            let cam = Camera {
                yaw,
                pitch,
                ..Camera::default()
            };
            let p = Projection::new(&s, &cam, 800.0, 600.0, 1.0);
            let c = p.project((s.min + s.max) / 2.0);
            (p.scale, c.x, c.y)
        };
        let a = at(0.3, 0.6);
        for (yaw, pitch) in [(1.2, 0.6), (2.5, 0.9), (-0.7, 0.2)] {
            let b = at(yaw, pitch);
            assert!((a.0 - b.0).abs() < 1e-9, "mierka {a:?} vs {b:?}");
            assert!(
                (a.1 - b.1).abs() < 1e-6 && (a.2 - b.2).abs() < 1e-6,
                "stred {a:?} vs {b:?}"
            );
        }
    }

    /// Matica pre GPU dáva rovnaké pixely ako `project` (perspektíva aj ortogonálne).
    #[test]
    fn clip_matrix_matches_project() {
        let s = scene();
        for perspective in [true, false] {
            let cam = Camera {
                perspective,
                zoom: 1.7,
                pan: [30.0, -12.0],
                ..Camera::default()
            };
            let p = Projection::new(&s, &cam, 800.0, 500.0, 1.25);
            let m = DMat4::from_cols_array_2d(&p.clip_matrix().map(|c| c.map(f64::from)));
            for q in [
                DVec3::new(0.0, 0.0, 0.0),
                DVec3::new(2400.0, 600.0, 900.0),
                DVec3::new(1200.0, 300.0, 450.0),
            ] {
                let c = m * q.extend(1.0);
                let ndc = c.truncate() / c.w;
                let x = (ndc.x + 1.0) / 2.0 * 800.0;
                let y = (1.0 - ndc.y) / 2.0 * 500.0;
                let e = p.project(q);
                assert!(
                    (x - e.x).abs() < 0.05 && (y - e.y).abs() < 0.05,
                    "{x},{y} vs {e}"
                );
                assert!((0.0..=1.0).contains(&ndc.z), "hĺbka {}", ndc.z);
            }
        }
    }

    /// Lúč z premietnutého stredu dielu ho zasiahne; lúč mimo nie.
    #[test]
    fn ray_from_projected_point_hits_part() {
        let s = scene();
        for cam in [
            Camera::default(),
            Camera {
                perspective: false,
                ..Camera::default()
            },
            Camera::default().top(),
            Camera::default().front(),
        ] {
            let p = Projection::new(&s, &cam, 800.0, 500.0, 1.0);
            let c = p.project(DVec3::new(400.0, 280.0, 360.0));
            let (o, d) = p.ray(c.x, c.y);
            assert_eq!(s.pick(o, d), Some(1), "{cam:?}");
            let far = p.project(DVec3::new(2300.0, 300.0, 450.0));
            let (o, d) = p.ray(far.x, far.y);
            assert_eq!(s.pick(o, d), None, "{cam:?}");
        }
    }

    #[test]
    fn unrotate_inverts_rotate() {
        let s = scene();
        let p = Projection::new(&s, &Camera::default(), 100.0, 100.0, 1.0);
        let v = DVec3::new(1.0, -2.0, 3.0);
        assert!((p.unrotate(p.rotate(v)) - v).length() < 1e-12);
    }

    #[test]
    fn fit_centers_box() {
        let s = scene();
        let cam =
            Camera::default().fit_to(DVec3::new(100.0, 0.0, 0.0), DVec3::new(700.0, 560.0, 720.0));
        let p = Projection::new(&s, &cam, 800.0, 600.0, 1.0);
        let c = p.project(DVec3::new(400.0, 280.0, 360.0));
        assert!(
            (c.x - 400.0).abs() < 40.0 && (c.y - 300.0).abs() < 40.0,
            "{c}"
        );
    }
}
