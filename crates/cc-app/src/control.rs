//! Riadiaci kanál v okne (`--control PORT`, `docs/control-protocol.md`).
//!
//! Metódy jadra vybaví `cc_control::engine::handle`, tu sú metódy okna:
//! stav okna, nastavenie záložky a pohľadu, snímka, vykreslenie 3D bez okna a
//! vloženie udalostí myši a klávesnice.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use cc_control::{engine, ControlError, ControlServer, Incoming, MethodInfo};
use eframe::egui;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app::{App, Tab};
use cc_scene::Camera;

pub(crate) const UI_METHODS: &[MethodInfo] = &[
    MethodInfo {
        method: "ui.inspect",
        params: "",
        description:
            "Stav okna: záložka, kamera, voľby 3D, výber, stavový riadok, rozmer okna, dialóg.",
    },
    MethodInfo {
        method: "ui.set",
        params: r#"{"tab"?: "plan"|"elevation"|"3d"|"reports"|"parts"|"part"|"issues"|"standard", "planMode"?: "home"|"walls"|"objects"|"floors"|"ceilings"|"tops"|"molding"|"autofill"|"section"|"cad", "cabinetView"?: "section"|"face"|"plan"|"end"|"3d"|"reports", "partView"?: "cam"|"front"|"top"|"end"|"3d"|"reports", "camera"?: {"yaw","pitch","zoom","pan":[x,y],"perspective"}, "resetCamera"?: true, "view"?: "front"|"top"|"iso"|"selection"|"default", "scene"?: {"backs","shelves","fronts","operations","walls","explode"}, "csv"?: {"merge","decimal_comma"}, "sheet"?: "part"|"job"|"cabinet:ID", "language"?: "sk"|"en"|kód súboru locales/*.toml}"#,
        description: "Nastaví záložku, pohľad (aj pomenovaný: spredu, zhora, izo, priblíženie na výber), výkres a voľby zobrazenia.",
    },
    MethodInfo {
        method: "ui.screenshot",
        params: r#"{"path": "C:/…/shot.png"}"#,
        description:
            "PNG celého okna (potrebuje vykreslenú snímku; okno nesmie byť minimalizované).",
    },
    MethodInfo {
        method: "ui.render",
        params: r#"{"path": "…png", "width"?: 1200, "height"?: 800, "window"?: false}"#,
        description: "3D náhľad do PNG softvérovým rastrom (aj pri zamknutej obrazovke), s aktuálnou kamerou, voľbami a výberom. `window: true` = rozmer a mierka 3D výrezu okna (na porovnanie so snímkou okna).",
    },
    MethodInfo {
        method: "ui.bench",
        params: r#"{"frames"?: 240, "yawStep"?: 0.01}"#,
        description: "Meranie plynulosti: prepne na 3D, otáča kamerou počas `frames` snímok a vráti snímky za sekundu, časy snímok a čas update na CPU; kamera sa potom vráti.",
    },
    MethodInfo {
        method: "ui.click",
        params: r#"{"x", "y", "button"?: "left"|"right"|"middle", "double"?: false}"#,
        description: "Klik myšou v bodoch okna (spracuje sa v nasledujúcich snímkach).",
    },
    MethodInfo {
        method: "ui.drag",
        params: r#"{"x", "y", "to": [x, y], "button"?: "left"}"#,
        description: "Stlačenie, posun a pustenie myši.",
    },
    MethodInfo {
        method: "ui.key",
        params: r#"{"key": "Enter"|"Escape"|"Z"|…, "ctrl"?: false, "shift"?: false, "alt"?: false}"#,
        description: "Stlačenie klávesu (mená ako egui::Key).",
    },
    MethodInfo {
        method: "ui.text",
        params: r#"{"text": "…"}"#,
        description: "Napíše text do poľa, ktoré má fokus.",
    },
    MethodInfo {
        method: "app.quit",
        params: r#"{"force"?: false}"#,
        description: "Zatvorí okno; pri neuložených zmenách iba s force: true.",
    },
];

/// Najväčší rozmer obrazu pre `ui.render`.
const MAX_RENDER: usize = 8000;

