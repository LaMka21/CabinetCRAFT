//! Balenie obdĺžnikov na tabule (MaxRects, viac heuristík, vyberie sa najlepší výsledok).
//!
//! Medzera medzi dielmi sa rieši zväčšením každého dielu o `spacing` a plochy tabule
//! o `spacing` (bez okrajov). Výsledok je deterministický: rovnaký vstup = rovnaké
//! rozloženie (žiadna náhoda, pri zhode skóre rozhoduje poradie).

/// Tolerancia porovnávania rozmerov (mm).
const EPS: f64 = 1e-6;

/// Najviac dielov v jednej skupine materiálu (vstup je nepriateľský).
pub const MAX_ITEMS: usize = 5000;

/// Nad týmto počtom sa vynechá pomalšia stratégia „najlepší diel na každé miesto“.
const GLOBAL_BEST_LIMIT: usize = 400;

/// Rozpočet lokálneho hľadania: počet krokov × počet dielov (≈ čas behu).
const LOCAL_SEARCH_WORK: usize = 40_000;

/// Povolené otočenie dielu na tabuli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// 0° alebo 90° (materiál bez vlákna).
    Free,
    /// Iba 0°: dĺžka dielu po dĺžke tabule.
    Fixed,
    /// Iba 90°: dĺžka dielu po šírke tabule.
    Rotated,
}

/// Diel na balenie: `length` po X tabule pri 0°, `width` po Y.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Item {
    pub length: f64,
    pub width: f64,
    pub rotation: Rotation,
}

/// Tabuľa: rozmer, okraj zo všetkých strán a medzera medzi dielmi (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SheetSpec {
    pub length: f64,
    pub width: f64,
    pub margin: f64,
    pub spacing: f64,
}

impl SheetSpec {
    fn bin_size(&self) -> (f64, f64) {
        (
            self.length - 2.0 * self.margin + self.spacing,
            self.width - 2.0 * self.margin + self.spacing,
        )
    }
}

/// Umiestnený diel: ľavý dolný roh `(x, y)` v súradniciach tabule, rozmer na tabuli `dx × dy`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// Index do vstupného zoznamu.
    pub item: usize,
    pub x: f64,
    pub y: f64,
    pub dx: f64,
    pub dy: f64,
    pub rotated: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PackedSheet {
    /// Zoradené zdola nahor, zľava doprava.
    pub placed: Vec<Placed>,
}

impl PackedSheet {
    pub fn used_area(&self) -> f64 {
        self.placed.iter().map(|p| p.dx * p.dy).sum()
    }

    /// Najvyššia hrana dielov (0 pri prázdnej tabuli).
    pub fn top(&self) -> f64 {
        self.placed.iter().map(|p| p.y + p.dy).fold(0.0, f64::max)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Packing {
    pub sheets: Vec<PackedSheet>,
    /// Diely, ktoré sa na tabuľu nezmestia (alebo majú neplatný rozmer).
    pub unplaced: Vec<usize>,
}

impl Packing {
    /// Využitie všetkých tabúľ (0–1).
    pub fn utilisation(&self, spec: &SheetSpec) -> f64 {
        let total = self.sheets.len() as f64 * spec.length * spec.width;
        if total <= 0.0 {
            return 0.0;
        }
        self.sheets.iter().map(PackedSheet::used_area).sum::<f64>() / total
    }
}

#[derive(Debug, Clone, Copy)]
enum Heuristic {
    /// Najmenší zvyšok kratšej strany voľného obdĺžnika.
    ShortSide,
    /// Čo najnižšie, potom čo najviac vľavo.
    BottomLeft,
    /// Čo najviac vľavo, potom čo najnižšie (plní stĺpce od ľavého okraja).
    LeftBottom,
    /// Najmenší zvyšok plochy voľného obdĺžnika.
    Area,
    /// Najdlhší dotyk s hranami tabule a inými dielmi.
    Contact,
}

#[derive(Debug, Clone, Copy)]
enum Order {
    Area,
    LongSide,
    Perimeter,
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x - EPS
            && o.y >= self.y - EPS
            && o.x + o.w <= self.x + self.w + EPS
            && o.y + o.h <= self.y + self.h + EPS
    }

    fn intersects(&self, o: &Rect) -> bool {
        o.x < self.x + self.w - EPS
            && o.x + o.w > self.x + EPS
            && o.y < self.y + self.h - EPS
            && o.y + o.h > self.y + EPS
    }
}

/// Jedna tabuľa počas balenia (súradnice bez okraja, rozmery vrátane medzery).
struct Bin {
    w: f64,
    h: f64,
    free: Vec<Rect>,
    used: Vec<(Rect, Placed)>,
}

/// Skóre umiestnenia: menšie je lepšie.
type Score = (f64, f64);

fn better(a: Score, b: Score) -> bool {
    a.0 < b.0 - EPS || ((a.0 - b.0).abs() <= EPS && a.1 < b.1 - EPS)
}

/// Overlap dvoch intervalov [a0, a1] a [b0, b1].
fn overlap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

impl Bin {
    fn new(w: f64, h: f64) -> Self {
        Bin {
            w,
            h,
            free: vec![Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
            }],
            used: Vec::new(),
        }
    }

