//! Hlavné okno: stav okna, súbory a rozloženie (pás nástrojov, lišta pohľadov,
//! ľavý panel, pracovná plocha, stavový riadok).
//!
//! Projekt drží [`cc_engine::Session`]; okno ho iba číta a mení príkazmi
//! ([`App::exec`]). Pás nástrojov je v `ribbon.rs`, panely a záložky v `side_panel.rs`,
//! `tabs.rs`, `reports.rs` a `standard_tab.rs`, riadiaci kanál v `control.rs`, vzhľad v `theme.rs`.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cc_engine::Session;
use cc_i18n::tr;
use cc_model::Severity;
use cc_pipeline::{Built, CsvOptions};
use eframe::egui::{self, Align2, RichText, Sense};
use serde_json::{json, Value};

use cc_scene::{Camera, SceneOptions};

use crate::theme;
use crate::view3d::View3d;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    View3d,
    Plan,
    Elevation,
    /// Zostavy: zoznam skriniek zákazky.
    Reports,
    /// Výroba: rozpis dielov s programami.
    Parts,
    /// Výkresy: výkres dielu s operáciami.
    Part,
    Issues,
    /// Štítky: tabule nestingu a tlač štítkov (ako +Label v CV).
    Labels,
    Standard,
    /// Editor skrinky: samotná skrinka v 3D, sekcie čiel a diely (dvojklik na skrinku).
    Cabinet,
    /// Jedna doska samostatne (dvojklik na dosku v editore skrinky).
    PartEdit,
}

impl Tab {
    /// Záložka a jej identifikátor pre riadiaci kanál a `CABINETCRAFT_TAB` (neprekladá sa).
    pub(crate) const ALL: [(Tab, &'static str); 11] = [
        (Tab::Plan, "plan"),
        (Tab::Elevation, "elevation"),
        (Tab::View3d, "3d"),
        (Tab::Reports, "reports"),
        (Tab::Part, "part"),
        (Tab::Parts, "parts"),
        (Tab::Issues, "issues"),
        (Tab::Labels, "labels"),
        (Tab::Cabinet, "cabinet"),
        (Tab::PartEdit, "part_edit"),
        (Tab::Standard, "standard"),
    ];

    /// Názov záložky v zvolenom jazyku.
    pub(crate) fn label(self) -> String {
        match self {
            Tab::Plan => tr!("tabs.plan"),
            Tab::Elevation => tr!("tabs.elevation"),
            Tab::View3d => tr!("tabs.view3d"),
            Tab::Reports => tr!("tabs.reports"),
            Tab::Part => tr!("tabs.drawings"),
            Tab::Parts => tr!("tabs.production"),
            Tab::Issues => tr!("tabs.issues"),
            Tab::Labels => tr!("tabs.labels"),
            Tab::Cabinet => tr!("tabs.cabinet"),
            Tab::PartEdit => tr!("tabs.part_edit"),
            Tab::Standard => tr!("tabs.standard"),
        }
    }

    /// Záložka v lište pohľadov, ktorá je pri tomto pohľade zvýraznená.
    pub(crate) fn strip(self) -> Tab {
        match self {
            Tab::Issues | Tab::Labels => Tab::Parts,
            Tab::Cabinet => Tab::View3d,
            Tab::PartEdit => Tab::Part,
            other => other,
        }
    }

    pub(crate) fn key(self) -> &'static str {
        Tab::ALL
            .iter()
            .find(|(t, _)| *t == self)
            .map_or("3d", |(_, k)| k)
    }

    pub(crate) fn from_key(key: &str) -> Option<Tab> {
        Tab::ALL.iter().find(|(_, k)| *k == key).map(|(t, _)| *t)
    }
}

/// Akcia, ktorá čaká na potvrdenie, lebo sú neuložené zmeny.
#[derive(Debug, Clone)]
pub(crate) enum Pending {
    Open(PathBuf),
    /// (súbor .cvj, cieľový project.toml)
    Import(PathBuf, PathBuf),
    New,
    Quit,
}

pub(crate) struct Status {
    pub text: String,
    pub error: bool,
    pub folder: Option<PathBuf>,
}

/// Automatická snímka okna (na kontrolu): `CABINETCRAFT_SCREENSHOT=cesta.png`.
pub(crate) struct Shot {
    pub path: PathBuf,
    pub requested: bool,
}

