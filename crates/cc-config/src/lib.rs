//! Načítanie projektu a jeho súvisiacich súborov (špecifikácia MVP, kap. 4).
//!
//! Rozloženie na disku (relatívne k `project.toml`):
//! - `materials.toml`
//! - `standards/<defaults.construction>.toml`
//! - `catalogs/<meno>.toml` (nepovinné katalógy skriniek dielne; staršie `catalog.toml`)

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

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use cc_i18n::tr;
use cc_model::{
    Catalog, ConnectionKind, MachineProfile, MaterialLibrary, NamedCatalog, ProductionMode,
    ProjectFile, Role, Standard, TemplateProblem, UnmatchedBrace,
};
use serde::de::DeserializeOwned;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("{}", invalid_message(.path, .problems))]
    Invalid {
        path: PathBuf,
        problems: Vec<String>,
    },
}

/// Text chyby neplatnej konfigurácie (cesta a zoznam problémov).
fn invalid_message(path: &Path, problems: &[String]) -> String {
    tr!(
        "config.invalid",
        path = path.display().to_string(),
        problems = problems.join("\n  - ")
    )
}

/// Projekt so všetkými súbormi, ktoré používa.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub project: ProjectFile,
    pub materials: MaterialLibrary,
    pub standard: Standard,
    pub project_path: PathBuf,
}

/// Načíta `project.toml`, `materials.toml` a konštrukčný štandard a skontroluje ich.
pub fn load(project_path: &Path) -> Result<LoadedProject, ConfigError> {
    let base = project_path.parent().unwrap_or_else(|| Path::new("."));
    let project: ProjectFile = read_toml(project_path)?;

    let materials_path = base.join("materials.toml");
    let materials: MaterialLibrary = read_toml(&materials_path)?;

    let standard_path = base
        .join("standards")
        .join(format!("{}.toml", project.defaults.construction));
    let standard: Standard = read_toml(&standard_path)?;

    check(project_path, validate_materials(&materials))?;
    check(&standard_path, validate_standard(&standard))?;
    check(project_path, validate_project(&project, &materials))?;

    Ok(LoadedProject {
        project,
        materials,
        standard,
        project_path: project_path.to_path_buf(),
    })
}

/// Najviac položiek katalógu.
const MAX_CATALOG: usize = 1000;

/// Najviac katalógov.
const MAX_CATALOGS: usize = 100;

/// Načíta katalógy skriniek `catalogs/*.toml` z priečinka projektu (podľa mena) a staršie
/// `catalog.toml` ako katalóg „Katalóg“. Bez súborov prázdny zoznam.
pub fn load_catalogs(base: &Path) -> Result<Vec<NamedCatalog>, ConfigError> {
    let mut out = Vec::new();
    let dir = base.join("catalogs");
    if dir.is_dir() {
        let entries = fs::read_dir(&dir).map_err(|source| ConfigError::Io {
            path: dir.clone(),
            source,
        })?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "toml") && p.is_file())
            .collect();
        paths.sort();
        for path in paths.into_iter().take(MAX_CATALOGS) {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
                continue;
            };
            if !cc_model::valid_catalog_name(&name) {
                continue;
            }
            let catalog: Catalog = read_toml(&path)?;
            check(&path, validate_catalog(&catalog))?;
            out.push(NamedCatalog { name, catalog });
        }
    }
    let legacy = base.join("catalog.toml");
    if legacy.is_file() && !out.iter().any(|c| c.name == "Katalóg") {
        let catalog: Catalog = read_toml(&legacy)?;
        check(&legacy, validate_catalog(&catalog))?;
        out.push(NamedCatalog {
            name: "Katalóg".to_owned(),
            catalog,
        });
    }
    Ok(out)
}

