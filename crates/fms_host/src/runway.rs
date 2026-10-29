use fms::plan::FlightPlan;
use fms_lnav::geo::place_bearing_distance;
use fms_lnav::Coord;
use serde_json::{json, Value};

use crate::facilities::Facilities;
use crate::outputs::Sink;

const NUMBER_TOPICS: [&str; 7] = ["gradient", "direction", "thresholdCrossingHeight", "latitude", "longitude", "elevation", "length"];
const STRING_TOPICS: [&str; 16] = ["location", "startLocation", "thresholdLocation", "srwyP1", "srwyP2", "srwyP3", "srwyP4", "srwyP5", "srwyP6", "srwyP7", "srwyP8", "srwyP9", "srwyPl1", "srwyPl2", "srwyPl3", "srwyPl4"];

fn p3(c: Coord, alt: f64) -> String {
    format!("{:.6}#{:.6}#{:.6}", c.lat, c.long, alt)
}

fn p2(c: Coord) -> String {
    format!("{:.6}#{:.6}", c.lat, c.long)
}

/// FBW `FmcAircraftInterface.updateDestinationData` runway bus topics (BTV/ROW data).
#[derive(Default)]
pub struct RunwayData {
    started: bool,
}

impl RunwayData {
    pub fn update(&mut self, plan: Option<&FlightPlan>, fac: &dyn Facilities, sink: &mut Sink) {
        if !self.started {
            self.started = true;
            for t in NUMBER_TOPICS {
                sink.bus_if_changed(t, json!(0));
            }
            sink.bus_if_changed("width", json!(0));
            for t in STRING_TOPICS {
                sink.bus_if_changed(t, json!(""));
            }
        }
        let Some(rwy) = plan.and_then(|p| p.destination_runway.as_ref()) else { return };
        let airport = rwy.fix.airport_ident.clone().or_else(|| plan.and_then(|p| p.destination_airport.as_ref()).map(|a| a.ident.clone())).unwrap_or_default();
        let details = fac.runway_details(&airport, &rwy.fix.ident).unwrap_or_default();
        let (gradient, tch, width) = (details.gradient, details.threshold_crossing_height, details.width);
        let start = Coord::new(rwy.fix.location.lat, rwy.fix.location.long);
        let thr = Coord::new(rwy.threshold_location.lat, rwy.threshold_location.long);
        let thr_alt = rwy.threshold_location.alt.unwrap_or(0.);
        let b = rwy.bearing;
        let end = place_bearing_distance(start, b, rwy.length / 1852.);
        let half = width / 2. / 1852.;
        let slope = (gradient / 180. * std::f64::consts::PI).tan();
        let p1_alt = tch - rwy.length * 3.281 * slope;
        let p1 = place_bearing_distance(end, b - 90., half);
        let p2c = place_bearing_distance(end, b + 90., half);
        let p3c = place_bearing_distance(thr, b + 90., half);
        let p4 = place_bearing_distance(thr, b - 90., half);
        let back = (b + 180.) % 360.;
        let p6 = place_bearing_distance(thr, back, 4.);
        let p7 = place_bearing_distance(p6, back, 4.);
        let p8 = place_bearing_distance(p7, back, 4.);
        let p9 = place_bearing_distance(p8, back, 4.);
        let papi = place_bearing_distance(thr, b, 311. / 1852.);
        let pl_alt = tch - 1020. * slope;
        let pl1 = place_bearing_distance(papi, b - 90., (width / 2. + 50.) / 1852.);
        let pl2 = place_bearing_distance(papi, b - 90., half);
        let pl3 = place_bearing_distance(papi, b + 90., (width / 2. + 50.) / 1852.);
        let pl4 = place_bearing_distance(papi, b + 90., half);
        let values: [(&str, Value); 24] = [
            ("gradient", json!(gradient)),
            ("location", json!(p2(start))),
            ("direction", json!(b)),
            ("startLocation", json!(p2(start))),
            ("thresholdLocation", json!(p3(thr, thr_alt))),
            ("thresholdCrossingHeight", json!(tch)),
            ("latitude", Value::Null),
            ("longitude", Value::Null),
            ("elevation", json!(tch)),
            ("length", json!(rwy.length)),
            ("width", json!(width)),
            ("srwyP1", json!(p3(p1, p1_alt))),
            ("srwyP2", json!(p3(p2c, p1_alt))),
            ("srwyP3", json!(p3(p3c, tch))),
            ("srwyP4", json!(p3(p4, tch))),
            ("srwyP5", json!(p3(thr, tch))),
            ("srwyP6", json!(p3(p6, tch))),
            ("srwyP7", json!(p3(p7, tch))),
            ("srwyP8", json!(p3(p8, tch))),
            ("srwyP9", json!(p3(p9, tch))),
            ("srwyPl1", json!(p3(pl1, pl_alt))),
            ("srwyPl2", json!(p3(pl2, pl_alt))),
            ("srwyPl3", json!(p3(pl3, pl_alt))),
            ("srwyPl4", json!(p3(pl4, pl_alt))),
        ];
        for (t, v) in values {
            sink.bus_if_changed(t, v);
        }
    }
}
