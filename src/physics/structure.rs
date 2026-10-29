//! Airframe strength: wing, stabiliser and fin bending and engine pylon
//! inertia loads, each held against its limit load and the CS 25.303
//! ultimate factor. Past limit a member takes a permanent set and loses part
//! of its ultimate margin for good; past what is left of it, it breaks.
//!
//! The airloads are X-Plane's own blade-element lift; the wing's structure,
//! engines and fuel relieve it by their inertia. Limit loads are the CS
//! 25.337 manoeuvres at the design weights, the lift spread the way X-Plane
//! spreads it in clean, wings-level flight, plus a full-aileron roll at VA.

use serde::{Deserialize, Serialize};

const G: f64 = 9.80665;
const FT: f64 = 0.3048;
const KT: f64 = 0.514444;

pub const PIECES: usize = 11;
pub const ELEMENTS: usize = 10;

/// CS 25.303.
pub const ULTIMATE: f64 = 1.5;
/// However often it is overstressed, a member still carries limit load.
pub const MIN_ULTIMATE: f64 = 1.05;
/// Ultimate margin lost per unit of overload past limit (generic).
const MARGIN_LOST_PER_OVERLOAD: f64 = 0.5;

/// CS 25.337.
const MANOEUVRE_N: f64 = 2.5;
const NEGATIVE_N: f64 = -1.0;
/// Taxi and landing load factor carried with no lift (generic).
const GROUND_N: f64 = 2.5;
/// Per side (generic).
const WING_STRUCTURE_KG: f64 = 33_000.0;
/// Engine, nacelle and pylon (generic).
pub const ENGINE_INSTALLED_KG: f64 = 9_500.0;
/// WV008 and WV011, the heaviest weight variants.
const DESIGN_MTOW_KG: f64 = 575_000.0;
const DESIGN_MZFW_KG: f64 = 373_000.0;
/// The wings' share of the aircraft's lift until X-Plane has shown its own.
const WING_LIFT_SHARE: f64 = 1.05;
/// Equivalent airspeeds (generic): VA, and VD as VMO plus a dive margin.
const VA_MS: f64 = 250.0 * KT;
const VD_MS: f64 = 365.0 * KT;
const RHO_0: f64 = 1.225;
/// Section lift a fully deflected aileron adds (generic).
const AILERON_DESIGN_DCL: f64 = 0.6;
/// Lift coefficients at VD the stabiliser and fin limit loads stand for (generic).
const STABILISER_DESIGN_CL: f64 = 0.6;
const FIN_DESIGN_CL: f64 = 0.4;
/// Up and down (generic).
const PYLON_LIMIT_N: [f64; 2] = [4.0, 2.0];
/// About the wing's first bending period: shorter spikes do not load it.
const RESPONSE_S: f64 = 0.15;
const CALIBRATION_S: f64 = 10.0;

/// Lateral metres: the fuselage side, then the .acf's wing 2, 3 and 4 roots.
pub const WING_STATION_M: [f64; 4] = [3.57, 11.718, 19.225, 32.646];
pub const ENGINE_LATERAL_M: [f64; 4] = [25.6, 14.84, 14.84, 25.6];
/// Outboard engine first.
pub const SIDE_ENGINES: [[usize; 2]; 2] = [[0, 1], [3, 2]];
/// FBW tank numbers, outboard in.
pub const SIDE_TANKS: [[usize; 5]; 2] = [[1, 2, 3, 4, 5], [10, 9, 8, 7, 6]];
/// flight_model.cfg `Tank.n` Position, lateral.
const TANK_LATERAL_M: [f64; 5] = [100.0 * FT, 71.0 * FT, 45.9 * FT, 24.7 * FT, 18.4 * FT];
/// flight_model.cfg capacities, gallons.
const TANK_GAL: [f64; 5] = [2731.5, 7299.6, 9632.0, 12189.4, 7753.2];
const FUEL_KG_PER_GAL: f64 = crate::fuel::JET_A_LBS_PER_GAL * 0.45359237;

/// `.acf` `_wing/N`: `_part_x`, `_semilen_SEG` (ft), `_sweep_design`,
/// `_dihed_design` (deg), `_Croot`, `_Ctip` (ft), `_els`. Even N is left.
const ACF: [(f64, f64, f64, f64, f64, f64, usize); PIECES] = [
    (0.0, 46.851937399, 34.462040923, 5.6, 64.645932727, 42.978074437, 10),
    (0.0, 46.851937399, 34.462040923, 5.6, 64.645932727, 42.978074437, 10),
    (38.445118263, 29.877354231, 34.075371652, 5.6, 42.978074437, 34.14587097, 6),
    (38.445118263, 29.877354231, 34.075371652, 5.6, 42.978074437, 34.14587097, 6),
    (63.074458333, 52.891496399, 33.227641149, 5.6, 34.14587097, 20.188401161, 10),
    (63.074458333, 52.891496399, 33.227641149, 5.6, 34.14587097, 20.188401161, 10),
    (107.107040343, 27.945241533, 31.176678816, 5.387038115, 20.188401161, 12.945491488, 6),
    (107.107040343, 27.945241533, 31.176678816, 5.387038115, 20.188401161, 12.945491488, 6),
    (0.0, 59.922302676, 33.387609986, 6.530352223, 32.093446338, 12.215859661, 10),
    (0.0, 59.922302676, 33.387609986, 6.530352223, 32.093446338, 12.215859661, 10),
    (0.0, 57.613371839, 40.149354942, 90.0, 36.495835463, 15.235100382, 10),
];
const STABILISER: usize = 8;
const FIN: usize = 10;
/// X-Plane's aileron sets: wing 3 elements 6-9, wing 4 elements 0-4.
const AILERON_ELEMENTS: [(usize, std::ops::Range<usize>); 2] = [(4, 6..10), (6, 0..5)];

