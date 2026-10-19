//! Nástroje workspace: `cargo xtask <príkaz>`.
//!
//! Iba std + serde_json. `cargo` sa spúšťa cez `std::process::Command`.

mod i18n;
mod layers;
mod stats;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
použitie: cargo xtask <príkaz>

príkazy:
  ci       fmt --check, clippy -D warnings, testy, vrstvy, preklady (zastaví sa pri prvej chybe)
  layers   kontrola vrstiev crate-ov (plan/architecture.md §2)
  i18n     preklady locales/*.toml: chýbajúce a nadbytočné kľúče voči en.toml, kľúče v kóde
  stats    počet riadkov a testov na crate
  parity   porovnanie so zákazkou CV REF diel po diele (cesta k .cvj v CABINETCRAFT_CVJ_REF;
           ďalšie voľby sa odovzdajú `cabinetcraft parity`, napr. --out sprava.md)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => cmd_ci(),
        Some("layers") => cmd_layers(),
        Some("i18n") => i18n::run(&root()),
        Some("stats") => stats::run(&root()),
        Some("parity") => cmd_parity(args.get(1..).unwrap_or_default()),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("neznámy príkaz `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("chyba: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Koreň workspace (nadradený priečinok crate-u xtask).
pub fn root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: nedá sa spustiť: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: skončil s {status}"))
    }
}

fn metadata() -> Result<serde_json::Value, String> {
    let out = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata zlyhal:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: zlý JSON: {e}"))
}

fn cmd_layers() -> Result<(), String> {
    let crates = layers::from_metadata(&metadata()?)?;
    println!("Vrstvy crate-ov (plan/architecture.md §2)\n");
    println!("{:<16} {:<8} závislosti vo workspace", "crate", "vrstva");
    for c in &crates {
        let deps: Vec<String> = c
            .deps
            .iter()
            .filter(|d| d.workspace)
            .map(|d| match d.kind {
                layers::DepKind::Normal => d.name.clone(),
                layers::DepKind::Dev => format!("{} (dev)", d.name),
                layers::DepKind::Build => format!("{} (build)", d.name),
            })
            .collect();
        println!(
            "{:<16} {:<8} {}",
            c.name,
            layers::describe(layers::classify(&c.name)),
            deps.join(", ")
        );
    }
    let violations = layers::check(&crates);
    println!();
    if violations.is_empty() {
        println!("OK: {} crate-ov, žiadne porušenie vrstiev.", crates.len());
        Ok(())
    } else {
        println!("Porušenia ({}):", violations.len());
        for v in &violations {
            println!("  - {v}");
        }
        Err(format!("{} porušení vrstiev", violations.len()))
    }
}

type Step = (&'static str, fn() -> Result<(), String>);

fn cmd_ci() -> Result<(), String> {
    let steps: [Step; 5] = [
        ("fmt", || {
            let mut c = cargo();
            c.args(["fmt", "--all", "--", "--check"]);
            run(c, "cargo fmt --all -- --check")
        }),
        ("clippy", || {
            let mut c = cargo();
            c.args([
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ]);
            run(c, "cargo clippy --workspace --all-targets -- -D warnings")
        }),
        ("test", || {
            let mut c = cargo();
            c.args(["test", "--workspace"]);
            run(c, "cargo test --workspace")
        }),
        ("layers", cmd_layers),
        ("i18n", || i18n::run(&root())),
    ];
    let mut done = Vec::new();
    for (name, step) in &steps {
        eprintln!("\n=== ci: {name} ===");
        if let Err(e) = step() {
            println!("\nSúhrn CI:");
            for d in &done {
                println!("  ok     {d}");
            }
            println!("  CHYBA  {name}: {e}");
            for (n, _) in steps.iter().skip(done.len() + 1) {
                println!("  vynechané {n}");
            }
            return Err(format!("ci zlyhalo v kroku `{name}`"));
        }
        done.push(*name);
    }
    println!(
        "\nSúhrn CI: všetkých {} krokov prešlo ({})",
        done.len(),
        done.join(", ")
    );
    Ok(())
}

/// `cargo xtask parity [voľby]`: `cabinetcraft parity $CABINETCRAFT_CVJ_REF [voľby]`.
/// Súbor `.cvj` nie je v repe (meno zákazníka), cesta ide z premennej prostredia.
fn cmd_parity(rest: &[String]) -> Result<(), String> {
    let cvj = std::env::var_os("CABINETCRAFT_CVJ_REF")
        .ok_or("nastav CABINETCRAFT_CVJ_REF na súbor .cvj zákazky REF2026")?;
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root())
        .args(["run", "-q", "-p", "cc-cli", "--", "parity"])
        .arg(cvj)
        .args(rest)
        .status()
        .map_err(|e| format!("cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("cabinetcraft parity skončil chybou".into())
    }
}
