//! Per-wheel main-gear tyre model (hyperrealism physics workstream 6:
//! failures, damage, MEL and persistence; continuation of `damage.rs`'s
//! tyre-burst work). `docs/physics/failures.md` has the wider threshold
//! table; sources for this file's own constants are cited alongside each
//! one below.
//!
//! Three continuous quantities per wheel, all driven by real inputs this
//! plugin already owns or FlyByWire already publishes:
//! - nitrogen pressure via the ideal gas law (constant-volume/Gay-Lussac
//!   form: a sealed tyre's own volume barely changes, so `P/T` at a fixed
//!   nitrogen mass is a constant -- this is the same law aviation tyre
//!   placards use for the standard "2% pressure rise per 5 C" rule of
//!   thumb), scaled down by whatever fraction of the original nitrogen
//!   mass a leak has removed;
//! - tyre temperature, a first-order heat-soak from FlyByWire's own real
//!   `BRAKE_TEMPERATURE_n` (`a380_systems/src/hydraulic/mod.rs:2074-2101`,
//!   the same array `damage.rs` reads) plus rolling/flex heating that grows
//!   as inflation falls (an underinflated tyre flexes more per revolution
//!   and runs hotter -- standard tyre-engineering fact, no A380-specific
//!   coefficient published so the *coefficient* is a generic derived
//!   figure, not the *mechanism*), less cooling to ambient;
//! - a fuse-plug melt at [`damage::FUSE_PLUG_MELT_C`], now driven by this
//!   module's own soaked tyre temperature, never the brake temperature
//!   directly (an ordinary landing takes the carbon stack past 177 C).
//!
//! The leak rate and tread wear-pin consumption both come from
//! `failures::magnitude(id)` on the same per-leg id `damage.rs` already
//! arms at melt (`32_101`..`32_104`, left wing/right wing/left body/right
//! body -- there is no per-wheel id, matching X-Plane's own five
//! `rel_tireN` failure datarefs, one per leg incl. the nose). A fractional
//! magnitude below 1.0 is read as "this leg has an active slow leak/
//! puncture of this severity", continuous and non-binary
//! (`failures.rs`'s own doc comment on `set_magnitude`); reaching 1.0 (a
//! full burst) is this module's own doing once the physics crosses a real
//! threshold, not scripted.

use crate::failures;
#[cfg(test)]
use crate::physics::damage;
use crate::xp::{DataRef, Xplm};
use systems::simulation::VariableIdentifier;

pub use super::tyre_model::*;

/// All 16 main-gear wheels (`LEG_WHEEL_INDICES`' own ordering, matching
/// `BRAKE_TEMPERATURE_1..16`), plus the X-Plane/FlyByWire bindings needed
/// to drive them and publish their state.
pub struct Tyres {
    pub wheels: [TyreWheel; WHEELS],
    /// One per *braked* wheel only: the six unbraked tyres have no brake
    /// stack to read a temperature from.
    brake_temperature: [VariableIdentifier; BRAKED_WHEELS],
    pressure_out: [VariableIdentifier; WHEELS],
    temp_out: [VariableIdentifier; WHEELS],
    leak_out: [VariableIdentifier; WHEELS],
    tread_out: [VariableIdentifier; WHEELS],
    fuse_plug_out: [VariableIdentifier; WHEELS],
    ambient_c: Option<DataRef>,
    groundspeed_ms: Option<DataRef>,
    on_ground: Option<DataRef>,
    heading_deg: Option<DataRef>,
    tpis_out: [Option<VariableIdentifier>; WHEELS],
    last_brake_c: [f64; BRAKED_WHEELS],
    last_heading_deg: Option<f64>,
    was_on_ground: bool,
    pub events: Vec<String>,
}

/// One tyre's state carried from flight to flight in the airframe file.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTyre {
    pub leaked_fraction: f64,
    pub tread_mm: f64,
    pub flat: bool,
}

static LATEST: std::sync::Mutex<Option<Vec<SavedTyre>>> = std::sync::Mutex::new(None);
static RESTORE: std::sync::Mutex<Option<Vec<SavedTyre>>> = std::sync::Mutex::new(None);

/// The tyres as of the last update, for the airframe file.
pub fn snapshot() -> Option<Vec<SavedTyre>> {
    LATEST.lock().ok().and_then(|l| l.clone())
}

/// Hand the airframe file's tyres to the model; an empty list fits new
/// tyres on every wheel.
pub fn restore_airframe(saved: Vec<SavedTyre>) {
    if let Ok(mut l) = LATEST.lock() {
        *l = None;
    }
    if let Ok(mut r) = RESTORE.lock() {
        *r = Some(saved);
    }
}

