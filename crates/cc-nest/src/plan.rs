//! Plán výroby na nestingu: rozloženie dielov po materiáloch a delenie obrábania.
//!
//! **Delenie práce** (findings 08, §3):
//! - nesting: obrys dielu (a onion skin) a **vŕtanie kolmé na plochu, ktorá je hore**,
//! - CX100: vŕtanie do hrán, **všetky drážky** (v modeli sú to drážky pílou, nesting pílu
//!   nemá) a obrábanie **plochy, ktorá je na nestingu dole**,
//! - materiály zo `skip_materials` (delia sa na píle) a diely, ktoré sa na tabuľu
//!   nezmestia: všetko obrábanie na CX100 (ako v režime `cx100`),
//! - diel bez práce pre CX100 nemá program CX100.
//!
//! **Plocha hore na nestingu** (po dieloch), v tomto poradí:
//! 1. menej programov CX100 (program pre plochu B je samostatný, ako v `cc-out-xcs`),
//! 2. viac dier urobených na nestingu,
//! 3. pri zhode plocha **F** (interiér; v S2M je tiež predvolená).
//!
//! Súradnice: diel s plochou B hore je otočený okolo svojej osi y (`x' = L − x`, ako
//! program `_B` pre CX100), potom prípadne otočený o 90° proti smeru hodín.

use std::collections::BTreeMap;

use cc_model::{Face, Grain, Model, NestingProfile, Operation, Part};

use crate::pack::{self, Item, Rotation, SheetSpec};

/// Najviac dielov v projekte pre nesting.
pub const MAX_PARTS: usize = 5000;

#[derive(Debug, thiserror::Error)]
pub enum NestError {
    #[error("príliš veľa dielov pre nesting ({0}, max. {MAX_PARTS})")]
    TooManyParts(usize),
}

/// Kde sa diel vyrába.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartRoute {
    /// Obrys a plocha hore na nestingu.
    Nesting,
    /// Materiál sa na nesting nedáva (píla); všetko obrábanie na CX100.
    Saw,
    /// Diel sa na tabuľu nezmestí; všetko obrábanie na CX100, obrys inde.
    Unplaced,
}

/// Plán pre jeden diel.
#[derive(Debug, Clone, PartialEq)]
pub struct PartPlan {
    pub route: PartRoute,
    /// Plocha hore na nestingu (F alebo B).
    pub face_up: Face,
    /// Operácie na nestingu (iba vŕtanie plochy `face_up`).
    pub nest_ops: Vec<Operation>,
    /// Operácie pre CX100.
    pub cx100_ops: Vec<Operation>,
}

/// Diel na tabuli.
#[derive(Debug, Clone, PartialEq)]
pub struct PartPlacement {
    pub part_id: String,
    /// Ľavý dolný roh na tabuli.
    pub x: f64,
    pub y: f64,
    /// Rozmer na tabuli (po otočení).
    pub dx: f64,
    pub dy: f64,
    /// Otočený o 90° proti smeru hodín (dĺžka dielu po Y tabule).
    pub rotated: bool,
    pub face_up: Face,
    /// Rozmer dielu `length × width`.
    pub length: f64,
    pub width: f64,
}

impl PartPlacement {
    /// Bod v súradniciach dielu (pri pohľade na plochu F) → bod na tabuli.
    pub fn to_sheet(&self, x: f64, y: f64) -> (f64, f64) {
        let x = if self.face_up == Face::B {
            self.length - x
        } else {
            x
        };
        if self.rotated {
            (self.x + (self.width - y), self.y + x)
        } else {
            (self.x + x, self.y + y)
        }
    }