#[derive(Default)]
pub(crate) struct ControlState {
    server: Option<ControlServer>,
    /// `ui.screenshot`, ktoré čakajú na snímku okna.
    shots: Vec<(Incoming, PathBuf)>,
    /// prebiehajúce meranie `ui.bench`
    bench: Option<Bench>,
}

pub(crate) struct Bench {
    incoming: Incoming,
    frames: usize,
    yaw_step: f64,
    camera: Camera,
    /// začiatok každej snímky
    starts: Vec<Instant>,
    /// čas `update` na CPU (ms)
    cpu_ms: Vec<f64>,
}

/// Meranie pred priradením požiadavky.
pub(crate) struct BenchStart {
    frames: usize,
    yaw_step: f64,
    camera: Camera,
    starts: Vec<Instant>,
    cpu_ms: Vec<f64>,
}

/// Najviac snímok merania.
const MAX_BENCH_FRAMES: usize = 5000;

impl ControlState {
    pub(crate) fn port(&self) -> Option<u16> {
        self.server.as_ref().map(ControlServer::port)
    }

    pub(crate) fn waiting(&self) -> bool {
        !self.shots.is_empty() || self.bench.is_some()
    }

    /// Zapíše čakajúce snímky okna a odpovie na ne.
    pub(crate) fn screenshot_taken(&mut self, image: &egui::ColorImage) {
        let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
        for (incoming, path) in self.shots.drain(..) {
            let r = cc_scene::write_png(&path, image.size[0], image.size[1], &rgba, 4)
                .map(|()| json!({ "path": path, "width": image.size[0], "height": image.size[1] }))
                .map_err(|e| ControlError::new("failed", e));
            incoming.respond(r);
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderParams {
    path: PathBuf,
    width: Option<usize>,
    height: Option<usize>,
    #[serde(default)]
    window: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct BenchParams {
    frames: Option<usize>,
    yaw_step: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CameraParams {
    yaw: Option<f64>,
    pitch: Option<f64>,
    zoom: Option<f64>,
    pan: Option<[f64; 2]>,
    perspective: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneParams {
    backs: Option<bool>,
    shelves: Option<bool>,
    fronts: Option<bool>,
    operations: Option<bool>,
    walls: Option<bool>,
    explode: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CsvParams {
    merge: Option<bool>,
    decimal_comma: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SetParams {
    tab: Option<String>,
    camera: Option<CameraParams>,
    #[serde(default)]
    reset_camera: bool,
    view: Option<String>,
    scene: Option<SceneParams>,
    csv: Option<CsvParams>,
    /// Výkres: `part`, `job`, `cabinet:ID`.
    sheet: Option<String>,
    /// Režim ľavého panela pôdorysu: `home`, `walls`, `objects`, … (ako F2–F7).
    plan_mode: Option<String>,
    /// Záložka editora skrinky: `section`, `face`, `plan`, `end`, `3d`, `reports`.
    cabinet_view: Option<String>,
    /// Záložka otvorenej dosky: `cam`, `front`, `top`, `end`, `3d`, `reports`.
    part_view: Option<String>,
    /// Jazyk okna (`sk`, `en`, kód súboru v `locales`); zapamätá sa v nastaveniach.
    language: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClickParams {
    x: f32,
    y: f32,
    button: Option<String>,
    #[serde(default)]
    double: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DragParams {
    x: f32,
    y: f32,
    to: [f32; 2],
    button: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyParams {
    key: String,
    #[serde(default)]
    ctrl: bool,
    #[serde(default)]
    shift: bool,
    #[serde(default)]
    alt: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextParams {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuitParams {
    #[serde(default)]
    force: bool,
}

fn parse<T: serde::de::DeserializeOwned>(method: &str, params: Value) -> Result<T, ControlError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| ControlError::params(format!("{method}: {e}")))
}

fn finite(name: &str, v: f64) -> Result<f64, ControlError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(ControlError::params(format!(
            "`{name}` musí byť konečné číslo"
        )))
    }
}

fn button(name: Option<&str>) -> Result<egui::PointerButton, ControlError> {
    match name.unwrap_or("left") {
        "left" => Ok(egui::PointerButton::Primary),
        "right" => Ok(egui::PointerButton::Secondary),
        "middle" => Ok(egui::PointerButton::Middle),
        other => Err(ControlError::params(format!(
            "neznáme tlačidlo `{other}` (left, right, middle)"
        ))),
    }
}

impl App {
    pub(crate) fn start_control(&mut self, ctx: &egui::Context, port: u16) {
        let ctx = ctx.clone();
        match ControlServer::start(port, Arc::new(move || ctx.request_repaint())) {
            Ok(server) => {
                eprintln!("riadiaci kanál: 127.0.0.1:{}", server.port());
                self.control.server = Some(server);
            }
            Err(e) => self.set_status(
                cc_i18n::tr!(
                    "app.status.control_failed",
                    port = port,
                    error = e.to_string()
                ),
                true,
            ),
        }
    }

    /// Vybaví požiadavky, ktoré prišli od poslednej snímky.
    pub(crate) fn poll_control(&mut self, ctx: &egui::Context) {
        let mut batch = Vec::new();
        if let Some(server) = &self.control.server {
            while let Some(incoming) = server.try_next() {
                batch.push(incoming);
            }
        }
        for incoming in batch {
            self.handle_control(ctx, incoming);
        }
    }

    fn handle_control(&mut self, ctx: &egui::Context, incoming: Incoming) {
        let method = incoming.request.method.clone();
        let params = incoming.request.params.clone();
        let result = match method.as_str() {
            "control.methods" => Ok(json!(engine::METHODS
                .iter()
                .chain(UI_METHODS)
                .collect::<Vec<_>>())),
            "ui.inspect" => Ok(self.ui_inspect(ctx)),
            "ui.set" => self.ui_set(params),
            "ui.screenshot" => match parse::<PathParams>(&method, params) {
                Ok(p) => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    ctx.request_repaint();
                    self.control.shots.push((incoming, p.path));
                    return;
                }
                Err(e) => Err(e),
            },
            "ui.render" => self.ui_render(params),
            "ui.bench" => match self.start_bench(params) {
                Ok(b) => {
                    let bench = Bench {
                        incoming,
                        frames: b.frames,
                        yaw_step: b.yaw_step,
                        camera: b.camera,
                        starts: b.starts,
                        cpu_ms: b.cpu_ms,
                    };
                    if let Some(old) = self.control.bench.replace(bench) {
                        old.incoming.respond(Err(ControlError::new(
                            "failed",
                            "meranie prerušilo nové ui.bench",
                        )));
                    }
                    ctx.request_repaint();
                    return;
                }
                Err(e) => Err(e),
            },
            "ui.click" | "ui.drag" | "ui.key" | "ui.text" => {
                let r = self.ui_inject(&method, params);
                ctx.request_repaint();
                r
            }
            "app.quit" => match parse::<QuitParams>(&method, params) {
                Ok(p) if self.dirty() && !p.force => Err(ControlError::new(
                    "failed",
                    "projekt má neuložené zmeny: ulož ho (app.save) alebo pošli force: true",
                )),
                Ok(_) => {
                    self.confirm = None;
                    self.allow_close = true;
                    ctx.request_repaint();
                    Ok(Value::Null)
                }
                Err(e) => Err(e),
            },
            _ => {
                let command = params.get("command").and_then(Value::as_str);
                let new_document = method == "app.open"
                    || (matches!(method.as_str(), "engine.execute" | "command")
                        && matches!(
                            command,
                            Some("project.open" | "project.new" | "project.import_cvj")
                        ));
                let r = engine::handle(&mut self.session, &method, params)
                    .unwrap_or_else(|| Err(ControlError::unknown_method(&method)));
                if r.is_ok() && new_document {
                    self.after_document_change();
                }
                r
            }
        };
        incoming.respond(result);
    }

    fn ui_inspect(&self, ctx: &egui::Context) -> Value {
        let screen = ctx.screen_rect();
        json!({
            "tab": self.tab.key(),
            "language": cc_i18n::language(),
            "languages": cc_i18n::languages().iter().map(|l| json!({
                "code": l.code, "name": l.name, "builtin": l.builtin, "error": l.error,
            })).collect::<Vec<_>>(),
            "plan_mode": self.plan_ui.mode.id(),
            "elevation_mode": self.plan_ui.elev_mode.id(),
            "cabinet_view": self.cab_tab.key(),
            "part_view": self.part_tab.key(),
            "camera": {
                "yaw": self.camera.yaw,
                "pitch": self.camera.pitch,
                "zoom": self.camera.zoom,
                "pan": self.camera.pan,
                "perspective": self.camera.perspective,
                "fit": self.camera.fit.map(|b| b.map(|v| v.to_array())),
            },
            "scene": {
                "backs": self.scene_opts.backs,
                "shelves": self.scene_opts.shelves,
                "fronts": self.scene_opts.fronts,
                "operations": self.scene_opts.operations,
                "walls": self.scene_opts.walls,
                "explode": self.scene_opts.explode,
            },
            "csv": { "merge": self.csv.merge, "decimal_comma": self.csv.decimal_comma },
            "selection": self.session.selection(),
            "status": self.status.as_ref().map(|s| json!({ "text": s.text, "error": s.error })),
            "confirm": self.confirm.as_ref().map(|c| format!("{c:?}")),
            "title": self.session.doc().map(|d| d.title()),
            "window": { "width": screen.width(), "height": screen.height(), "pixelsPerPoint": ctx.pixels_per_point() },
            "keyboardFocus": ctx.wants_keyboard_input(),
            "frame": self.frame,
            "renderer": self.renderer_name(),
            // kto vyžiadal predchádzajúcu snímku (súbor:riadok); prázdne = vstup okna
            "repaintCauses": ctx.repaint_causes().iter().map(|c| c.to_string()).collect::<Vec<_>>(),
            "view3d":self.view.viewport.map(|v| json!({
                "x": v.min[0], "y": v.min[1], "width": v.size[0], "height": v.size[1],
            })),
            "version": self.session.version(),
        })
    }

    fn ui_set(&mut self, params: Value) -> Result<Value, ControlError> {
        let p: SetParams = parse("ui.set", params)?;
        if let Some(code) = &p.language {
            self.set_language(code).map_err(ControlError::params)?;
        }
        if let Some(tab) = &p.tab {
            self.tab = Tab::from_key(tab).ok_or_else(|| {
                ControlError::params(format!(
                    "neznáma záložka `{tab}` (plan, elevation, 3d, parts, part, issues, standard)"
                ))
            })?;
        }
        if let Some(v) = &p.part_view {
            self.part_tab = crate::part_editor::PartTab::from_key(v).ok_or_else(|| {
                ControlError::params(format!(
                    "neznáma záložka dosky `{v}` (cam, front, top, end, 3d, reports)"
                ))
            })?;
        }
        if let Some(v) = &p.cabinet_view {
            self.cab_tab = crate::cabinet_editor::CabTab::from_key(v).ok_or_else(|| {
                ControlError::params(format!(
                    "neznáma záložka editora skrinky `{v}` (section, face, plan, end, 3d, reports)"
                ))
            })?;
        }
        if let Some(mode) = &p.plan_mode {
            let m = crate::plan_modes::PlanMode::from_id(mode).ok_or_else(|| {
                ControlError::params(format!(
                    "neznámy režim pôdorysu `{mode}` (home, walls, objects, floors, ceilings, tops, molding, autofill, section, cad)"
                ))
            })?;
            self.set_plan_mode(m);
        }
        if let Some(sheet) = &p.sheet {
            self.drawing.sheet = crate::drawings::Sheet::parse(sheet).ok_or_else(|| {
                ControlError::params(format!("neznámy výkres `{sheet}` (part, job, cabinet:ID)"))
            })?;
        }
        if p.reset_camera {
            self.camera = Camera::default();
        }
        if let Some(view) = &p.view {
            self.set_view(view).map_err(ControlError::params)?;
        }
        if let Some(c) = p.camera {
            let mut cam = self.camera;
            if let Some(v) = c.yaw {
                cam.yaw = finite("yaw", v)?;
            }
            if let Some(v) = c.pitch {
                cam.pitch = finite("pitch", v)?.clamp(-Camera::PITCH_LIMIT, Camera::PITCH_LIMIT);
            }
            if let Some(v) = c.zoom {
                cam.zoom = finite("zoom", v)?.clamp(0.2, 20.0);
            }
            if let Some([x, y]) = c.pan {
                cam.pan = [finite("pan", x)?, finite("pan", y)?];
            }
            if let Some(v) = c.perspective {
                cam.perspective = v;
            }
            self.camera = cam;
        }
        if let Some(s) = p.scene {
            let mut o = self.scene_opts;
            if let Some(v) = s.backs {
                o.backs = v;
            }
            if let Some(v) = s.shelves {
                o.shelves = v;
            }
            if let Some(v) = s.fronts {
                o.fronts = v;
            }
            if let Some(v) = s.operations {
                o.operations = v;
            }
            if let Some(v) = s.walls {
                o.walls = v;
            }
            if let Some(v) = s.explode {
                o.explode = finite("explode", v)?.clamp(0.0, 1.0);
            }
            self.scene_opts = o;
        }
        if let Some(c) = p.csv {
            if let Some(v) = c.merge {
                self.csv.merge = v;
            }
            if let Some(v) = c.decimal_comma {
                self.csv.decimal_comma = v;
            }
        }
        Ok(json!({ "tab": self.tab.key(), "language": cc_i18n::language() }))
    }

    /// Parametre merania; `incoming` doplní volajúci.
    fn start_bench(&mut self, params: Value) -> Result<BenchStart, ControlError> {
        let p: BenchParams = parse("ui.bench", params)?;
        let frames = p.frames.unwrap_or(240);
        if !(2..=MAX_BENCH_FRAMES).contains(&frames) {
            return Err(ControlError::params(format!("frames 2…{MAX_BENCH_FRAMES}")));
        }
        let yaw_step = finite("yawStep", p.yaw_step.unwrap_or(0.01))?;
        self.tab = Tab::View3d;
        Ok(BenchStart {
            frames,
            yaw_step,
            camera: self.camera,
            starts: Vec::with_capacity(frames + 1),
            cpu_ms: Vec::with_capacity(frames + 1),
        })
    }

    /// Jedna snímka merania: otočí kamerou, po poslednej odpovie výsledkom.
    pub(crate) fn bench_frame(&mut self, ctx: &egui::Context, started: Instant) {
        let Some(bench) = &mut self.control.bench else {
            return;
        };
        bench.starts.push(started);
        bench.cpu_ms.push(started.elapsed().as_secs_f64() * 1000.0);
        if bench.starts.len() <= bench.frames {
            self.camera.yaw += bench.yaw_step;
            ctx.request_repaint();
            return;
        }
        let Some(bench) = self.control.bench.take() else {
            return;
        };
        self.camera = bench.camera;
        ctx.request_repaint();
        // prvá snímka ešte nesie zmenu záložky a nahratie scény
        let intervals: Vec<f64> = bench
            .starts
            .windows(2)
            .skip(1)
            .map(|w| (w[1] - w[0]).as_secs_f64() * 1000.0)
            .collect();
        let total: f64 = intervals.iter().sum();
        let mut sorted = intervals.clone();
        sorted.sort_by(f64::total_cmp);
        let pct = |q: f64| {
            sorted
                .get(((sorted.len() as f64 - 1.0) * q).round() as usize)
                .copied()
                .unwrap_or(0.0)
        };
        let cpu = &bench.cpu_ms[1..];
        let cpu_avg = cpu.iter().sum::<f64>() / cpu.len().max(1) as f64;
        let cpu_max = cpu.iter().copied().fold(0.0, f64::max);
        let result = json!({
            "frames": intervals.len(),
            "fps": if total > 0.0 { intervals.len() as f64 * 1000.0 / total } else { 0.0 },
            "frameMs": { "avg": total / intervals.len().max(1) as f64, "p50": pct(0.5), "p95": pct(0.95), "max": pct(1.0) },
            "updateCpuMs": { "avg": cpu_avg, "max": cpu_max },
            "renderer": self.renderer_name(),
            "parts": self.view.scene.parts.len(),
            "triangles": self.view.mesh.as_ref().map_or(self.view.scene.tris().len(), |m| m.triangles()),
            "viewport": self.view.viewport.map(|v| v.size),
        });
        bench.incoming.respond(Ok(result));
    }

    pub(crate) fn renderer_name(&self) -> String {
        match &self.gpu {
            Some(g) if !g.failed() => format!("wgpu: {}", g.adapter),
            Some(_) => "cpu (chyba GPU)".to_owned(),
            None => "cpu".to_owned(),
        }
    }

    fn ui_render(&mut self, params: Value) -> Result<Value, ControlError> {
        let p: RenderParams = parse("ui.render", params)?;
        let (width, height, pan_scale) = match (p.window, self.view.viewport) {
            (true, Some(v)) => (v.size[0], v.size[1], v.pixels_per_point),
            (true, None) => {
                return Err(ControlError::new(
                    "failed",
                    "3D výrez ešte nebol vykreslený (ui.set {\"tab\": \"3d\"})",
                ))
            }
            (false, _) => (p.width.unwrap_or(1200), p.height.unwrap_or(800), 1.0),
        };
        if width == 0 || height == 0 || width > MAX_RENDER || height > MAX_RENDER {
            return Err(ControlError::params(format!(
                "rozmer obrazu 1…{MAX_RENDER} px"
            )));
        }
        if self.model_built().is_none() {
            return Err(ControlError::new("failed", "model sa nedá zostaviť"));
        }
        self.update_scene();
        let img = cc_scene::render(
            &self.view.scene,
            &self.camera,
            width,
            height,
            2,
            pan_scale,
            self.scene_selection(),
            crate::view3d::background(false),
        );
        cc_scene::write_png(&p.path, img.width, img.height, &img.rgb, 3)
            .map_err(|e| ControlError::new("failed", e))?;
        Ok(json!({ "path": p.path, "width": img.width, "height": img.height }))
    }

    /// Vloží udalosti vstupu; každá dávka sa spracuje v samostatnej snímke.
    fn ui_inject(&mut self, method: &str, params: Value) -> Result<Value, ControlError> {
        let none = egui::Modifiers::NONE;
        let check = |x: f32, y: f32| {
            if x.is_finite() && y.is_finite() {
                Ok(egui::pos2(x, y))
            } else {
                Err(ControlError::params("súradnice musia byť konečné čísla"))
            }
        };
        let press = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: none,
        };
        match method {
            "ui.click" => {
                let p: ClickParams = parse(method, params)?;
                let pos = check(p.x, p.y)?;
                let b = button(p.button.as_deref())?;
                self.injected
                    .push_back(vec![egui::Event::PointerMoved(pos), press(pos, b, true)]);
                self.injected.push_back(vec![press(pos, b, false)]);
                if p.double {
                    self.injected.push_back(vec![press(pos, b, true)]);
                    self.injected.push_back(vec![press(pos, b, false)]);
                }
            }
            "ui.drag" => {
                let p: DragParams = parse(method, params)?;
                let from = check(p.x, p.y)?;
                let to = check(p.to[0], p.to[1])?;
                let b = button(p.button.as_deref())?;
                self.injected
                    .push_back(vec![egui::Event::PointerMoved(from), press(from, b, true)]);
                for k in 1..=8 {
                    let t = k as f32 / 8.0;
                    self.injected
                        .push_back(vec![egui::Event::PointerMoved(from + (to - from) * t)]);
                }
                self.injected.push_back(vec![press(to, b, false)]);
            }
            "ui.key" => {
                let p: KeyParams = parse(method, params)?;
                let key = egui::Key::from_name(&p.key)
                    .ok_or_else(|| ControlError::params(format!("neznámy kláves `{}`", p.key)))?;
                let modifiers = egui::Modifiers {
                    alt: p.alt,
                    ctrl: p.ctrl,
                    shift: p.shift,
                    mac_cmd: false,
                    command: p.ctrl,
                };
                let ev = |pressed| egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                };
                self.injected.push_back(vec![ev(true)]);
                self.injected.push_back(vec![ev(false)]);
            }
            _ => {
                let p: TextParams = parse(method, params)?;
                if p.text.len() > 10_000 {
                    return Err(ControlError::params("text najviac 10 000 znakov"));
                }
                self.injected.push_back(vec![egui::Event::Text(p.text)]);
            }
        }
        Ok(json!({ "queuedFrames": self.injected.len() }))
    }
}