#[derive(Clone, Copy, Default)]
struct Element {
    /// From the centreline, or up from the fin root.
    at_m: f64,
    chord_m: f64,
    width_m: f64,
    /// Chord times its share of `_semilen_SEG`, as X-Plane sizes it.
    area_m2: f64,
}

#[derive(Clone, Copy, Default)]
struct Piece {
    count: usize,
    elements: [Element; ELEMENTS],
}

impl Piece {
    fn elements(&self) -> &[Element] {
        &self.elements[..self.count]
    }
}

fn layout() -> &'static [Piece; PIECES] {
    static LAYOUT: std::sync::OnceLock<[Piece; PIECES]> = std::sync::OnceLock::new();
    LAYOUT.get_or_init(|| {
        ACF.map(|(root, semilen, sweep, dihedral, root_chord, tip_chord, count)| {
            let extent = semilen * sweep.to_radians().cos();
            let span = extent * if dihedral < 45.0 { dihedral.to_radians().cos() } else { dihedral.to_radians().sin() };
            let mut piece = Piece { count, ..Default::default() };
            for (i, e) in piece.elements[..count].iter_mut().enumerate() {
                let t = (i as f64 + 0.5) / count as f64;
                let chord = root_chord + t * (tip_chord - root_chord);
                *e = Element { at_m: (root.abs() + t * span) * FT, chord_m: chord * FT, width_m: span / count as f64 * FT, area_m2: chord * semilen / count as f64 * FT * FT };
            }
            piece
        })
    })
}

fn wing_pieces(side: usize) -> [usize; 4] {
    [side, 2 + side, 4 + side, 6 + side]
}

/// Lateral metres and kg, one side, outboard of the fuselage.
fn wing_structure() -> &'static [(f64, f64)] {
    static MASSES: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();
    MASSES.get_or_init(|| {
        let exposed: Vec<Element> = wing_pieces(0).iter().flat_map(|&p| layout()[p].elements().iter().copied()).filter(|e| e.at_m > WING_STATION_M[0]).collect();
        let area: f64 = exposed.iter().map(|e| e.chord_m * e.width_m).sum();
        exposed.iter().map(|e| (e.at_m, WING_STRUCTURE_KG * e.chord_m * e.width_m / area)).collect()
    })
}

/// Every wing element's share of one wing's lift, by Schrenk's
/// approximation: the mean of the planform and the ellipse of equal area.
fn schrenk_shares() -> Vec<(usize, usize, f64)> {
    let elements: Vec<(usize, usize, Element)> = wing_pieces(0).iter().flat_map(|&p| layout()[p].elements().iter().enumerate().map(move |(i, e)| (p, i, *e))).collect();
    let semispan = elements.iter().map(|(_, _, e)| e.at_m + e.width_m / 2.0).fold(0.0, f64::max);
    let area: f64 = elements.iter().map(|(_, _, e)| e.chord_m * e.width_m).sum();
    let ellipse_root = 4.0 * area / (std::f64::consts::PI * semispan);
    let weights: Vec<f64> = elements.iter().map(|(_, _, e)| (e.chord_m + ellipse_root * (1.0 - (e.at_m / semispan).powi(2)).max(0.0).sqrt()) / 2.0 * e.width_m).collect();
    let total: f64 = weights.iter().sum();
    elements.iter().zip(weights).map(|((p, i, _), w)| (*p, *i, w / total)).collect()
}

/// Moment of one wing's lift outboard of each station, per newton of it.
fn schrenk_arm() -> [f64; 4] {
    let shares = schrenk_shares();
    WING_STATION_M.map(|s| shares.iter().map(|(p, i, w)| w * (layout()[*p].elements[*i].at_m - s).max(0.0)).sum())
}

/// What a fully deflected aileron adds to each station's bending at VA.
fn roll_allowance() -> [f64; 4] {
    let q = 0.5 * RHO_0 * VA_MS * VA_MS;
    WING_STATION_M.map(|s| {
        AILERON_ELEMENTS
            .iter()
            .flat_map(|(p, range)| layout()[*p].elements[range.clone()].iter())
            .map(|e| q * AILERON_DESIGN_DCL * e.area_m2 * (e.at_m - s).max(0.0))
            .sum()
    })
}

/// Σ m·d and Σ m·y·d of the masses outboard of each station: d from the
/// station, y from the centreline.
fn inertia(masses: &[(f64, f64)]) -> [(f64, f64); 4] {
    let mut out = [(0.0, 0.0); 4];
    for &(y, kg) in masses {
        for (s, station) in WING_STATION_M.iter().enumerate() {
            if y > *station {
                out[s].0 += kg * (y - station);
                out[s].1 += kg * y * (y - station);
            }
        }
    }
    out
}