    fn contact(&self, r: &Rect) -> f64 {
        let mut c = 0.0;
        if r.x.abs() <= EPS || (r.x + r.w - self.w).abs() <= EPS {
            c += r.h;
        }
        if r.y.abs() <= EPS || (r.y + r.h - self.h).abs() <= EPS {
            c += r.w;
        }
        for (u, _) in &self.used {
            if (u.x - (r.x + r.w)).abs() <= EPS || (u.x + u.w - r.x).abs() <= EPS {
                c += overlap(u.y, u.y + u.h, r.y, r.y + r.h);
            }
            if (u.y - (r.y + r.h)).abs() <= EPS || (u.y + u.h - r.y).abs() <= EPS {
                c += overlap(u.x, u.x + u.w, r.x, r.x + r.w);
            }
        }
        c
    }

    /// Najlepšie miesto pre obdĺžnik `w × h`.
    fn find(&self, w: f64, h: f64, heur: Heuristic) -> Option<(Score, Rect)> {
        let mut best: Option<(Score, Rect)> = None;
        for f in &self.free {
            if w > f.w + EPS || h > f.h + EPS {
                continue;
            }
            let r = Rect {
                x: f.x,
                y: f.y,
                w,
                h,
            };
            let (lw, lh) = (f.w - w, f.h - h);
            let score = match heur {
                Heuristic::ShortSide => (lw.min(lh), lw.max(lh)),
                Heuristic::BottomLeft => (f.y + h, f.x),
                Heuristic::LeftBottom => (f.x, f.y),
                Heuristic::Area => (f.w * f.h - w * h, lw.min(lh)),
                Heuristic::Contact => (-self.contact(&r), f.y + h),
            };
            if best.as_ref().is_none_or(|(s, _)| better(score, *s)) {
                best = Some((score, r));
            }
        }
        best
    }

    fn place(&mut self, r: Rect, placed: Placed) {
        let mut next = Vec::with_capacity(self.free.len() + 4);
        for f in &self.free {
            if !f.intersects(&r) {
                next.push(*f);
                continue;
            }
            // rozdelenie voľného obdĺžnika na až štyri časti okolo `r`
            if r.x > f.x + EPS {
                next.push(Rect {
                    x: f.x,
                    y: f.y,
                    w: r.x - f.x,
                    h: f.h,
                });
            }
            if r.x + r.w < f.x + f.w - EPS {
                next.push(Rect {
                    x: r.x + r.w,
                    y: f.y,
                    w: f.x + f.w - (r.x + r.w),
                    h: f.h,
                });
            }
            if r.y > f.y + EPS {
                next.push(Rect {
                    x: f.x,
                    y: f.y,
                    w: f.w,
                    h: r.y - f.y,
                });
            }
            if r.y + r.h < f.y + f.h - EPS {
                next.push(Rect {
                    x: f.x,
                    y: r.y + r.h,
                    w: f.w,
                    h: f.y + f.h - (r.y + r.h),
                });
            }
        }
        // odstrániť obdĺžniky obsiahnuté v iných (pri zhode ostane prvý)
        let mut keep = vec![true; next.len()];
        for i in 0..next.len() {
            if !keep[i] {
                continue;
            }
            for j in 0..next.len() {
                if i == j || !keep[j] {
                    continue;
                }
                let (a, b) = (&next[i], &next[j]);
                if a.contains(b) {
                    keep[j] = false;
                }
            }
        }
        self.free = next
            .into_iter()
            .zip(keep)
            .filter_map(|(r, k)| k.then_some(r))
            .collect();
        self.used.push((r, placed));
    }
}

