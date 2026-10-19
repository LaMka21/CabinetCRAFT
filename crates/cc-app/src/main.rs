//! CabinetCRAFT – okenná aplikácia.
//!
//! `cabinetcraft-app [--control PORT] [project.toml]`. Bez projektu otvorí naposledy
//! použitý projekt, inak vzorový projekt dielne (`examples/dielna`), ak ho nájde.
//! `--control PORT` (alebo `CABINETCRAFT_CONTROL_PORT`) zapne riadiaci kanál
//! (`docs/control-protocol.md`).

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
// v release zostave bez okna konzoly
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cabinet_editor;
mod canvas2d;
mod catalog_panel;
mod control;
mod drawings;
mod gpu3d;
mod grid;
mod icons;
mod labels_view;
mod part_editor;
mod part_view;
mod plan_modes;
mod plan_view;
mod production;
mod props_dialog;
mod reports;
mod ribbon;
mod shell;
mod side_panel;
mod standard_tab;
mod tabs;
mod theme;
mod view3d;
mod wall_view;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;

/// Projekt a port riadiaceho kanála z príkazového riadku.
fn parse_args() -> (Option<PathBuf>, Option<u16>) {
    let mut start = None;
    let mut port = std::env::var("CABINETCRAFT_CONTROL_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--control" {
            match args.next().and_then(|p| p.to_str()?.parse().ok()) {
                Some(p) => port = Some(p),
                None => eprintln!("--control: chýba číslo portu"),
            }
        } else {
            start = Some(PathBuf::from(arg));
        }
    }
    (start, port)
}

/// Okno beží na wgpu (na Windows DirectX 12, `WGPU_BACKEND` to zmení). Ak GPU nie je
/// k dispozícii, spustí sa znova na OpenGL (glow) s 3D náhľadom na CPU.
/// `CABINETCRAFT_RENDERER=glow` vynúti záložnú cestu, `cpu` iba CPU náhľad 3D.
fn main() -> eframe::Result {
    let (start, control) = parse_args();
    load_language();
    let options = |renderer| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CabinetCRAFT")
            .with_inner_size([1440.0, 880.0])
            .with_min_inner_size([960.0, 600.0])
            .with_icon(Arc::new(icon())),
        renderer,
        wgpu_options: wgpu_options(),
        ..Default::default()
    };
    let created = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let creator = |start: Option<PathBuf>| -> eframe::AppCreator<'static> {
        let created = created.clone();
        Box::new(move |cc| {
            created.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(Box::new(app::App::new(cc, start, control)))
        })
    };
    if std::env::var("CABINETCRAFT_RENDERER").as_deref() != Ok("glow") {
        match eframe::run_native(
            "CabinetCRAFT",
            options(eframe::Renderer::Wgpu),
            creator(start.clone()),
        ) {
            Ok(()) => return Ok(()),
            // okno už bežalo: chybu nahlásiť, nespúšťať znova
            Err(e) if created.load(std::sync::atomic::Ordering::Relaxed) => return Err(e),
            Err(e) => eprintln!("wgpu sa nedá použiť ({e}); okno pobeží na OpenGL s 3D na CPU"),
        }
    }
    eframe::run_native(
        "CabinetCRAFT",
        options(eframe::Renderer::Glow),
        creator(start),
    )
}

fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{wgpu, WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    let mut setup = WgpuSetupCreateNew::default();
    let preferred = if cfg!(windows) {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::PRIMARY
    };
    setup.instance_descriptor.backends = wgpu::Backends::from_env().unwrap_or(preferred);
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// Písmo Segoe UI z Windows (ak je), inak vstavané písma egui.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windows =
        std::env::var_os("WINDIR").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    for (name, file, family) in [
        ("segoe", "segoeui.ttf", egui::FontFamily::Proportional),
        ("consolas", "consola.ttf", egui::FontFamily::Monospace),
    ] {
        if let Ok(bytes) = std::fs::read(windows.join("Fonts").join(file)) {
            fonts
                .font_data
                .insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.to_owned());
        }
    }
    ctx.set_fonts(fonts);
}

/// Ikona: skrinka s dvomi dvierkami na modrom pozadí.
fn icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    for y in 0..N {
        for x in 0..N {
            let inside =
                |x0: usize, y0: usize, x1: usize, y1: usize| x >= x0 && x < x1 && y >= y0 && y < y1;
            let corner = {
                let r = 10.0f32;
                let cx = (x as f32 + 0.5).clamp(r, N as f32 - r);
                let cy = (y as f32 + 0.5).clamp(r, N as f32 - r);
                (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) <= r
            };
            let c: [u8; 4] = if !corner {
                [0, 0, 0, 0]
            } else if inside(14, 10, 50, 54) {
                if inside(17, 13, 31, 51) || inside(33, 13, 47, 51) {
                    if inside(27, 28, 29, 38) || inside(35, 28, 37, 38) {
                        [43, 93, 122, 255]
                    } else {
                        [244, 240, 232, 255]
                    }
                } else {
                    [205, 130, 50, 255]
                }
            } else {
                [43, 93, 122, 255]
            };
            rgba[(y * N + x) * 4..][..4].copy_from_slice(&c);
        }
    }
    egui::IconData {
        rgba,
        width: N as u32,
        height: N as u32,
    }
}

fn app_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("CabinetCRAFT"))
}

fn settings_file() -> Option<PathBuf> {
    Some(app_dir()?.join("posledny_projekt.txt"))
}

/// Nastavenia používateľa (`%APPDATA%\CabinetCRAFT\nastavenia.toml`).
fn user_settings_file() -> Option<PathBuf> {
    Some(app_dir()?.join("nastavenia.toml"))
}

fn read_user_settings() -> toml::Table {
    user_settings_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| t.parse::<toml::Table>().ok())
        .unwrap_or_default()
}

/// Jazyk z nastavení používateľa; neznámy alebo poškodený súbor jazyka = slovenčina.
pub fn load_language() {
    let settings = read_user_settings();
    if let Some(code) = settings.get("language").and_then(toml::Value::as_str) {
        if let Err(e) = cc_i18n::set_language(code) {
            eprintln!("jazyk {code}: {e}");
        }
    }
}

/// Zapamätá jazyk v nastaveniach používateľa (ostatné nastavenia ostanú).
pub fn remember_language(code: &str) {
    let Some(file) = user_settings_file() else {
        return;
    };
    let mut settings = read_user_settings();
    settings.insert("language".into(), toml::Value::String(code.to_owned()));
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = toml::to_string(&settings) {
        let _ = std::fs::write(file, text);
    }
}

/// Priečinok jazykov používateľa (`%APPDATA%\CabinetCRAFT\locales`), vytvorí ho.
pub fn user_locales_dir() -> Option<PathBuf> {
    let dir = app_dir()?.join("locales");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Naposledy otvorený projekt, ak ešte existuje.
pub fn last_project() -> Option<PathBuf> {
    let text = std::fs::read_to_string(settings_file()?).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_file().then_some(path)
}

pub fn remember_project(path: &Path) {
    if let Some(file) = settings_file() {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let text = path.display().to_string();
        let text = text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned();
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, text);
    }
}

/// `examples/dielna/project.toml` v aktuálnom priečinku alebo nad priečinkom programu.
pub fn example_project() -> Option<PathBuf> {
    let rel = Path::new("examples").join("dielna").join("project.toml");
    let mut candidates = vec![std::env::current_dir().ok()?.join(&rel)];
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(exe.ancestors().skip(1).map(|d| d.join(&rel)));
    }
    candidates.into_iter().find(|p| p.is_file())
}
