//! Štítky dielov po tabuliach (findings 14): číslo dielu = poradie prerezania obrysu na tabuli,
//! poradie tlače tabule podľa `[labels] order` v profile stroja, rozmer štítku `[labels]`.
//!
//! To isté používa výstup (`labels/stitky.pdf`) aj okno (pohľad na tabuľu a tlač štítkov ako
//! program +Label v CV).

use std::collections::BTreeMap;

use cc_model::{Face, LabelOrder, LabelSettings, MachineProfile, Model};
use cc_out_pdf::{LabelInfo, NestInfo};
use cc_out_xcs::nest::NestSheetFile;

/// Diel na tabuli.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetPart {
    pub part_id: String,
    /// Číslo na štítku: poradie prerezania obrysu na tabuli (od 1).
    pub number: usize,
    /// Ľavý dolný roh na tabuli a rozmer na tabuli (po otočení).
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub rotated: bool,
    pub face_up: Face,
}

/// Tabuľa nestingu.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelSheet {
    /// Program tabule bez prípony, napr. `0101N`.
    pub name: String,
    pub material: String,
    pub thickness: f64,
    pub length: f64,
    pub width: f64,
    /// Poradie tabule v materiáli (od 1) a počet tabúľ materiálu.
    pub index: usize,
    pub count: usize,
    /// Diely v poradí tlače štítkov.
    pub parts: Vec<SheetPart>,
}

/// Štítky zákazky: tabule a štítky v poradí tlače (tabule, potom diely mimo nestingu).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelJob {
    pub sheets: Vec<LabelSheet>,
    pub labels: Vec<LabelInfo>,
    /// Rozmer štítku (mm).
    pub size: (f64, f64),
    pub order: LabelOrder,
}

impl LabelJob {
    /// Štítky tabule `sheet` (v poradí tlače).
    pub fn sheet_labels(&self, sheet: &str) -> Vec<LabelInfo> {
        let Some(s) = self.sheets.iter().find(|s| s.name == sheet) else {
            return Vec::new();
        };
        s.parts
            .iter()
            .filter_map(|p| self.labels.iter().find(|l| l.part.id == p.part_id))
            .cloned()
            .collect()
    }

    /// Štítky vybraných dielov (v poradí `ids`).
    pub fn part_labels(&self, ids: &[String]) -> Vec<LabelInfo> {
        ids.iter()
            .filter_map(|id| self.labels.iter().find(|l| &l.part.id == id))
            .cloned()
            .collect()
    }
}

/// Tabule z výstupu nestingu, diely v poradí `order`. Pri `offcut_number` sa odrezaný zvyšok
/// tabule počíta ako prvý v poradí (diely od 2, ako CV, findings 14 §6.2).
pub fn label_sheets(
    sheets: &[NestSheetFile],
    order: LabelOrder,
    offcut_number: bool,
) -> Vec<LabelSheet> {
    let mut out = Vec::new();
    for s in sheets {
        let same: Vec<&NestSheetFile> = sheets
            .iter()
            .filter(|o| o.material == s.material && o.thickness == s.thickness)
            .collect();
        let index = same
            .iter()
            .position(|o| std::ptr::eq(*o, s))
            .map_or(1, |i| i + 1);
        let first = if offcut_number && s.remnant_cut { 2 } else { 1 };
        let mut parts: Vec<SheetPart> = s
            .placements
            .iter()
            .map(|p| SheetPart {
                part_id: p.part_id.clone(),
                number: s
                    .cut_order
                    .iter()
                    .position(|id| id == &p.part_id)
                    .map_or(0, |i| i + first),
                x: p.x,
                y: p.y,
                dx: p.dx,
                dy: p.dy,
                rotated: p.rotated,
                face_up: p.face_up,
            })
            .collect();
        parts.sort_by(|a, b| {
            let (ka, kb) = (order.key(a.x, a.y, a.number), order.key(b.x, b.y, b.number));
            ka.0.total_cmp(&kb.0)
                .then(ka.1.total_cmp(&kb.1))
                .then(ka.2.cmp(&kb.2))
        });
        out.push(LabelSheet {
            name: s
                .name
                .rsplit_once('.')
                .map_or(s.name.as_str(), |(n, _)| n)
                .to_owned(),
            material: s.material.clone(),
            thickness: s.thickness,
            length: s.sheet_length,
            width: s.sheet_width,
            index,
            count: same.len(),
            parts,
        });
    }
    out
}

/// Štítky v poradí tlače. `programs`: diel → mená programov (aj programy tabúľ, tie sa vynechajú).
pub fn build_label_job(
    model: &Model,
    programs: &BTreeMap<String, String>,
    sheets: Vec<LabelSheet>,
    settings: &LabelSettings,
) -> LabelJob {
    let sheet_files: std::collections::BTreeSet<String> = sheets
        .iter()
        .map(|s| format!("{}.xcs", s.name).to_ascii_lowercase())
        .collect();
    let mut by_id: BTreeMap<&str, LabelInfo> = BTreeMap::new();
    let mut model_order: Vec<&str> = Vec::new();
    for cabinet in &model.cabinets {
        for (i, part) in cabinet.parts.iter().enumerate() {
            let cx100: Vec<String> = programs
                .get(&part.id)
                .map(|names| {
                    names
                        .split(',')
                        .filter(|n| {
                            let n = n.to_ascii_lowercase();
                            n.ends_with(".xcs") && !sheet_files.contains(&n)
                        })
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            model_order.push(part.id.as_str());
            by_id.insert(
                part.id.as_str(),
                LabelInfo {
                    project: model.project.clone(),
                    part: part.clone(),
                    number: i + 1,
                    programs: cx100,
                    nesting: None,
                },
            );
        }
    }
    let mut labels = Vec::new();
    let mut done: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for s in &sheets {
        for p in &s.parts {
            if let Some(mut l) = by_id.get(p.part_id.as_str()).cloned() {
                l.number = p.number;
                l.nesting = Some(NestInfo {
                    sheet: s.name.clone(),
                    face_up: p.face_up,
                });
                labels.push(l);
                done.insert(p.part_id.clone());
            }
        }
    }
    for id in model_order {
        if !done.contains(id) {
            if let Some(l) = by_id.get(id) {
                labels.push(l.clone());
            }
        }
    }
    LabelJob {
        sheets,
        labels,
        size: settings.size(),
        order: settings.order,
    }
}

/// Štítky zákazky pre okno: nesting a programy sa vypočítajú znova z modelu a profilu stroja.
pub fn label_job(model: &Model, machine: Option<&MachineProfile>) -> LabelJob {
    let settings = machine.map(|m| m.labels.clone()).unwrap_or_default();
    let programs = crate::program_names(model, machine);
    let sheets = crate::nesting_profiles(machine)
        .and_then(|(xcs, nest)| cc_out_xcs::nest::render_nesting(model, xcs, nest).ok())
        .map(|o| label_sheets(&o.sheets, settings.order, settings.offcut_number))
        .unwrap_or_default();
    build_label_job(model, &programs, sheets, &settings)
}