/// Orientácie dielu: (šírka po X, výška po Y, otočený).
fn orientations(item: &Item) -> Vec<(f64, f64, bool)> {
    match item.rotation {
        Rotation::Fixed => vec![(item.length, item.width, false)],
        Rotation::Rotated => vec![(item.width, item.length, true)],
        Rotation::Free if (item.length - item.width).abs() <= EPS => {
            vec![(item.length, item.width, false)]
        }
        Rotation::Free => vec![
            (item.length, item.width, false),
            (item.width, item.length, true),
        ],
    }
}

fn valid(item: &Item) -> bool {
    item.length.is_finite() && item.width.is_finite() && item.length > 0.0 && item.width > 0.0
}

fn fits_empty(item: &Item, spec: &SheetSpec) -> bool {
    let (bw, bh) = spec.bin_size();
    valid(item)
        && orientations(item)
            .iter()
            .any(|(w, h, _)| w + spec.spacing <= bw + EPS && h + spec.spacing <= bh + EPS)
}

/// Rozloží diely na čo najmenej tabúľ. Pri rovnakom počte tabúľ vyhrá rozloženie,
/// ktoré na poslednej tabuli nechá najmenej dielov (plochy) a najnižšiu hornú hranu
/// (najväčší súvislý zvyšok).
///
/// Postup: niekoľko pevných stratégií (poradie × heuristika), potom deterministické
/// lokálne hľadanie nad poradím a otočením dielov (pevné semienko, obmedzený počet krokov).
pub fn pack(items: &[Item], spec: &SheetSpec) -> Packing {
    let spec_ok = [spec.length, spec.width, spec.margin, spec.spacing]
        .iter()
        .all(|v| v.is_finite())
        && spec.margin >= 0.0
        && spec.spacing >= 0.0
        && spec.bin_size().0 > 0.0
        && spec.bin_size().1 > 0.0;
    if !spec_ok || items.len() > MAX_ITEMS {
        return Packing {
            sheets: Vec::new(),
            unplaced: (0..items.len()).collect(),
        };
    }
    let (ok, unplaced): (Vec<usize>, Vec<usize>) =
        (0..items.len()).partition(|&i| items.get(i).is_some_and(|it| fits_empty(it, spec)));
    if ok.is_empty() {
        return Packing {
            sheets: Vec::new(),
            unplaced,
        };
    }

    let heuristics = [
        Heuristic::BottomLeft,
        Heuristic::LeftBottom,
        Heuristic::ShortSide,
        Heuristic::Contact,
        Heuristic::Area,
    ];
    let mut best: Option<(Key, Vec<PackedSheet>)> = None;
    let consider = |sheets: Vec<PackedSheet>, best: &mut Option<(Key, Vec<PackedSheet>)>| {
        let k = key(&sheets);
        if best.as_ref().is_none_or(|(b, _)| k.better(b)) {
            *best = Some((k, sheets));
        }
    };

    if ok.len() <= GLOBAL_BEST_LIMIT {
        for heur in heuristics {
            consider(global_best(items, &ok, spec, heur), &mut best);
        }
    }
    // pevné poradia; najlepšie z nich je štart lokálneho hľadania
    let mut start: Option<(Key, Genome)> = None;
    for order in [Order::Area, Order::LongSide, Order::Perimeter] {
        for heur in heuristics {
            let g = Genome {
                order: sorted(items, &ok, order),
                orient: vec![Orient::Auto; items.len()],
                heur,
            };
            let sheets = first_fit(items, &g, spec);
            let k = key(&sheets);
            if start.as_ref().is_none_or(|(b, _)| k.better(b)) {
                start = Some((k, g));
            }
            consider(sheets, &mut best);
        }
    }
    if let Some((k, g)) = start {
        consider(local_search(items, spec, g, k), &mut best);
    }
    let mut sheets = best.map(|(_, s)| s).unwrap_or_default();
    if let Some(last) = sheets.pop() {
        sheets.push(compact_last(items, spec, last));
    }
    Packing { sheets, unplaced }
}

