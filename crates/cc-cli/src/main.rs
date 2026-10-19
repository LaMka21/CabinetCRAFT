//! `cabinetcraft build project.toml --out out/`
//!
//! `cabinetcraft run project.toml --script prikazy.jsonl [--save]` vykoná príkazy
//! z `cc-engine` bez okna; `cabinetcraft commands` vypíše ich zoznam.
//!
//! `cabinetcraft import-cvj úloha.cvj --out project.toml [--template priečinok]`
//!
//! `cabinetcraft render --view 3d --out nahlad.png project.toml` vykreslí 3D náhľad
//! softvérovým rastrom (`cc-scene`).
//!
//! `cabinetcraft parity úloha.cvj [--dataset tests/fixtures/cv-ref] [--out sprava.md]`
//! porovná import `.cvj` diel po diele s datasetom z CV (W6).

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

mod parity;

use cc_engine::import_cvj;
use cc_i18n::tr;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "cabinetcraft",
    version,
    about = "CabinetCRAFT – skrinky → diely → CNC"
)]
struct Cli {
    /// Jazyk správ a výkresov PDF (`sk`, `en` alebo kód súboru v `locales`); predvolene `sk`.
    /// Programy pre stroj, CSV a `model.json` sa jazykom nemenia.
    #[arg(long, global = true)]
    lang: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Vygeneruje diely, spoje a operácie, skontroluje ich a zapíše `cutlist.csv`, `model.json`, `xcs/*.xcs` + `RUN.bat` (a `mpr/*.mpr`; pri `production.mode = "nesting"` aj `nesting/*.xcs` + `CncRun.txt`).
    Build {
        /// Cesta k `project.toml`.
        project: PathBuf,
        /// Výstupný priečinok (vytvorí sa, ak neexistuje).
        #[arg(long, default_value = "out")]
        out: PathBuf,
        /// Profil stroja; predvolene `machine.toml` vedľa projektu, ak existuje.
        #[arg(long)]
        machine: Option<PathBuf>,
        /// Zlúčiť rovnaké diely do jedného riadku s `qty > 1`.
        #[arg(long)]
        merge: bool,
        /// Desatinná čiarka v CSV namiesto bodky.
        #[arg(long)]
        decimal_comma: bool,
    },
    /// Vykoná príkazy zo skriptu (JSON riadky `{"command": "cabinet.add", "params": {...}}`).
    ///
    /// Každý výsledok vypíše ako jeden riadok JSON. Prázdne riadky a riadky
    /// začínajúce `#` sa preskočia. Pri prvej chybe skončí s kódom 1.
    Run {
        /// Cesta k `project.toml`.
        project: PathBuf,
        /// Súbor so skriptom; `-` = štandardný vstup.
        #[arg(long)]
        script: PathBuf,
        /// Na konci projekt uloží (`project.save`).
        #[arg(long)]
        save: bool,
        /// Pokračovať aj po chybe príkazu.
        #[arg(long)]
        keep_going: bool,
    },
    /// Vypíše zoznam príkazov (JSON).
    Commands,
    /// Vykreslí náhľad projektu do PNG softvérovým rastrom (bez okna a GPU).
    Render {
        /// Cesta k `project.toml`.
        project: PathBuf,
        /// Pohľad; zatiaľ iba `3d`.
        #[arg(long, default_value = "3d")]
        view: String,
        /// Výstupný súbor PNG.
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 1200)]
        width: usize,
        #[arg(long, default_value_t = 800)]
        height: usize,
        /// Kamera: `default`, `front` (spredu), `top` (zhora), `iso`.
        #[arg(long, default_value = "default")]
        camera: String,
        /// Ortogonálny pohľad namiesto perspektívy.
        #[arg(long)]
        ortho: bool,
        /// Zvýrazní diel (ID dielu) alebo skrinku (ID skrinky).
        #[arg(long)]
        select: Option<String>,
    },
    /// Riadiaci kanál bez okna: JSON riadky na 127.0.0.1:PORT (docs/control-protocol.md).
    ///
    /// Na začiatku vypíše `{"port": N}`. Skončí metódou `app.quit`.
    Serve {
        /// Cesta k `project.toml`.
        project: PathBuf,
        /// Port; 0 = ľubovoľný voľný.
        #[arg(long, default_value_t = 0)]
        control: u16,
    },
    /// Importuje úlohu CABINET VISION (`.cvj`, iba čítanie) do `project.toml`: steny, skrinky pri stenách, okná/dvere.
    ImportCvj {
        /// Cesta k súboru `.cvj`.
        file: PathBuf,
        /// Výstupný `project.toml`.
        #[arg(long)]
        out: PathBuf,
        /// Vzorový projekt (priečinok s `project.toml` alebo súbor): predvolené materiály a konštrukcia.
        /// Predvolene `examples/dielna`.
        #[arg(long)]
        template: Option<PathBuf>,
    },
    /// Zapíše projekt do novej zákazky CABINET VISION (`.cvj`) podľa šablóny (pôvodná zákazka CV).
    ExportCvj {
        /// `project.toml`.
        project: PathBuf,
        /// Výstupný súbor `.cvj` (nesmie byť šablóna).
        #[arg(long)]
        out: PathBuf,
        /// Zákazka CV ako šablóna; predvolene tá, z ktorej sa projekt importoval.
        #[arg(long)]
        template: Option<PathBuf>,
    },
    /// Skontroluje súbor prekladu (`locales/<kód>.toml`) voči vstavanej angličtine:
    /// chybu v súbore, chýbajúce kľúče, kľúče navyše a iné parametre `{…}`.
    CheckLocale {
        /// Súbor prekladu, napr. `de.toml`.
        file: PathBuf,
    },
    /// Porovná import `.cvj` s datasetom referenčnej zákazky CV diel po diele (rozmery, diery).
    Parity {
        /// Cesta k súboru `.cvj` (iba čítanie).
        file: PathBuf,
        /// Priečinok datasetu (`parts.csv`, `ops.csv`).
        #[arg(long, default_value = "tests/fixtures/cv-ref")]
        dataset: PathBuf,
        /// Vzorový projekt pre import; predvolene `examples/dielna`.
        #[arg(long)]
        template: Option<PathBuf>,
        /// Správa v Markdowne; bez voľby na štandardný výstup.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("{}", tr!("cli.error", error = e.to_string()));
            ExitCode::FAILURE
        }
    }
}

