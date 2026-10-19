//! Softvérový raster so z-bufferom: snímky bez okna (`ui.render`, `cabinetcraft render`),
//! testy a náhľad v okne bez GPU.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use glam::DVec3;

use crate::style;
use crate::{Camera, Projection, Scene, Selection};

/// Vykreslený obraz.
pub struct Image {
    pub width: usize,
    pub height: usize,
    /// RGB, `width × height × 3`
    pub rgb: Vec<u8>,
    /// index dielu + 1 pre každý pixel (0 = pozadie)
    pub ids: Vec<u32>,
    /// projekcia v pixeloch výsledného obrazu
    pub projection: Projection,
}

/// Vykreslí scénu do obrazu `width × height` s prevzorkovaním `ss` (1 = bez).
#[allow(clippy::too_many_arguments)]
pub fn render(
    scene: &Scene,
    cam: &Camera,
    width: usize,
    height: usize,
    ss: usize,
    pan_scale: f64,
    selection: Selection,
    background: [u8; 3],
) -> Image {
    let selected = selection.part;
    let ss = ss.max(1);
    let (w, h) = (width.max(1) * ss, height.max(1) * ss);
    let proj = Projection::new(scene, cam, w as f64, h as f64, pan_scale * ss as f64);
    let mut color = vec![
        [
            background[0] as f64 / 255.0,
            background[1] as f64 / 255.0,
            background[2] as f64 / 255.0
        ];
        w * h
    ];
    let mut depth = vec![f64::INFINITY; w * h];
    let mut ids = vec![0u32; w * h];
    let l = DVec3::from_array(style::LIGHT).normalize();

    for tri in &scene.tris {
        if tri.cull.is_some_and(|n| proj.wall_hidden(n)) {
            continue; // stena medzi kamerou a miestnosťou
        }
        let nr = proj.rotate(tri.n);
        if !proj.facing(tri.n, (tri.p[0] + tri.p[1] + tri.p[2]) / 3.0) {
            continue; // odvrátená plocha
        }
        let shade = if tri.lit {
            style::AMBIENT + style::DIFFUSE * nr.dot(l).max(0.0)
        } else {
            1.0
        };
        let mut c = tri.color;
        if selected == Some(tri.part) && tri.lit {
            for (v, h) in c.iter_mut().zip(style::HIGHLIGHT) {
                *v = *v * (1.0 - style::HIGHLIGHT_MIX) + h * style::HIGHLIGHT_MIX;
            }
        }
        let c = c.map(|v| (v * shade).min(1.0));
        let p = tri.p.map(|v| proj.project(v));
        let area = edge(p[0], p[1], p[2]);
        if area.abs() < 1e-12 {
            continue;
        }
        let min_x = p
            .iter()
            .map(|v| v.x)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as usize;
        let max_x = p
            .iter()
            .map(|v| v.x)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(w as f64 - 1.0);
        let min_y = p
            .iter()
            .map(|v| v.y)
            .fold(f64::INFINITY, f64::min)
            .floor()
            .max(0.0) as usize;
        let max_y = p
            .iter()
            .map(|v| v.y)
            .fold(f64::NEG_INFINITY, f64::max)
            .ceil()
            .min(h as f64 - 1.0);
        if max_x < 0.0 || max_y < 0.0 {
            continue;
        }
        let (max_x, max_y) = (max_x as usize, max_y as usize);
        // otvory a drážky ležia tesne nad plochou: malé zvýhodnenie v hĺbke
        let bias = if tri.lit {
            0.0
        } else {
            proj.depth_bias(style::MARK_BIAS_MM, (p[0].z + p[1].z + p[2].z) / 3.0)
        };
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let q = DVec3::new(x as f64 + 0.5, y as f64 + 0.5, 0.0);
                let w0 = edge(p[1], p[2], q) / area;
                let w1 = edge(p[2], p[0], q) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * p[0].z + w1 * p[1].z + w2 * p[2].z - bias;
                let i = y * w + x;
                if z < depth[i] {
                    depth[i] = z;
                    color[i] = c;
                    ids[i] = tri.part;
                }
            }
        }
    }

    // hrany dielov
    let thick = ss as isize;
    for line in &scene.lines {
        if line.cull.is_some_and(|n| proj.wall_hidden(n)) {
            continue;
        }
        let a = proj.project(line.a);
        let b = proj.project(line.b);
        let steps = (b.x - a.x).abs().max((b.y - a.y).abs()).ceil().max(1.0) as usize;
        let c = if selected == Some(line.part) {
            style::EDGE_SELECTED
        } else {
            style::EDGE
        };
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let p = a + (b - a) * t;
            let (cx, cy) = (p.x.floor() as isize, p.y.floor() as isize);
            for dy in 0..thick {
                for dx in 0..thick {
                    let (x, y) = (cx + dx - thick / 2, cy + dy - thick / 2);
                    if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
                        continue;
                    }
                    let i = y as usize * w + x as usize;
                    if p.z - proj.depth_bias(style::EDGE_BIAS_MM, p.z) <= depth[i] {
                        color[i] = c;
                    }
                }
            }
        }
    }

    // obrys vybranej skrinky navrch (bez hĺbky)
    if let Some(index) = selection.cabinet {
        let half = (style::OUTLINE_WIDTH * pan_scale * ss as f64 / 2.0).max(0.5);
        for [a, b] in scene.cabinet_outline(index) {
            let a = proj.project(a);
            let b = proj.project(b);
            let steps = (b.x - a.x).abs().max((b.y - a.y).abs()).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let p = a + (b - a) * (s as f64 / steps as f64);
                let (x0, x1) = ((p.x - half).round() as isize, (p.x + half).round() as isize);
                let (y0, y1) = ((p.y - half).round() as isize, (p.y + half).round() as isize);
                for y in y0.max(0)..y1.min(h as isize) {
                    for x in x0.max(0)..x1.min(w as isize) {
                        color[y as usize * w + x as usize] = style::OUTLINE;
                    }
                }
            }
        }
    }

    // zmenšenie (priemer ss × ss)
    let mut rgb = Vec::with_capacity(width * height * 3);
    let mut out_ids = Vec::with_capacity(width * height);
    let n = (ss * ss) as f64;
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 3];
            for sy in 0..ss {
                for sx in 0..ss {
                    let c = color[(y * ss + sy) * w + x * ss + sx];
                    for k in 0..3 {
                        sum[k] += c[k];
                    }
                }
            }
            for v in sum {
                rgb.push((v / n * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            out_ids.push(ids[(y * ss + ss / 2) * w + x * ss + ss / 2]);
        }
    }
    Image {
        width,
        height,
        rgb,
        ids: out_ids,
        projection: Projection::new(scene, cam, width as f64, height as f64, pan_scale),
    }
}