/// One wing's masses still attached inboard of `beyond_m`.
fn side_masses(tank_kg: [f64; 5], engines: [bool; 2], beyond_m: f64) -> Vec<(f64, f64)> {
    let engine_y = [ENGINE_LATERAL_M[0], ENGINE_LATERAL_M[1]];
    wing_structure()
        .iter()
        .copied()
        .chain(TANK_LATERAL_M.into_iter().zip(tank_kg))
        .chain(engine_y.into_iter().zip(engines).filter(|(_, on)| *on).map(|(y, _)| (y, ENGINE_INSTALLED_KG)))
        .filter(|(y, _)| *y <= beyond_m)
        .collect()
}

/// Lift (N) on every element: X-Plane's lift coefficient at the element's
/// own airspeed, on its area. Flat `[wing][element]` arrays.
pub fn element_lift(rho: f64, cl: &[f32], speed_ms: &[f32], area_m2: &[f32]) -> [[f64; ELEMENTS]; PIECES] {
    let mut out = [[0.0; ELEMENTS]; PIECES];
    for (p, row) in out.iter_mut().enumerate() {
        for (i, lift) in row.iter_mut().enumerate() {
            let k = p * ELEMENTS + i;
            let get = |a: &[f32]| a.get(k).copied().unwrap_or(0.0) as f64;
            *lift = 0.5 * rho * get(speed_ms).powi(2) * get(area_m2) * get(cl);
        }
    }
    out
}

pub fn element_areas(area_m2: &[f32]) -> [[f64; ELEMENTS]; PIECES] {
    std::array::from_fn(|p| std::array::from_fn(|i| area_m2.get(p * ELEMENTS + i).copied().unwrap_or(0.0) as f64))
}

fn root_moment(per_element: &[[f64; ELEMENTS]; PIECES], piece: usize) -> f64 {
    layout()[piece].elements().iter().zip(per_element[piece]).map(|(e, v)| v * e.at_m).sum()
}

fn wing_aero(lift: &[[f64; ELEMENTS]; PIECES], wing_break: [Option<usize>; 2]) -> ([[f64; 4]; 2], [f64; 2]) {
    let mut moments = [[0.0; 4]; 2];
    let mut side_lift = [0.0; 2];
    for side in 0..2 {
        for p in wing_pieces(side).into_iter().take(wing_break[side].unwrap_or(4)) {
            for (e, l) in layout()[p].elements().iter().zip(lift[p]) {
                side_lift[side] += l;
                for (s, station) in WING_STATION_M.iter().enumerate() {
                    moments[side][s] += l * (e.at_m - station).max(0.0);
                }
            }
        }
    }
    (moments, side_lift)
}

fn ratio(load: f64, up: f64, down: f64) -> f64 {
    let limit = if load >= 0.0 { up } else { down };
    if limit > 0.0 { load.abs() / limit } else { 0.0 }
}

/// A break at `station` takes the engine with the wing.
pub fn takes_engine(station: usize, engine: usize) -> bool {
    ENGINE_LATERAL_M[engine] > WING_STATION_M[station]
}

pub fn side_of_engine(engine: usize) -> usize {
    usize::from(engine >= 2)
}

/// FBW tank numbers a break at `station` takes with the wing.
pub fn tanks_lost(side: usize, station: usize) -> Vec<usize> {
    SIDE_TANKS[side].iter().zip(TANK_LATERAL_M).filter(|(_, y)| *y > WING_STATION_M[station]).map(|(n, _)| *n).collect()
}

/// The tank a break at `station` tears open: the next one inboard of it.
pub fn tank_torn(side: usize, station: usize) -> Option<usize> {
    SIDE_TANKS[side].iter().zip(TANK_LATERAL_M).find(|(_, y)| *y <= WING_STATION_M[station]).map(|(n, _)| *n)
}

