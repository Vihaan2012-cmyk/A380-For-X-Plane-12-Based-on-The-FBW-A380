use crate::arinc::{Arinc, Ssm};
use crate::geo::{place_bearing_distance, Coordinates, Position, METRES_TO_NAUTICAL_MILES};
use crate::projection::global_to_airport_coordinates;
use crate::turf::{boolean_point_in_polygon, line_offset_segment, metres_to_degrees, polygons_disjoint};

pub const OANS_WORD_1: &str = "L:A32NX_OANS_WORD_1";
const TRIGGER_TIMEOUT_MS: f64 = 30_000.0;

#[derive(Clone, Debug)]
pub struct RunwayArea {
    pub idrwy: Option<String>,
    pub rings: Vec<Vec<Position>>,
}

#[derive(Clone, Copy, Debug)]
pub struct RwyAheadInputs {
    pub on_ground: bool,
    pub ground_speed: Arinc,
    pub ppos_local: Option<Position>,
    pub global_pos: Coordinates,
    pub airport_ref: Coordinates,
    pub aircraft_bearing: f64,
    pub now_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RwyAheadTransmit {
    pub word: Arinc,
    pub qfu: String,
}

#[derive(Clone, Debug)]
pub struct RwyAhead {
    skip: u64,
    triggered: bool,
    qfu: String,
    triggered_time: f64,
    word: Arinc,
    pub volume: [Position; 5],
}

impl Default for RwyAhead {
    fn default() -> Self {
        Self { skip: 0, triggered: false, qfu: String::new(), triggered_time: 0.0, word: Arinc { value: 0.0, ssm: Ssm::FailureWarning }, volume: [[0.0, 0.0]; 5] }
    }
}

fn qfu_of(idrwy: &str) -> String {
    idrwy.replacen('.', " - ", 1)
}

impl RwyAhead {
    pub fn word(&self) -> Arinc {
        self.word
    }

    pub fn update(&mut self, i: &RwyAheadInputs, runways: &[RunwayArea]) -> Option<RwyAheadTransmit> {
        self.skip += 1;
        if self.skip % 10 != 0 {
            return None;
        }
        let gs = i.ground_speed;
        let Some(ppos) = i.ppos_local.filter(|_| i.on_ground && gs.ssm == Ssm::NormalOperation && !(gs.value > 40.0) && !(gs.value < 1.0)) else {
            return Some(self.transmit(false, String::new(), true));
        };

        let dist_nose = 73.0 / 2.0 / METRES_TO_NAUTICAL_MILES;
        let dist_7_sec = (gs.value / 60.0 / 60.0) * 7.0 + dist_nose;
        let nose = place_bearing_distance(i.global_pos, i.aircraft_bearing, dist_nose);
        let horizon = place_bearing_distance(i.global_pos, i.aircraft_bearing, dist_7_sec);
        let a = [nose.lat, nose.long];
        let b = [horizon.lat, horizon.long];
        let left = line_offset_segment(a, b, metres_to_degrees(30.0));
        let right = line_offset_segment(a, b, metres_to_degrees(-30.0));
        let local = |p: Position| global_to_airport_coordinates(i.airport_ref, Coordinates { lat: p[0], long: p[1] });
        self.volume = [local(left[0]), local(left[1]), local(right[1]), local(right[0]), local(left[0])];
        let volume = vec![self.volume.to_vec()];

        let mut inside: Vec<String> = Vec::new();
        for r in runways {
            if let Some(id) = r.idrwy.as_deref().filter(|s| !s.is_empty()) {
                if boolean_point_in_polygon(ppos, &r.rings, true) {
                    inside.push(qfu_of(id));
                }
            }
        }
        let mut will_enter: Vec<String> = Vec::new();
        for r in runways {
            if let Some(id) = r.idrwy.as_deref().filter(|s| !s.is_empty()) {
                let qfu = qfu_of(id);
                if !polygons_disjoint(&volume, &r.rings) && !inside.contains(&qfu) {
                    will_enter.push(qfu);
                    break;
                }
            }
        }

        if (self.triggered && i.now_ms - self.triggered_time > TRIGGER_TIMEOUT_MS) || will_enter.is_empty() {
            self.triggered = false;
            self.triggered_time = 0.0;
            self.qfu = String::new();
        } else {
            if !self.triggered {
                self.triggered_time = i.now_ms;
            }
            self.triggered = true;
            self.qfu = will_enter.swap_remove(0);
        }
        Some(self.transmit(self.triggered && !self.qfu.is_empty(), self.qfu.clone(), false))
    }

    fn transmit(&mut self, bit: bool, qfu: String, faulty: bool) -> RwyAheadTransmit {
        self.word.ssm = if faulty { Ssm::FailureWarning } else { Ssm::NormalOperation };
        self.word.set_bit(11, bit);
        RwyAheadTransmit { word: self.word, qfu }
    }
}
