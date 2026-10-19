//! Výkresy do PDF: zákazka, skrinky, diely (`cc-out-pdf`), pohľady kreslí `cc-scene`.

use std::path::PathBuf;

use cc_i18n::tr;
use cc_model::{Model, ProjectFile};
use cc_out_pdf::drawings::{
    self as sheets, PlanKey, RenderedView, Sheet, SheetBody, TitleInfo, ViewPlan, WallCabinet,
    WallElevation, WallOpening,
};
use cc_out_pdf::pdf::RgbImage;
use cc_room::{Room, Row};
use cc_scene::{Camera, Scene, SceneOptions, Selection};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{has_doc, parse, CommandSpec, Effect};
use crate::{EngineError, Session};

pub static COMMANDS: &[CommandSpec] = &[CommandSpec {
    id: "export.drawings",
    menu: true,
    shortcut: Some("Ctrl+P"),
    params: r#"{"path"?: "výkresy.pdf (predvolene out/vykresy.pdf vedľa projektu)", "sheets"?: ["job", "walls", "cabinets", "parts", "wall:W1", "cabinet:SP1", "part:SP1-SIDE-L"] (predvolene zákazka, steny so skrinkami alebo otvormi, všetky skrinky a diely), "parts"?: "all"|"machined"}"#,
    effect: Effect::Read,
    enabled: has_doc,
    run: export,
}];

/// Najviac strán v jednom PDF (ochrana pred nezmyselným vstupom).
const MAX_SHEETS: usize = 2000;

#[derive(Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum PartsFilter {
    #[default]
    All,
    Machined,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    path: Option<PathBuf>,
    sheets: Option<Vec<String>>,
    #[serde(default)]
    parts: PartsFilter,
}

