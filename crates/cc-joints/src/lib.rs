//! Spoje → operácie obrábania (špecifikácia MVP, kap. 6).
//!
//! Operácie sa najprv počítajú v súradniciach skrinky (bod vstupu, smer
//! nástroja do materiálu) a potom sa prevedú do súradníc dielu. Strana
//! obrábania sa odvodí zo smeru nástroja (kap. 3.4).

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

mod blind;
mod filler;
mod placement;

use std::collections::{BTreeSet, HashMap};

pub mod bill;

use cc_model::{
    CabinetModel, CabinetType, ConnectionKind, EndGroups, Face, Hardware, Issue, IssueCode,
    JointRule, JointType, LineBoreLayout, Model, Operation, Part, Role, ShelfSupport, Standard, P2,
};

pub use bill::{hardware_bill, BillLine};
use cc_i18n::tr;
use glam::DVec3;

pub use placement::{
    dowel_positions, dowel_positions_even, dowel_positions_pairs, joint_positions,
    line_bore_heights, shelf_hole_heights,
};

#[derive(Debug, thiserror::Error)]
pub enum JointError {
    /// `code` je kód pravidla z kap. 8 (`JointTooShort`, `UnsupportedToolAxis`), ak existuje.
    #[error("{}", error_message(.code, .cabinet, .message))]
    Invalid {
        cabinet: String,
        code: Option<IssueCode>,
        message: String,
    },
}

impl JointError {
    pub fn code(&self) -> Option<IssueCode> {
        match self {
            JointError::Invalid { code, .. } => *code,
        }
    }
}

fn code_prefix(code: &Option<IssueCode>) -> String {
    code.map(|c| format!("[{c}] ")).unwrap_or_default()
}

/// Text chyby: kód pravidla (ak je), skrinka a správa.
fn error_message(code: &Option<IssueCode>, cabinet: &str, message: &str) -> String {
    let text = tr!("joints.error", cabinet = cabinet, message = message);
    format!("{}{text}", code_prefix(code))
}

/// Chyba vnútorných funkcií: správa a voliteľný kód pravidla.
struct Fail {
    code: Option<IssueCode>,
    message: String,
}

impl From<String> for Fail {
    fn from(message: String) -> Self {
        Fail {
            code: None,
            message,
        }
    }
}

impl From<&str> for Fail {
    fn from(message: &str) -> Self {
        Fail::from(message.to_owned())
    }
}

/// Výsledok: model s operáciami a varovania (napr. dotyk bez pravidla, `JointRuleMissing`).
#[derive(Debug, Clone)]
pub struct Applied {
    pub model: Model,
    pub warnings: Vec<Issue>,
}

/// Doplní operácie do všetkých dielov modelu podľa pravidiel štandardu.
pub fn apply(mut model: Model, standard: &Standard) -> Result<Applied, JointError> {
    let rules = RuleTable::new(standard);
    let mut warnings = Vec::new();
    for cabinet in &mut model.cabinets {
        apply_cabinet(cabinet, standard, &rules, &mut warnings)?;
    }
    Ok(Applied { model, warnings })
}

/// Strana obrábania zo smeru nástroja v súradniciach dielu (kap. 3.4).
pub fn face_from_tool_axis(v: DVec3) -> Option<Face> {
    let tol = 1e-6;
    let is = |target: DVec3| (v - target).length() < tol;
    [
        (DVec3::NEG_Z, Face::F),
        (DVec3::Z, Face::B),
        (DVec3::X, Face::X0),
        (DVec3::NEG_X, Face::X1),
        (DVec3::Y, Face::Y0),
        (DVec3::NEG_Y, Face::Y1),
    ]
    .into_iter()
    .find(|(t, _)| is(*t))
    .map(|(_, f)| f)
}

/// Neusporiadaná dvojica mien rolí → pravidlo spoja.
struct RuleTable<'a>(HashMap<(&'static str, &'static str), &'a JointRule>);

impl<'a> RuleTable<'a> {
    fn new(standard: &'a Standard) -> Self {
        let mut map = HashMap::new();
        for rule in &standard.joint_rules {
            for a in rule.a.items() {
                for b in rule.b.items() {
                    if let (Some(a), Some(b)) = (static_name(a), static_name(b)) {
                        map.insert(key(a, b), rule);
                    }
                }
            }
        }
        RuleTable(map)
    }