/// `Ok(false)`: validácia našla chyby a výstupy sa nezapísali.
fn run(cli: Cli) -> Result<bool, Box<dyn std::error::Error>> {
    if let Some(lang) = &cli.lang {
        cc_i18n::set_language(lang)?;
    }
    match cli.command {
        Command::CheckLocale { file } => check_locale(&file),
        Command::Run {
            project,
            script,
            save,
            keep_going,
        } => run_script(&project, &script, save, keep_going),
        Command::Serve { project, control } => serve(&project, control),
        Command::Render {
            project,
            view,
            out,
            width,
            height,
            camera,
            ortho,
            select,
        } => render(
            &project,
            &view,
            &out,
            [width, height],
            &camera,
            ortho,
            select.as_deref(),
        ),
        Command::Commands => {
            let session = cc_engine::Session::new();
            let list: Vec<serde_json::Value> = cc_engine::commands()
                .map(|c| c.describe(&session))
                .collect();
            println!("{}", serde_json::to_string_pretty(&list)?);
            Ok(true)
        }
        Command::Parity {
            file,
            dataset,
            template,
            out,
        } => run_parity(&file, &dataset, template.as_deref(), out.as_deref()),
        Command::ExportCvj {
            project,
            out,
            template,
        } => {
            let loaded = cc_config::load(&project)?;
            let outcome = cc_engine::export_cvj::run(
                &loaded.project,
                &loaded.standard,
                &out,
                template.as_deref(),
            )?;
            for n in &outcome.notes {
                eprintln!("{}", tr!("cli.note", note = n));
            }
            println!(
                "{}",
                tr!(
                    "cli.export_cvj_done",
                    project = project.display().to_string(),
                    out = out.display().to_string(),
                    template = outcome.template.display().to_string(),
                    cabinets = outcome.cabinets.len(),
                    notes = outcome.notes.len()
                )
            );
            Ok(true)
        }
        Command::ImportCvj {
            file,
            out,
            template,
        } => {
            let outcome = import_cvj::run(&file, &out, template.as_deref())?;
            for n in &outcome.notes {
                eprintln!("{}", tr!("cli.note", note = n));
            }
            for c in &outcome.copied {
                eprintln!("{}", tr!("cli.copied", file = c.display().to_string()));
            }
            println!(
                "{}",
                tr!(
                    "cli.import_cvj_done",
                    file = file.display().to_string(),
                    out = out.display().to_string(),
                    walls = outcome.walls,
                    cabinets = outcome.cabinets,
                    openings = outcome.openings,
                    notes = outcome.notes.len()
                )
            );
            println!(
                "{}",
                tr!("cli.import_cvj_fronts", fronts = outcome.fronts.to_string())
            );
            Ok(true)
        }
        Command::Build {
            project,
            out,
            machine,
            merge,
            decimal_comma,
        } => {
            let loaded = cc_config::load(&project)?;
            let machine_path = machine.or_else(|| cc_pipeline::default_machine(&project));
            let machine = match &machine_path {
                Some(path) => Some(cc_config::load_machine(path)?),
                None => None,
            };

            let built = cc_pipeline::build(
                &loaded.project,
                &loaded.standard,
                &loaded.materials,
                machine.as_ref(),
            )?;

            // Validácia (kap. 8): chyby zastavia výstup, varovania sa iba vypíšu.
            for issue in built.issues() {
                eprintln!("{issue}");
            }
            match &machine_path {
                Some(p) => eprintln!(
                    "{}",
                    tr!("cli.machine_profile", file = p.display().to_string())
                ),
                None => eprintln!("{}", tr!("cli.no_machine_profile")),
            }
            if built.has_errors() {
                eprintln!(
                    "{}",
                    tr!(
                        "cli.validation_failed",
                        count = built.report.errors().count()
                    )
                );
                return Ok(false);
            }

            let written = cc_pipeline::write_outputs(
                &built.model,
                machine.as_ref(),
                &out,
                cc_pipeline::CsvOptions {
                    merge,
                    decimal_comma,
                },
            )?;
            for notice in &written.notices {
                eprintln!("{}", tr!("cli.notice", notice = notice));
            }
            if written.sheets > 0 {
                println!(
                    "{}",
                    tr!(
                        "cli.nesting_done",
                        count = written.sheets,
                        dir = out.join("nesting").display().to_string()
                    )
                );
                for line in &written.nesting {
                    println!("  {line}");
                }
            }
            if written.labels > 0 {
                println!(
                    "{}",
                    tr!(
                        "cli.labels_done",
                        count = written.labels,
                        dir = out.join("labels").display().to_string()
                    )
                );
            }
            println!(
                "{}",
                tr!(
                    "cli.build_done",
                    project = &built.model.project,
                    cabinets = built.model.cabinets.len(),
                    parts = written.parts,
                    operations = written.operations,
                    xcs = written.xcs,
                    mpr = written.mpr,
                    dir = out.display().to_string()
                )
            );
            Ok(true)
        }
    }
}