pub fn apply_saved(wheels: &mut [TyreWheel; WHEELS], saved: &[SavedTyre]) {
    for (i, w) in wheels.iter_mut().enumerate() {
        match saved.get(i) {
            Some(s) => {
                w.leaked_fraction = s.leaked_fraction.clamp(0.0, 1.0);
                w.tread_mm = s.tread_mm.clamp(0.0, NEW_TREAD_DEPTH_MM);
                w.burst = s.flat;
                w.fuse_plug_melted = false;
            }
            None => w.replace(),
        }
    }
}

static GROUND_CREW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SERVICE_DUE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The GSE ground crew is at a parked, chocked aircraft with engines off:
/// on arrival it tops every tyre up to the service pressure.
pub fn ground_crew_check(present: bool) {
    let was = GROUND_CREW.swap(present, std::sync::atomic::Ordering::Relaxed);
    if present && !was {
        SERVICE_DUE.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The speed the tyres roll at: the ground speed while the aircraft is on its
/// wheels, nothing once they leave the runway.
pub fn rolling_speed_ms(groundspeed_ms: f64, on_ground: bool) -> f64 {
    if on_ground {
        groundspeed_ms
    } else {
        0.0
    }
}

impl Tyres {
    pub fn new<W: systems::simulation::VariableRegistry>(vars: &mut W, xplm: Option<&Xplm>) -> Self {
        Self {
            wheels: [TyreWheel::default(); WHEELS],
            brake_temperature: std::array::from_fn(|i| vars.get(format!("BRAKE_TEMPERATURE_{}", i + 1))),
            pressure_out: std::array::from_fn(|i| vars.get(format!("TYRE_PRESSURE_PA:{}", i + 1))),
            temp_out: std::array::from_fn(|i| vars.get(format!("TYRE_TEMPERATURE_C:{}", i + 1))),
            leak_out: std::array::from_fn(|i| vars.get(format!("TYRE_LEAKED_FRACTION:{}", i + 1))),
            tread_out: std::array::from_fn(|i| vars.get(format!("TYRE_TREAD_MM:{}", i + 1))),
            fuse_plug_out: std::array::from_fn(|i| vars.get(format!("TYRE_FUSE_PLUG_MELTED:{}", i + 1))),
            ambient_c: xplm.and_then(|x| x.find("sim/weather/aircraft/temperature_ambient_deg_c")),
            groundspeed_ms: xplm.and_then(|x| x.find("sim/flightmodel/position/groundspeed")),
            on_ground: xplm.and_then(|x| x.find("sim/flightmodel/failures/onground_any")),
            heading_deg: xplm.and_then(|x| x.find("sim/flightmodel/position/psi")),
            tpis_out: std::array::from_fn(|i| {
                let n = sd_wheel_number(i).map_or_else(|| format!("NOSE_{}", i - 15), |n| n.to_string());
                Some(vars.get(format!("TPIS_PRESSURE_PSI_{n}")))
            }),
            last_brake_c: [f64::NAN; BRAKED_WHEELS],
            last_heading_deg: None,
            was_on_ground: true,
            events: Vec::new(),
        }
    }

    fn get_f(xplm: Option<&Xplm>, d: Option<DataRef>) -> f64 {
        match (xplm, d) {
            (Some(x), Some(d)) => x.get_f(d) as f64,
            _ => 0.0,
        }
    }

    pub fn update<W: systems::simulation::SimulatorReaderWriter>(&mut self, vars: &mut W, xplm: Option<&Xplm>, delta: f64) {
        let ambient_c = Self::get_f(xplm, self.ambient_c);
        let on_ground = match (xplm, self.on_ground) {
            (Some(x), Some(d)) => x.get_i(d) != 0,
            _ => true,
        };
        let groundspeed_ms = rolling_speed_ms(Self::get_f(xplm, self.groundspeed_ms), on_ground);
        if let Some(saved) = RESTORE.lock().ok().and_then(|mut r| r.take()) {
            apply_saved(&mut self.wheels, &saved);
        }
        if SERVICE_DUE.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.wheels.iter_mut().for_each(TyreWheel::top_up);
            self.events.push("ground crew topped up the tyres".into());
        }
        let heading = self.heading_deg.map(|_| Self::get_f(xplm, self.heading_deg));
        let turn_rad = match (heading, self.last_heading_deg) {
            (Some(h), Some(was)) => ((h - was + 540.0).rem_euclid(360.0) - 180.0).to_radians(),
            _ => 0.0,
        };
        self.last_heading_deg = heading;
        let touchdown = on_ground && !self.was_on_ground;
        self.was_on_ground = on_ground;
        let rolled_m = groundspeed_ms * delta.max(0.0);
        for i in 0..WHEELS {
            let brake_heat_c = if i < BRAKED_WHEELS {
                let now = vars.read(&self.brake_temperature[i]);
                let was = std::mem::replace(&mut self.last_brake_c[i], now);
                if was.is_finite() { now - was } else { 0.0 }
            } else {
                0.0
            };
            if touchdown {
                self.wheels[i].spin_up(Self::get_f(xplm, self.groundspeed_ms));
            }
            self.wheels[i].wear(rolled_m, brake_heat_c, turn_rad * turn_scrub_share(i));
            self.wheels[i].natural_loss(delta);
            let (melted_now, id) = if i < BRAKED_WHEELS {
                let brake_temp_c = vars.read(&self.brake_temperature[i]);
                let id = LEG_FAILURE_IDS[leg_of_wheel(i)];
                (self.wheels[i].step(brake_temp_c, ambient_c, groundspeed_ms, failures::magnitude(id), delta), id)
            } else {
                // No brake inside these wheels, so no brake-heat path.
                let id = UNBRAKED_WHEELS[i - BRAKED_WHEELS].1;
                (self.wheels[i].step_unbraked(ambient_c, groundspeed_ms, failures::magnitude(id), delta), id)
            };
            if melted_now {
                self.events.push(format!(
                    "{} tyre {} ({:.0} C tyre, {:.1} mm tread) -- flat",
                    WHEEL_NAMES[i],
                    if self.wheels[i].fuse_plug_melted { "fuse plug melted" } else { "burst" },
                    self.wheels[i].temp_c,
                    self.wheels[i].tread_mm,
                ));
                if leg_flat(&self.wheels, id) {
                    failures::set_magnitude(id, 1.0);
                }
            }
            if let Some(out) = &self.tpis_out[i] {
                vars.write(out, self.wheels[i].tpis_psi());
            }
            vars.write(&self.pressure_out[i], self.wheels[i].pressure_pa());
            vars.write(&self.temp_out[i], self.wheels[i].temp_c);
            vars.write(&self.leak_out[i], self.wheels[i].leaked_fraction);
            vars.write(&self.tread_out[i], self.wheels[i].tread_mm);
            vars.write(&self.fuse_plug_out[i], if self.wheels[i].fuse_plug_melted { 1.0 } else { 0.0 });
        }
        if let Ok(mut l) = LATEST.lock() {
            *l = Some(self.wheels.iter().map(|w| SavedTyre { leaked_fraction: w.leaked_fraction, tread_mm: w.tread_mm, flat: w.is_flat() }).collect());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tread_wears_on_every_landing_and_taxi_without_a_fault() {
        let mut w = TyreWheel::default();
        w.spin_up(SPIN_UP_REF_SPEED_MS);
        assert!((w.tread_mm - (NEW_TREAD_DEPTH_MM - SPIN_UP_WEAR_MM)).abs() < 1e-12);
        let before = w.tread_mm;
        w.wear(10_000.0, 200.0, 1.0);
        let expected = ROLL_WEAR_MM_PER_M * 10_000.0 + BRAKE_WEAR_MM_PER_C * 200.0 + TURN_SCRUB_MM_PER_RAD;
        assert!((before - w.tread_mm - expected).abs() < 1e-12);
        let parked = w.tread_mm;
        w.wear(0.0, 300.0, 2.0);
        assert_eq!(w.tread_mm, parked, "a parked tyre does not wear");
        let per_cycle = SPIN_UP_WEAR_MM + ROLL_WEAR_MM_PER_M * 15_000.0 + BRAKE_WEAR_MM_PER_C * 200.0;
        let cycles = NEW_TREAD_DEPTH_MM / per_cycle;
        assert!((150.0..600.0).contains(&cycles), "a tyre lasts a few hundred landings, got {cycles:.0}");
    }

    #[test]
    fn a_worn_through_tyre_bursts_at_speed_and_only_its_own_wheel_goes_flat() {
        let mut wheels = [TyreWheel::default(); WHEELS];
        wheels[0].tread_mm = 0.0;
        assert!(!wheels[0].step(15.0, 15.0, 10.0, 0.0, 0.1), "slow taxi on a bald tyre");
        assert!(wheels[0].step(15.0, 15.0, 60.0, 0.0, 0.1));
        assert!(wheels[0].burst && wheels[0].pressure_pa() == 0.0);
        assert!(!wheels[1].is_flat());
        assert!(!leg_flat(&wheels, LEG_FAILURE_IDS[0]), "one of four is under the leg share");
        wheels[4].burst = true;
        assert!(leg_flat(&wheels, LEG_FAILURE_IDS[0]));
        wheels[16].fuse_plug_melted = true;
        assert!(leg_flat(&wheels, NOSE_FAILURE_ID));
    }

    #[test]
    fn service_tops_up_and_the_airframe_file_round_trips() {
        let mut w = TyreWheel::default();
        w.natural_loss(86_400.0);
        assert!((w.leaked_fraction - NATURAL_LOSS_FRACTION_PER_DAY).abs() < 1e-12);
        w.top_up();
        assert_eq!(w.leaked_fraction, 0.0);
        let mut wheels = [TyreWheel::default(); WHEELS];
        wheels[3].tread_mm = 2.5;
        wheels[5].burst = true;
        let saved: Vec<SavedTyre> = wheels.iter().map(|w| SavedTyre { leaked_fraction: w.leaked_fraction, tread_mm: w.tread_mm, flat: w.is_flat() }).collect();
        let json = serde_json::to_string(&saved).unwrap();
        let mut fresh = [TyreWheel::default(); WHEELS];
        apply_saved(&mut fresh, &serde_json::from_str::<Vec<SavedTyre>>(&json).unwrap());
        assert_eq!(fresh[3].tread_mm, 2.5);
        assert!(fresh[5].is_flat());
        fresh[5].top_up();
        assert!(fresh[5].is_flat(), "a flat tyre is replaced, not inflated");
        apply_saved(&mut fresh, &[]);
        assert!(fresh.iter().all(|w| w.tread_mm == NEW_TREAD_DEPTH_MM && !w.is_flat()));
    }

    #[test]
    fn tpis_reads_5_psi_steps_under_sd_wheel_numbers() {
        let w = TyreWheel { temp_c: COLD_TEMP_K - 273.15, ..Default::default() };
        assert_eq!(w.tpis_psi(), 225.0);
        assert_eq!(TyreWheel { leaked_fraction: 1.0, ..w }.tpis_psi(), 0.0);
        let numbers: Vec<usize> = (0..WHEELS).filter_map(sd_wheel_number).collect();
        assert_eq!(numbers, (1..=20).collect::<Vec<_>>());
    }

    #[test]
    fn tyres_do_not_roll_or_heat_once_airborne() {
        let mut nose = TyreWheel { temp_c: 40.0, ..Default::default() };
        for _ in 0..600 {
            assert!(!nose.step_unbraked(15.0, rolling_speed_ms(82.0, false), 0.0, 1.0), "burst in the climb");
        }
        assert!(nose.temp_c < 40.0, "an airborne tyre must cool, got {:.1} C", nose.temp_c);
        assert_eq!(rolling_speed_ms(30.0, true), 30.0);
    }

    /// Gas law sanity: doubling absolute temperature at zero leak doubles
    /// pressure (Gay-Lussac, sealed constant volume) -- the textbook
    /// relation, checked against this module's own formula independently
    /// of any simulated stepping.
    #[test]
    fn pressure_scales_with_absolute_temperature_at_full_mass() {
        let mut w = TyreWheel { temp_c: COLD_TEMP_K - 273.15, leaked_fraction: 0.0, ..Default::default() };
        let p_cold = w.pressure_pa();
        assert!((p_cold - COLD_PRESSURE_PA).abs() < 1.0, "{p_cold}");
        w.temp_c = 2.0 * COLD_TEMP_K - 273.15; // doubled absolute temperature
        let p_hot = w.pressure_pa();
        assert!((p_hot / p_cold - 2.0).abs() < 1e-9, "{}", p_hot / p_cold);
    }

    /// A leak alone, cold and stationary, does not raise temperature (no
    /// heat source): only pressure falls, linearly with leaked fraction.
    #[test]
    fn leak_alone_lowers_pressure_without_heating_the_tyre() {
        let mut w = TyreWheel::default();
        w.temp_c = 15.0; // matches COLD_TEMP_K's 15 C reference
        for _ in 0..2000 {
            w.step(/* brake */ 15.0, /* ambient */ 15.0, /* groundspeed */ 0.0, /* magnitude */ 0.6, /* delta */ 1.0);
        }
        let expected_leaked = (LEAK_RATE_FRACTION_PER_S_AT_FULL_MAGNITUDE * 0.6 * 2000.0_f64).min(1.0);
        assert!((w.leaked_fraction - expected_leaked).abs() < 1e-9, "{} vs {}", w.leaked_fraction, expected_leaked);
        assert!((w.temp_c - 15.0).abs() < 0.05, "temp drifted: {}", w.temp_c);
        let expected_pressure = COLD_PRESSURE_PA * (1.0 - expected_leaked);
        assert!((w.pressure_pa() - expected_pressure).abs() / expected_pressure < 1e-6);
    }

    /// THE INTERSECTION TEST (external prediction, not the sim's own
    /// output): a single normal-energy landing, fully cooled before the
    /// next brake application, never reaches the fuse-plug melt point --
    /// but a slow leak (lowering inflation, and so raising flex heating)
    /// PLUS a short turnaround (residual brake heat not fully soaked away
    /// before the next taxi-out) together push the same wheel over
    /// `FUSE_PLUG_MELT_C`, which neither factor produces alone.
    ///
    /// Hand derivation (all closed-form, solved independently of the
    /// `step()` Euler loop under test):
    ///
    /// Phase A -- brake soak, stationary (no leak, no flex-heat term,
    /// `groundspeed = 0`): `dT/dt = k_soak*(T_brake - T) - k_cool*(T -
    /// T_amb)` is linear, `dT/dt = -(k_soak+k_cool)*(T - T_eq)` with
    /// `T_eq = (k_soak*T_brake + k_cool*T_amb) / (k_soak+k_cool)`.
    /// With `T_brake = 280 C` (a firm but unremarkable landing -- well
    /// under any brake-overtemp threshold), `T_amb = 20 C`,
    /// `k_soak = 1/900`, `k_cool = 1/1200`: `k_total = 7/3600 /s`,
    /// `T_eq = (280/900 + 20/1200) / (7/3600) = 0.327778/0.0019444
    /// = 168.6 C`. That equilibrium itself is under the 177 C melt point,
    /// so this single landing cannot melt the plug *however long* it
    /// soaks, stationary and leak-free -- the first half of "neither alone
    /// does it". Over `t = 900 s` (15 min, parked): `T(900) = 168.6 +
    /// (15-168.6)*exp(-900*7/3600/900)`... `exp(-7*900/3600/900)` --
    /// simplified, `exp(-k_total*t) = exp(-0.0019444*900) = exp(-1.75)
    /// = 0.17377`, so `T(900) = 168.6 - 153.6*0.17377 = 141.90 C`.
    /// Meanwhile the leak (magnitude 0.8, `LEAK_RATE = 0.0005/s`) removes
    /// `0.0005*0.8*900 = 0.36` of the nitrogen (`pressure_ratio = 0.64`).
    ///
    /// Phase B -- short turnaround taxi-out, `t = 250 s`: brake has
    /// dropped to taxi-idle `T_brake = 90 C`; groundspeed `15 m/s`. The
    /// leak (`magnitude = 0.8`) is still running through this phase too
    /// (same as Phase A), so `pressure_ratio` keeps falling and
    /// `flex_heat = ROLL_HEAT_COEFF*(1/pressure_ratio)*groundspeed` keeps
    /// rising through the 250 s window -- no longer the constant-coefficient
    /// linear ODE Phase A is (there is no elementary closed form for
    /// `1/(p0 - r*t)` convolved against `exp(-k*(t-s))`). The numbers below
    /// freeze `pressure_ratio` at its Phase-A-end value (`0.64`) as a
    /// *lower-bound* estimate -- real flex heat only grows from there -- which
    /// is enough for the ">"/"<" melt-point sanity checks the code below
    /// runs before trusting the sim, but not tight enough to assert an exact
    /// final temperature against (see the decoupled wheel below, where the
    /// test does exactly that and so integrates the real, non-frozen
    /// pressure ratio instead).
    /// Leaking wheel's flex heat at Phase B's start: `0.02*(1/0.64)*15 =
    /// 0.46875 C/s`; `T_eq_B = (90/900 + 0.46875 + 20/1200)/(7/3600)
    /// = 0.585417/0.0019444 = 301.07 C`. `T(250) = 301.07 +
    /// (141.90-301.07)*exp(-0.0019444*250) = 301.07 - 159.17*0.61510
    /// = 203.2 C` (lower bound) -- past the 177 C melt point already, so the
    /// real (hotter) trajectory crosses it too, and sooner.
    /// Control wheel (no leak, `pressure_ratio = 1.0` throughout, so this one
    /// *is* exact): flex heat `0.02*1*15 = 0.3 C/s`; `T_eq_B =
    /// (0.1+0.3+0.016667)/0.0019444 = 214.29 C`. `T(250) = 214.29 -
    /// 72.39*0.61510 = 169.8 C` -- stays under 177 C on the identical
    /// brake/taxi profile: the leak is necessary.
    ///
    /// Decoupling proof (the brake-heat-soak link cut, per the brief):
    /// the *same* leaking wheel, but every `step()` call is fed its own
    /// current temperature as `brake_temp_c`, so `k_soak*(T_brake-T) = 0`
    /// identically -- no scripted flag, the coupling term itself is
    /// simply never given anything to couple to. Phase A (no flex, no
    /// soak): `dT/dt = -k_cool*(T-20)`, `T(900) = 20 + (15-20)*
    /// exp(-900/1200) = 20 - 5*0.47237 = 17.64 C`. Phase B (soak still
    /// severed) is where the still-running leak's effect on flex heat
    /// actually matters for an exact number: with `pressure_ratio` frozen
    /// at Phase A's `0.64` the naive target is `20 + 0.46875*1200 =
    /// 582.5 C`, `T(250) = 582.5 + (17.64-582.5)*exp(-250/1200) = 582.5 -
    /// 564.86*0.81213 = 123.7 C`, but the leak keeps draining
    /// `pressure_ratio` by another `0.0005*0.8*250 = 0.1` over the same
    /// window (`0.64` -> `0.54`), so `flex_heat` keeps climbing (`0.46875`
    /// -> `0.3/0.54 = 0.5556 C/s`) instead of holding flat; folded through
    /// the same `exp(-k*(t-s))` weighting the closed form uses, that raises
    /// the true `T(250)` by roughly ten degrees over the frozen-ratio
    /// estimate above -- large next to the `1 C` integration tolerance this
    /// test holds `TyreWheel::step` to, so `predicted_decoupled_final`
    /// below is instead integrated one second at a time with the same
    /// falling `pressure_ratio`, independently of `TyreWheel::step` (same
    /// equations, no call into the module under test), to get a number
    /// accurate enough to assert against. It still stays under 177 C: the
    /// brake-soak coupling is necessary too, not just the leak.
    #[test]
    fn slow_leak_plus_short_turnaround_together_melt_a_fuse_plug_that_neither_does_alone() {
        const T_BRAKE_LANDING: f64 = 280.0;
        const T_AMB: f64 = 20.0;
        const T_BRAKE_TAXI: f64 = 90.0;
        const GROUNDSPEED_MS: f64 = 15.0;
        const MAGNITUDE: f64 = 0.8;
        const PHASE_A_S: f64 = 900.0;
        const PHASE_B_S: f64 = 250.0;
        const K_SOAK: f64 = SOAK_RATE_PER_S;
        const K_COOL: f64 = COOL_RATE_PER_S;
        const K_TOTAL: f64 = K_SOAK + K_COOL;

        fn relax(t0: f64, t_eq: f64, rate: f64, t: f64) -> f64 {
            t_eq + (t0 - t_eq) * (-rate * t).exp()
        }

        // Phase B, `1 s` Euler-integrated with the leak still running (see
        // the doc comment above): independent of `TyreWheel::step` (same
        // governing equations, no call into the module under test), but
        // tracking the same falling `pressure_ratio` that function does
        // rather than freezing it at its Phase-A-end value. `soak_coupled
        // = false` reproduces the decoupled wheel's severed brake-soak term
        // (fed its own temperature every tick, so `k_soak*(T_brake-T) = 0`
        // identically) without needing a `brake_temp` argument at all.
        fn integrate_phase_b(t0: f64, brake_temp: f64, initial_pressure_ratio: f64, soak_coupled: bool) -> f64 {
            let mut temp = t0;
            let mut pressure_ratio = initial_pressure_ratio;
            for _ in 0..(PHASE_B_S as u64) {
                // Leak first, same order as `TyreWheel::step`: this tick's
                // flex heat uses the fraction *after* this tick's leak.
                pressure_ratio -= LEAK_RATE_FRACTION_PER_S_AT_FULL_MAGNITUDE * MAGNITUDE; // dt = 1 s
                let flex = ROLL_HEAT_COEFF_C_PER_MS_PER_S * (1.0 / pressure_ratio) * GROUNDSPEED_MS;
                let soak = if soak_coupled { K_SOAK * (brake_temp - temp) } else { 0.0 };
                let d_temp = soak + flex - K_COOL * (temp - T_AMB);
                temp += d_temp; // dt = 1 s
            }
            temp
        }

        // --- Phase A: parked, brake soaking, leak accumulating. ---
        let t_eq_a = (K_SOAK * T_BRAKE_LANDING + K_COOL * T_AMB) / K_TOTAL;
        assert!(t_eq_a < damage::FUSE_PLUG_MELT_C, "derivation error: a single landing's own equilibrium must stay under melt: {t_eq_a}");
        let predicted_after_a = relax(15.0, t_eq_a, K_TOTAL, PHASE_A_S);

        let mut leaking = TyreWheel { temp_c: 15.0, ..Default::default() };
        for _ in 0..(PHASE_A_S as u64) {
            leaking.step(T_BRAKE_LANDING, T_AMB, 0.0, MAGNITUDE, 1.0);
        }
        assert!((leaking.temp_c - predicted_after_a).abs() < 0.5, "{} vs {}", leaking.temp_c, predicted_after_a);
        assert!(leaking.temp_c < damage::FUSE_PLUG_MELT_C, "phase A alone must not melt the plug: {}", leaking.temp_c);
        let leaked_after_a = LEAK_RATE_FRACTION_PER_S_AT_FULL_MAGNITUDE * MAGNITUDE * PHASE_A_S;
        assert!((leaking.leaked_fraction - leaked_after_a).abs() < 1e-9);

        let mut control = TyreWheel { temp_c: 15.0, ..Default::default() };
        for _ in 0..(PHASE_A_S as u64) {
            control.step(T_BRAKE_LANDING, T_AMB, 0.0, 0.0, 1.0);
        }
        assert!((control.temp_c - predicted_after_a).abs() < 0.5);
        assert_eq!(control.leaked_fraction, 0.0);

        // Decoupled wheel: same leak as `leaking`, but the brake-soak link
        // is severed for the whole scenario (see derivation above).
        let mut decoupled = TyreWheel { temp_c: 15.0, ..Default::default() };
        for _ in 0..(PHASE_A_S as u64) {
            let own_temp = decoupled.temp_c;
            decoupled.step(own_temp, T_AMB, 0.0, MAGNITUDE, 1.0);
        }
        let predicted_decoupled_after_a = relax(15.0, T_AMB, K_COOL, PHASE_A_S);
        assert!((decoupled.temp_c - predicted_decoupled_after_a).abs() < 0.5, "{} vs {}", decoupled.temp_c, predicted_decoupled_after_a);

        // --- Phase B: short turnaround taxi-out. ---
        // Frozen-ratio lower bound, only used for the ">" sanity check
        // below (real leaking+coupled runs hotter than this, see doc
        // comment above); not asserted against the sim's exact value.
        let leaking_pressure_ratio = 1.0 - leaked_after_a;
        let leaking_flex = ROLL_HEAT_COEFF_C_PER_MS_PER_S * (1.0 / leaking_pressure_ratio) * GROUNDSPEED_MS;
        let t_eq_b_leaking = (K_SOAK * T_BRAKE_TAXI + leaking_flex + K_COOL * T_AMB) / K_TOTAL;
        let predicted_leaking_final = relax(predicted_after_a, t_eq_b_leaking, K_TOTAL, PHASE_B_S);

        let control_flex = ROLL_HEAT_COEFF_C_PER_MS_PER_S * 1.0 * GROUNDSPEED_MS;
        let t_eq_b_control = (K_SOAK * T_BRAKE_TAXI + control_flex + K_COOL * T_AMB) / K_TOTAL;
        let predicted_control_final = relax(predicted_after_a, t_eq_b_control, K_TOTAL, PHASE_B_S);

        // Exact (not frozen-ratio) prediction: this one IS asserted against
        // the sim's final temperature below, so it has to track the leak's
        // continued effect on pressure_ratio through Phase B.
        let predicted_decoupled_final =
            integrate_phase_b(predicted_decoupled_after_a, 0.0, leaking_pressure_ratio, false);

        // Sanity on the hand derivation itself before trusting the sim.
        assert!(predicted_leaking_final > damage::FUSE_PLUG_MELT_C, "derivation error: leaking+coupled must cross melt: {predicted_leaking_final}");
        assert!(predicted_control_final < damage::FUSE_PLUG_MELT_C, "derivation error: no-leak control must stay under melt: {predicted_control_final}");
        assert!(predicted_decoupled_final < damage::FUSE_PLUG_MELT_C, "derivation error: leak-without-soak-coupling must stay under melt: {predicted_decoupled_final}");

        let mut leaking_melted = false;
        for _ in 0..(PHASE_B_S as u64) {
            if leaking.step(T_BRAKE_TAXI, T_AMB, GROUNDSPEED_MS, MAGNITUDE, 1.0) {
                leaking_melted = true;
            }
        }
        let mut control_melted = false;
        for _ in 0..(PHASE_B_S as u64) {
            if control.step(T_BRAKE_TAXI, T_AMB, GROUNDSPEED_MS, 0.0, 1.0) {
                control_melted = true;
            }
        }
        let mut decoupled_melted = false;
        for _ in 0..(PHASE_B_S as u64) {
            let own_temp = decoupled.temp_c;
            if decoupled.step(own_temp, T_AMB, GROUNDSPEED_MS, MAGNITUDE, 1.0) {
                decoupled_melted = true;
            }
        }

        // The external prediction: leak + short turnaround, WITH the
        // brake-heat-soak coupling intact, together melt the plug (and
        // fully deflate/burst that wheel). Cut either ingredient --  the
        // leak (control) or the soak coupling itself (decoupled) -- on the
        // identical brake/taxi profile, and it does not.
        assert!(leaking_melted, "leaking+coupled wheel should have melted its fuse plug and burst");
        assert!(leaking.fuse_plug_melted);
        assert_eq!(leaking.leaked_fraction, 1.0, "a melted fuse plug fully deflates the wheel");
        assert!(!control_melted, "no-leak control must not melt under the same brake/taxi profile");
        assert!(!control.fuse_plug_melted);
        assert!(!decoupled_melted, "leak without the brake-soak coupling must not melt: the coupling itself is a necessary ingredient, not just the leak");
        assert!(!decoupled.fuse_plug_melted);

        // Sim trajectories match the closed-form predictions within
        // integration tolerance (1 s Euler steps against an analytic
        // solution of the same linear ODE).
        assert!((control.temp_c - predicted_control_final).abs() < 1.0, "{} vs {}", control.temp_c, predicted_control_final);
        assert!((decoupled.temp_c - predicted_decoupled_final).abs() < 1.0, "{} vs {}", decoupled.temp_c, predicted_decoupled_final);
    }

    #[test]
    fn tread_wears_only_when_a_wear_fault_is_active() {
        let mut w = TyreWheel::default();
        w.step(15.0, 15.0, 0.0, 0.0, 1000.0);
        assert_eq!(w.tread_mm, NEW_TREAD_DEPTH_MM, "no magnitude -> no abnormal wear");
        w.step(15.0, 15.0, 0.0, 1.0, 500.0);
        let expected = NEW_TREAD_DEPTH_MM - WEAR_RATE_MM_PER_S_AT_FULL_MAGNITUDE * 500.0;
        assert!((w.tread_mm - expected).abs() < 1e-9);
    }

    /// A cold nose tyre stands at its service pressure, and goes on
    /// behaving like a tyre with no brake behind it: it does not soak up
    /// brake heat it has no path to, but it still heats as it rolls and
    /// still loses pressure to a leak.
    #[test]
    fn a_cold_nose_tyre_reads_service_pressure_and_has_no_brake_to_soak_from() {
        let mut nose = TyreWheel { temp_c: 15.0, ..Default::default() };
        assert!((nose.pressure_pa() - COLD_PRESSURE_PA).abs() < 1.0, "{}", nose.pressure_pa());

        // Parked next to a wing wheel that has just landed hot: the nose
        // tyre cannot feel it, because there is no brake in the nose hub.
        let mut braked = TyreWheel { temp_c: 15.0, ..Default::default() };
        for _ in 0..900 {
            nose.step_unbraked(15.0, 0.0, 0.0, 1.0);
            braked.step(300.0, 15.0, 0.0, 0.0, 1.0);
        }
        assert!((nose.temp_c - 15.0).abs() < 0.1, "nose tyre drifted to {:.2} C with no brake behind it", nose.temp_c);
        assert!(braked.temp_c > 100.0, "the braked wheel should have soaked: {:.1} C", braked.temp_c);
        assert!((nose.pressure_pa() - COLD_PRESSURE_PA).abs() < 6_000.0, "{}", nose.pressure_pa());

        // Rolling heats it, and a leak still takes nitrogen out of it.
        let before = nose.temp_c;
        for _ in 0..300 {
            nose.step_unbraked(15.0, 20.0, 0.5, 1.0);
        }
        assert!(nose.temp_c > before + 10.0, "rolling flex must heat it: {:.1} -> {:.1} C", before, nose.temp_c);
        assert!((nose.leaked_fraction - LEAK_RATE_FRACTION_PER_S_AT_FULL_MAGNITUDE * 0.5 * 300.0).abs() < 1e-9);
        // It is hotter than cold, so its pressure is *up* on the placard
        // figure -- but down on what the same tyre at the same temperature
        // would read with all its nitrogen still in it. Gay-Lussac and the
        // leak are both acting, and the leak is not hidden by the heat.
        let sound = TyreWheel { temp_c: nose.temp_c, ..Default::default() };
        assert!(nose.pressure_pa() > COLD_PRESSURE_PA, "{}", nose.pressure_pa());
        assert!(
            (nose.pressure_pa() - sound.pressure_pa() * (1.0 - nose.leaked_fraction)).abs() < 1.0,
            "{} vs {}",
            nose.pressure_pa(),
            sound.pressure_pa()
        );
    }

    #[test]
    fn every_wheel_has_a_name_and_a_failure_id_exactly_once() {
        assert_eq!(WHEEL_NAMES.len(), WHEELS);
        assert_eq!(UNBRAKED_WHEELS.len(), WHEELS - BRAKED_WHEELS);
        for (n, (index, _)) in UNBRAKED_WHEELS.iter().enumerate() {
            assert_eq!(*index, BRAKED_WHEELS + n, "the unbraked wheels follow the braked ones in order");
        }
        // The nose pair are the only wheels on the nose leg's own id.
        let nose: Vec<_> = UNBRAKED_WHEELS.iter().filter(|(_, id)| *id == NOSE_FAILURE_ID).collect();
        assert_eq!(nose.len(), 2);
        // The body legs' rear axles share their leg's existing id.
        assert_eq!(UNBRAKED_WHEELS[2].1, LEG_FAILURE_IDS[2]);
        assert_eq!(UNBRAKED_WHEELS[4].1, LEG_FAILURE_IDS[3]);
    }

    #[test]
    fn leg_wheel_mapping_covers_all_sixteen_wheels_exactly_once() {
        let mut seen = [false; 16];
        for leg in 0..4 {
            for &w in &LEG_WHEEL_INDICES[leg] {
                assert!(!seen[w], "wheel {w} claimed by two legs");
                seen[w] = true;
                assert_eq!(leg_of_wheel(w), leg);
            }
        }
        assert!(seen.iter().all(|&s| s));
    }
}