    fn rule(&self, a: Role, b: Role) -> Option<&'a JointRule> {
        self.0.get(&key(a.rule_name(), b.rule_name())).copied()
    }

    fn get(&self, a: Role, b: Role) -> Option<ConnectionKind> {
        self.rule(a, b).map(|r| r.connection)
    }

    /// Odsadenie krajného kolíka pre dvojicu (pravidlo spoja), ak ho pravidlo má.
    fn end_offset(&self, a: Role, b: Role) -> Option<f64> {
        self.rule(a, b)
            .and_then(|r| r.end_offset)
            .filter(|v| v.is_finite() && *v > 0.0)
    }

    /// Skupiny kolíkov pri koncoch hrany (pravidlo spoja), ak ich pravidlo má.
    fn end_groups(&self, a: Role, b: Role) -> Option<&'a EndGroups> {
        self.rule(a, b).and_then(|r| r.end_groups.as_ref())
    }
}

fn static_name(name: &str) -> Option<&'static str> {
    Role::RULE_NAMES.iter().copied().find(|n| *n == name)
}

fn key(a: &'static str, b: &'static str) -> (&'static str, &'static str) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Os-zarovnaný kváder dielu v súradniciach skrinky.
#[derive(Debug, Clone, Copy)]
struct Bbox {
    min: DVec3,
    max: DVec3,
}

impl Bbox {
    fn of(p: &Part) -> Bbox {
        let a = p.transform.point_to_cabinet(DVec3::ZERO);
        let b = p
            .transform
            .point_to_cabinet(DVec3::new(p.length, p.width, p.thickness));
        Bbox {
            min: a.min(b),
            max: a.max(b),
        }
    }

    fn center(&self) -> DVec3 {
        (self.min + self.max) / 2.0
    }
}

/// Index osi skrinky (0 = X, 1 = Y, 2 = Z), s ktorou je rovnobežná os `v`.
fn axis_index(v: DVec3) -> usize {
    let a = v.abs();
    if a.x >= a.y && a.x >= a.z {
        0
    } else if a.y >= a.z {
        1
    } else {
        2
    }
}

fn unit(axis: usize) -> DVec3 {
    match axis {
        0 => DVec3::X,
        1 => DVec3::Y,
        _ => DVec3::Z,
    }
}

/// Dotyk dvoch kvádrov: jedna os s nulovou medzerou, na ostatných kladný prienik.
struct Contact {
    axis: usize,
    plane: f64,
    /// Prienik na ostatných osiach (index osi → interval).
    overlap: [(f64, f64); 3],
}

/// Kvádre sa prekrývajú objemom (nielen dotykom plôch).
fn overlaps(a: &Bbox, b: &Bbox) -> bool {
    let tol = 1e-6;
    (0..3).all(|k| a.min[k] < b.max[k] - tol && b.min[k] < a.max[k] - tol)
}

#[allow(clippy::needless_range_loop)] // indexuje min/max oboch kvádrov aj prienik
fn contact(a: &Bbox, b: &Bbox) -> Option<Contact> {
    let tol = 1e-6;
    let mut touch_axis = None;
    let mut overlap = [(0.0, 0.0); 3];
    for k in 0..3 {
        let lo = a.min[k].max(b.min[k]);
        let hi = a.max[k].min(b.max[k]);
        overlap[k] = (lo, hi);
        let len = hi - lo;
        if len.abs() <= tol {
            if touch_axis.is_some() {
                return None; // dotyk iba v hrane alebo v bode
            }
            touch_axis = Some(k);
        } else if len < 0.0 {
            return None; // medzera
        }
    }
    let axis = touch_axis?;
    Some(Contact {
        axis,
        plane: overlap[axis].0,
        overlap,
    })
}

/// Vložená operácia s bodom vstupu a smerom v súradniciach skrinky.
struct CabHole {
    entry: DVec3,
    dir: DVec3,
    diameter: f64,
    depth: f64,
    dowel: bool,
}

fn to_part_hole(part: &Part, h: &CabHole) -> Result<Operation, Fail> {
    let p = part.transform.point_to_part(h.entry);
    let v = part.transform.dir_to_part(h.dir);
    let face = face_from_tool_axis(v).ok_or_else(|| Fail {
        code: Some(IssueCode::UnsupportedToolAxis),
        message: tr!(
            "joints.unsupported_axis",
            id = &part.id,
            axis = format!("{v:?}")
        ),
    })?;
    Ok(Operation::Hole {
        face,
        x: clean(p.x),
        y: clean(p.y),
        z: clean(p.z),
        diameter: h.diameter,
        depth: h.depth,
        through: false,
        dowel: h.dowel,
    })
}

/// Osi dielu sú rovnobežné s osami skrinky (spoje, drážky a kovanie vedia iba také diely).
fn axis_aligned(p: &Part) -> bool {
    [p.transform.x_axis, p.transform.y_axis, p.transform.z_axis]
        .iter()
        .all(|a| a.abs().max_element() > 1.0 - 1e-9)
}