/// Najväčší rozmer obrazu pre `render`.
const MAX_RENDER: usize = 8000;

fn render(
    project: &std::path::Path,
    view: &str,
    out: &std::path::Path,
    [width, height]: [usize; 2],
    camera: &str,
    ortho: bool,
    select: Option<&str>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if view != "3d" {
        return Err(tr!("cli.render.view", view = view).into());
    }
    if width == 0 || height == 0 || width > MAX_RENDER || height > MAX_RENDER {
        return Err(tr!("cli.render.size", max = MAX_RENDER).into());
    }
    let base = cc_scene::Camera::default();
    let mut cam = match camera {
        "default" => base,
        "front" => base.front(),
        "top" => base.top(),
        "iso" => base.iso(),
        other => return Err(tr!("cli.render.camera", camera = other).into()),
    };
    cam.perspective = !ortho;
    let loaded = cc_config::load(project)?;
    let built = cc_pipeline::build(&loaded.project, &loaded.standard, &loaded.materials, None)?;
    let scene = cc_scene::Scene::build(
        &built.model,
        &loaded.project,
        &cc_scene::SceneOptions::default(),
    );
    // --select: ID dielu (zvýrazní diel aj jeho skrinku) alebo ID skrinky (obrys)
    let selection = match select {
        Some(id) => match scene.part_index(id) {
            Some(part) => scene.selection(Some(id), scene.cabinet_of(part)),
            None => {
                let s = scene.selection(None, Some(id));
                if s.cabinet.is_none() {
                    return Err(tr!("cli.render.no_selection", id = id).into());
                }
                s
            }
        },
        None => cc_scene::Selection::default(),
    };
    let img = cc_scene::render(
        &scene,
        &cam,
        width,
        height,
        2,
        1.0,
        selection,
        cc_scene::style::BACKGROUND_LIGHT,
    );
    cc_scene::write_png(out, img.width, img.height, &img.rgb, 3)?;
    println!(
        "{}",
        tr!(
            "cli.render.done",
            project = &built.model.project,
            parts = scene.parts.len(),
            width = width,
            height = height,
            file = out.display().to_string()
        )
    );
    Ok(true)
}