pub fn validate_catalog(c: &Catalog) -> Vec<String> {
    let mut problems = Vec::new();
    if c.items.len() > MAX_CATALOG {
        problems.push(tr!("config.catalog.too_many_items", max = MAX_CATALOG));
    }
    let mut ids = HashSet::new();
    for item in &c.items {
        if item.id.trim().is_empty() || item.id.len() > 100 {
            problems.push(tr!("config.catalog.bad_id"));
        }
        if !ids.insert(item.id.as_str()) {
            problems.push(tr!("config.catalog.duplicate", id = &item.id));
        }
        for (name, v) in [("w", item.w), ("h", item.h), ("d", item.d)] {
            if !is_positive(v) || v > 10_000.0 {
                problems.push(tr!("config.catalog.bad_size", id = &item.id, name = name));
            }
        }
        if item.fronts.len() > 12 || item.adjustable_shelves > 50 {
            problems.push(tr!("config.catalog.too_many_fronts", id = &item.id));
        }
        if item
            .fronts
            .iter()
            .filter_map(|f| f.height)
            .chain(item.drawer_length)
            .any(|v| !is_positive(v))
        {
            problems.push(tr!("config.catalog.bad_front_heights", id = &item.id));
        }
    }
    problems
}

/// Načíta a skontroluje profil stroja `machine.toml` (kap. 7.3).
pub fn load_machine(path: &Path) -> Result<MachineProfile, ConfigError> {
    let machine: MachineProfile = read_toml(path)?;
    check(path, validate_machine(&machine))?;
    Ok(machine)
}

pub fn validate_machine(m: &MachineProfile) -> Vec<String> {
    let mut problems = Vec::new();
    for (name, v) in [
        ("work_area.max_length", m.work_area.max_length),
        ("work_area.max_width", m.work_area.max_width),
        ("min_part.length", m.min_part.length),
        ("min_part.width", m.min_part.width),
    ] {
        if !is_positive(v) {
            problems.push(tr!("config.must_be_positive", name = name, value = v));
        }
    }
    if m.min_part.length > m.work_area.max_length || m.min_part.width > m.work_area.max_width {
        problems.push(tr!("config.machine.min_part_too_big"));
    }
    if m.drills.diameters.is_empty() {
        problems.push(tr!("config.machine.no_drills"));
    }
    for d in &m.drills.diameters {
        if !is_positive(*d) {
            problems.push(tr!("config.machine.bad_drill", value = *d));
        }
    }
    if let Some(xcs) = &m.xcs {
        problems.extend(xcs.template_problems().iter().map(template_problem));
    }
    if let Some(mpr) = &m.mpr {
        problems.extend(mpr.template_problems().iter().map(template_problem));
    }
    if let Some(nesting) = &m.nesting {
        problems.extend(nesting.template_problems().iter().map(template_problem));
    }
    if m.production.mode == ProductionMode::Nesting {
        if m.nesting.is_none() {
            problems.push(tr!("config.machine.nesting_needs_nesting"));
        }
        if m.xcs.is_none() {
            problems.push(tr!("config.machine.nesting_needs_xcs"));
        }
    }
    problems
}