pub struct App {
    pub(crate) session: Session,
    pub(crate) open_error: Option<String>,
    /// Posledný úspešne postavený model (keď stavba zlyhá, náhľad ostane).
    pub(crate) last_good: Option<Arc<Built>>,
    /// Identita posledného videného modelu (`Arc::as_ptr`).
    pub(crate) model_key: usize,
    pub(crate) programs: BTreeMap<String, String>,
    pub(crate) tab: Tab,
    pub(crate) hovered_op: Option<usize>,
    pub(crate) camera: Camera,
    pub(crate) scene_opts: SceneOptions,
    pub(crate) view: View3d,
    /// 3D na GPU (`None`: okno na OpenGL alebo GPU nevie vytvoriť pipelines → CPU raster).
    pub(crate) gpu: Option<crate::gpu3d::Gpu>,
    pub(crate) plan: crate::plan_view::PlanState,
    /// Ľavý panel pôdorysu: režim (Steny, Objekty, …) ako v CV.
    pub(crate) plan_ui: crate::plan_modes::PlanUi,
    pub(crate) wall_view: crate::wall_view::WallViewState,
    pub(crate) catalog_ui: crate::catalog_panel::CatalogUi,
    /// Otvorené vlastnosti skrinky (ako Assembly Properties v CV).
    pub(crate) props: Option<crate::props_dialog::PropsDialog>,
    pub(crate) status: Option<Status>,
    pub(crate) last_export: Option<PathBuf>,
    pub(crate) csv: CsvOptions,
    /// Výroba pri exporte: `true` = nesting + CX100, `false` = všetko na CX100, `None` = podľa profilu.
    pub(crate) export_nesting: Option<bool>,
    pub(crate) labels: crate::labels_view::LabelsState,
    /// Editor skrinky: skrinka, jej 3D scéna a kamera, záložka na návrat.
    pub(crate) cab_focus: Option<String>,
    pub(crate) cab_view: View3d,
    pub(crate) cab_camera: Camera,
    pub(crate) cab_return: Tab,
    /// Záložka editora skrinky (Rez, Čelo, Pôdorys, Bok, 3D, Zostavy).
    pub(crate) cab_tab: crate::cabinet_editor::CabTab,
    /// Otvorená doska: záložka (CAM, Čelo, …), pracovná rovina, kamera 3D a záložka,
    /// do ktorej vráti Návrat.
    pub(crate) part_tab: crate::part_editor::PartTab,
    pub(crate) part_plane: cc_model::Face,
    pub(crate) part_camera: Camera,
    pub(crate) part_back: Option<Tab>,
    /// Je .cvj napojená na CabinetCRAFT v ponuke Windows? (`None` = zistiť znova)
    pub(crate) shell_registered: Option<bool>,
    pub(crate) confirm: Option<Pending>,
    pub(crate) allow_close: bool,
    pub(crate) shot: Option<Shot>,
    pub(crate) frame: u64,
    /// Titulok okna naposledy poslaný oknu (posiela sa iba pri zmene: každý
    /// `send_viewport_cmd` vyžiada ďalšiu snímku).
    pub(crate) title: String,
    /// Rozpísaný text (ID skrinky, názov projektu): `(kľúč, text)`; potvrdí sa
    /// pri opustení poľa.
    pub(crate) text_edit: Option<(String, String)>,
    pub(crate) last_path: Option<PathBuf>,
    pub(crate) control: crate::control::ControlState,
    /// Vložené udalosti vstupu z riadiaceho kanála, jedna dávka na snímku.
    pub(crate) injected: VecDeque<Vec<egui::Event>>,
    /// Pás nástrojov a rozbalené skupiny ľavého panela.
    pub(crate) layout: crate::ribbon::Layout,
    /// Výkresy: čo je na papieri a vykreslené obrazy.
    pub(crate) drawing: crate::drawings::DrawingState,
    /// Jazyky v prepínači (obnoví sa pri otvorení zoznamu).
    pub(crate) languages: Vec<cc_i18n::Language>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        start: Option<PathBuf>,
        control_port: Option<u16>,
    ) -> App {
        crate::setup_fonts(&cc.egui_ctx);
        crate::theme::apply(&cc.egui_ctx);
        let mut app = App {
            session: Session::new(),
            open_error: None,
            last_good: None,
            model_key: 0,
            programs: BTreeMap::new(),
            tab: Tab::View3d,
            hovered_op: None,
            camera: Camera::default(),
            plan: Default::default(),
            plan_ui: Default::default(),
            wall_view: Default::default(),
            catalog_ui: Default::default(),
            props: None,
            scene_opts: SceneOptions::default(),
            view: Default::default(),
            gpu: match std::env::var("CABINETCRAFT_RENDERER").as_deref() {
                Ok("cpu") => None,
                _ => cc
                    .wgpu_render_state
                    .as_ref()
                    .and_then(crate::gpu3d::Gpu::new),
            },
            status: None,
            last_export: None,
            csv: CsvOptions::default(),
            export_nesting: None,
            labels: Default::default(),
            cab_focus: None,
            // iné generácie ako hlavná scéna (kľúč bufferov na GPU)
            cab_view: View3d {
                generation: 1 << 40,
                ..View3d::default()
            },
            cab_camera: Camera::default(),
            cab_return: Tab::View3d,
            cab_tab: Default::default(),
            part_tab: Default::default(),
            part_plane: cc_model::Face::F,
            part_camera: Camera::default(),
            part_back: None,
            shell_registered: None,
            confirm: None,
            allow_close: false,
            shot: std::env::var_os("CABINETCRAFT_SCREENSHOT").map(|p| Shot {
                path: PathBuf::from(p),
                requested: false,
            }),
            frame: 0,
            title: String::new(),
            text_edit: None,
            last_path: None,
            control: Default::default(),
            injected: VecDeque::new(),
            layout: Default::default(),
            drawing: Default::default(),
            languages: cc_i18n::languages(),
        };
        if let Some(path) = start
            .or_else(crate::last_project)
            .or_else(crate::example_project)
        {
            app.open(&path);
        }
        if let Ok(tab) = std::env::var("CABINETCRAFT_TAB") {
            app.tab = Tab::from_key(&tab).unwrap_or(Tab::View3d);
        }
        if std::env::var("CABINETCRAFT_RIBBON").as_deref() == Ok("tools") {
            app.layout.ribbon = crate::ribbon::RibbonTab::Tools;
        }
        if let Ok(part) = std::env::var("CABINETCRAFT_PART") {
            app.select_part(Some(part));
        }
        if let Some(port) = control_port {
            app.start_control(&cc.egui_ctx, port);
        }
        app
    }

    // ----- príkazy -----

    /// Vykoná príkaz; chybu ukáže v stavovom riadku.
    pub(crate) fn exec(&mut self, id: &str, params: Value) -> Option<Value> {
        match self.session.run(id, params) {
            Ok(v) => Some(v),
            Err(e) => {
                self.set_status(e.to_string(), true);
                None
            }
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.session
            .doc()
            .is_some_and(|d| d.dirty || d.standard_dirty)
    }

    /// Index vybranej skrinky v zozname (bez výberu prvá).
    pub(crate) fn selected_index(&self) -> usize {
        let Some(doc) = self.session.doc() else {
            return 0;
        };
        self.session
            .selection()
            .cabinet
            .as_ref()
            .and_then(|id| doc.project.cabinets.iter().position(|c| &c.id == id))
            .unwrap_or(0)
    }

    pub(crate) fn select_index(&mut self, i: usize) {
        let id = self
            .session
            .doc()
            .and_then(|d| d.project.cabinets.get(i))
            .map(|c| c.id.clone());
        if let Some(id) = id {
            if self.session.selection().cabinet.as_ref() != Some(&id) {
                self.exec("selection.set", json!({ "cabinet": id }));
            }
        }
    }

    pub(crate) fn selected_part(&self) -> Option<String> {
        self.session.selection().part.clone()
    }

    pub(crate) fn select_part(&mut self, part: Option<String>) {
        // diel sa dá vybrať iba v aktuálnom modeli (nie v záložnom po chybe stavby)
        if part.is_some() && self.session.model().is_none() {
            return;
        }
        if self.session.selection().part != part {
            self.exec("selection.set", json!({ "part": part }));
        }
    }

    /// Po zmene modelu obnoví zoznam programov a záložný model.
    fn refresh_model(&mut self) {
        let Some(built) = self.session.model() else {
            return;
        };
        let key = Arc::as_ptr(&built) as usize;
        if key == self.model_key {
            return;
        }
        self.model_key = key;
        let machine = self.session.doc().and_then(|d| d.machine.as_ref());
        self.programs = cc_pipeline::program_names(&built.model, machine);
        self.last_good = Some(built);
    }

    pub(crate) fn model_built(&self) -> Option<Arc<Built>> {
        self.session.model().or_else(|| self.last_good.clone())
    }

    /// Chyba stavby modelu, ak je.
    pub(crate) fn build_error(&self) -> Option<String> {
        match self.session.built() {
            Some(Err(e)) => Some(e.clone()),
            _ => None,
        }
    }

    pub(crate) fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(Status {
            text: text.into(),
            error,
            folder: None,
        });
    }

    /// Prepne jazyk okna (bez reštartu) a zapamätá ho v nastaveniach používateľa.
    pub(crate) fn set_language(&mut self, code: &str) -> Result<(), String> {
        cc_i18n::set_language(code).map_err(|e| e.to_string())?;
        crate::remember_language(&cc_i18n::language());
        self.languages = cc_i18n::languages();
        let name = self
            .languages
            .iter()
            .find(|l| l.code == cc_i18n::language())
            .map_or_else(cc_i18n::language, |l| l.name.clone());
        self.set_status(tr!("status.language_set", name = name), false);
        Ok(())
    }

    // ----- súbory -----

    pub(crate) fn open(&mut self, path: &Path) {
        // zákazka CV: otvorí sa priamo (projekt CabinetCRAFT vedľa nej)
        if is_cvj(path) {
            self.open_cvj(path);
            return;
        }
        match self
            .session
            .run("project.open", json!({ "path": path.to_string_lossy() }))
        {
            Ok(_) => {
                self.open_error = None;
                self.after_document_change();
                self.set_status(
                    tr!("app.files.opened", path = path.display().to_string()),
                    false,
                );
            }
            Err(e) => {
                self.open_error = Some(e.to_string());
                self.set_status(tr!("app.files.open_failed", error = e.to_string()), true);
            }
        }
    }

    /// Po otvorení alebo novom projekte: pôvodný pohľad, žiadny starý model.
    pub(crate) fn after_document_change(&mut self) {
        self.last_good = None;
        self.model_key = 0;
        self.camera = Camera::default();
        self.text_edit = None;
    }

    pub(crate) fn request(&mut self, action: Pending) {
        if self.dirty() {
            self.confirm = Some(action);
        } else {
            self.run_pending(action);
        }
    }

    fn run_pending(&mut self, action: Pending) {
        match action {
            Pending::Open(path) => self.open(&path),
            Pending::Import(cvj, out) => self.import_cvj(&cvj, &out),
            Pending::New => {
                if self.exec("project.new", json!({})).is_some() {
                    self.after_document_change();
                    self.set_status(tr!("app.files.new_project"), false);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
            }
        }
    }

    /// Otvorí zákazku CV: ak je vedľa nej náš projekt novší než `.cvj`, otvorí ten (úpravy
    /// v CabinetCRAFT ostanú); inak `.cvj` importuje do `MENO.cabinetcraft.toml` (starší
    /// projekt sa odloží ako `.toml.bak`). Pôvodná `.cvj` sa nemení.
    pub(crate) fn open_cvj(&mut self, cvj: &Path) {
        let project = project_for_cvj(cvj);
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let ours_newer = matches!(
            (modified(&project), modified(cvj)),
            (Some(a), Some(b)) if a >= b
        );
        if project.is_file() && ours_newer {
            self.open(&project);
            self.set_status(
                tr!(
                    "app.files.opened_cvj_project",
                    path = project.display().to_string(),
                    cvj = cvj.display().to_string()
                ),
                false,
            );
            return;
        }
        if project.is_file() {
            let _ = std::fs::copy(&project, project.with_extension("toml.bak"));
        }
        self.import_cvj(cvj, &project);
    }

    /// Po uložení projektu zapíše zákazku pre CV ako `MENO_cabinetcraft.cvj` vedľa pôvodnej
    /// zákazky (šablóny); pôvodná `.cvj` ostane nedotknutá. Vráti text do stavového riadku.
    fn save_cvj_copy(&mut self) -> String {
        let Some(template) = self
            .session
            .doc()
            .and_then(|d| d.project.project.cv_template.clone())
        else {
            return tr!("app.files.cvj_not_from_cv");
        };
        let template = PathBuf::from(template);
        let out = cvj_copy_path(&template);
        // otvorená bola už naša kópia: zapísať cez dočasný súbor, šablóna sa číta celá
        let target = if out == template {
            out.with_file_name(format!(
                "{}.tmp.cvj",
                out.file_stem()
                    .map_or_else(String::new, |s| s.to_string_lossy().into_owned())
            ))
        } else {
            out.clone()
        };
        match self.session.run(
            "project.export_cvj",
            json!({ "out": target.to_string_lossy() }),
        ) {
            Ok(_) => {
                if target != out {
                    if let Err(e) = std::fs::rename(&target, &out) {
                        return tr!("app.files.cvj_not_written", error = e.to_string());
                    }
                }
                tr!("app.files.cvj_written", path = out.display().to_string())
            }
            Err(e) => tr!("app.files.cvj_not_written", error = e.to_string()),
        }
    }

    /// Výber úlohy `.cvj` a miesta pre nový projekt.
    pub(crate) fn pick_import(&mut self) {
        let mut dialog =
            rfd::FileDialog::new().add_filter(tr!("app.files.filter_cv_job"), &["cvj"]);
        if let Some(dir) = self.session.doc().map(|d| d.base.clone()) {
            dialog = dialog.set_directory(dir);
        }
        let Some(cvj) = dialog.pick_file() else {
            return;
        };
        let stem = cvj
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "import".to_owned());
        let mut save = rfd::FileDialog::new()
            .add_filter(tr!("app.files.filter_project"), &["toml"])
            .set_title(tr!("app.files.import_target_title"))
            .set_file_name(format!("{stem}.toml"));
        if let Some(dir) = cvj.parent() {
            save = save.set_directory(dir);
        }
        if let Some(out) = save.save_file() {
            self.request(Pending::Import(cvj, out));
        }
    }

    /// Uloženie do novej zákazky CV podľa šablóny (zákazka, z ktorej sa projekt importoval).
    pub(crate) fn pick_export_cvj(&mut self) {
        let Some(doc) = self.session.doc() else {
            return;
        };
        let stem = doc
            .project
            .project
            .cv_template
            .as_deref()
            .and_then(|t| {
                Path::new(t)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "zakazka".to_owned());
        let save = rfd::FileDialog::new()
            .add_filter(tr!("app.files.filter_cv_order"), &["cvj"])
            .set_title(tr!("app.files.export_cvj_title"))
            .set_directory(doc.base.clone())
            .set_file_name(format!("{stem}_cabinetcraft.cvj"));
        let Some(out) = save.save_file() else {
            return;
        };
        let params = json!({ "out": out.to_string_lossy() });
        match self.session.run("project.export_cvj", params) {
            Ok(r) => {
                let notes = r["notes"].as_array().map_or(0, Vec::len);
                self.set_status(
                    tr!(
                        "app.files.cvj_saved",
                        path = out.display().to_string(),
                        cabinets = r["cabinets"].to_string(),
                        notes = notes
                    ),
                    false,
                );
            }
            Err(e) => self.set_status(
                tr!("app.files.cvj_save_failed", error = e.to_string()),
                true,
            ),
        }
    }

    pub(crate) fn import_cvj(&mut self, cvj: &Path, out: &Path) {
        let params = json!({ "file": cvj.to_string_lossy(), "out": out.to_string_lossy() });
        match self.session.run("project.import_cvj", params) {
            Ok(r) => {
                self.open_error = None;
                self.after_document_change();
                let notes = r["notes"].as_array().map_or(0, Vec::len);
                self.set_status(
                    tr!(
                        "app.files.imported",
                        file = cvj.display().to_string(),
                        walls = r["walls"].to_string(),
                        cabinets = r["cabinets"].to_string(),
                        openings = r["openings"].to_string(),
                        notes = notes,
                        project = out.display().to_string()
                    ),
                    false,
                );
            }
            Err(e) => self.set_status(tr!("app.files.import_failed", error = e.to_string()), true),
        }
    }

    /// Otvoriť: predvolene zákazka CV (`.cvj`), aj projekt CabinetCRAFT (`.toml`).
    pub(crate) fn pick_open(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .add_filter(tr!("app.files.filter_cv_order"), &["cvj"])
            .add_filter(tr!("app.files.filter_project"), &["toml"])
            .add_filter(tr!("app.files.filter_job_or_project"), &["cvj", "toml"]);
        if let Some(dir) = self.session.doc().map(|d| d.base.clone()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.request(Pending::Open(path));
        }
    }

    pub(crate) fn save(&mut self) -> bool {
        let Some(doc) = self.session.doc() else {
            return false;
        };
        if doc.path.is_none() {
            return self.save_as();
        }
        match self.exec("project.save", json!({})) {
            Some(_) => {
                let cvj = self.save_cvj_copy();
                self.set_status(tr!("app.files.saved", cvj = cvj), false);
                true
            }
            None => false,
        }
    }

    pub(crate) fn save_as(&mut self) -> bool {
        let Some(doc) = self.session.doc() else {
            return false;
        };
        let dialog = rfd::FileDialog::new()
            .add_filter(tr!("app.files.filter_project"), &["toml"])
            .set_directory(&doc.base)
            .set_file_name("project.toml");
        let Some(path) = dialog.save_file() else {
            return false;
        };
        match self.exec("project.save_as", json!({ "path": path.to_string_lossy() })) {
            Some(r) => {
                let notes: Vec<String> = r["notes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|n| n.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                let path = path.display().to_string();
                let cvj = self.save_cvj_copy();
                let text = if notes.is_empty() {
                    tr!("app.files.saved_as", path = path, cvj = cvj)
                } else {
                    tr!(
                        "app.files.saved_as_notes",
                        path = path,
                        notes = notes.join("; "),
                        cvj = cvj
                    )
                };
                self.set_status(text, false);
                true
            }
            None => false,
        }
    }

    pub(crate) fn export(&mut self, dir: PathBuf) {
        let mut params = json!({
            "dir": dir.to_string_lossy(),
            "merge": self.csv.merge,
            "decimal_comma": self.csv.decimal_comma,
        });
        if let Some(nesting) = self.export_nesting {
            params["mode"] = json!(if nesting { "nesting" } else { "cx100" });
        }
        let Some(w) = self.exec("export.machine", params) else {
            return;
        };
        let mut text = tr!(
            "app.export.done",
            dir = dir.display().to_string(),
            parts = w["parts"].to_string(),
            operations = w["operations"].to_string(),
            xcs = w["xcs"].to_string(),
            mpr = w["mpr"].to_string()
        );
        if w["sheets"].as_u64().unwrap_or(0) > 0 {
            text.push_str(&tr!("app.export.sheets", sheets = w["sheets"].to_string()));
        }
        if w["labels"].as_u64().unwrap_or(0) > 0 {
            text.push_str(&tr!("app.export.labels", labels = w["labels"].to_string()));
        }
        for n in w["notices"].as_array().into_iter().flatten() {
            if let Some(n) = n.as_str() {
                text.push_str(&tr!("app.export.notice", notice = n));
            }
        }
        self.status = Some(Status {
            text,
            error: false,
            folder: Some(dir.clone()),
        });
        self.last_export = Some(dir);
    }

    /// Výkresy do PDF (`export.drawings`); `ask` = vybrať súbor v dialógu.
    /// Po uložení sa PDF otvorí v predvolenom prehliadači.
    pub(crate) fn print_drawings(&mut self, sheets: Option<Vec<String>>, ask: bool) {
        let default = self.default_export_dir().map(|d| d.join("vykresy.pdf"));
        let path = if ask {
            let mut dialog = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .set_file_name("vykresy.pdf");
            if let Some(dir) = default.as_ref().and_then(|p| p.parent()) {
                let _ = std::fs::create_dir_all(dir);
                dialog = dialog.set_directory(dir);
            }
            match dialog.save_file() {
                Some(p) => Some(p),
                None => return,
            }
        } else {
            default
        };
        let mut params = json!({});
        if let Some(p) = &path {
            params["path"] = json!(p.to_string_lossy());
        }
        if let Some(s) = sheets {
            params["sheets"] = json!(s);
        }
        let Some(r) = self.exec("export.drawings", params) else {
            return;
        };
        let file = r["path"].as_str().map(PathBuf::from);
        let warnings: Vec<&str> = r["warnings"]
            .as_array()
            .map(|w| w.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let mut text = tr!(
            "app.export.drawings_saved",
            path = file
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            count = r["pages"].as_u64().unwrap_or(0)
        );
        if !warnings.is_empty() {
            text.push_str(&tr!("app.export.no_logo", warnings = warnings.join("; ")));
        }
        self.status = Some(Status {
            text,
            error: !warnings.is_empty(),
            folder: file
                .as_ref()
                .and_then(|p| p.parent())
                .map(Path::to_path_buf),
        });
        if let Some(file) = file {
            let _ = std::process::Command::new("explorer").arg(file).spawn();
        }
    }

    pub(crate) fn default_export_dir(&self) -> Option<PathBuf> {
        self.last_export
            .clone()
            .or_else(|| self.session.doc().map(|d| d.base.join("out")))
    }

    pub(crate) fn undo(&mut self, redo: bool) {
        let id = if redo { "edit.redo" } else { "edit.undo" };
        if let Some(r) = self.exec(id, json!({})) {
            let label = r["label"].as_str().unwrap_or("");
            let text = if redo {
                tr!("app.status.redone", label = label)
            } else {
                tr!("app.status.undone", label = label)
            };
            self.set_status(text, false);
            self.text_edit = None;
        }
    }

    // ----- stavový riadok -----

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            let mut open_folder = None;
            match &self.status {
                Some(status) => {
                    let color = if status.error {
                        theme::ERROR
                    } else {
                        theme::TEXT
                    };
                    ui.label(
                        RichText::new(&status.text)
                            .size(theme::SMALL + 0.5)
                            .color(color),
                    );
                    if let Some(folder) = &status.folder {
                        if ui.small_button(tr!("app.status.open_folder")).clicked() {
                            open_folder = Some(folder.clone());
                        }
                    }
                }
                None => {
                    ui.label(RichText::new(tr!("app.status.ready")).size(theme::SMALL + 0.5));
                }
            }
            if let Some(folder) = open_folder {
                let _ = std::process::Command::new("explorer").arg(folder).spawn();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(tr!("app.status.unit")).size(theme::SMALL + 0.5));
                ui.separator();
                if let Some(port) = self.control.port() {
                    ui.label(
                        RichText::new(tr!("app.status.control", port = port))
                            .size(theme::SMALL)
                            .color(theme::TEXT_WEAK),
                    );
                    ui.separator();
                }
                let Some(built) = self.model_built() else {
                    return;
                };
                let errors = built.report.errors().count();
                let warnings = built
                    .issues()
                    .filter(|i| i.severity == Severity::Warning)
                    .count();
                if warnings > 0 {
                    ui.label(
                        RichText::new(tr!("app.status.warnings", count = warnings))
                            .size(theme::SMALL + 0.5)
                            .color(theme::WARNING),
                    );
                }
                let (text, color) = if errors > 0 {
                    (tr!("app.status.errors", count = errors), theme::ERROR)
                } else {
                    (tr!("app.status.no_errors"), theme::OK)
                };
                if ui
                    .add(
                        egui::Label::new(RichText::new(text).size(theme::SMALL + 0.5).color(color))
                            .sense(Sense::click()),
                    )
                    .on_hover_text(tr!("app.status.open_issues"))
                    .clicked()
                {
                    self.tab = Tab::Issues;
                }
                ui.separator();
                let ops: usize = built.model.parts().map(|p| p.operations.len()).sum();
                ui.label(
                    RichText::new(tr!(
                        "app.status.summary",
                        cabinets = built.model.cabinets.len(),
                        parts = built.model.parts().count(),
                        operations = ops
                    ))
                    .size(theme::SMALL + 0.5),
                );
            });
        });
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(action) = self.confirm.clone() else {
            return;
        };
        let mut decision: Option<Option<bool>> = None; // Some(Some(true)) uložiť, Some(Some(false)) zahodiť, Some(None) zrušiť
        egui::Window::new(tr!("app.confirm.title"))
            .id(egui::Id::new("confirm_unsaved"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(tr!("app.confirm.text"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr!("app.confirm.save")).clicked() {
                        decision = Some(Some(true));
                    }
                    if ui.button(tr!("app.confirm.discard")).clicked() {
                        decision = Some(Some(false));
                    }
                    if ui.button(tr!("app.confirm.cancel")).clicked() {
                        decision = Some(None);
                    }
                });
            });
        match decision {
            Some(Some(save)) => {
                self.confirm = None;
                if !save || self.save() {
                    self.run_pending(action);
                }
            }
            Some(None) => self.confirm = None,
            None => {}
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = &image {
            self.control.screenshot_taken(image);
        }
        let Some(shot) = &mut self.shot else {
            return;
        };
        if !shot.requested && self.frame >= 12 {
            shot.requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        if let Some(image) = image {
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = cc_scene::write_png(&shot.path, image.size[0], image.size[1], &rgba, 4)
            {
                eprintln!("snímka: {e}");
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.pick_open();
        }
        if self.session.model().is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P))
        {
            self.print_drawings(None, true);
        }
        // pôdorys a pohľad: F2–F7 prepínajú režim ľavého panela (ako v CV)
        if matches!(self.tab, Tab::Plan | Tab::Elevation) && !ctx.wants_keyboard_input() {
            for &mode in self.mode_list() {
                if let Some(key) = mode.key() {
                    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                        self.set_plan_mode(mode);
                    }
                }
            }
        }
        // v textovom poli robí Ctrl+Z späť samotné pole
        if !ctx.wants_keyboard_input() {
            let redo_shift = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
                    || i.consume_key(redo_shift, egui::Key::Z)
            }) {
                self.undo(true);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.undo(false);
            }
        }
    }
}