/// Zaokrúhli numerický šum (napr. 9.000000000000002 → 9).
fn clean(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn apply_cabinet(
    cab: &mut CabinetModel,
    standard: &Standard,
    rules: &RuleTable,
    warnings: &mut Vec<Issue>,
) -> Result<(), JointError> {
    let invalid = |fail: Fail| JointError::Invalid {
        cabinet: cab.id.clone(),
        code: fail.code,
        message: fail.message,
    };
    let boxes: Vec<Bbox> = cab.parts.iter().map(Bbox::of).collect();

    // Použiteľná hĺbka `yb = D − e − gw`: po prednú hranu drážky zadnej steny
    // (kap. 5 a 6, „dĺžka spoja“). Naložený chrbát (za bokmi): po chrbát. Bez chrbta celá hĺbka.
    let back_box = cab
        .parts
        .iter()
        .zip(&boxes)
        .find(|(p, _)| p.role == Role::Back);
    // naložený = za celým korpusom (bokmi, dnom aj vrchom); chrbát za bokmi, ale v drážke
    // dna a vrchu (horná skrinka v CV) naložený nie je
    let sides_rear = cab
        .parts
        .iter()
        .zip(&boxes)
        .filter(|(p, _)| {
            axis_aligned(p)
                && matches!(
                    p.role,
                    Role::SideLeft
                        | Role::SideRight
                        | Role::Bottom
                        | Role::Top
                        | Role::StretcherFront
                        | Role::StretcherRear
                )
        })
        .map(|(_, b)| b.max.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let back_applied = back_box.is_some_and(|(_, b)| b.min.y >= sides_rear - 1e-6);
    let y_usable = match back_box {
        Some((_, b)) if back_applied => b.min.y,
        Some((back, _)) => {
            let bs = &standard.back;
            cab.d - bs.groove_offset_from_rear - (back.thickness + bs.groove_width_clearance)
        }
        // pohľadový chrbát z dosky (nika): po jeho prednú plochu
        None => cab
            .parts
            .iter()
            .zip(&boxes)
            .find(|(p, _)| p.role == Role::FinishedBack)
            .map_or(cab.d, |(_, b)| b.min.y),
    };

    let mut new_ops: Vec<Vec<Operation>> = vec![Vec::new(); cab.parts.len()];
    let mut line_bore_sides = BTreeSet::new();

    for i in 0..cab.parts.len() {
        for j in i + 1..cab.parts.len() {
            let (pa, pb) = (&cab.parts[i], &cab.parts[j]);
            // čelá nie sú s korpusom spojené spojmi (drží ich kovanie)
            // naložený chrbát sa pribíja, spoj s korpusom nemá
            let applied_back = |r: Role| back_applied && r == Role::Back;
            // šikmý bok (natočený diel) sa spojmi nespája, v CV REF nemá ani jednu dieru
            if !axis_aligned(pa) || !axis_aligned(pb) {
                continue;
            }
            if pa.role.is_loose()
                || pb.role.is_loose()
                || applied_back(pa.role)
                || applied_back(pb.role)
            {
                continue;
            }
            let rule = rules.get(pa.role, pb.role);
            match rule {
                // atribút „Spoje skrinky = nie“: bez kolíkov a konfirmátov
                Some(ConnectionKind::Dowel) if !cab.options.joints => {}
                Some(ConnectionKind::Dowel) => {
                    if let Some(c) = contact(&boxes[i], &boxes[j]) {
                        let pattern = Pattern {
                            end_offset: rules.end_offset(pa.role, pb.role),
                            end_groups: rules.end_groups(pa.role, pb.role),
                        };
                        let ops = dowel_ops(cab, i, j, &boxes, &c, y_usable, standard, pattern)
                            .map_err(invalid)?;
                        new_ops[i].extend(ops.0);
                        new_ops[j].extend(ops.1);
                    }
                }
                // naložený chrbát: bez drážky (pribíja sa)
                Some(ConnectionKind::BackGroove) if back_applied => {}
                // drážka iba v diele, do ktorého chrbát zasahuje (bok pred chrbtom ju nemá)
                Some(ConnectionKind::BackGroove) if !overlaps(&boxes[i], &boxes[j]) => {}
                Some(ConnectionKind::BackGroove) => {
                    let (holder, back) = match (pa.role, pb.role) {
                        (_, Role::Back) => (i, j),
                        (Role::Back, _) => (j, i),
                        _ => {
                            return Err(invalid(Fail::from(tr!(
                                "joints.back_groove_without_back",
                                a = &pa.id,
                                b = &pb.id
                            ))))
                        }
                    };
                    let op = groove_op(cab, holder, back, &boxes, standard).map_err(invalid)?;
                    new_ops[holder].push(op);
                }
                // bez policového kovania nie je rad dier
                Some(ConnectionKind::LineBore)
                    if cab.options.shelf_support == ShelfSupport::None => {}
                Some(ConnectionKind::LineBore) => {
                    for (k, p) in [(i, pa), (j, pb)] {
                        if matches!(p.role, Role::SideLeft | Role::SideRight) {
                            line_bore_sides.insert(k);
                        }
                    }
                }
                // iba dotyk (pravidlo štandardu), bez obrábania
                Some(ConnectionKind::Touch) => {}
                None => {
                    if contact(&boxes[i], &boxes[j]).is_some() {
                        warnings.push(Issue::warning(
                            IssueCode::JointRuleMissing,
                            Some(&pa.id),
                            tr!(
                                "joints.rule_missing",
                                cabinet = &cab.id,
                                a = &pa.id,
                                b = &pb.id
                            ),
                        ));
                    }
                }
            }
        }
    }

    let shelf_pins = !line_bore_sides.is_empty();
    for k in line_bore_sides {
        let ops = line_bore_ops(cab, k, &boxes, y_usable, standard).map_err(invalid)?;
        new_ops[k].extend(ops);
    }
    if shelf_pins {
        shelf_pin_ops(cab, &boxes, y_usable, standard, &mut new_ops).map_err(invalid)?;
    }

    // slepý roh: pevný panel na prednej hrane boku a pred dnom
    blind::blind_panel_ops(cab, &boxes, standard, &mut new_ops).map_err(invalid)?;
    hardware_ops(cab, &boxes, standard, &mut new_ops).map_err(invalid)?;
    if cab.options.bumpers {
        bumper_ops(cab, standard, &mut new_ops);
    }

    for (part, ops) in cab.parts.iter_mut().zip(new_ops) {
        for op in ops {
            // rovnaká diera z dvoch zdrojov (napr. podložka pántu a rad dier) iba raz
            if !part.operations.contains(&op) {
                part.operations.push(op);
            }
        }
    }
    Ok(())
}

/// Obrábanie pre kovanie: miska pántu do plochy F dvierok (s ďalšími dierami pri nej)
/// a diery podložky do vnútornej plochy boku.
fn hardware_ops(
    cab: &CabinetModel,
    boxes: &[Bbox],
    standard: &Standard,
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let index = |id: &str| -> Result<usize, Fail> {
        cab.parts
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| Fail::from(tr!("joints.hardware_part_missing", id = id)))
    };
    for hw in &cab.hardware {
        match hw {
            Hardware::Hinge {
                hinge,
                door,
                side,
                z,
                cup_x,
                plate_y,
                plate_x,
            } => {
                let spec = standard
                    .hinge(hinge)
                    .ok_or_else(|| Fail::from(tr!("joints.hinge_missing", hinge = hinge)))?;
                let (di, si) = (index(door)?, index(side)?);
                let (Some(door_box), Some(side_box)) = (boxes.get(di), boxes.get(si)) else {
                    continue;
                };
                // miska: z plochy F (k skrinke) smerom dopredu
                let y_f = door_box.max.y;
                let toward_center = if *cup_x <= door_box.center().x {
                    1.0
                } else {
                    -1.0
                };
                let mut door_holes = vec![CabHole {
                    entry: DVec3::new(*cup_x, y_f, *z),
                    dir: DVec3::NEG_Y,
                    diameter: spec.cup_diameter,
                    depth: spec.cup_depth,
                    dowel: false,
                }];
                door_holes.extend(spec.cup_holes.iter().map(|h| CabHole {
                    entry: DVec3::new(cup_x + toward_center * h.x, y_f, z + h.z),
                    dir: DVec3::NEG_Y,
                    diameter: h.diameter,
                    depth: h.depth,
                    dowel: false,
                }));
                // podložka: vnútorná plocha boku smerom von zo skrinky, pri slepom pánte
                // vnútorná plocha pevného panelu (smerom dopredu)
                let side_holes: Vec<CabHole> = match plate_x {
                    Some(px) => spec
                        .plate_holes
                        .iter()
                        .map(|h| CabHole {
                            entry: DVec3::new(px + h.x, side_box.max.y, z + h.z),
                            dir: DVec3::NEG_Y,
                            diameter: h.diameter,
                            depth: h.depth,
                            dowel: false,
                        })
                        .collect(),
                    None => {
                        let (x_f, dir) = match cab.parts.get(si).map(|p| p.role) {
                            Some(Role::SideRight) => (side_box.min.x, DVec3::X),
                            _ => (side_box.max.x, DVec3::NEG_X),
                        };
                        spec.plate_holes
                            .iter()
                            .map(|h| CabHole {
                                entry: DVec3::new(x_f, plate_y + h.x, z + h.z),
                                dir,
                                diameter: h.diameter,
                                depth: h.depth,
                                dowel: false,
                            })
                            .collect()
                    }
                };
                push_holes(cab, new_ops, di, &door_holes)?;
                push_holes(cab, new_ops, si, &side_holes)?;
            }
            Hardware::Drawer {
                drawer,
                front,
                side_left,
                side_right,
                box_bottom,
                box_height,
                box_left,
                box_right,
                length,
            } => {
                let spec = standard
                    .drawer(drawer)
                    .ok_or_else(|| Fail::from(tr!("joints.drawer_missing", drawer = drawer)))?;
                // výsuvy: diery do vnútornej plochy oboch bokov
                for side in [side_left, side_right] {
                    let si = index(side)?;
                    let Some(side_box) = boxes.get(si) else {
                        continue;
                    };
                    let (x_f, dir) = match cab.parts.get(si).map(|p| p.role) {
                        Some(Role::SideRight) => (side_box.min.x, DVec3::X),
                        _ => (side_box.max.x, DVec3::NEG_X),
                    };
                    let holes: Vec<CabHole> = spec
                        .runner_holes_for(*length)
                        .iter()
                        .map(|h| CabHole {
                            entry: DVec3::new(x_f, h.x, box_bottom + h.z),
                            dir,
                            diameter: h.diameter,
                            depth: h.depth,
                            dowel: false,
                        })
                        .collect();
                    push_holes(cab, new_ops, si, &holes)?;
                }
                // upevnenie čela: z vnútornej plochy čela v rovinách bočníc korpusu
                let fi = index(front)?;
                let Some(front_box) = boxes.get(fi) else {
                    continue;
                };
                let y_f = front_box.max.y;
                let holes: Vec<CabHole> = spec
                    .front_holes
                    .iter()
                    .filter(|h| h.min_box_height.is_none_or(|m| *box_height + 1e-6 >= m))
                    .flat_map(|h| {
                        [(*box_left, 1.0), (*box_right, -1.0)].map(|(x, inward)| CabHole {
                            entry: DVec3::new(x + inward * h.x, y_f, box_bottom + h.z),
                            dir: DVec3::NEG_Y,
                            diameter: h.diameter,
                            depth: h.depth,
                            dowel: false,
                        })
                    })
                    .collect();
                push_holes(cab, new_ops, fi, &holes)?;
            }
            Hardware::Filler { strip } => {
                filler::strip_ops(cab, boxes, standard, strip, new_ops)?;
            }
        }
    }
    Ok(())
}

/// Tlmiace čočky (findings 11 §1.8): dvierka 2 diery pri hrane oproti pántom, čelo
/// zásuvky 2 diery v strede výšky (nízke) alebo 4; z plochy F.
fn bumper_ops(cab: &CabinetModel, standard: &Standard, new_ops: &mut [Vec<Operation>]) {
    let bs = &standard.bumpers;
    let rows = bs.rows(cab.cabinet_type);
    for (k, part) in cab.parts.iter().enumerate() {
        let (l, w) = (part.length, part.width);
        let points: Vec<(f64, f64)> = match part.role {
            Role::Door => {
                // strana pántov podľa misky; bez pántu sú pánty vľavo
                let hinge_left = cab
                    .hardware
                    .iter()
                    .find_map(|h| match h {
                        Hardware::Hinge { door, cup_x, .. } if *door == part.id => {
                            let left = part.transform.point_to_cabinet(DVec3::ZERO).x;
                            Some(*cup_x - left < w / 2.0)
                        }
                        _ => None,
                    })
                    .unwrap_or(true);
                let y = if hinge_left { w - bs.edge } else { bs.edge };
                vec![(rows.bottom, y), (l - rows.top, y)]
            }
            Role::DrawerFront if l <= bs.small_front => {
                vec![(l / 2.0, bs.edge), (l / 2.0, w - bs.edge)]
            }
            Role::DrawerFront => vec![
                (rows.bottom, bs.edge),
                (rows.bottom, w - bs.edge),
                (l - rows.top, bs.edge),
                (l - rows.top, w - bs.edge),
            ],
            _ => continue,
        };
        let Some(ops) = new_ops.get_mut(k) else {
            continue;
        };
        for (x, y) in points {
            let op = Operation::Hole {
                face: Face::F,
                x: clean(x),
                y: clean(y),
                z: part.thickness,
                diameter: bs.diameter,
                depth: bs.depth,
                through: false,
                dowel: false,
            };
            if !ops.contains(&op) {
                ops.push(op);
            }
        }
    }
}

/// Diery kovania do dielu `k` (rovnaká diera iba raz).
fn push_holes(
    cab: &CabinetModel,
    new_ops: &mut [Vec<Operation>],
    k: usize,
    holes: &[CabHole],
) -> Result<(), Fail> {
    let Some(part) = cab.parts.get(k) else {
        return Ok(());
    };
    for h in holes {
        let op = to_part_hole(part, h)?;
        if let Some(ops) = new_ops.get_mut(k) {
            if !ops.contains(&op) {
                ops.push(op);
            }
        }
    }
    Ok(())
}

/// Úprava vzoru kolíkov z pravidla spoja.
struct Pattern<'a> {
    /// Nahradí odsadenie krajného kolíka zo štandardu.
    end_offset: Option<f64>,
    /// Dve skupiny pri koncoch hrany namiesto vzoru (ak hranu nesie diel `edge`).
    end_groups: Option<&'a EndGroups>,
}

