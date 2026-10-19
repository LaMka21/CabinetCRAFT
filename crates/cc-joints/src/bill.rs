//! Súpis kovania skriniek (ako súpis v CV): pánty, zásuvky, kolíky, konfirmáty, nožičky,
//! podperky políc, tlmiace čočky, TIP-ON. Počíta sa z modelu s operáciami a štandardu.

use std::collections::BTreeMap;

use cc_i18n::tr;
use cc_model::{
    CabinetModel, CabinetType, Face, Hardware, JointType, Model, Operation, Role, ShelfSupport,
    Standard, TipOn, TipOnColor, TipOnKind,
};
use serde::Serialize;

/// Riadok súpisu: meno a počet kusov.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BillLine {
    pub name: String,
    pub qty: u32,
}

/// Súpis kovania celého modelu (poradie: pánty, zásuvky, spoje, nožičky, police, čelá).
pub fn hardware_bill(model: &Model, standard: &Standard) -> Vec<BillLine> {
    // (poradie skupiny, meno) → počet
    let mut lines: BTreeMap<(u8, String), u32> = BTreeMap::new();
    for cab in &model.cabinets {
        for (order, name, qty) in cabinet_bill(cab, standard) {
            if qty > 0 {
                *lines.entry((order, name)).or_default() += qty;
            }
        }
    }
    lines
        .into_iter()
        .map(|((_, name), qty)| BillLine { name, qty })
        .collect()
}

/// Kovanie jednej skrinky: (skupina, meno, počet).
pub fn cabinet_bill(cab: &CabinetModel, standard: &Standard) -> Vec<(u8, String, u32)> {
    let names = &standard.fittings;
    let o = &cab.options;
    let mut out = Vec::new();
    for hw in &cab.hardware {
        match hw {
            Hardware::Hinge { hinge, .. } => {
                let name = standard
                    .hinge(hinge)
                    .map_or_else(|| hinge.clone(), |h| h.name.clone());
                out.push((0, name, 1));
            }
            Hardware::Drawer {
                drawer,
                length,
                box_height,
                ..
            } => {
                let name = standard
                    .drawer(drawer)
                    .map_or_else(|| drawer.clone(), |d| d.name.clone());
                out.push((
                    1,
                    tr!(
                        "joints.bill.drawer",
                        name = name,
                        length = mm(*length),
                        height = mm(*box_height)
                    ),
                    1,
                ));
            }
            // kolíky lišty blendy sa počítajú medzi kolíky nižšie
            Hardware::Filler { .. } => {}
        }
    }
    // kolík = diera v hrane; konfirmát v strede každej dvojice kolíkov korpusu
    // (kolíky bočného sokla konfirmát nemajú)
    let edge_dowels = |loose: bool| {
        cab.parts
            .iter()
            .filter(|p| p.role.is_loose() == loose)
            .flat_map(|p| &p.operations)
            .filter(|op| {
                matches!(op, Operation::Hole { dowel: true, face, .. }
                    if !matches!(face, Face::F | Face::B))
            })
            .count() as u32
    };
    let carcass_dowels = edge_dowels(false);
    let dowels = carcass_dowels + edge_dowels(true);
    out.push((2, names.dowel.clone(), dowels));
    // konfirmáty iba medzi kolíkmi korpusu (blenda, sokel a krycí bok sú voľné diely, ich
    // kolíky sa do `carcass_dowels` nepočítajú)
    match o.joint_type {
        JointType::Confirmat if o.joints => {
            out.push((3, names.confirmat.clone(), carcass_dowels / 2))
        }
        JointType::Screw if o.joints => out.push((3, names.screw.clone(), carcass_dowels / 2)),
        _ => {}
    }
    let has_carcass = cab.parts.iter().any(|p| p.role == Role::SideLeft);
    if o.legs && has_carcass && matches!(cab.cabinet_type, CabinetType::Base | CabinetType::Tall) {
        out.push((4, names.leg.clone(), standard.legs.count(cab.w)));
    }
    if o.shelf_support == ShelfSupport::Sekura7 {
        let shelves = cab
            .parts
            .iter()
            .filter(|p| p.role == Role::AdjustableShelf)
            .count() as u32;
        out.push((5, names.shelf_support.clone(), 4 * shelves));
    }
    let doors = cab.parts.iter().filter(|p| p.role == Role::Door).count() as u32;
    if o.bumpers {
        let small = standard.bumpers.small_front;
        let fronts: u32 = cab
            .parts
            .iter()
            .map(|p| match p.role {
                Role::Door => 2,
                Role::DrawerFront if p.length <= small => 2,
                Role::DrawerFront => 4,
                _ => 0,
            })
            .sum();
        out.push((6, names.bumper.clone(), fronts));
    }
    if o.tip_on != TipOn::Off {
        let kind = match o.tip_on_kind {
            TipOnKind::Stop => &names.tip_on_stop,
            TipOnKind::Magnet => &names.tip_on_magnet,
        };
        let color = match o.tip_on_color {
            TipOnColor::White => tr!("joints.bill.white"),
            TipOnColor::Grey => tr!("joints.bill.grey"),
            TipOnColor::Black => tr!("joints.bill.black"),
        };
        let name = if o.tip_on == TipOn::Adapter {
            tr!("joints.bill.tip_on_adapter", kind = kind, color = color)
        } else {
            tr!("joints.bill.tip_on", kind = kind, color = color)
        };
        out.push((7, name, doors));
    }
    out
}

fn mm(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}