fn edge(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Zápis PNG; `channels`: 3 = RGB, 4 = RGBA.
pub fn write_png(
    path: &Path,
    width: usize,
    height: usize,
    data: &[u8],
    channels: usize,
) -> Result<(), String> {
    if data.len() != width * height * channels {
        return Err(format!(
            "PNG {width} × {height} × {channels}: nesprávna dĺžka dát {}",
            data.len()
        ));
    }
    let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(if channels == 4 {
        png::ColorType::Rgba
    } else {
        png::ColorType::Rgb
    });
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(data).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneOptions;
    use cc_model::{Model, ProjectFile};

    fn project(name: &str) -> (Model, ProjectFile) {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let loaded = cc_config::load(&root.join("examples/dielna").join(name)).unwrap();
        let model = cc_pipeline::build(&loaded.project, &loaded.standard, &loaded.materials, None)
            .unwrap()
            .model;
        (model, loaded.project)
    }

    #[test]
    fn renders_all_parts_and_picks_ids() {
        let (model, project) = project("project.toml");
        let scene = Scene::build(&model, &project, &SceneOptions::default());
        assert_eq!(scene.parts.len(), model.parts().count());
        assert_eq!(scene.boxes.len(), scene.parts.len());
        let img = render(
            &scene,
            &Camera::default(),
            400,
            300,
            1,
            1.0,
            Selection::default(),
            [255, 255, 255],
        );
        assert_eq!(img.rgb.len(), 400 * 300 * 3);
        let visible: std::collections::BTreeSet<u32> =
            img.ids.iter().copied().filter(|&i| i > 0).collect();
        // väčšina dielov je z pohľadu spredu zľava zhora viditeľná
        assert!(visible.len() >= 8, "{visible:?}");
    }

    /// Výber lúčom sa zhoduje s dielom, ktorý raster nakreslil v danom pixeli.
    #[test]
    fn ray_pick_agrees_with_raster_ids() {
        let (model, project) = project("kuchyna.toml");
        let scene = Scene::build(&model, &project, &SceneOptions::default());
        for cam in [
            Camera::default(),
            Camera::default().iso(),
            Camera::default().front(),
        ] {
            let img = render(
                &scene,
                &cam,
                320,
                240,
                1,
                1.0,
                Selection::default(),
                [255, 255, 255],
            );
            let (mut same, mut total) = (0, 0);
            for y in (0..240).step_by(7) {
                for x in (0..320).step_by(7) {
                    let id = img.ids[y * 320 + x];
                    if id == 0 {
                        continue;
                    }
                    total += 1;
                    let (o, d) = img.projection.ray(x as f64 + 0.5, y as f64 + 0.5);
                    if scene.pick(o, d) == Some(id) {
                        same += 1;
                    }
                }
            }
            // rozdiely iba na hranách dielov a pri otvoroch tesne nad plochou
            assert!(
                total > 100 && same * 100 >= total * 95,
                "{same}/{total} {cam:?}"
            );
        }
    }

    /// Obrys vybranej skrinky má farbu obrysu a skrinka sa dá nájsť podľa ID.
    #[test]
    fn cabinet_outline_is_drawn() {
        let (model, project) = project("kuchyna.toml");
        let scene = Scene::build(&model, &project, &SceneOptions::default());
        assert_eq!(scene.cabinet_boxes.len(), model.cabinets.len());
        let sel = scene.selection(None, Some("SP1"));
        assert_eq!(sel.cabinet, Some(1));
        let outline = [0.0, 0.784, 0.902].map(|v: f64| (v * 255.0).round() as u8);
        let count = |sel| {
            let img = render(
                &scene,
                &Camera::default(),
                400,
                300,
                1,
                1.0,
                sel,
                [255, 255, 255],
            );
            img.rgb.chunks(3).filter(|c| *c == outline).count()
        };
        assert_eq!(count(Selection::default()), 0);
        assert!(count(sel) > 200);
        let (lo, hi) = scene.cabinet_bounds("SP1").unwrap();
        assert!((hi - lo).max_element() >= 649.0);
    }

    #[test]
    fn room_has_walls_and_cabinets_inside() {
        let (model, project) = project("kuchyna.toml");
        let scene = Scene::build(&model, &project, &SceneOptions::default());
        assert_eq!(scene.parts.len(), model.parts().count());
        let (min, max) = scene.bounds();
        // miestnosť 3600 × 2800 + hrúbka stien 120
        assert!(min.x <= -119.0 && max.x >= 3719.0, "{min:?} {max:?}");
        assert!(max.z >= 2599.0);
        let img = render(
            &scene,
            &Camera::default(),
            400,
            300,
            1,
            1.0,
            Selection::default(),
            [255, 255, 255],
        );
        let visible = img.ids.iter().filter(|&&i| i > 0).count();
        assert!(
            visible > 5_000,
            "skrinky musia byť vidieť cez skrytú prednú stenu: {visible}"
        );
    }

    /// Uloží náhľad do PNG pre vizuálnu kontrolu: `CABINETCRAFT_RENDER_PNG=cesta cargo test`.
    #[test]
    fn optional_png_dump() {
        let Ok(path) = std::env::var("CABINETCRAFT_RENDER_PNG") else {
            return;
        };
        let (model, project) = project("kuchyna.toml");
        let scene = Scene::build(&model, &project, &SceneOptions::default());
        let img = render(
            &scene,
            &Camera::default(),
            900,
            650,
            2,
            1.0,
            Selection {
                part: Some(3),
                cabinet: Some(0),
            },
            style::BACKGROUND_LIGHT,
        );
        write_png(Path::new(&path), img.width, img.height, &img.rgb, 3).unwrap();
    }
}
