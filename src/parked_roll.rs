//! Logs X-Plane's force breakdown while the aircraft moves on the ground with
//! its engines giving no thrust: a parked aircraft that rolls with no tug has
//! to show where the push comes from (gear and slope, engines, aerodynamics,
//! or a plugin's added force).

use systems::simulation::{SimulatorReaderWriter, VariableIdentifier, VariableRegistry};

use crate::xp::{DataRef, Xplm};
use crate::Vars;

const MOVING_M_S: f32 = 0.2;
const STILL_M_S: f32 = 0.05;
const NO_THRUST_N: f32 = 5_000.0;
const LINES_PER_ROLL: u32 = 20;

struct Refs {
    on_ground: Option<DataRef>,
    groundspeed: Option<DataRef>,
    psi: Option<DataRef>,
    theta: Option<DataRef>,
    phi: Option<DataRef>,
    wind: Option<DataRef>,
    park: Option<DataRef>,
    left_brake: Option<DataRef>,
    right_brake: Option<DataRef>,
    steer: Option<DataRef>,
    axial: [Option<DataRef>; 5],
    side: [Option<DataRef>; 3],
    yaw: [Option<DataRef>; 3],
}

pub struct ParkedRoll {
    refs: Refs,
    chocks: VariableIdentifier,
    park_lever: VariableIdentifier,
    lines_left: u32,
    since_line_s: f64,
}

impl ParkedRoll {
    pub fn new(vars: &mut Vars, xplm: &Xplm) -> Self {
        let f = |name: &str| xplm.find(name);
        let force = |kind: &str, source: &str| f(&format!("sim/flightmodel/forces/{kind}_{source}"));
        Self {
            refs: Refs {
                on_ground: f("sim/flightmodel/failures/onground_any"),
                groundspeed: f("sim/flightmodel/position/groundspeed"),
                psi: f("sim/flightmodel/position/psi"),
                theta: f("sim/flightmodel/position/theta"),
                phi: f("sim/flightmodel/position/phi"),
                wind: f("sim/weather/aircraft/wind_now_speed_msc"),
                park: f("sim/cockpit2/controls/parking_brake_ratio"),
                left_brake: f("sim/cockpit2/controls/left_brake_ratio"),
                right_brake: f("sim/cockpit2/controls/right_brake_ratio"),
                steer: f("sim/flightmodel2/gear/tire_steer_actual_deg"),
                axial: ["gear", "prop", "aero", "plug_acf", "total"].map(|s| force("faxil", s)),
                side: ["gear", "plug_acf", "total"].map(|s| force("fside", s)),
                yaw: ["gear", "plug_acf", "total"].map(|s| force("N", s)),
            },
            chocks: vars.get("MODEL_WHEELCHOCKS_ENABLED".to_owned()),
            park_lever: vars.get("PARK_BRAKE_LEVER_POS".to_owned()),
            lines_left: LINES_PER_ROLL,
            since_line_s: 1.0,
        }
    }

    pub fn update(&mut self, vars: &mut Vars, xplm: &Xplm, dt: f64) {
        let r = |d: Option<DataRef>| d.map_or(0.0, |d| xplm.get_f(d));
        let on_ground = self.refs.on_ground.is_some_and(|d| xplm.get_i(d) != 0);
        let speed = r(self.refs.groundspeed);
        let engines = r(self.refs.axial[1]);
        if !on_ground || speed < STILL_M_S {
            self.lines_left = LINES_PER_ROLL;
            self.since_line_s = 1.0;
            return;
        }
        self.since_line_s += dt;
        if speed < MOVING_M_S || engines.abs() > NO_THRUST_N || self.lines_left == 0 || self.since_line_s < 1.0 {
            return;
        }
        self.since_line_s = 0.0;
        self.lines_left -= 1;
        let mut steer = [0f32; 3];
        if let Some(d) = self.refs.steer {
            xplm.get_vf(d, &mut steer);
        }
        let n = |refs: &[Option<DataRef>]| refs.iter().map(|d| format!("{:.0}", r(*d))).collect::<Vec<_>>().join(" ");
        crate::log(&format!(
            "parked roll: {speed:.2} m/s hdg {:.1} pitch {:.2} roll {:.2} wind {:.1} m/s | park {:.2} (lever {:.0}) brakes {:.2}/{:.2} chocks {:.0} nose steer {:.1} | axial N gear/engines/aero/plugins/total {} | side N gear/plugins/total {} | yaw Nm gear/plugins/total {}",
            r(self.refs.psi),
            r(self.refs.theta),
            r(self.refs.phi),
            r(self.refs.wind),
            r(self.refs.park),
            vars.read(&self.park_lever),
            r(self.refs.left_brake),
            r(self.refs.right_brake),
            vars.read(&self.chocks),
            steer[0],
            n(&self.refs.axial),
            n(&self.refs.side),
            n(&self.refs.yaw),
        ));
    }
}