/// Mass (kg) a break at `station` takes away, structure and engines, and
/// its lateral centroid (m).
pub fn carried_away(station: usize) -> (f64, f64) {
    let masses = side_masses([0.0; 5], [true; 2], f64::INFINITY);
    let gone: Vec<&(f64, f64)> = masses.iter().filter(|(y, _)| *y > WING_STATION_M[station]).collect();
    let kg: f64 = gone.iter().map(|(_, m)| m).sum();
    let at = if kg > 0.0 { gone.iter().map(|(y, m)| y * m).sum::<f64>() / kg } else { 0.0 };
    (kg, at)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Member {
    Wing { side: usize, station: usize },
    Stabiliser(usize),
    Fin,
    Pylon(usize),
}

impl Member {
    pub fn all() -> Vec<Member> {
        let mut v: Vec<Member> = (0..2).flat_map(|side| (0..4).map(move |station| Member::Wing { side, station })).collect();
        v.extend([Member::Stabiliser(0), Member::Stabiliser(1), Member::Fin]);
        v.extend((0..4).map(Member::Pylon));
        v
    }

    pub fn describe(self) -> String {
        const SIDE: [&str; 2] = ["left", "right"];
        match self {
            Member::Wing { side, station } => format!("{} wing at {:.1} m", SIDE[side], WING_STATION_M[station]),
            Member::Stabiliser(side) => format!("{} horizontal stabiliser", SIDE[side]),
            Member::Fin => "fin".into(),
            Member::Pylon(e) => format!("engine {} pylon", e + 1),
        }
    }

    /// The part separation this member's failure is.
    pub fn separation_failure(self) -> u64 {
        use crate::failures::extra::{ENGINE_SEPARATION, FIN_SEPARATION, HSTAB_SEPARATION};
        match self {
            Member::Wing { side, station } => wing_break_failure(side, station),
            Member::Stabiliser(side) => HSTAB_SEPARATION + side as u64,
            Member::Fin => FIN_SEPARATION,
            Member::Pylon(e) => ENGINE_SEPARATION + e as u64,
        }
    }
}

/// The wing separation at `station`; the outermost is the existing outer wing one.
pub fn wing_break_failure(side: usize, station: usize) -> u64 {
    use crate::failures::extra::{WINGTIP_SEPARATION, WING_SEPARATION};
    if station == 3 { WINGTIP_SEPARATION + side as u64 } else { WING_SEPARATION + (side * 3 + station) as u64 }
}

/// Each member's remaining ultimate strength, in multiples of its limit load.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Strength {
    pub wing: [[f64; 4]; 2],
    pub stabiliser: [f64; 2],
    pub fin: f64,
    pub pylon: [f64; 4],
}

impl Default for Strength {
    fn default() -> Self {
        Self::filled(ULTIMATE)
    }
}

impl Strength {
    fn filled(v: f64) -> Self {
        Self { wing: [[v; 4]; 2], stabiliser: [v; 2], fin: v, pylon: [v; 4] }
    }

    fn at(&mut self, m: Member) -> &mut f64 {
        match m {
            Member::Wing { side, station } => &mut self.wing[side][station],
            Member::Stabiliser(side) => &mut self.stabiliser[side],
            Member::Fin => &mut self.fin,
            Member::Pylon(e) => &mut self.pylon[e],
        }
    }

    /// A saved value this model could not have produced is a sound member.
    pub fn sanitized(mut self) -> Self {
        for m in Member::all() {
            let v = self.at(m);
            if !(MIN_ULTIMATE..=ULTIMATE).contains(v) {
                *v = ULTIMATE;
            }
        }
        self
    }

    fn overstress_failures(&self) -> Vec<(u64, f64)> {
        use crate::failures::extra::{FIN_OVERSTRESS, PYLON_OVERSTRESS, STABILISER_OVERSTRESS, WING_OVERSTRESS};
        let mut v = Vec::new();
        for side in 0..2 {
            v.push((WING_OVERSTRESS + side as u64, self.wing[side].iter().copied().fold(ULTIMATE, f64::min)));
            v.push((STABILISER_OVERSTRESS + side as u64, self.stabiliser[side]));
        }
        v.push((FIN_OVERSTRESS, self.fin));
        v.extend((0..4).map(|e| (PYLON_OVERSTRESS + e as u64, self.pylon[e])));
        v
    }

    /// Each overstressed part's failure and how much of its margin is gone.
    pub fn overstress_levels(&self) -> Vec<(u64, f64)> {
        self.overstress_failures()
            .into_iter()
            .filter(|(_, left)| *left < ULTIMATE)
            .map(|(id, left)| (id, ((ULTIMATE - left) / (ULTIMATE - MIN_ULTIMATE)).clamp(0.01, 1.0)))
            .collect()
    }

    /// Restore the part an overstress failure stands for. False for any other id.
    pub fn repair(&mut self, id: u64) -> bool {
        use crate::failures::extra::{FIN_OVERSTRESS, PYLON_OVERSTRESS, STABILISER_OVERSTRESS, WING_OVERSTRESS};
        match id {
            _ if (WING_OVERSTRESS..WING_OVERSTRESS + 2).contains(&id) => self.wing[(id - WING_OVERSTRESS) as usize] = [ULTIMATE; 4],
            _ if (STABILISER_OVERSTRESS..STABILISER_OVERSTRESS + 2).contains(&id) => self.stabiliser[(id - STABILISER_OVERSTRESS) as usize] = ULTIMATE,
            _ if id == FIN_OVERSTRESS => self.fin = ULTIMATE,
            _ if (PYLON_OVERSTRESS..PYLON_OVERSTRESS + 4).contains(&id) => self.pylon[(id - PYLON_OVERSTRESS) as usize] = ULTIMATE,
            _ => return false,
        }
        true
    }
}

pub struct Inputs<'a> {
    pub lift_n: &'a [[f64; ELEMENTS]; PIECES],
    pub area_m2: &'a [[f64; ELEMENTS]; PIECES],
    pub n_z: f64,
    /// Positive rolling right.
    pub roll_accel_rad_s2: f64,
    pub mass_kg: f64,
    /// By FBW tank number less one.
    pub tank_kg: [f64; 10],
    pub engine_attached: [bool; 4],
    pub wing_break: [Option<usize>; 2],
    pub stabiliser_gone: [bool; 2],
    pub fin_gone: bool,
    /// Clean, wings-level, unhurried flight: X-Plane's lift shape is the one
    /// the limit loads are spread by.
    pub steady_clean: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Overstressed { member: Member, peak: f64, ultimate: f64 },
    Broke { member: Member, ratio: f64 },
}