/// Diely poslednej tabule rozloží znova tak, aby ich horná hrana bola čo najnižšie
/// (väčší súvislý zvyšok na odrezanie). Ponechá pôvodné rozloženie, ak nové nie je lepšie.
fn compact_last(items: &[Item], spec: &SheetSpec, last: PackedSheet) -> PackedSheet {
    let ids: Vec<usize> = last.placed.iter().map(|p| p.item).collect();
    let mut best = last;
    let heuristics = [
        Heuristic::BottomLeft,
        Heuristic::Contact,
        Heuristic::ShortSide,
        Heuristic::LeftBottom,
        Heuristic::Area,
    ];
    let mut candidates = Vec::new();
    if ids.len() <= GLOBAL_BEST_LIMIT {
        for heur in heuristics {
            candidates.push(global_best(items, &ids, spec, heur));
        }
    }
    for order in [Order::Area, Order::LongSide, Order::Perimeter] {
        for heur in heuristics {
            let g = Genome {
                order: sorted(items, &ids, order),
                orient: vec![Orient::Auto; items.len()],
                heur,
            };
            candidates.push(first_fit(items, &g, spec));
        }
    }
    for mut c in candidates {
        if c.len() == 1 {
            if let Some(sheet) = c.pop() {
                if sheet.placed.len() == ids.len() && sheet.top() < best.top() - EPS {
                    best = sheet;
                }
            }
        }
    }
    best
}

/// Hodnotenie rozloženia (menšie je lepšie).
#[derive(Debug, Clone, Copy)]
struct Key {
    sheets: usize,
    /// Plocha dielov na poslednej tabuli: menej = bližšie k ušetreniu tabule.
    last_area: f64,
    /// Horná hrana dielov na poslednej tabuli.
    last_top: f64,
}

impl Key {
    fn better(&self, o: &Key) -> bool {
        if self.sheets != o.sheets {
            return self.sheets < o.sheets;
        }
        if (self.last_area - o.last_area).abs() > EPS {
            return self.last_area < o.last_area;
        }
        self.last_top < o.last_top - EPS
    }
}

/// Teplota žíhania na začiatku a na konci (v jednotkách „tabuľa“).
const T_START: f64 = 0.03;
const T_END: f64 = 0.0005;

/// Spojité hodnotenie pre žíhanie: počet tabúľ + podiel plochy na poslednej tabuli.
fn cost(k: &Key, sheet_area: f64) -> f64 {
    k.sheets as f64 + k.last_area / sheet_area.max(1.0)
}

fn key(sheets: &[PackedSheet]) -> Key {
    Key {
        sheets: sheets.len(),
        last_area: sheets.last().map_or(0.0, PackedSheet::used_area),
        last_top: sheets.last().map_or(0.0, PackedSheet::top),
    }
}

/// Otočenie dielu v riešení lokálneho hľadania.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Orient {
    /// Zvolí heuristika.
    Auto,
    /// Vynútené 0°.
    Normal,
    /// Vynútené 90°.
    Turned,
}

/// Riešenie: poradie dielov, otočenie a heuristika umiestnenia.
#[derive(Debug, Clone)]
struct Genome {
    order: Vec<usize>,
    /// Podľa indexu dielu.
    orient: Vec<Orient>,
    heur: Heuristic,
}