/// Dve skupiny kolík – konfirmát – kolík na úseku `[s0, s1]`: najbližší kolík `a` od začiatku
/// a `b` od konca, kolíky `pitch` od seba, konfirmát medzi nimi. `None`, ak sa nezmestia.
fn end_group_positions(
    s0: f64,
    s1: f64,
    g: &EndGroups,
    pitch: f64,
) -> Option<(Vec<f64>, Vec<f64>)> {
    let (a, b) = (g.from_start, g.from_end);
    let ok = [a, b, pitch].iter().all(|v| v.is_finite() && *v > 0.0)
        && s0 + a + pitch < s1 - b - pitch - 1e-6;
    ok.then(|| {
        (
            vec![s0 + a, s0 + a + pitch, s1 - b - pitch, s1 - b],
            vec![s0 + a + pitch / 2.0, s1 - b - pitch / 2.0],
        )
    })
}

/// Kolíky (kap. 6.1): diera do plochy jedného dielu a do hrany druhého. `pattern` z pravidla
/// spoja upraví vzor štandardu.
#[allow(clippy::too_many_arguments)]
fn dowel_ops(
    cab: &CabinetModel,
    i: usize,
    j: usize,
    boxes: &[Bbox],
    c: &Contact,
    y_usable: f64,
    standard: &Standard,
    pattern: Pattern,
) -> Result<(Vec<Operation>, Vec<Operation>), Fail> {
    let mut local = standard.dowel.clone();
    if let Some(e) = pattern.end_offset {
        local.end_offset = e;
    }
    let std = &local;
    let (pa, pb) = (&cab.parts[i], &cab.parts[j]);
    let thick_a = axis_index(pa.transform.z_axis);
    let thick_b = axis_index(pb.transform.z_axis);

    // „face“ diel má hrúbku v smere normály dotyku, „edge“ diel ju má v rovine dotyku.
    let (face_k, edge_k) = if thick_a == c.axis && thick_b != c.axis {
        (i, j)
    } else if thick_b == c.axis && thick_a != c.axis {
        (j, i)
    } else {
        return Err(tr!("joints.dowel_orientation", a = &pa.id, b = &pb.id).into());
    };
    let face_part = &cab.parts[face_k];
    let edge_part = &cab.parts[edge_k];
    let center_axis = axis_index(edge_part.transform.z_axis);
    let line_axis = 3 - c.axis - center_axis;

    // vzor sa ráta z celej hrany dielu, ktorý nesie hranu (CV: dno 330 aj pri boku 309,5);
    // diery mimo dotyku sa vynechajú
    let (o0, o1) = c.overlap[line_axis];
    let (mut s0, mut s1) = (boxes[edge_k].min[line_axis], boxes[edge_k].max[line_axis]);
    if line_axis == 1 {
        s0 = s0.max(0.0);
        if std.stop_at_back {
            s1 = s1.min(y_usable);
        }
    }
    let inside =
        |v: &f64| *v - std.diameter / 2.0 >= o0 - 1e-6 && *v + std.diameter / 2.0 <= o1 + 1e-6;
    // skupiny pri koncoch iba na hrane dielu, pre ktorý ich pravidlo určuje
    let groups = pattern
        .end_groups
        .filter(|g| edge_part.role.rule_name() == g.edge);
    let laid_out = match groups {
        Some(g) => end_group_positions(s0, s1, g, std.pair_pitch),
        None => joint_positions(s0, s1, std),
    };
    let (positions, connectors) = laid_out
        .map(|(d, k)| {
            (
                d.into_iter().filter(inside).collect::<Vec<_>>(),
                k.into_iter().filter(inside).collect::<Vec<_>>(),
            )
        })
        .filter(|(d, _)| !d.is_empty())
        .ok_or_else(|| Fail {
            code: Some(IssueCode::JointTooShort),
            message: tr!(
                "joints.too_short",
                a = &face_part.id,
                b = &edge_part.id,
                length = format!("{:.1}", s1 - s0),
                min = format!("{:.1}", 2.0 * std.end_offset + 8.0)
            ),
        })?;
    let center = (c.overlap[center_axis].0 + c.overlap[center_axis].1) / 2.0;

    // Smer od face dielu k edge dielu pozdĺž normály dotyku.
    let sign = (boxes[edge_k].center()[c.axis] - boxes[face_k].center()[c.axis]).signum();
    let n = unit(c.axis) * sign;

    let mut face_ops = Vec::new();
    let mut edge_ops = Vec::new();
    // konfirmát (vrut) medzi kolíkmi podľa vzoru cez bok (findings 11 §1.3) a cez dno pri
    // zvislom zadnom vlyse drezu (findings 12 §4) a cez pohľadový chrbát šikmej skrinky
    // (REF diel 0029); vrch naložený na boky hornej skrinky ho v CV nemá – skrutka by bola
    // na pohľadovej ploche; to isté dno hornej skrinky (pohľad zospodu, CV REF nika: dno pod
    // bokmi bez Ø5)
    let through_face = match face_part.role {
        Role::SideLeft | Role::SideRight | Role::FinishedBack => true,
        Role::Bottom => cab.cabinet_type != CabinetType::Wall,
        _ => false,
    };
    if matches!(
        cab.options.joint_type,
        JointType::Confirmat | JointType::Screw
    ) && through_face
    {
        for &k in &connectors {
            let mut entry = DVec3::ZERO;
            entry[c.axis] = c.plane;
            entry[line_axis] = k;
            entry[center_axis] = center;
            face_ops.push(to_part_hole(
                face_part,
                &CabHole {
                    entry,
                    dir: -n,
                    diameter: std.confirmat_diameter,
                    depth: std.confirmat_depth,
                    dowel: false,
                },
            )?);
            // konfirmát: aj diera do hrany druhého dielu (vrut nie)
            if let (JointType::Confirmat, Some([d, depth])) =
                (cab.options.joint_type, std.confirmat_edge)
            {
                edge_ops.push(to_part_hole(
                    edge_part,
                    &CabHole {
                        entry,
                        dir: n,
                        diameter: d,
                        depth,
                        dowel: false,
                    },
                )?);
            }
        }
    }
    for s in positions {
        let mut entry = DVec3::ZERO;
        entry[c.axis] = c.plane;
        entry[line_axis] = s;
        entry[center_axis] = center;
        face_ops.push(to_part_hole(
            face_part,
            &CabHole {
                entry,
                dir: -n,
                diameter: std.diameter,
                depth: std.face_depth,
                dowel: true,
            },
        )?);
        edge_ops.push(to_part_hole(
            edge_part,
            &CabHole {
                entry,
                dir: n,
                diameter: std.diameter,
                depth: std.edge_depth,
                dowel: true,
            },
        )?);
    }
    let (ops_i, ops_j) = if face_k == i {
        (face_ops, edge_ops)
    } else {
        (edge_ops, face_ops)
    };
    Ok((ops_i, ops_j))
}