/// Text problému šablóny alebo parametra profilu stroja v zvolenom jazyku.
pub fn template_problem(p: &TemplateProblem) -> String {
    use TemplateProblem as P;
    match p {
        P::Unmatched {
            block,
            brace: UnmatchedBrace::Open,
            template,
        } => tr!(
            "config.template.unmatched_open",
            block = block,
            template = template
        ),
        P::Unmatched {
            block,
            brace: UnmatchedBrace::Close,
            template,
        } => tr!(
            "config.template.unmatched_close",
            block = block,
            template = template
        ),
        P::UnknownVariable {
            block,
            var,
            allowed,
        } => tr!(
            "config.template.unknown_variable",
            block = block,
            var = var,
            allowed = if allowed.is_empty() {
                tr!("config.template.no_variables")
            } else {
                allowed.join(", ")
            }
        ),
        P::EmptyFieldName { block } => tr!("config.template.empty_field_name", block = block),
        P::TooManyDecimals { name, value } => tr!(
            "config.template.too_many_decimals",
            name = name,
            value = *value
        ),
        P::NotPositive { name } => tr!("config.template.not_positive", name = name),
        P::Negative { name } => tr!("config.template.negative", name = name),
        P::NegativeValue { name, value } => {
            tr!("config.must_be_non_negative", name = name, value = *value)
        }
        P::Empty { name } => tr!("config.template.empty", name = name),
        P::StepOverKerf { step, kerf } => {
            tr!("config.template.step_over_kerf", step = *step, kerf = *kerf)
        }
        P::SheetNameVars => tr!("config.template.sheet_name_vars"),
        P::SheetSize { name, max } => tr!("config.template.sheet_size", name = name, max = *max),
        P::SheetBelowMargins { name } => {
            tr!("config.template.sheet_below_margins", name = name)
        }
        P::SpacingBelowTool { spacing, tool } => tr!(
            "config.template.spacing_below_tool",
            spacing = *spacing,
            tool = *tool
        ),
        P::ThroughExtra { value } => tr!("config.template.through_extra", value = *value),
        P::LeadInDepth => tr!("config.template.lead_in_depth"),
        P::OverlapBelowTool { overlap, tool } => tr!(
            "config.template.overlap_below_tool",
            overlap = *overlap,
            tool = *tool
        ),
        P::RemnantOffset { offset, radius } => tr!(
            "config.template.remnant_offset",
            offset = *offset,
            radius = *radius
        ),
        P::RemnantOverrun { overrun, radius } => tr!(
            "config.template.remnant_overrun",
            overrun = *overrun,
            radius = *radius
        ),
        P::DrillOrderDiameter { value } => {
            tr!("config.template.drill_order_diameter", value = *value)
        }
        P::VariantDiameter { value } => {
            tr!("config.template.variant_diameter", value = *value)
        }
        P::VariantMaxDepth { diameter } => {
            tr!("config.template.variant_max_depth", diameter = *diameter)
        }
        P::VariantNoTool { diameter } => {
            tr!("config.template.variant_no_tool", diameter = *diameter)
        }
    }
}

fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Kladné konečné číslo (NaN a nekonečno neprejdú).
fn is_positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

fn check(path: &Path, problems: Vec<String>) -> Result<(), ConfigError> {
    if problems.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::Invalid {
            path: path.to_path_buf(),
            problems,
        })
    }
}

pub fn validate_materials(lib: &MaterialLibrary) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = HashSet::new();
    for m in &lib.materials {
        if !seen.insert(m.id.as_str()) {
            problems.push(tr!("config.materials.duplicate", id = &m.id));
        }
        if !is_positive(m.thickness) {
            problems.push(tr!("config.materials.bad_thickness", id = &m.id));
        }
        if let Some(band) = &m.edge_band {
            if lib.edge_band(band).is_none() {
                problems.push(tr!(
                    "config.materials.unknown_band",
                    id = &m.id,
                    band = band
                ));
            }
        }
    }
    let mut seen = HashSet::new();
    for e in &lib.edge_bands {
        if !seen.insert(e.id.as_str()) {
            problems.push(tr!("config.materials.band_duplicate", id = &e.id));
        }
        if !is_positive(e.thickness) {
            problems.push(tr!("config.materials.band_bad_thickness", id = &e.id));
        }
    }
    problems
}