/// Jednoduchý deterministický generátor (xorshift64*), pevné semienko.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Číslo v [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// Lokálne hľadanie: výmena dvoch dielov v poradí, presun dielu, otočenie, iná heuristika.
/// Prijme aj rovnako dobré riešenie (posun po plató). Počet krokov je obmedzený prácou.
fn local_search(
    items: &[Item],
    spec: &SheetSpec,
    start: Genome,
    start_key: Key,
) -> Vec<PackedSheet> {
    let n = start.order.len();
    let steps = (LOCAL_SEARCH_WORK / n.max(1)).clamp(40, 2000);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let free: Vec<usize> = start
        .order
        .iter()
        .copied()
        .filter(|&i| items.get(i).is_some_and(|it| it.rotation == Rotation::Free))
        .collect();
    let mut best_sheets = first_fit(items, &start, spec);
    let mut best_key = start_key;
    let mut cur = start;
    let mut cur_key = start_key;
    let sheet_area = spec.length * spec.width;
    for step in 0..steps {
        let mut cand = cur.clone();
        match rng.below(10) {
            0..=3 if n > 1 => {
                let (a, b) = (rng.below(n), rng.below(n));
                cand.order.swap(a, b);
            }
            4..=6 if n > 1 => {
                let a = rng.below(n);
                let v = cand.order.remove(a);
                let b = rng.below(cand.order.len() + 1);
                cand.order.insert(b, v);
            }
            7 | 8 if !free.is_empty() => {
                let i = free[rng.below(free.len())];
                if let Some(o) = cand.orient.get_mut(i) {
                    *o = match *o {
                        Orient::Auto => Orient::Normal,
                        Orient::Normal => Orient::Turned,
                        Orient::Turned => Orient::Auto,
                    };
                }
            }
            _ => {
                cand.heur = match rng.below(5) {
                    0 => Heuristic::BottomLeft,
                    1 => Heuristic::ShortSide,
                    2 => Heuristic::Contact,
                    3 => Heuristic::LeftBottom,
                    _ => Heuristic::Area,
                };
            }
        }
        let sheets = first_fit(items, &cand, spec);
        let k = key(&sheets);
        // žíhanie: horšie riešenie sa prijme s pravdepodobnosťou exp(−Δ/T), T klesá k nule
        let delta = cost(&k, sheet_area) - cost(&cur_key, sheet_area);
        let t = T_START * (1.0 - step as f64 / steps as f64) + T_END;
        if delta <= 0.0 || rng.unit() < (-delta / t).exp() {
            cur = cand;
            cur_key = k;
            if k.better(&best_key) {
                best_key = k;
                best_sheets = sheets;
            }
        }
    }
    best_sheets
}

/// Poradie dielov (veľké skôr; pri zhode poradie na vstupe).
fn sorted(items: &[Item], ok: &[usize], order: Order) -> Vec<usize> {
    let key = |i: usize| -> f64 {
        items.get(i).map_or(0.0, |it| match order {
            Order::Area => it.length * it.width,
            Order::LongSide => it.length.max(it.width),
            Order::Perimeter => it.length + it.width,
        })
    };
    let mut v = ok.to_vec();
    v.sort_by(|&a, &b| key(b).total_cmp(&key(a)).then(a.cmp(&b)));
    v
}

fn make_placed(spec: &SheetSpec, idx: usize, r: &Rect, rotated: bool) -> Placed {
    Placed {
        item: idx,
        x: spec.margin + r.x,
        y: spec.margin + r.y,
        dx: r.w - spec.spacing,
        dy: r.h - spec.spacing,
        rotated,
    }
}