/// Drážka pre zadnú stenu (kap. 6.2): pás v hĺbke `[D − e − gw, D − e]`, cez celý diel.
fn groove_op(
    cab: &CabinetModel,
    holder: usize,
    back: usize,
    boxes: &[Bbox],
    standard: &Standard,
) -> Result<Operation, Fail> {
    let bs = &standard.back;
    if !bs.groove_through {
        return Err(tr!("joints.stopped_groove").into());
    }
    let part = &cab.parts[holder];
    let back_part = &cab.parts[back];
    let gw = back_part.thickness + bs.groove_width_clearance;
    let y_rear = cab.d - bs.groove_offset_from_rear;
    let y_center = y_rear - gw / 2.0;

    let t = &part.transform;
    // Ktorá os dielu ide pozdĺž hĺbky skrinky (Y)?
    let along_y = |axis: DVec3| axis.dot(DVec3::Y).abs() > 1.0 - 1e-9;
    let c_part = t.point_to_part(DVec3::new(0.0, y_center, 0.0));
    let (start, end) = if along_y(t.y_axis) {
        let y = clean(c_part.y);
        (P2 { x: 0.0, y }, P2 { x: part.length, y })
    } else if along_y(t.x_axis) {
        let x = clean(c_part.x);
        (P2 { x, y: 0.0 }, P2 { x, y: part.width })
    } else {
        return Err(tr!("joints.groove_orientation", id = &part.id).into());
    };

    // Drážka je na ploche dielu, ktorá je obrátená k zadnej stene.
    let back_center = t.point_to_part(boxes[back].center());
    let face = if back_center.z >= part.thickness / 2.0 {
        Face::F
    } else {
        Face::B
    };
    Ok(Operation::Groove {
        face,
        start,
        end,
        width: gw,
        depth: bs.groove_depth,
    })
}