#[derive(Clone, Copy, Default)]
struct Loads {
    wing: [[f64; 4]; 2],
    stabiliser: [f64; 2],
    fin: f64,
    pylon: [f64; 4],
}

impl Loads {
    fn toward(mut self, now: &Loads, a: f64) -> Loads {
        let step = |x: &mut f64, y: f64| *x += (y - *x) * a;
        for side in 0..2 {
            for s in 0..4 {
                step(&mut self.wing[side][s], now.wing[side][s]);
            }
            step(&mut self.stabiliser[side], now.stabiliser[side]);
        }
        step(&mut self.fin, now.fin);
        for e in 0..4 {
            step(&mut self.pylon[e], now.pylon[e]);
        }
        self
    }
}

pub struct Structure {
    pub strength: Strength,
    peak: Strength,
    loads: Option<Loads>,
    arm: [f64; 4],
    wing_lift_share: f64,
    relief_full: [f64; 4],
    relief_empty: [f64; 4],
    roll: [f64; 4],
    broken: Vec<Member>,
}

impl Default for Structure {
    fn default() -> Self {
        Self::new()
    }
}

impl Structure {
    pub fn new() -> Self {
        let full = TANK_GAL.map(|g| g * FUEL_KG_PER_GAL);
        Self {
            strength: Strength::default(),
            peak: Strength::filled(0.0),
            loads: None,
            arm: schrenk_arm(),
            wing_lift_share: WING_LIFT_SHARE,
            relief_full: inertia(&side_masses(full, [true; 2], f64::INFINITY)).map(|m| m.0),
            relief_empty: inertia(&side_masses([0.0; 5], [true; 2], f64::INFINITY)).map(|m| m.0),
            roll: roll_allowance(),
            broken: Vec::new(),
        }
    }

    /// Limit bending (N·m) at each station, up and down.
    fn wing_limits(&self) -> ([f64; 4], [f64; 4]) {
        let mut up = [0.0; 4];
        let mut down = [0.0; 4];
        for s in 0..4 {
            for (kg, relief) in [(DESIGN_MTOW_KG, self.relief_full[s]), (DESIGN_MZFW_KG, self.relief_empty[s])] {
                let per_g = self.wing_lift_share * kg * G / 2.0 * self.arm[s] - G * relief;
                up[s] = f64::max(up[s], MANOEUVRE_N * per_g);
                down[s] = f64::max(down[s], f64::max(-NEGATIVE_N * per_g, GROUND_N * G * relief));
            }
            up[s] += self.roll[s];
            down[s] += self.roll[s];
        }
        (up, down)
    }

    fn calibrate(&mut self, aero: &[[f64; 4]; 2], side_lift: [f64; 2], weight_n: f64, dt: f64) {
        let total = side_lift[0] + side_lift[1];
        if side_lift.iter().any(|l| *l <= 0.0) || (side_lift[0] - side_lift[1]).abs() > 0.05 * total || weight_n <= 0.0 {
            return;
        }
        let a = 1.0 - (-dt / CALIBRATION_S).exp();
        for s in 0..4 {
            let arm = (aero[0][s] / side_lift[0] + aero[1][s] / side_lift[1]) / 2.0;
            self.arm[s] += (arm - self.arm[s]) * a;
        }
        self.wing_lift_share += ((total / weight_n).clamp(0.8, 1.3) - self.wing_lift_share) * a;
    }

    pub fn step(&mut self, input: &Inputs, dt: f64) -> Vec<Event> {
        let (aero, side_lift) = wing_aero(input.lift_n, input.wing_break);
        if input.steady_clean && input.wing_break == [None; 2] {
            self.calibrate(&aero, side_lift, input.mass_kg * G * input.n_z, dt);
        }
        let mut now = Loads::default();
        for side in 0..2 {
            let beyond = input.wing_break[side].map_or(f64::INFINITY, |k| WING_STATION_M[k]);
            let tanks = SIDE_TANKS[side].map(|n| input.tank_kg[n - 1]);
            let engines = SIDE_ENGINES[side].map(|e| input.engine_attached[e]);
            let roll = if side == 0 { input.roll_accel_rad_s2 } else { -input.roll_accel_rad_s2 };
            for (s, (m, my)) in inertia(&side_masses(tanks, engines, beyond)).into_iter().enumerate() {
                now.wing[side][s] = aero[side][s] - (G * input.n_z * m + roll * my);
            }
            now.stabiliser[side] = root_moment(input.lift_n, STABILISER + side);
            for e in SIDE_ENGINES[side] {
                now.pylon[e] = input.n_z + roll * ENGINE_LATERAL_M[e] / G;
            }
        }
        now.fin = root_moment(input.lift_n, FIN);
        let loads = match self.loads {
            Some(l) => l.toward(&now, 1.0 - (-dt / RESPONSE_S).exp()),
            None => now,
        };
        self.loads = Some(loads);
        let ratios = self.ratios(&loads, input);
        self.judge(ratios)
    }