/// Diely v poradí `g.order`; každý na prvú tabuľu, kam sa zmestí.
fn first_fit(items: &[Item], g: &Genome, spec: &SheetSpec) -> Vec<PackedSheet> {
    let (bw, bh) = spec.bin_size();
    let s = spec.spacing;
    let mut bins: Vec<Bin> = Vec::new();
    for &idx in &g.order {
        let Some(item) = items.get(idx) else { continue };
        let orient = g.orient.get(idx).copied().unwrap_or(Orient::Auto);
        let mut done = false;
        for bin in bins.iter_mut() {
            if let Some((r, rot)) = best_in_bin(bin, item, orient, s, g.heur) {
                bin.place(r, make_placed(spec, idx, &r, rot));
                done = true;
                break;
            }
        }
        if !done {
            let mut bin = Bin::new(bw, bh);
            // vynútené otočenie sa na prázdnu tabuľu nemusí zmestiť → potom ľubovoľné povolené
            let placed = best_in_bin(&bin, item, orient, s, g.heur)
                .or_else(|| best_in_bin(&bin, item, Orient::Auto, s, g.heur));
            if let Some((r, rot)) = placed {
                bin.place(r, make_placed(spec, idx, &r, rot));
                bins.push(bin);
            }
        }
    }
    finish(bins)
}

/// Tabuľa po tabuli: vždy najlepšia dvojica (diel, miesto) zo zvyšných dielov.
fn global_best(
    items: &[Item],
    ok: &[usize],
    spec: &SheetSpec,
    heur: Heuristic,
) -> Vec<PackedSheet> {
    let (bw, bh) = spec.bin_size();
    let s = spec.spacing;
    let mut bins: Vec<Bin> = Vec::new();
    let mut remaining = sorted(items, ok, Order::Area);
    while !remaining.is_empty() {
        let mut bin = Bin::new(bw, bh);
        loop {
            let mut choice: Option<(Score, usize, Rect, bool)> = None;
            for (pos, &idx) in remaining.iter().enumerate() {
                let Some(item) = items.get(idx) else { continue };
                for (w, h, rot) in orientations(item) {
                    if let Some((score, r)) = bin.find(w + s, h + s, heur) {
                        if choice.as_ref().is_none_or(|(c, ..)| better(score, *c)) {
                            choice = Some((score, pos, r, rot));
                        }
                    }
                }
            }
            let Some((_, pos, r, rot)) = choice else {
                break;
            };
            let idx = remaining.remove(pos);
            bin.place(r, make_placed(spec, idx, &r, rot));
        }
        if bin.used.is_empty() {
            // nemalo by nastať (diely sú vopred overené), poistka proti nekonečnému cyklu
            break;
        }
        bins.push(bin);
    }
    finish(bins)
}

fn finish(bins: Vec<Bin>) -> Vec<PackedSheet> {
    bins.into_iter()
        .map(|b| {
            let mut placed: Vec<Placed> = b.used.into_iter().map(|(_, p)| p).collect();
            placed.sort_by(|a, b| {
                a.y.total_cmp(&b.y)
                    .then(a.x.total_cmp(&b.x))
                    .then(a.item.cmp(&b.item))
            });
            PackedSheet { placed }
        })
        .collect()
}