/// Najdlhší riadok skriptu (ochrana pred nezmyselným vstupom).
const MAX_LINE: usize = 1 << 20;

/// `Ok(false)`: niektorý príkaz zlyhal.
fn run_script(
    project: &std::path::Path,
    script: &std::path::Path,
    save: bool,
    keep_going: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    use serde_json::{json, Value};
    use std::io::Read;

    let mut session = cc_engine::Session::open(project)?;
    let mut text = String::new();
    if script.as_os_str() == "-" {
        std::io::stdin().read_to_string(&mut text)?;
    } else {
        text = std::fs::read_to_string(script).map_err(|e| format!("{}: {e}", script.display()))?;
    }
    let mut ok = true;
    let mut steps: Vec<(usize, String, Value)> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.len() > MAX_LINE {
            return Err(tr!("cli.script.too_long", line = n).into());
        }
        let v: Value = serde_json::from_str(line)
            .map_err(|e| tr!("cli.script.bad_json", line = n, error = e.to_string()))?;
        let Some(command) = v.get("command").and_then(Value::as_str) else {
            return Err(tr!("cli.script.no_command", line = n).into());
        };
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        steps.push((n, command.to_owned(), params));
    }
    if save {
        steps.push((0, "project.save".to_owned(), Value::Null));
    }
    for (n, command, params) in steps {
        let line = match session.run(&command, params) {
            Ok(result) => json!({"line": n, "command": command, "ok": true, "result": result}),
            Err(e) => {
                ok = false;
                json!({"line": n, "command": command, "ok": false, "kind": e.kind(), "error": e.to_string()})
            }
        };
        println!("{line}");
        if !ok && !keep_going {
            break;
        }
    }
    Ok(ok)
}

/// Riadiaci kanál bez okna nad `cc-engine`. Metódy `ui.*` nie sú k dispozícii.
fn serve(project: &std::path::Path, port: u16) -> Result<bool, Box<dyn std::error::Error>> {
    use cc_control::{engine, ControlError, ControlServer, MethodInfo};
    use serde_json::json;
    use std::io::Write;

    const QUIT: MethodInfo = MethodInfo {
        method: "app.quit",
        params: "",
        description: "Ukončí server.",
    };
    let mut session = cc_engine::Session::open(project)?;
    let server = ControlServer::start(port, std::sync::Arc::new(|| {}))?;
    println!("{}", json!({ "port": server.port() }));
    std::io::stdout().flush()?;
    loop {
        let Some(incoming) = server.next_timeout(std::time::Duration::from_secs(1)) else {
            continue;
        };
        let method = incoming.request.method.clone();
        let params = incoming.request.params.clone();
        if method == "app.quit" {
            incoming.respond(Ok(json!(null)));
            return Ok(true);
        }
        let result = match engine::handle(&mut session, &method, params) {
            Some(r) => r.map(|v| {
                if method == "control.methods" {
                    let mut all: Vec<MethodInfo> = engine::METHODS.to_vec();
                    all.push(QUIT);
                    json!(all)
                } else {
                    v
                }
            }),
            None if method.starts_with("ui.") => Err(ControlError::new(
                "unavailable",
                format!("`{method}` vyžaduje okno: spusti cabinetcraft-app --control PORT"),
            )),
            None => Err(ControlError::unknown_method(&method)),
        };
        incoming.respond(result);
    }
}