    fn tail_limit(area: &[[f64; ELEMENTS]; PIECES], piece: usize, cl: f64) -> f64 {
        let measured = root_moment(area, piece);
        let arm_area = if measured > 0.0 { measured } else { layout()[piece].elements().iter().map(|e| e.area_m2 * e.at_m).sum() };
        0.5 * RHO_0 * VD_MS * VD_MS * cl * arm_area
    }

    fn ratios(&self, l: &Loads, input: &Inputs) -> Vec<(Member, f64)> {
        let (up, down) = self.wing_limits();
        let mut out = Vec::new();
        for side in 0..2 {
            for station in 0..input.wing_break[side].unwrap_or(4) {
                out.push((Member::Wing { side, station }, ratio(l.wing[side][station], up[station], down[station])));
            }
            if !input.stabiliser_gone[side] {
                let limit = Self::tail_limit(input.area_m2, STABILISER + side, STABILISER_DESIGN_CL);
                out.push((Member::Stabiliser(side), ratio(l.stabiliser[side], limit, limit)));
            }
        }
        if !input.fin_gone {
            let limit = Self::tail_limit(input.area_m2, FIN, FIN_DESIGN_CL);
            out.push((Member::Fin, ratio(l.fin, limit, limit)));
        }
        for e in (0..4).filter(|e| input.engine_attached[*e]) {
            out.push((Member::Pylon(e), ratio(l.pylon[e], PYLON_LIMIT_N[0], PYLON_LIMIT_N[1])));
        }
        out
    }