    pub fn area(&self) -> f64 {
        self.length * self.width
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SheetLayout {
    /// Zoradené zdola nahor, zľava doprava.
    pub parts: Vec<PartPlacement>,
}

impl SheetLayout {
    /// Najvyššia hrana dielov.
    pub fn top(&self) -> f64 {
        self.parts.iter().map(|p| p.y + p.dy).fold(0.0, f64::max)
    }

    pub fn used_area(&self) -> f64 {
        self.parts.iter().map(PartPlacement::area).sum()
    }
}

/// Tabule jedného materiálu a hrúbky.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetGroup {
    /// Poradové číslo materiálu (od 1) pre mená programov.
    pub number: usize,
    pub material: String,
    pub thickness: f64,
    /// Vlákno materiálu (podľa prvého dielu skupiny).
    pub grain: Grain,
    pub sheet_length: f64,
    pub sheet_width: f64,
    pub sheets: Vec<SheetLayout>,
}

impl SheetGroup {
    /// Využitie tabúľ skupiny (0–1).
    pub fn utilisation(&self) -> f64 {
        let total = self.sheets.len() as f64 * self.sheet_length * self.sheet_width;
        if total <= 0.0 {
            0.0
        } else {
            self.sheets.iter().map(SheetLayout::used_area).sum::<f64>() / total
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NestPlan {
    pub groups: Vec<SheetGroup>,
    /// Plán pre každý diel modelu (podľa ID).
    pub parts: BTreeMap<String, PartPlan>,
    /// Diely, ktoré sa na tabuľu nezmestili.
    pub unplaced: Vec<String>,
}

impl NestPlan {
    pub fn sheet_count(&self) -> usize {
        self.groups.iter().map(|g| g.sheets.len()).sum()
    }

    /// Model s operáciami iba pre CX100 (diely bez plánu ostanú bez zmeny).
    pub fn cx100_model(&self, model: &Model) -> Model {
        let mut m = model.clone();
        for cab in &mut m.cabinets {
            for part in &mut cab.parts {
                if let Some(plan) = self.parts.get(&part.id) {
                    part.operations = plan.cx100_ops.clone();
                }
            }
        }
        m
    }
}

fn is_face(face: Face) -> bool {
    matches!(face, Face::F | Face::B)
}

fn op_face(op: &Operation) -> Face {
    match op {
        Operation::Hole { face, .. } | Operation::Groove { face, .. } => *face,
    }
}

/// Patrí operácia nestingu, ak je plocha `up` hore?
fn on_nesting(op: &Operation, up: Face) -> bool {
    matches!(op, Operation::Hole { face, .. } if *face == up)
}

/// Počet programov CX100 pre zvyšnú prácu (rovnaké delenie ako `cc-out-xcs`).
fn cx100_programs(ops: &[&Operation]) -> usize {
    let b = ops.iter().any(|op| op_face(op) == Face::B);
    let other = ops.iter().any(|op| op_face(op) != Face::B);
    usize::from(b) + usize::from(other)
}

/// Plocha hore na nestingu (pravidlo v hlavičke modulu).
pub fn choose_face_up(part: &Part) -> Face {
    let mut best: Option<((usize, usize), Face)> = None;
    for up in [Face::F, Face::B] {
        let rest: Vec<&Operation> = part
            .operations
            .iter()
            .filter(|op| !on_nesting(op, up))
            .collect();
        let nest_holes = part.operations.len() - rest.len();
        // menšie je lepšie: programy CX100, potom záporný počet dier na nestingu
        let key = (cx100_programs(&rest), usize::MAX - nest_holes);
        if best.as_ref().is_none_or(|(k, _)| key < *k) {
            best = Some((key, up));
        }
    }
    best.map_or(Face::F, |(_, f)| f)
}

fn rotation_for(grain: Grain) -> Rotation {
    match grain {
        Grain::None => Rotation::Free,
        Grain::Length => Rotation::Fixed,
        Grain::Width => Rotation::Rotated,
    }
}

/// Je diel malý (onion skin)?
pub fn is_small(length: f64, width: f64, profile: &NestingProfile) -> bool {
    let r = &profile.route;
    length * width < r.small_area * 1e6 || length.min(width) < r.small_min_dim
}

/// Rozloží diely po materiáloch na tabule a rozdelí obrábanie medzi nesting a CX100.
pub fn plan(model: &Model, profile: &NestingProfile) -> Result<NestPlan, NestError> {
    let count = model.parts().count();
    if count > MAX_PARTS {
        return Err(NestError::TooManyParts(count));
    }
    let mut out = NestPlan::default();

    // skupiny (materiál, hrúbka v µm) v poradí podľa mena materiálu
    let mut groups: BTreeMap<(String, i64), Vec<&Part>> = BTreeMap::new();
    for part in model.parts() {
        let all_on_cx100 = PartPlan {
            route: PartRoute::Saw,
            face_up: Face::F,
            nest_ops: Vec::new(),
            cx100_ops: part.operations.clone(),
        };
        if !profile.nests_material(&part.material) {
            out.parts.insert(part.id.clone(), all_on_cx100);
            continue;
        }
        let key = (
            part.material.clone(),
            (part.thickness * 1000.0).round() as i64,
        );
        groups.entry(key).or_default().push(part);
    }

    for (number, ((material, _), parts)) in groups.into_iter().enumerate() {
        let (sheet_length, sheet_width) = profile.sheet_size(&material);
        let spec = SheetSpec {
            length: sheet_length,
            width: sheet_width,
            margin: profile.layout.margin,
            spacing: profile.layout.spacing,
        };
        let items: Vec<Item> = parts
            .iter()
            .map(|p| Item {
                length: p.length,
                width: p.width,
                rotation: rotation_for(p.grain),
            })
            .collect();
        let packing = pack::pack(&items, &spec);

        for &idx in &packing.unplaced {
            if let Some(p) = parts.get(idx) {
                out.unplaced.push(p.id.clone());
                out.parts.insert(
                    p.id.clone(),
                    PartPlan {
                        route: PartRoute::Unplaced,
                        face_up: Face::F,
                        nest_ops: Vec::new(),
                        cx100_ops: p.operations.clone(),
                    },
                );
            }
        }

        let mut sheets = Vec::new();
        for packed in &packing.sheets {
            let mut layout = SheetLayout::default();
            for pl in &packed.placed {
                let Some(part) = parts.get(pl.item) else {
                    continue;
                };
                let face_up = choose_face_up(part);
                let (nest_ops, cx100_ops): (Vec<Operation>, Vec<Operation>) = part
                    .operations
                    .iter()
                    .cloned()
                    .partition(|op| on_nesting(op, face_up));
                out.parts.insert(
                    part.id.clone(),
                    PartPlan {
                        route: PartRoute::Nesting,
                        face_up,
                        nest_ops,
                        cx100_ops,
                    },
                );
                layout.parts.push(PartPlacement {
                    part_id: part.id.clone(),
                    x: pl.x,
                    y: pl.y,
                    dx: pl.dx,
                    dy: pl.dy,
                    rotated: pl.rotated,
                    face_up,
                    length: part.length,
                    width: part.width,
                });
            }
            sheets.push(layout);
        }
        let thickness = parts.first().map_or(0.0, |p| p.thickness);
        let grain = parts.first().map_or(Grain::None, |p| p.grain);
        out.groups.push(SheetGroup {
            number: number + 1,
            material,
            thickness,
            grain,
            sheet_length,
            sheet_width,
            sheets,
        });
    }
    Ok(out)
}

/// Pomocník pre testy a výpisy: strana je plocha (F/B), nie hrana.
pub fn is_face_op(op: &Operation) -> bool {
    is_face(op_face(op))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_model::{CabinetModel, CabinetType, Edges, Role, Transform, P2};
    use glam::DVec3;

    fn hole(face: Face, x: f64, y: f64) -> Operation {
        Operation::Hole {
            face,
            x,
            y,
            z: 9.0,
            diameter: 8.0,
            depth: 13.0,
            through: false,
            dowel: true,
        }
    }

    fn groove(face: Face) -> Operation {
        Operation::Groove {
            face,
            start: P2 { x: 0.0, y: 300.0 },
            end: P2 { x: 600.0, y: 300.0 },
            width: 3.5,
            depth: 9.0,
        }
    }

    fn part(id: &str, material: &str, l: f64, w: f64, ops: Vec<Operation>) -> Part {
        Part {
            id: id.into(),
            cabinet: "S".into(),
            role: Role::Bottom,
            material: material.into(),
            length: l,
            width: w,
            thickness: 18.0,
            grain: Grain::None,
            edges: Edges::default(),
            transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
            operations: ops,
        }
    }

    fn model(parts: Vec<Part>) -> Model {
        Model {
            project: "T".into(),
            cabinets: vec![CabinetModel {
                id: "S".into(),
                cabinet_type: CabinetType::Base,
                w: 600.0,
                h: 720.0,
                d: 560.0,
                parts,
                hardware: Vec::new(),
                options: Default::default(),
                openings: Vec::new(),
            }],
        }
    }

    fn profile() -> NestingProfile {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/dielna/machine-nesting.toml");
        cc_config::load_machine(&path).unwrap().nesting.unwrap()
    }

    #[test]
    fn face_up_prefers_face_with_holes_and_fewer_cx100_programs() {
        // iba diery na B → B hore, CX100 nič
        let p = part("A", "M", 600.0, 400.0, vec![hole(Face::B, 50.0, 50.0)]);
        assert_eq!(choose_face_up(&p), Face::B);
        // bez operácií → F
        assert_eq!(
            choose_face_up(&part("B", "M", 600.0, 400.0, vec![])),
            Face::F
        );
        // diery na F aj B a hrana: F hore = 2 programy CX100 (B + hrany), B hore = 1
        let p = part(
            "C",
            "M",
            600.0,
            400.0,
            vec![
                hole(Face::F, 50.0, 50.0),
                hole(Face::F, 80.0, 50.0),
                hole(Face::B, 50.0, 50.0),
                hole(Face::X0, 0.0, 50.0),
            ],
        );
        assert_eq!(choose_face_up(&p), Face::B);
        // drážka na B (vždy CX100) a diera na F → F hore
        let p = part(
            "D",
            "M",
            600.0,
            400.0,
            vec![hole(Face::F, 50.0, 50.0), groove(Face::B)],
        );
        assert_eq!(choose_face_up(&p), Face::F);
        // rovnaký počet programov → viac dier na nestingu
        let p = part(
            "E",
            "M",
            600.0,
            400.0,
            vec![
                hole(Face::F, 50.0, 50.0),
                hole(Face::B, 50.0, 50.0),
                hole(Face::B, 90.0, 50.0),
            ],
        );
        assert_eq!(choose_face_up(&p), Face::B);
    }

    #[test]
    fn split_saw_material_and_unplaced() {
        let prof = profile();
        let m = model(vec![
            part(
                "N",
                "DTDL",
                600.0,
                400.0,
                vec![
                    hole(Face::F, 50.0, 50.0),
                    hole(Face::X0, 0.0, 40.0),
                    groove(Face::F),
                ],
            ),
            part(
                "H",
                "HDF_3_biela",
                600.0,
                400.0,
                vec![hole(Face::F, 50.0, 50.0)],
            ),
            part(
                "BIG",
                "DTDL",
                3000.0,
                2500.0,
                vec![hole(Face::F, 50.0, 50.0)],
            ),
        ]);
        let plan = plan(&m, &prof).unwrap();
        let n = &plan.parts["N"];
        assert_eq!(n.route, PartRoute::Nesting);
        assert_eq!(n.face_up, Face::F);
        assert_eq!(n.nest_ops, vec![hole(Face::F, 50.0, 50.0)]);
        assert_eq!(
            n.cx100_ops,
            vec![hole(Face::X0, 0.0, 40.0), groove(Face::F)]
        );
        let h = &plan.parts["H"];
        assert_eq!(h.route, PartRoute::Saw);
        assert_eq!(h.cx100_ops.len(), 1);
        assert_eq!(plan.parts["BIG"].route, PartRoute::Unplaced);
        assert_eq!(plan.unplaced, vec!["BIG".to_owned()]);
        assert_eq!(plan.sheet_count(), 1);
        let cx = plan.cx100_model(&m);
        let ops: Vec<usize> = cx.parts().map(|p| p.operations.len()).collect();
        assert_eq!(ops, vec![2, 1, 1]);
    }

    #[test]
    fn flipped_and_rotated_parts_map_to_sheet() {
        let p = PartPlacement {
            part_id: "A".into(),
            x: 100.0,
            y: 200.0,
            dx: 600.0,
            dy: 400.0,
            rotated: false,
            face_up: Face::B,
            length: 600.0,
            width: 400.0,
        };
        assert_eq!(p.to_sheet(50.0, 30.0), (650.0, 230.0));
        let r = PartPlacement {
            rotated: true,
            face_up: Face::F,
            dx: 400.0,
            dy: 600.0,
            ..p
        };
        assert_eq!(r.to_sheet(50.0, 30.0), (470.0, 250.0));
    }

    #[test]
    fn too_many_parts_is_an_error() {
        let parts: Vec<Part> = (0..MAX_PARTS + 1)
            .map(|i| part(&format!("P{i}"), "DTDL", 100.0, 100.0, vec![]))
            .collect();
        assert!(matches!(
            plan(&model(parts), &profile()),
            Err(NestError::TooManyParts(_))
        ));
    }
}