/// `cabinetcraft parity`: import do dočasného priečinka, generovanie, porovnanie.
fn run_parity(
    file: &std::path::Path,
    dataset: &std::path::Path,
    template: Option<&std::path::Path>,
    out: Option<&std::path::Path>,
) -> Result<bool, Box<dyn std::error::Error>> {
    let ds = parity::load_dataset(dataset)?;
    let dir = std::env::temp_dir().join(format!("cabinetcraft-parity-{}", std::process::id()));
    let project = dir.join("project.toml");
    let result = (|| -> Result<(parity::Totals, String), Box<dyn std::error::Error>> {
        let outcome = import_cvj::run(file, &project, template)?;
        // poznámky importu môžu obsahovať mená zo zákazky: iba na konzolu, nie do správy
        for n in &outcome.notes {
            eprintln!("{}", tr!("cli.import_note", note = n));
        }
        let loaded = cc_config::load(&project)?;
        let machine = match cc_pipeline::default_machine(&project) {
            Some(path) => Some(cc_config::load_machine(&path)?),
            None => None,
        };
        let built = cc_pipeline::build(
            &loaded.project,
            &loaded.standard,
            &loaded.materials,
            machine.as_ref(),
        )?;
        for issue in built.issues() {
            eprintln!("{issue}");
        }
        Ok(parity::compare(&built.model, &ds))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    let (totals, report) = result?;
    match out {
        Some(path) => {
            std::fs::write(path, &report)?;
            println!(
                "{}",
                tr!(
                    "cli.parity_done",
                    parts = totals.matched_parts,
                    cv_parts = totals.cv_parts,
                    same_size = totals.same_size,
                    holes = totals.matched_holes,
                    cv_holes = totals.cv_holes,
                    extra = totals.our_extra_holes,
                    file = path.display().to_string()
                )
            );
        }
        None => print!("{report}"),
    }
    Ok(true)
}

/// `cabinetcraft check-locale de.toml`: výpis rozdielov voči vstavanej angličtine.
/// `Ok(false)` iba pri chybe súboru; chýbajúce kľúče sa doplnia z angličtiny, preto nie sú chyba.
fn check_locale(file: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let code = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    let text = std::fs::read_to_string(file)?;
    let catalog = match cc_i18n::Catalog::parse(&code, &text) {
        Ok(c) => c,
        Err(e) => {
            println!("{}: {e}", file.display());
            return Ok(false);
        }
    };
    let reference = cc_i18n::BUILTIN
        .iter()
        .find(|(c, _)| *c == cc_i18n::REFERENCE_LANGUAGE)
        .and_then(|(c, t)| cc_i18n::Catalog::parse(c, t).ok())
        .unwrap_or_else(|| cc_i18n::Catalog::empty(cc_i18n::REFERENCE_LANGUAGE));
    let d = cc_i18n::diff(&reference, &catalog);
    println!(
        "{} ({}): {} of {} texts translated, missing {}, extra {}, parameters {}",
        file.display(),
        catalog.name,
        reference.len() - d.missing.len(),
        reference.len(),
        d.missing.len(),
        d.extra.len(),
        d.placeholders.len()
    );
    for k in &d.missing {
        println!("  missing    {k}");
    }
    for k in &d.extra {
        println!("  extra      {k}");
    }
    for k in &d.placeholders {
        println!("  parameters {k}");
    }
    for w in &catalog.warnings {
        println!("  warning    {w}");
    }
    Ok(true)
}