impl eframe::App for App {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(batch) = self.injected.pop_front() {
            raw_input.events.extend(batch);
        }
        if !self.injected.is_empty() {
            ctx.request_repaint();
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        self.frame += 1;
        self.poll_control(ctx);
        self.refresh_model();

        let path = self.session.doc().and_then(|d| d.path.clone());
        if path != self.last_path {
            if let Some(p) = &path {
                crate::remember_project(p);
            }
            self.last_path = path;
        }

        self.shortcuts(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        let title = match self.session.doc() {
            Some(doc) => format!("CabinetCRAFT – {}", doc.title()),
            None => "CabinetCRAFT".to_owned(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::TopBottomPanel::top("ribbon")
            .frame(
                egui::Frame::new()
                    .fill(theme::RIBBON)
                    .inner_margin(egui::Margin::symmetric(0, 2)),
            )
            .show_separator_line(true)
            .show(ctx, |ui| self.ribbon(ui));
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(0, 3)),
            )
            .show(ctx, |ui| self.status_bar(ui));
        egui::SidePanel::left("side")
            .resizable(true)
            .default_width(360.0)
            .min_width(290.0)
            .max_width(560.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.side_panel(ui));
            });
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin {
                        left: 2,
                        right: 4,
                        top: 4,
                        bottom: 2,
                    }),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.view_tabs(ui);
                egui::Frame::new()
                    .fill(theme::SHEET)
                    .stroke(egui::Stroke::new(1.0_f32, theme::TEXT_WEAK))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
                        ui.set_min_size(ui.available_size());
                        match self.tab {
                            Tab::View3d => self.tab_3d(ui),
                            Tab::Plan => self.tab_plan(ui),
                            Tab::Elevation => self.tab_elevation(ui),
                            Tab::Reports => self.tab_reports(ui),
                            Tab::Parts => self.tab_parts(ui),
                            Tab::Part => self.tab_part(ui),
                            Tab::Issues => self.tab_issues(ui),
                            Tab::Labels => self.tab_labels(ui),
                            Tab::Cabinet => self.tab_cabinet(ui),
                            Tab::PartEdit => self.tab_part_edit(ui),
                            Tab::Standard => self.tab_standard(ui),
                        }
                    });
            });
        self.confirm_window(ctx);
        self.props_window(ctx);

        // Ťahanie a písanie sa zlučuje do jedného kroku späť; po pustení myši
        // a opustení textového poľa sa krok uzavrie.
        if !ctx.input(|i| i.pointer.any_down()) && !ctx.wants_keyboard_input() {
            self.session.seal_history();
        }
        if self.allow_close && self.confirm.is_none() && self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.handle_screenshot(ctx);
        self.bench_frame(ctx, started);
        if self.shot.is_some() || self.control.waiting() {
            ctx.request_repaint();
        }
    }
}