/// Diery pre čapy podperiek v spodnej ploche polohovateľných políc (Sekura 7 v CV):
/// pri oboch koncoch police v predom aj zadnom rade radu dier.
fn shelf_pin_ops(
    cab: &CabinetModel,
    boxes: &[Bbox],
    y_usable: f64,
    standard: &Standard,
    new_ops: &mut [Vec<Operation>],
) -> Result<(), Fail> {
    let std = &standard.line_bore;
    let Some([diameter, depth, from_end]) = std.shelf_pin_holes else {
        return Ok(());
    };
    let rows = [std.front_offset, y_usable - std.rear_offset];
    for (k, (part, b)) in cab.parts.iter().zip(boxes).enumerate() {
        if part.role != Role::AdjustableShelf {
            continue;
        }
        for x in [b.min.x + from_end, b.max.x - from_end] {
            for y in rows {
                let op = to_part_hole(
                    part,
                    &CabHole {
                        entry: DVec3::new(x, y, b.min.z),
                        dir: DVec3::Z,
                        diameter,
                        depth,
                        dowel: false,
                    },
                )?;
                if let Some(ops) = new_ops.get_mut(k) {
                    ops.push(op);
                }
            }
        }
    }
    Ok(())
}

/// Rad dier pre police (kap. 6.3) na ploche F boku, rozdelený pevnými policami.
fn line_bore_ops(
    cab: &CabinetModel,
    side: usize,
    boxes: &[Bbox],
    y_usable: f64,
    standard: &Standard,
) -> Result<Vec<Operation>, Fail> {
    let std = &standard.line_bore;
    if !cab.parts.iter().any(|p| p.role == Role::AdjustableShelf) {
        return Ok(Vec::new());
    }
    let part = &cab.parts[side];
    let roles_z = |roles: &[Role]| {
        cab.parts
            .iter()
            .zip(boxes)
            .filter(|(p, _)| roles.contains(&p.role))
            .map(|(_, b)| *b)
            .collect::<Vec<_>>()
    };
    let lo = roles_z(&[Role::Bottom])
        .iter()
        .map(|b| b.max.z)
        .fold(f64::NEG_INFINITY, f64::max);
    // horný koniec: vrch alebo predný vlys (zadný vlys drezu stojí dole na dne)
    let hi = roles_z(&[Role::Top, Role::StretcherFront])
        .iter()
        .map(|b| b.min.z)
        .fold(f64::INFINITY, f64::min);
    if !lo.is_finite() || !hi.is_finite() {
        return Err(tr!("joints.line_bore_no_bottom").into());
    }

    // Úseky medzi dnom, pevnými policami a vrchom.
    let mut fixed: Vec<Bbox> = roles_z(&[Role::FixedShelf]);
    fixed.sort_by(|a, b| a.min.z.total_cmp(&b.min.z));
    let mut segments = Vec::new();
    let mut cur = lo;
    for f in &fixed {
        segments.push((cur, f.min.z));
        cur = f.max.z;
    }
    segments.push((cur, hi));

    // Bod vstupu leží na ploche F boku; nástroj ide proti normále F.
    let f_plane = part
        .transform
        .point_to_cabinet(DVec3::new(0.0, 0.0, part.thickness));
    let dir = -part.transform.z_axis;
    let rows = [std.front_offset, y_usable - std.rear_offset];

    // Spodné plochy polohovateľných políc (nominálna poloha z generátora).
    let shelf_bottoms: Vec<f64> = roles_z(&[Role::AdjustableShelf])
        .iter()
        .map(|b| b.min.z)
        .collect();

    let mut ops = Vec::new();
    for (seg_lo, seg_hi) in segments {
        let heights = match std.layout {
            LineBoreLayout::Full => line_bore_heights(seg_lo, seg_hi, std),
            LineBoreLayout::PerShelf => shelf_hole_heights(seg_lo, seg_hi, &shelf_bottoms, std),
        };
        for z in heights {
            for y in rows {
                let entry = DVec3::new(f_plane.x, y, z);
                ops.push(to_part_hole(
                    part,
                    &CabHole {
                        entry,
                        dir,
                        diameter: std.diameter,
                        depth: std.depth,
                        dowel: false,
                    },
                )?);
            }
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_model::{Edges, Grain, Transform};

    #[test]
    fn diagonal_tool_axis_is_unsupported() {
        let part = Part {
            id: "P".into(),
            cabinet: "S".into(),
            role: Role::Bottom,
            material: "M".into(),
            length: 500.0,
            width: 400.0,
            thickness: 18.0,
            grain: Grain::None,
            edges: Edges::default(),
            transform: Transform::from_axes(DVec3::X, DVec3::Y, DVec3::ZERO),
            operations: Vec::new(),
        };
        let hole = CabHole {
            entry: DVec3::new(100.0, 100.0, 18.0),
            dir: DVec3::new(1.0, 0.0, -1.0).normalize(),
            diameter: 8.0,
            depth: 10.0,
            dowel: false,
        };
        let fail = to_part_hole(&part, &hole).expect_err("šikmá os musí zlyhať");
        assert_eq!(fail.code, Some(IssueCode::UnsupportedToolAxis));
    }
}