fn export(s: &mut Session, params: Value) -> Result<Value, EngineError> {
    const C: &str = "export.drawings";
    let p: Params = parse(C, params)?;
    let doc = s.require_doc()?;
    let built = s
        .model()
        .ok_or_else(|| EngineError::Failed(tr!("engine.errors.model_failed")))?;
    let model = &built.model;
    let project = &doc.project;
    let programs = cc_pipeline::program_names(model, doc.machine.as_ref());
    let mut warnings: Vec<String> = Vec::new();
    let mut base_title = title_info(project);
    // logo: chyba (napr. presunutý súbor) výkresy nezastaví, iba sa ohlási
    if let Some(path) = project.project.logo.as_deref() {
        match crate::logo::load(&doc.base, path) {
            Ok(img) => base_title.logo = cc_out_pdf::pdf::PdfImage::new(&img),
            Err(e) => warnings.push(e),
        }
    }

    let wanted = p.sheets.unwrap_or_else(|| {
        ["job", "walls", "cabinets", "parts"]
            .map(str::to_owned)
            .to_vec()
    });
    let mut out: Vec<Sheet> = Vec::new();
    for key in &wanted {
        let before = out.len();
        match key.as_str() {
            "job" => out.push(job_sheet(model, project, &base_title)),
            "walls" => {
                for w in &project.walls {
                    let used = project
                        .cabinets
                        .iter()
                        .any(|c| c.wall.as_deref() == Some(&w.id))
                        || project.openings.iter().any(|o| o.wall == w.id);
                    if used {
                        out.extend(wall_sheet(project, &base_title, &w.id));
                    }
                }
            }
            "cabinets" => {
                for c in &model.cabinets {
                    out.extend(cabinet_sheet(model, project, &base_title, &c.id));
                }
            }
            "parts" => {
                for part in model.parts() {
                    if p.parts == PartsFilter::Machined && part.operations.is_empty() {
                        continue;
                    }
                    out.extend(part_sheet(model, project, &base_title, &programs, &part.id));
                }
            }
            other => {
                if let Some(id) = other.strip_prefix("wall:") {
                    out.extend(wall_sheet(project, &base_title, id));
                } else if let Some(id) = other.strip_prefix("cabinet:") {
                    out.extend(cabinet_sheet(model, project, &base_title, id));
                } else if let Some(id) = other.strip_prefix("part:") {
                    out.extend(part_sheet(model, project, &base_title, &programs, id));
                } else {
                    return Err(EngineError::Params {
                        command: C.into(),
                        message: tr!("engine.drawings.unknown_sheet", sheet = other),
                    });
                }
                if out.len() == before {
                    return Err(tr!("engine.drawings.not_in_job", sheet = other).into());
                }
            }
        }
        if out.len() > MAX_SHEETS {
            return Err(tr!("engine.drawings.too_many_sheets", max = MAX_SHEETS).into());
        }
    }
    if out.is_empty() {
        return Err(tr!("engine.drawings.nothing_to_print").into());
    }
    let path = p
        .path
        .unwrap_or_else(|| doc.base.join("out").join("vykresy.pdf"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let pdf = sheets::drawings_pdf(
        &out,
        &cc_out_pdf::Fonts::new(cc_out_pdf::FontChoice::System),
    );
    std::fs::write(&path, &pdf).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json!({ "path": path, "pages": out.len(), "bytes": pdf.len(), "warnings": warnings }))
}

fn title_info(project: &ProjectFile) -> TitleInfo {
    let info = &project.project;
    TitleInfo {
        company: info.company.clone().unwrap_or_default(),
        project: info.name.clone(),
        number: info.number.clone().unwrap_or_default(),
        customer: info.customer.clone().unwrap_or_default(),
        designer: info.designer.clone().unwrap_or_default(),
        title: String::new(),
        content: String::new(),
        logo: None,
    }
}

fn job_sheet(model: &Model, project: &ProjectFile, base: &TitleInfo) -> Sheet {
    let opts = SceneOptions {
        walls: true,
        operations: false,
        ..SceneOptions::default()
    };
    let views = sheets::plan_job()
        .into_iter()
        .map(|plan| render_view(model, project, &opts, plan))
        .collect();
    let parts: usize = model.cabinets.iter().map(|c| c.parts.len()).sum();
    Sheet {
        title: TitleInfo {
            title: tr!("engine.drawings.job_title"),
            content: tr!(
                "engine.drawings.job_content",
                cabinets = model.cabinets.len(),
                parts = parts,
                material = &project.defaults.panel_material
            ),
            ..base.clone()
        },
        body: SheetBody::Views(views),
    }
}

/// Pohľad na stenu: stena, otvory, skrinky pri nej, kótovacie body a pôdorys.
fn wall_sheet(project: &ProjectFile, base: &TitleInfo, id: &str) -> Option<Sheet> {
    let room = Room::new(project);
    let index = room.wall_index(id)?;
    let wall = room.walls.get(index)?;
    let on_wall: Vec<&cc_model::CabinetSpec> = project
        .cabinets
        .iter()
        .filter(|c| c.wall.as_deref() == Some(id))
        .collect();
    let cabinets: Vec<WallCabinet> = on_wall
        .iter()
        .map(|c| WallCabinet {
            id: c.id.clone(),
            kind: c.cabinet_type,
            x: c.at,
            z: room.elevation(c),
            w: c.w,
            h: c.h,
            d: c.d,
            shelves: c.adjustable_shelves as usize + c.fixed_shelves.len(),
        })
        .collect();
    let openings: Vec<WallOpening> = project
        .openings
        .iter()
        .filter(|o| o.wall == id)
        .map(|o| WallOpening {
            kind: o.kind,
            x: o.at,
            z: o.elevation,
            w: o.width,
            h: o.height,
        })
        .collect();
    let mut heights = vec![0.0, wall.height];
    for c in &cabinets {
        heights.extend([c.z, c.z + c.h]);
    }
    for o in &openings {
        heights.extend([o.z, o.z + o.h]);
    }
    heights.retain(|z| z.is_finite());
    heights.sort_by(f64::total_cmp);
    heights.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    // šípka pohľadu: z miestnosti kolmo na stred steny
    let reach = room
        .bounds()
        .map_or(1000.0, |(lo, hi)| (hi - lo).min_element() * 0.3)
        .max(300.0);
    let mid = wall.length / 2.0;
    let arrow = [wall.point(mid, reach), wall.point(mid, reach * 0.25)].map(|p| [p.x, p.y]);
    let plan = PlanKey {
        walls: (0..room.walls.len())
            .map(|i| room.wall_outline(i).map(|p| [p.x, p.y]))
            .collect(),
        current: index,
        cabinets: project
            .cabinets
            .iter()
            .filter_map(|c| {
                let t = room.placement(c)?;
                Some(Room::footprint(&t, c).map(|p| [p.x, p.y]))
            })
            .collect(),
        arrow: Some(arrow),
    };
    let scale = sheets::wall_sheet_scale(wall.length, wall.height);
    Some(Sheet {
        title: TitleInfo {
            title: tr!("engine.drawings.wall_title", id = id),
            content: tr!(
                "engine.drawings.wall_content",
                length = fmt(wall.length),
                height = fmt(wall.height),
                cabinets = cabinets.len(),
                openings = openings.len(),
                scale = fmt(scale)
            ),
            ..base.clone()
        },
        body: SheetBody::Wall(Box::new(WallElevation {
            wall: id.to_owned(),
            length: wall.length,
            height: wall.height,
            soffit: room.spec.soffit,
            openings,
            cabinets,
            base_points: cc_room::dimension_points(project, id, Row::Base),
            upper_points: cc_room::dimension_points(project, id, Row::Upper),
            heights,
            plan,
        })),
    })
}

fn cabinet_sheet(
    model: &Model,
    project: &ProjectFile,
    base: &TitleInfo,
    id: &str,
) -> Option<Sheet> {
    let only = filtered(model, Some(id), None);
    let c = only.cabinets.first()?;
    let opts = SceneOptions {
        walls: false,
        operations: true,
        ..SceneOptions::default()
    };
    let views = sheets::plan_cabinet(c.w, c.h, c.d)
        .into_iter()
        .map(|plan| render_view(&only, project, &opts, plan))
        .collect();
    Some(Sheet {
        title: TitleInfo {
            title: tr!("engine.drawings.cabinet_title", id = id),
            content: tr!(
                "engine.drawings.cabinet_content",
                w = fmt(c.w),
                h = fmt(c.h),
                d = fmt(c.d),
                kind = sheets::cabinet_type_name(c.cabinet_type),
                parts = c.parts.len()
            ),
            ..base.clone()
        },
        body: SheetBody::Views(views),
    })
}

fn part_sheet(
    model: &Model,
    project: &ProjectFile,
    base: &TitleInfo,
    programs: &std::collections::BTreeMap<String, String>,
    id: &str,
) -> Option<Sheet> {
    let part = model.parts().find(|p| p.id == id)?.clone();
    let only = filtered(model, None, Some(id));
    let opts = SceneOptions {
        walls: false,
        ..SceneOptions::default()
    };
    let area = sheets::part_thumb_area();
    let (w, h) = area.pixels();
    let (img, _) = render(&only, project, &opts, &iso_camera(), w, h);
    let programs: Vec<String> = programs
        .get(id)
        .map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default();
    Some(Sheet {
        title: TitleInfo {
            title: tr!("engine.drawings.part_title", id = id),
            content: tr!(
                "engine.drawings.part_content",
                length = fmt(part.length),
                width = fmt(part.width),
                thickness = fmt(part.thickness),
                material = &part.material
            ),
            ..base.clone()
        },
        body: SheetBody::Part {
            part: Box::new(part),
            programs,
            thumb: Some(img),
        },
    })
}

fn iso_camera() -> Camera {
    Camera::default()
}

fn render_view(
    model: &Model,
    project: &ProjectFile,
    opts: &SceneOptions,
    plan: ViewPlan,
) -> RenderedView {
    let (yaw, pitch, perspective) = plan.kind.camera();
    let camera = Camera {
        yaw,
        pitch,
        zoom: plan.zoom,
        pan: [0.0, 0.0],
        perspective,
        fit: None,
    };
    let (w, h) = plan.area.pixels();
    let (image, labels) = render(model, project, opts, &camera, w, h);
    RenderedView {
        plan,
        image: Some(image),
        labels,
    }
}

/// Obraz scény a popisy skriniek (poloha 0…1 v obraze, zhora).
fn render(
    model: &Model,
    project: &ProjectFile,
    opts: &SceneOptions,
    camera: &Camera,
    width: usize,
    height: usize,
) -> (RgbImage, Vec<(f64, f64, String)>) {
    let scene = Scene::build(model, project, opts);
    let img = cc_scene::render(
        &scene,
        camera,
        width,
        height,
        2,
        1.0,
        Selection::default(),
        [255, 255, 255],
    );
    let labels = scene
        .labels
        .iter()
        .map(|(p, text)| {
            let s = img.projection.project(*p);
            (
                s.x / img.width.max(1) as f64,
                s.y / img.height.max(1) as f64,
                text.clone(),
            )
        })
        .collect();
    (
        RgbImage {
            width: img.width,
            height: img.height,
            rgb: img.rgb,
        },
        labels,
    )
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

fn fmt(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