/// Je to zákazka CABINET VISION (`.cvj`)?
pub(crate) fn is_cvj(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cvj"))
}

/// Projekt CabinetCRAFT k zákazke CV: `MENO.cabinetcraft.toml` vedľa `MENO.cvj`.
pub(crate) fn project_for_cvj(cvj: &Path) -> PathBuf {
    let stem = cvj.file_stem().map_or_else(
        || "zakazka".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    cvj.with_file_name(format!("{stem}.cabinetcraft.toml"))
}

/// Zákazka pre CV po uložení: `MENO_cabinetcraft.cvj` vedľa pôvodnej (prípona sa nezdvojí).
pub(crate) fn cvj_copy_path(template: &Path) -> PathBuf {
    let stem = template.file_stem().map_or_else(
        || "zakazka".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let stem = stem
        .strip_suffix("_cabinetcraft")
        .unwrap_or(&stem)
        .to_owned();
    template.with_file_name(format!("{stem}_cabinetcraft.cvj"))
}

#[cfg(test)]
mod cvj_paths {
    use super::*;

    #[test]
    fn project_and_copy_names() {
        let cvj = Path::new("C:/zakazky/REF KUCHYNA.cvj");
        assert!(is_cvj(cvj));
        assert!(is_cvj(Path::new("a.CVJ")));
        assert!(!is_cvj(Path::new("a.toml")));
        assert_eq!(
            project_for_cvj(cvj),
            Path::new("C:/zakazky/REF KUCHYNA.cabinetcraft.toml")
        );
        assert_eq!(
            cvj_copy_path(cvj),
            Path::new("C:/zakazky/REF KUCHYNA_cabinetcraft.cvj")
        );
        // otvorená naša kópia: názov sa nezdvojí
        let copy = Path::new("C:/zakazky/REF KUCHYNA_cabinetcraft.cvj");
        assert_eq!(cvj_copy_path(copy), copy);
    }
}