pub fn validate_standard(std: &Standard) -> Vec<String> {
    let mut problems = Vec::new();
    let mut positive = |name: &str, v: f64| {
        if !is_positive(v) {
            problems.push(tr!("config.must_be_positive", name = name, value = v));
        }
    };
    positive("carcass.stretcher_width", std.carcass.stretcher_width);
    positive("back.groove_depth", std.back.groove_depth);
    positive("dowel.diameter", std.dowel.diameter);
    positive("dowel.face_depth", std.dowel.face_depth);
    positive("dowel.edge_depth", std.dowel.edge_depth);
    positive("dowel.end_offset", std.dowel.end_offset);
    positive("dowel.max_spacing", std.dowel.max_spacing);
    positive("dowel.pair_pitch", std.dowel.pair_pitch);
    positive("dowel.max_pair_spacing", std.dowel.max_pair_spacing);
    positive("dowel.short_end_offset", std.dowel.short_end_offset);
    positive("line_bore.diameter", std.line_bore.diameter);
    positive("line_bore.depth", std.line_bore.depth);
    positive("line_bore.pitch", std.line_bore.pitch);
    if std.plinth.side {
        let d = &std.plinth.side_dowel;
        positive("plinth.side_dowel.diameter", d.diameter);
        positive("plinth.side_dowel.face_depth", d.face_depth);
        positive("plinth.side_dowel.edge_depth", d.edge_depth);
        positive("plinth.side_dowel.from_edge", d.from_edge);
    }

    let mut non_negative = |name: &str, v: f64| {
        if v.is_nan() || v < 0.0 {
            problems.push(tr!("config.must_be_non_negative", name = name, value = v));
        }
    };
    non_negative(
        "carcass.shelf_side_clearance",
        std.carcass.shelf_side_clearance,
    );
    non_negative(
        "carcass.shelf_front_setback",
        std.carcass.shelf_front_setback,
    );
    non_negative(
        "back.groove_offset_from_rear",
        std.back.groove_offset_from_rear,
    );
    non_negative(
        "back.groove_width_clearance",
        std.back.groove_width_clearance,
    );
    non_negative("back.fit_clearance", std.back.fit_clearance);
    non_negative("line_bore.front_offset", std.line_bore.front_offset);
    non_negative("line_bore.rear_offset", std.line_bore.rear_offset);
    non_negative(
        "line_bore.start_above_bottom",
        std.line_bore.start_above_bottom,
    );
    non_negative("line_bore.end_below_top", std.line_bore.end_below_top);
    non_negative("plinth.max_length", std.plinth.max_length);
    non_negative("plinth.side_back_gap", std.plinth.side_back_gap);
    non_negative("cover_side.top", std.cover_side.top);
    non_negative("cover_side.front", std.cover_side.front);
    non_negative("cover_side.back", std.cover_side.back);
    non_negative("blind_corner.side_extend", std.blind_corner.side_extend);
    for v in &std.blind_corner.panel_bottom_dowels {
        if !is_positive(*v) {
            problems.push(tr!("config.standard.bad_panel_dowels", value = *v));
        }
    }

    problems.extend(validate_joint_rules(std));
    problems
}

/// Mená rolí musia byť známe a jedna dvojica rolí nesmie mať dva rôzne spoje.
fn validate_joint_rules(std: &Standard) -> Vec<String> {
    let mut problems = Vec::new();
    let mut pairs: HashMap<(String, String), ConnectionKind> = HashMap::new();
    for (i, rule) in std.joint_rules.iter().enumerate() {
        for name in rule.a.items().into_iter().chain(rule.b.items()) {
            if !Role::RULE_NAMES.contains(&name) {
                problems.push(tr!(
                    "config.standard.unknown_role",
                    rule = i + 1,
                    name = name,
                    allowed = Role::RULE_NAMES.join(", ")
                ));
            }
        }
        for a in rule.a.items() {
            for b in rule.b.items() {
                let key = if a <= b {
                    (a.to_owned(), b.to_owned())
                } else {
                    (b.to_owned(), a.to_owned())
                };
                match pairs.get(&key) {
                    Some(prev) if *prev != rule.connection => problems.push(tr!(
                        "config.standard.conflicting_rule",
                        rule = i + 1,
                        a = &key.0,
                        b = &key.1,
                        prev = format!("{prev:?}"),
                        connection = format!("{:?}", rule.connection)
                    )),
                    _ => {
                        pairs.insert(key, rule.connection);
                    }
                }
            }
        }
    }
    problems
}

