//! Otvorený projekt: zadanie skriniek, materiály, štandard a profil stroja.

use std::fs;
use std::path::{Path, PathBuf};

use cc_model::{
    CabinetSpec, CabinetType, MachineProfile, MaterialLibrary, NamedCatalog, ProjectFile,
    ProjectInfo, Standard,
};

use cc_i18n::tr;

use crate::tomlmerge;

#[derive(Debug, Clone)]
pub struct Document {
    /// `None`: nový projekt, ešte neuložený.
    pub path: Option<PathBuf>,
    /// Priečinok s `materials.toml`, `standards/` a `machine.toml`.
    pub base: PathBuf,
    pub project: ProjectFile,
    pub materials: MaterialLibrary,
    pub standard: Standard,
    /// Katalógy skriniek dielne (`catalogs/*.toml`).
    pub catalogs: Vec<NamedCatalog>,
    pub machine: Option<MachineProfile>,
    pub machine_path: Option<PathBuf>,
    /// Neuložené zmeny projektu.
    pub dirty: bool,
    /// Neuložené zmeny štandardu.
    pub standard_dirty: bool,
}

impl Document {
    pub fn open(path: &Path) -> Result<Document, String> {
        let loaded = cc_config::load(path).map_err(|e| e.to_string())?;
        let machine_path = cc_pipeline::default_machine(path);
        let machine = match &machine_path {
            Some(p) => Some(cc_config::load_machine(p).map_err(|e| e.to_string())?),
            None => None,
        };
        let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let catalogs = cc_config::load_catalogs(&base).map_err(|e| e.to_string())?;
        Ok(Document {
            path: Some(path.to_path_buf()),
            base,
            project: loaded.project,
            materials: loaded.materials,
            standard: loaded.standard,
            catalogs,
            machine,
            machine_path,
            dirty: false,
            standard_dirty: false,
        })
    }

    /// Nový projekt s materiálmi, štandardom a strojom z `other`.
    pub fn new_like(other: &Document) -> Document {
        Document {
            path: None,
            base: other.base.clone(),
            project: ProjectFile {
                // firma, logo a kto kreslí ostávajú z predchádzajúcej zákazky
                project: ProjectInfo {
                    company: other.project.project.company.clone(),
                    designer: other.project.project.designer.clone(),
                    logo: other.project.project.logo.clone(),
                    ..ProjectInfo::named("Nový projekt")
                },
                defaults: other.project.defaults.clone(),
                cabinets: vec![new_cabinet(CabinetType::Base, "SP1")],
                room: None,
                walls: Vec::new(),
                openings: Vec::new(),
            },
            materials: other.materials.clone(),
            standard: other.standard.clone(),
            catalogs: other.catalogs.clone(),
            machine: other.machine.clone(),
            machine_path: other.machine_path.clone(),
            dirty: true,
            standard_dirty: other.standard_dirty,
        }
    }