    fn judge(&mut self, ratios: Vec<(Member, f64)>) -> Vec<Event> {
        let mut events = Vec::new();
        for (member, r) in ratios {
            if self.broken.contains(&member) {
                continue;
            }
            let ultimate = *self.strength.at(member);
            if r > ultimate {
                self.broken.push(member);
                events.push(Event::Broke { member, ratio: r });
                continue;
            }
            let peak = *self.peak.at(member);
            if r > 1.0 {
                *self.peak.at(member) = peak.max(r);
            } else if peak > 1.0 {
                let left = (ultimate - MARGIN_LOST_PER_OVERLOAD * (peak - 1.0)).max(MIN_ULTIMATE);
                *self.strength.at(member) = left;
                *self.peak.at(member) = 0.0;
                events.push(Event::Overstressed { member, peak, ultimate: left });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: [f64; 10] = {
        let mut t = [0.0; 10];
        let mut i = 0;
        while i < 5 {
            t[SIDE_TANKS[0][i] - 1] = TANK_GAL[i] * FUEL_KG_PER_GAL;
            t[SIDE_TANKS[1][i] - 1] = TANK_GAL[i] * FUEL_KG_PER_GAL;
            i += 1;
        }
        t
    };

    fn areas() -> [[f64; ELEMENTS]; PIECES] {
        std::array::from_fn(|p| std::array::from_fn(|i| if i < layout()[p].count { layout()[p].elements[i].area_m2 } else { 0.0 }))
    }

    fn wing_lift(side_n: [f64; 2]) -> [[f64; ELEMENTS]; PIECES] {
        let mut lift = [[0.0; ELEMENTS]; PIECES];
        for (p, i, w) in schrenk_shares() {
            lift[p][i] = w * side_n[0];
            lift[p + 1][i] = w * side_n[1];
        }
        lift
    }

    struct Flight {
        mass_kg: f64,
        tank_kg: [f64; 10],
        n_z: f64,
        roll: f64,
        aileron_dcl_q: [f64; 2],
        on_ground: bool,
    }

    impl Flight {
        fn new(mass_kg: f64, tank_kg: [f64; 10], n_z: f64) -> Self {
            Flight { mass_kg, tank_kg, n_z, roll: 0.0, aileron_dcl_q: [0.0; 2], on_ground: false }
        }

        fn lift(&self) -> [[f64; ELEMENTS]; PIECES] {
            let side = if self.on_ground { 0.0 } else { self.n_z * self.mass_kg * G * WING_LIFT_SHARE / 2.0 };
            let mut lift = wing_lift([side; 2]);
            for side in 0..2 {
                for (p, range) in &AILERON_ELEMENTS {
                    for i in range.clone() {
                        lift[p + side][i] += self.aileron_dcl_q[side] * layout()[*p].elements[i].area_m2;
                    }
                }
            }
            lift
        }

        fn fly(&self, s: &mut Structure, seconds: f64) -> Vec<Event> {
            let lift = self.lift();
            let area = areas();
            let input = Inputs {
                lift_n: &lift,
                area_m2: &area,
                n_z: self.n_z,
                roll_accel_rad_s2: self.roll,
                mass_kg: self.mass_kg,
                tank_kg: self.tank_kg,
                engine_attached: [true; 4],
                wing_break: [None; 2],
                stabiliser_gone: [false; 2],
                fin_gone: false,
                steady_clean: false,
            };
            let mut events = Vec::new();
            for _ in 0..(seconds / 0.05) as usize {
                events.extend(s.step(&input, 0.05));
            }
            events
        }

        fn peak_wing_ratio(&self) -> f64 {
            let mut s = Structure::new();
            self.fly(&mut s, 2.0);
            let loads = s.loads.unwrap();
            let (up, down) = s.wing_limits();
            (0..2).flat_map(|side| (0..4).map(move |st| (side, st))).map(|(side, st)| ratio(loads.wing[side][st], up[st], down[st])).fold(0.0, f64::max)
        }
    }

    fn half_fuel() -> [f64; 10] {
        FULL.map(|kg| kg / 2.0)
    }

    #[test]
    fn the_layout_matches_the_acf_piece_roots_and_tip() {
        let l = layout();
        for (k, station) in WING_STATION_M.iter().enumerate().skip(1) {
            let p = &l[2 * k];
            assert!((p.elements[0].at_m - p.elements[0].width_m / 2.0 - station).abs() < 0.01, "piece {k}");
        }
        let tip = &l[6].elements[5];
        assert!((tip.at_m + tip.width_m / 2.0 - 39.90).abs() < 0.05);
        let fin = &l[FIN].elements[9];
        assert!((fin.at_m + fin.width_m / 2.0 - 13.42).abs() < 0.05);
    }

    #[test]
    fn cruise_and_parked_loads_are_far_inside_limit() {
        assert!(Flight::new(500_000.0, half_fuel(), 1.0).peak_wing_ratio() < 0.5);
        let parked = Flight { on_ground: true, ..Flight::new(575_000.0, FULL, 1.0) };
        assert!(parked.peak_wing_ratio() < 0.5);
    }

    #[test]
    fn the_manoeuvring_limit_at_any_weight_never_overstresses() {
        for (mass, fuel) in [(575_000.0, FULL), (373_000.0, [0.0; 10]), (450_000.0, half_fuel()), (300_000.0, [0.0; 10])] {
            let r = Flight::new(mass, fuel, 2.5).peak_wing_ratio();
            assert!(r <= 1.0, "{mass} kg: {r}");
        }
    }

    #[test]
    fn a_full_aileron_roll_at_the_manoeuvring_limit_and_va_stays_inside_limit() {
        let q = 0.5 * RHO_0 * VA_MS * VA_MS;
        let rolling = Flight { aileron_dcl_q: [q * AILERON_DESIGN_DCL, 0.0], ..Flight::new(575_000.0, FULL, 2.5) };
        assert!(rolling.peak_wing_ratio() <= 1.0 + 1e-9);
    }

    #[test]
    fn a_firm_landing_is_inside_limit() {
        let landing = Flight { on_ground: true, ..Flight::new(395_000.0, half_fuel(), 2.0) };
        assert!(landing.peak_wing_ratio() < 1.0);
    }

    #[test]
    fn fuel_in_the_wing_relieves_its_root() {
        let dry = Flight::new(500_000.0, [0.0; 10], 2.0);
        let wet = Flight::new(500_000.0, FULL, 2.0);
        let root = |f: &Flight| {
            let mut s = Structure::new();
            f.fly(&mut s, 2.0);
            s.loads.unwrap().wing[0][0]
        };
        assert!(root(&wet) < root(&dry) * 0.8);
    }

    #[test]
    fn a_heavy_pull_far_past_limit_breaks_the_wing() {
        let mut s = Structure::new();
        let events = Flight::new(560_000.0, half_fuel(), 6.0).fly(&mut s, 2.0);
        assert!(events.iter().any(|e| matches!(e, Event::Broke { member: Member::Wing { .. }, .. })), "{events:?}");
    }

    #[test]
    fn a_one_frame_spike_does_not_load_the_structure() {
        let mut s = Structure::new();
        let cruise = Flight::new(500_000.0, half_fuel(), 1.0);
        cruise.fly(&mut s, 1.0);
        let spike = Flight::new(500_000.0, half_fuel(), 9.0);
        let lift = spike.lift();
        let area = areas();
        let input = Inputs {
            lift_n: &lift,
            area_m2: &area,
            n_z: 9.0,
            roll_accel_rad_s2: 0.0,
            mass_kg: 500_000.0,
            tank_kg: half_fuel(),
            engine_attached: [true; 4],
            wing_break: [None; 2],
            stabiliser_gone: [false; 2],
            fin_gone: false,
            steady_clean: false,
        };
        assert!(s.step(&input, 0.016).is_empty());
        assert!(cruise.fly(&mut s, 2.0).is_empty());
        assert_eq!(s.strength, Strength::default());
    }

    #[test]
    fn an_overstress_short_of_ultimate_weakens_the_wing_for_good() {
        let per_g = Flight::new(575_000.0, FULL, 1.0).peak_wing_ratio();
        let at = |r: f64| Flight::new(575_000.0, FULL, r / per_g);
        let wing_broke = |events: &[Event]| events.iter().any(|e| matches!(e, Event::Broke { member: Member::Wing { .. }, .. }));
        let mut s = Structure::new();
        assert!(at(1.35).fly(&mut s, 2.0).is_empty(), "nothing is reported while the overload lasts");
        let events = Flight::new(575_000.0, FULL, 1.0).fly(&mut s, 2.0);
        let (peak, ultimate) = events
            .iter()
            .filter_map(|e| match e {
                Event::Overstressed { member: Member::Wing { .. }, peak, ultimate } => Some((*peak, *ultimate)),
                _ => None,
            })
            .fold((0.0, ULTIMATE), |a, b| if b.0 > a.0 { b } else { a });
        assert!((peak - 1.35).abs() < 0.01, "{events:?}");
        assert!(ultimate < 1.4);
        assert!(s.strength.overstress_levels().iter().any(|(id, _)| *id == crate::failures::extra::WING_OVERSTRESS));
        assert!(!wing_broke(&at(1.4).fly(&mut Structure::new(), 2.0)), "a sound wing holds");
        assert!(wing_broke(&at(1.4).fly(&mut s, 2.0)), "the weakened wing goes");
    }

    #[test]
    fn repairing_the_overstress_restores_the_wing() {
        let mut st = Strength::default();
        st.wing[1][2] = 1.2;
        let levels = st.overstress_levels();
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].0, crate::failures::extra::WING_OVERSTRESS + 1);
        assert!(st.repair(levels[0].0));
        assert_eq!(st, Strength::default());
        assert!(!st.repair(57_300));
    }

    #[test]
    fn a_saved_strength_out_of_range_reads_as_sound() {
        let mut st = Strength::default();
        st.fin = 7.0;
        st.pylon[2] = 1.1;
        let clean = st.sanitized();
        assert_eq!(clean.fin, ULTIMATE);
        assert_eq!(clean.pylon[2], 1.1);
    }

    #[test]
    fn a_stabiliser_at_its_design_lift_and_vd_is_at_limit() {
        let area = areas();
        let q = 0.5 * RHO_0 * VD_MS * VD_MS;
        let mut lift = [[0.0; ELEMENTS]; PIECES];
        for (i, a) in area[STABILISER].iter().enumerate() {
            lift[STABILISER][i] = -q * a * STABILISER_DESIGN_CL;
        }
        let l = Loads { stabiliser: [root_moment(&lift, STABILISER), 0.0], ..Default::default() };
        let limit = Structure::tail_limit(&area, STABILISER, STABILISER_DESIGN_CL);
        assert!((ratio(l.stabiliser[0], limit, limit) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_crash_landing_tears_the_engines_off_their_pylons() {
        let mut s = Structure::new();
        let crash = Flight { on_ground: true, ..Flight::new(395_000.0, half_fuel(), 7.0) };
        let events = crash.fly(&mut s, 1.0);
        for e in 0..4 {
            assert!(events.iter().any(|x| matches!(x, Event::Broke { member: Member::Pylon(n), .. } if *n == e)), "{e}: {events:?}");
        }
    }

    #[test]
    fn calibration_takes_x_planes_lift_shape() {
        let mut s = Structure::new();
        let schrenk = s.arm;
        let mut lift = [[0.0; ELEMENTS]; PIECES];
        for p in [4, 5, 6, 7] {
            for i in 0..layout()[p].count {
                lift[p][i] = 100_000.0;
            }
        }
        let (aero, side_lift) = wing_aero(&lift, [None; 2]);
        for _ in 0..1000 {
            s.calibrate(&aero, side_lift, side_lift[0] + side_lift[1], 0.1);
        }
        assert!(s.arm[3] > schrenk[3] * 2.0);
        assert!((s.wing_lift_share - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_break_takes_what_lies_outboard_of_it() {
        assert!(takes_engine(0, 1) && takes_engine(1, 1) && !takes_engine(2, 1));
        assert!(takes_engine(2, 0) && !takes_engine(3, 0));
        assert_eq!(tanks_lost(0, 0), vec![1, 2, 3, 4, 5]);
        assert_eq!(tanks_lost(1, 1), vec![10, 9, 8]);
        assert_eq!(tanks_lost(0, 3), Vec::<usize>::new());
        assert_eq!(tank_torn(0, 3), Some(1));
        assert_eq!(tank_torn(1, 2), Some(8));
        assert_eq!(tank_torn(0, 0), None);
        let (root_kg, root_at) = carried_away(0);
        let (tip_kg, tip_at) = carried_away(3);
        assert!(root_kg > WING_STRUCTURE_KG * 0.9 + 2.0 * ENGINE_INSTALLED_KG && tip_kg < 5_000.0);
        assert!(root_at < tip_at && tip_at > WING_STATION_M[3]);
    }

    #[test]
    fn member_failures_are_the_separations() {
        use crate::failures::extra::{ENGINE_SEPARATION, FIN_SEPARATION, WINGTIP_SEPARATION, WING_SEPARATION};
        assert_eq!(Member::Wing { side: 1, station: 3 }.separation_failure(), WINGTIP_SEPARATION + 1);
        assert_eq!(Member::Wing { side: 1, station: 2 }.separation_failure(), WING_SEPARATION + 5);
        assert_eq!(Member::Fin.separation_failure(), FIN_SEPARATION);
        assert_eq!(Member::Pylon(3).separation_failure(), ENGINE_SEPARATION + 3);
        assert_eq!(Member::all().len(), 15);
    }
}