pub fn validate_project(project: &ProjectFile, lib: &MaterialLibrary) -> Vec<String> {
    let mut problems = Vec::new();
    if project.project.units != "mm" {
        problems.push(tr!(
            "config.project.bad_units",
            units = &project.project.units
        ));
    }
    for (field, id) in [
        ("defaults.panel_material", &project.defaults.panel_material),
        ("defaults.back_material", &project.defaults.back_material),
    ] {
        if lib.material(id).is_none() {
            problems.push(tr!(
                "config.project.unknown_material",
                field = field,
                id = id
            ));
        }
    }
    let mut seen = HashSet::new();
    for c in &project.cabinets {
        if c.id.trim().is_empty() {
            problems.push(tr!("config.project.empty_cabinet_id"));
        } else if !seen.insert(c.id.as_str()) {
            problems.push(tr!("config.project.duplicate_cabinet", id = &c.id));
        }
        for (name, v) in [("w", c.w), ("h", c.h), ("d", c.d)] {
            if !is_positive(v) {
                problems.push(tr!(
                    "config.project.bad_cabinet_size",
                    id = &c.id,
                    name = name
                ));
            }
        }
    }
    if project.cabinets.is_empty() {
        problems.push(tr!("config.project.no_cabinets"));
    }
    problems.extend(validate_room(project));
    problems
}