    pub fn title(&self) -> String {
        let file = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| tr!("engine.document.unsaved"));
        format!(
            "{}{} – {}",
            self.project.project.name,
            if self.dirty { " *" } else { "" },
            file
        )
    }

    pub fn standard_path(&self) -> PathBuf {
        self.base
            .join("standards")
            .join(format!("{}.toml", self.project.defaults.construction))
    }

    /// Mená štandardov v `standards/`.
    pub fn standards_available(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.base.join("standards"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension().is_some_and(|x| x == "toml"))
                    .then(|| p.file_stem()?.to_str().map(str::to_owned))
                    .flatten()
            })
            .collect();
        names.sort();
        names
    }

    /// Prepne konštrukčný štandard a načíta ho zo súboru.
    pub fn set_construction(&mut self, name: &str) -> Result<(), String> {
        if name.is_empty() || name.contains(['/', '\\', ':']) || name.contains("..") {
            return Err(tr!("engine.document.bad_standard_name", name = name));
        }
        let path = self.base.join("standards").join(format!("{name}.toml"));
        self.standard = read_standard(&path)?;
        self.project.defaults.construction = name.to_owned();
        self.standard_dirty = false;
        self.dirty = true;
        Ok(())
    }

    pub fn reload_standard(&mut self) -> Result<(), String> {
        self.standard = read_standard(&self.standard_path())?;
        self.standard_dirty = false;
        Ok(())
    }

    /// Súbor katalógu `catalogs/<meno>.toml`.
    pub fn catalog_path(&self, name: &str) -> PathBuf {
        self.base.join("catalogs").join(format!("{name}.toml"))
    }

    /// Zapíše katalóg (komentáre v súbore ostanú, ak sa nezmenil počet položiek).
    pub fn write_catalog(&self, name: &str) -> Result<(), String> {
        let Some(c) = self.catalogs.iter().find(|c| c.name == name) else {
            return Err(tr!("engine.document.catalog_missing", name = name));
        };
        let problems = cc_config::validate_catalog(&c.catalog);
        if !problems.is_empty() {
            return Err(tr!(
                "engine.document.catalog_invalid",
                problems = problems.join("\n")
            ));
        }
        let path = self.catalog_path(name);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        write_merged(&path, &c.catalog, None)
    }

    pub fn save_standard(&mut self) -> Result<(), String> {
        let problems = cc_config::validate_standard(&self.standard);
        if !problems.is_empty() {
            return Err(tr!(
                "engine.document.standard_invalid",
                problems = problems.join("\n")
            ));
        }
        write_merged(&self.standard_path(), &self.standard, None)?;
        self.standard_dirty = false;
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), String> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| tr!("engine.document.no_file"))?;
        write_merged(&path, &self.project, None)?;
        self.dirty = false;
        Ok(())
    }

    /// Uloží projekt do `path`. Ak v cieľovom priečinku chýbajú materiály, štandard
    /// alebo profil stroja, skopíruje ich. Vráti poznámky pre používateľa.
    pub fn save_as(&mut self, path: &Path) -> Result<Vec<String>, String> {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut notes = Vec::new();
        if dir != self.base {
            let standard_rel =
                Path::new("standards").join(format!("{}.toml", self.project.defaults.construction));
            for rel in [
                PathBuf::from("materials.toml"),
                standard_rel.clone(),
                PathBuf::from("machine.toml"),
            ] {
                let (from, to) = (self.base.join(&rel), dir.join(&rel));
                if to.exists() {
                    notes.push(tr!(
                        "engine.document.already_in_target",
                        file = rel.display().to_string()
                    ));
                } else if from.exists() {
                    if let Some(parent) = to.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
                    if rel == standard_rel && self.standard_dirty {
                        write_merged(&to, &self.standard, None)?;
                    }
                    notes.push(tr!(
                        "engine.document.copied",
                        file = rel.display().to_string()
                    ));
                }
            }
        }
        // komentáre pôvodného projektu sa prenesú aj do nového súboru
        let template = self.path.as_ref().and_then(|p| fs::read_to_string(p).ok());
        write_merged(path, &self.project, template)?;
        let standard_dirty = self.standard_dirty && dir == self.base;
        let standard = self.standard.clone();
        *self = Document::open(path)?;
        if standard_dirty {
            self.standard = standard;
            self.standard_dirty = true;
        }
        Ok(notes)
    }
}

/// Skrinka s predvolenými rozmermi podľa typu.
pub fn new_cabinet(cabinet_type: CabinetType, id: &str) -> CabinetSpec {
    let (h, d, shelves) = match cabinet_type {
        CabinetType::Base => (720.0, 560.0, 1),
        CabinetType::Wall => (720.0, 330.0, 2),
        CabinetType::Tall => (2100.0, 560.0, 4),
    };
    CabinetSpec {
        id: id.to_owned(),
        cabinet_type,
        w: 600.0,
        h,
        d,
        adjustable_shelves: shelves,
        fixed_shelves: Vec::new(),
        wall: None,
        at: 0.0,
        elevation: None,
        outset: 0.0,
        fronts: Vec::new(),
        reveals: Default::default(),
        front_mount: None,
        hinge: None,
        drawer: None,
        drawer_length: None,
        top_style: None,
        stretcher_drop: None,
        back: true,
        carcass: true,
        lock_width: false,
        comment: None,
        placement: None,
        catalog: None,
        cv_id: None,
        cv_fronts: None,
        filler: None,
        slant: None,
        niche: false,
        led_rail: None,
        options: Default::default(),
    }
}

fn read_standard(path: &Path) -> Result<Standard, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let standard: Standard =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let problems = cc_config::validate_standard(&standard);
    if problems.is_empty() {
        Ok(standard)
    } else {
        Err(format!("{}:\n{}", path.display(), problems.join("\n")))
    }
}

fn write_merged<T: serde::Serialize>(
    path: &Path,
    value: &T,
    template: Option<String>,
) -> Result<(), String> {
    let existing = template.or_else(|| fs::read_to_string(path).ok());
    let text = tomlmerge::merge(existing.as_deref(), value)?;
    fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}