fn best_in_bin(
    bin: &Bin,
    item: &Item,
    orient: Orient,
    s: f64,
    heur: Heuristic,
) -> Option<(Rect, bool)> {
    let mut best: Option<(Score, Rect, bool)> = None;
    for (w, h, rot) in orientations(item) {
        let allowed = match orient {
            Orient::Auto => true,
            Orient::Normal => !rot,
            Orient::Turned => rot,
        };
        if !allowed {
            continue;
        }
        if let Some((score, r)) = bin.find(w + s, h + s, heur) {
            if best.as_ref().is_none_or(|(b, ..)| better(score, *b)) {
                best = Some((score, r, rot));
            }
        }
    }
    best.map(|(_, r, rot)| (r, rot))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sheet() -> SheetSpec {
        SheetSpec {
            length: 2800.0,
            width: 2070.0,
            margin: 10.0,
            spacing: 14.2,
        }
    }

    fn item(l: f64, w: f64) -> Item {
        Item {
            length: l,
            width: w,
            rotation: Rotation::Free,
        }
    }

    /// Nič sa neprekrýva (vrátane medzery), všetko je v okrajoch.
    pub(crate) fn assert_valid(p: &Packing, spec: &SheetSpec) {
        let tol = 1e-6;
        for sheet in &p.sheets {
            for a in &sheet.placed {
                assert!(
                    a.x >= spec.margin - tol && a.y >= spec.margin - tol,
                    "{a:?}"
                );
                assert!(a.x + a.dx <= spec.length - spec.margin + tol, "{a:?}");
                assert!(a.y + a.dy <= spec.width - spec.margin + tol, "{a:?}");
            }
            for (i, a) in sheet.placed.iter().enumerate() {
                for b in sheet.placed.iter().skip(i + 1) {
                    let sep_x = a.x + a.dx + spec.spacing <= b.x + tol
                        || b.x + b.dx + spec.spacing <= a.x + tol;
                    let sep_y = a.y + a.dy + spec.spacing <= b.y + tol
                        || b.y + b.dy + spec.spacing <= a.y + tol;
                    assert!(sep_x || sep_y, "prekrytie alebo malá medzera: {a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn empty_input() {
        let p = pack(&[], &sheet());
        assert!(p.sheets.is_empty() && p.unplaced.is_empty());
    }

    #[test]
    fn single_part_in_corner() {
        let p = pack(&[item(500.0, 300.0)], &sheet());
        assert_eq!(p.sheets.len(), 1);
        let a = p.sheets[0].placed[0];
        assert_eq!((a.x, a.y), (10.0, 10.0));
    }

    #[test]
    fn too_large_and_invalid_are_unplaced() {
        let items = [
            item(3000.0, 2100.0),
            item(f64::NAN, 100.0),
            item(-5.0, 100.0),
            item(100.0, 100.0),
        ];
        let p = pack(&items, &sheet());
        assert_eq!(p.unplaced, vec![0, 1, 2]);
        assert_eq!(p.sheets.len(), 1);
    }

    #[test]
    fn rotation_rules_are_respected() {
        // 2500 × 300 sa zmestí iba ležato; s vláknom po šírke sa musí otočiť → nezmestí sa
        let mut a = item(2500.0, 300.0);
        a.rotation = Rotation::Rotated;
        let mut b = item(2500.0, 300.0);
        b.rotation = Rotation::Fixed;
        let p = pack(&[a, b], &sheet());
        assert_eq!(p.unplaced, vec![0]);
        assert!(!p.sheets[0].placed[0].rotated);
        let mut c = item(300.0, 600.0);
        c.rotation = Rotation::Rotated;
        let p = pack(&[c], &sheet());
        let pl = p.sheets[0].placed[0];
        assert!(pl.rotated);
        assert_eq!((pl.dx, pl.dy), (600.0, 300.0));
    }

    #[test]
    fn many_parts_valid_and_deterministic() {
        let mut items = Vec::new();
        for i in 0..60 {
            let l = 200.0 + f64::from(i * 37 % 700);
            let w = 100.0 + f64::from(i * 53 % 500);
            items.push(item(l, w));
        }
        let spec = sheet();
        let p1 = pack(&items, &spec);
        let p2 = pack(&items, &spec);
        assert_eq!(p1, p2);
        assert_valid(&p1, &spec);
        let placed: usize = p1.sheets.iter().map(|s| s.placed.len()).sum();
        assert_eq!(placed, items.len());
        // spodná hranica počtu tabúľ podľa plochy
        let area: f64 = items.iter().map(|i| i.length * i.width).sum();
        let min = (area / (spec.length * spec.width)).ceil() as usize;
        assert!(
            p1.sheets.len() <= min + 1,
            "{} tabúľ, minimum {min}",
            p1.sheets.len()
        );
    }

    #[test]
    fn exact_spacing_between_neighbours() {
        // dva diely 1380 široké sa zmestia vedľa seba iba s presnou medzerou
        let w = (2800.0 - 20.0 - 14.2) / 2.0;
        let p = pack(&[item(w, 500.0), item(w, 500.0)], &sheet());
        assert_eq!(p.sheets.len(), 1);
        assert_eq!(p.sheets[0].placed.len(), 2);
        assert_valid(&p, &sheet());
    }
}