/// Miestnosť, steny, otvory a umiestnenie skriniek (kap. 13).
pub fn validate_room(project: &ProjectFile) -> Vec<String> {
    const MAX_WALLS: usize = 200;
    const MAX_OPENINGS: usize = 500;
    const MAX_LEN: f64 = 100_000.0;
    let mut problems = Vec::new();
    let finite_nonneg = |v: f64| v.is_finite() && v >= 0.0;
    if let Some(room) = &project.room {
        for (name, v) in [
            ("room.wall_height", room.wall_height),
            ("room.wall_thickness", room.wall_thickness),
        ] {
            if !is_positive(v) || v > MAX_LEN {
                problems.push(tr!("config.room.bad_size", name = name, max = MAX_LEN));
            }
        }
        for (name, v) in [
            ("room.soffit", room.soffit),
            ("room.toe_height", room.toe_height),
        ] {
            if !finite_nonneg(v) || v > MAX_LEN {
                problems.push(tr!("config.room.negative", name = name));
            }
        }
        if let Some(top) = room.upper_top {
            if !is_positive(top) || top > MAX_LEN {
                problems.push(tr!("config.room.bad_upper_top"));
            }
        }
    }
    if project.walls.len() > MAX_WALLS {
        problems.push(tr!("config.room.too_many_walls", max = MAX_WALLS));
    }
    if project.openings.len() > MAX_OPENINGS {
        problems.push(tr!("config.room.too_many_openings", max = MAX_OPENINGS));
    }
    let mut ids = HashSet::new();
    for w in &project.walls {
        if w.id.trim().is_empty() {
            problems.push(tr!("config.room.empty_wall_id"));
        } else if !ids.insert(w.id.as_str()) {
            problems.push(tr!("config.room.duplicate_wall", id = &w.id));
        }
        if !is_positive(w.length) || w.length > MAX_LEN {
            problems.push(tr!(
                "config.room.bad_wall_length",
                id = &w.id,
                max = MAX_LEN
            ));
        }
        for (name, v) in [("x", w.x), ("y", w.y), ("angle", w.angle)] {
            if !v.is_finite() || v.abs() > MAX_LEN {
                problems.push(tr!("config.room.bad_wall_value", id = &w.id, name = name));
            }
        }
        for (name, v) in [("height", w.height), ("thickness", w.thickness)] {
            if let Some(v) = v {
                if !is_positive(v) || v > MAX_LEN {
                    problems.push(tr!("config.room.bad_wall_size", id = &w.id, name = name));
                }
            }
        }
    }
    for c in &project.cabinets {
        if let Some(wall) = &c.wall {
            if !ids.contains(wall.as_str()) {
                problems.push(tr!("config.room.unknown_wall", id = &c.id, wall = wall));
            }
        }
        if !c.at.is_finite() || c.at.abs() > MAX_LEN {
            problems.push(tr!("config.room.bad_at", id = &c.id));
        }
        if !finite_nonneg(c.outset) || c.outset > MAX_LEN {
            problems.push(tr!("config.room.bad_outset", id = &c.id));
        }
        if let Some(e) = c.elevation {
            if !finite_nonneg(e) || e > MAX_LEN {
                problems.push(tr!("config.room.bad_elevation", id = &c.id));
            }
        }
        if let Some(
            cc_model::Placement::AboveElevation(e) | cc_model::Placement::BelowElevation(e),
        ) = c.placement
        {
            if !finite_nonneg(e) || e > MAX_LEN {
                problems.push(tr!("config.room.bad_placement", id = &c.id));
            }
        }
    }
    for (i, o) in project.openings.iter().enumerate() {
        let n = i + 1;
        if !ids.contains(o.wall.as_str()) {
            problems.push(tr!(
                "config.room.opening_unknown_wall",
                n = n,
                wall = &o.wall
            ));
        }
        for (name, v) in [("width", o.width), ("height", o.height)] {
            if !is_positive(v) || v > MAX_LEN {
                problems.push(tr!("config.room.opening_bad_size", n = n, name = name));
            }
        }
        for (name, v) in [("at", o.at), ("elevation", o.elevation)] {
            if !finite_nonneg(v) || v > MAX_LEN {
                problems.push(tr!("config.room.opening_negative", n = n, name = name));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    const STANDARD: &str = include_str!("../../../examples/s1/standards/frameless32.toml");

    fn standard() -> Standard {
        toml::from_str(STANDARD).expect("vzorový štandard sa musí načítať")
    }

    #[test]
    fn example_standard_is_valid() {
        assert!(validate_standard(&standard()).is_empty());
    }

    #[test]
    fn unknown_role_is_reported() {
        let mut s = standard();
        s.joint_rules[0].a = cc_model::OneOrMany::One("sidee".into());
        let problems = validate_standard(&s);
        assert!(
            problems.iter().any(|p| p.contains("neznáma rola `sidee`")),
            "{problems:?}"
        );
    }

    #[test]
    fn conflicting_rule_is_reported() {
        let mut s = standard();
        s.joint_rules.push(cc_model::JointRule {
            a: cc_model::OneOrMany::One("bottom".into()),
            b: cc_model::OneOrMany::One("side".into()),
            connection: ConnectionKind::LineBore,
            end_offset: None,
            end_groups: None,
        });
        let problems = validate_standard(&s);
        assert!(
            problems.iter().any(|p| p.contains("má už iný spoj")),
            "{problems:?}"
        );
    }

    fn dielna_machine(name: &str) -> MachineProfile {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna")
            .join(name);
        load_machine(&path).unwrap()
    }

    #[test]
    fn example_nesting_profiles_are_valid() {
        let m = dielna_machine("machine.toml");
        assert_eq!(m.production.mode, ProductionMode::Cx100);
        assert!(m.nesting.is_some());
        let n = dielna_machine("machine-nesting.toml");
        assert_eq!(n.production.mode, ProductionMode::Nesting);
        // nič nie je overené na stroji
        let nest = n.nesting.unwrap();
        assert!(!nest.verified && !nest.drill.verified && !nest.route.verified);
    }

    #[test]
    fn nesting_mode_requires_sections() {
        let mut m = dielna_machine("machine-nesting.toml");
        m.nesting = None;
        m.xcs = None;
        let problems = validate_machine(&m);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn unsafe_nesting_numbers_are_reported() {
        let mut m = dielna_machine("machine-nesting.toml");
        let n = m.nesting.as_mut().unwrap();
        n.layout.spacing = 10.0; // menej ako Ø12
        n.route.through_extra = 5.0; // hlboko do podkladovej dosky
        n.route.lead_in_depth = 2.0; // nábeh v materiáli
        n.route.overlap = 5.0;
        n.remnant.offset = 4.0; // rez by zasiahol diely
        n.sheet_name = "N{mat}".into(); // mená tabúľ by sa opakovali
        n.drill.line.push_str("{neznama}");
        let problems = validate_machine(&m);
        for needle in [
            "spacing",
            "through_extra",
            "lead_in_depth",
            "overlap",
            "remnant.offset",
            "sheet_name",
            "{neznama}",
        ] {
            assert!(
                problems.iter().any(|p| p.contains(needle)),
                "{needle}: {problems:?}"
            );
        }
    }

    /// Texty problémov šablón (kódy z cc-model, preklad tu) ostali po slovensky bez zmeny.
    #[test]
    fn template_problem_texts() {
        use cc_model::{TemplateProblem as P, UnmatchedBrace};
        let cases = [
            (
                P::UnknownVariable {
                    block: "xcs.drill.line".into(),
                    var: "zz".into(),
                    allowed: vec!["x".into(), "y".into()],
                },
                "xcs.drill.line: neznáma premenná `{zz}` (povolené: x, y)",
            ),
            (
                P::UnknownVariable {
                    block: "nesting.drill.select".into(),
                    var: "a".into(),
                    allowed: Vec::new(),
                },
                "nesting.drill.select: neznáma premenná `{a}` (povolené: žiadne)",
            ),
            (
                P::Unmatched {
                    block: "mpr.header.XA".into(),
                    brace: UnmatchedBrace::Open,
                    template: "{l".into(),
                },
                "mpr.header.XA: nespárovaná `{` v šablóne `{l`",
            ),
            (
                P::Unmatched {
                    block: "xcs.header".into(),
                    brace: UnmatchedBrace::Close,
                    template: "l}".into(),
                },
                "xcs.header: nespárovaná `}` v šablóne `l}`",
            ),
            (
                P::SheetNameVars,
                "nesting.sheet_name musí obsahovať `{sheet}` a `{mat}` alebo `{material}` (jedinečné mená tabúľ)",
            ),
            (
                P::NegativeValue {
                    name: "nesting.route.skin".into(),
                    value: -0.5,
                },
                "`nesting.route.skin` musí byť ≥ 0 (je -0.5)",
            ),
            (
                P::SheetSize {
                    name: "nesting.layout".into(),
                    max: 20_000.0,
                },
                "nesting.layout: rozmer tabule musí byť > 0 a ≤ 20000 mm",
            ),
            (
                P::TooManyDecimals {
                    name: "xcs.decimals".into(),
                    value: 9,
                },
                "xcs.decimals: max 6 (je 9)",
            ),
            (
                P::VariantMaxDepth { diameter: 8.0 },
                "nesting.drill.variant Ø8: max_depth musí byť > 0",
            ),
        ];
        for (p, text) in cases {
            assert_eq!(template_problem(&p), text);
        }
    }

    #[test]
    fn missing_material_is_reported() {
        let project: ProjectFile = toml::from_str(
            r#"
            [project]
            name = "x"
            [defaults]
            panel_material = "nie"
            back_material = "nie"
            construction = "frameless32"
            [[cabinet]]
            id = "S1"
            type = "base"
            w = 600
            h = 720
            d = 560
            "#,
        )
        .unwrap();
        let problems = validate_project(&project, &MaterialLibrary::default());
        assert_eq!(problems.len(), 2, "{problems:?}");
    }
}
