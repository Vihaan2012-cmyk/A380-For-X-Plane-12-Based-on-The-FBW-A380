use crate::online::WeatherSource;
use crate::winds::{
    UplinkedCruiseWindEntry, UplinkedCruiseWindSet, UplinkedWindLevel, WindFix, WindRequestMessage, WindUplinkMessage,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindSample {
    pub direction_true_deg: f64,
    pub speed_kt: f64,
    pub temperature_c: f64,
}

pub trait XPlaneWeatherHost {
    fn metar_for_airport(&mut self, icao: &str) -> Option<String>;
    fn weather_at(&mut self, latitude: f64, longitude: f64, altitude_msl_m: f64) -> Option<WindSample>;
}

pub struct XPlaneWeather<'a> {
    pub host: &'a mut dyn XPlaneWeatherHost,
}

impl WeatherSource for XPlaneWeather<'_> {
    fn metar(&mut self, icao: &str) -> Option<String> {
        self.host.metar_for_airport(icao).filter(|m| !m.trim().is_empty())
    }

    fn taf(&mut self, _icao: &str) -> Option<String> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteStage {
    Climb,
    Cruise,
    Descent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutePoint {
    pub fix: WindFix,
    pub latitude: f64,
    pub longitude: f64,
    pub stage: RouteStage,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindRoute {
    pub points: Vec<RoutePoint>,
    pub alternate: Option<(f64, f64, f64)>,
}

pub const CLIMB_LEVELS: [f64; 4] = [50.0, 100.0, 150.0, 200.0];
pub const DESCENT_LEVELS: [f64; 4] = [200.0, 150.0, 100.0, 50.0];

fn feet_to_m(ft: f64) -> f64 {
    ft * 0.3048
}

fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

fn level(host: &mut dyn XPlaneWeatherHost, lat: f64, lon: f64, fl: f64) -> Option<UplinkedWindLevel> {
    let s = host.weather_at(lat, lon, feet_to_m(fl * 100.0))?;
    Some(UplinkedWindLevel {
        flight_level: fl,
        true_degrees: round(s.direction_true_deg.rem_euclid(360.0)),
        magnitude: round(s.speed_kt),
    })
}

fn levels_with(cruise: Option<f64>, base: &[f64], descending_order: bool) -> Vec<f64> {
    let mut out: Vec<f64> = base.iter().copied().filter(|l| !matches!(cruise, Some(c) if *l >= c)).collect();
    if let Some(c) = cruise {
        out.push(c);
    }
    out.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if !descending_order {
        out.reverse();
    }
    out
}

fn fix_matches(point: &RoutePoint, wanted: &WindFix) -> bool {
    match (&point.fix, wanted) {
        (WindFix::Ident(a), WindFix::Ident(b)) => a == b,
        (WindFix::LatLon { lat: a, long: b }, WindFix::LatLon { lat: c, long: d }) => {
            (a - c).abs() < 1e-4 && (b - d).abs() < 1e-4
        }
        _ => false,
    }
}

pub fn xplane_winds(
    host: &mut dyn XPlaneWeatherHost,
    route: &WindRoute,
    request: &WindRequestMessage,
) -> WindUplinkMessage {
    let first = route.points.iter().find(|p| p.stage == RouteStage::Climb).or(route.points.first());
    let last = route.points.iter().rev().find(|p| p.stage == RouteStage::Descent).or(route.points.last());

    let climb_winds = request.climb_wind_level.map(|cruise| {
        let Some(p) = first else { return Vec::new() };
        levels_with(cruise, &CLIMB_LEVELS, true)
            .into_iter()
            .filter_map(|fl| level(host, p.latitude, p.longitude, fl))
            .collect()
    });

    let cruise_winds = request.cruise_winds.as_ref().filter(|c| !c.flight_levels.is_empty()).map(|c| {
        let points: Vec<&RoutePoint> = route
            .points
            .iter()
            .filter(|p| c.waypoints.iter().any(|w| fix_matches(p, w)))
            .collect();
        c.flight_levels
            .iter()
            .map(|&fl| UplinkedCruiseWindSet {
                flight_level: fl,
                fixes: points
                    .iter()
                    .filter_map(|p| {
                        let s = host.weather_at(p.latitude, p.longitude, feet_to_m(fl * 100.0))?;
                        Some(UplinkedCruiseWindEntry {
                            fix: p.fix.clone(),
                            flight_level: Some(fl),
                            temperature: Some(s.temperature_c),
                            true_degrees: round(s.direction_true_deg.rem_euclid(360.0)),
                            magnitude: round(s.speed_kt),
                        })
                    })
                    .collect(),
            })
            .collect()
    });

    let descent_winds = request.descent_wind_level.map(|cruise| {
        let Some(p) = last else { return Vec::new() };
        levels_with(cruise, &DESCENT_LEVELS, true)
            .into_iter()
            .filter_map(|fl| level(host, p.latitude, p.longitude, fl))
            .collect()
    });

    let alternate_wind = match (&request.alternate_wind, route.alternate) {
        (Some(_), Some((lat, lon, fl))) => level(host, lat, lon, fl),
        _ => None,
    };

    WindUplinkMessage { climb_winds, cruise_winds, descent_winds, alternate_wind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::winds::{format_wind_uplink_message, AlternateWindRequest, CruiseWindRequest};

    struct FakeSim {
        queries: Vec<(f64, f64, f64)>,
    }

    impl XPlaneWeatherHost for FakeSim {
        fn metar_for_airport(&mut self, icao: &str) -> Option<String> {
            (icao == "EGLL").then(|| "EGLL 221250Z 24012KT 9999 FEW030 15/08 Q1015".to_string())
        }
        fn weather_at(&mut self, lat: f64, lon: f64, alt_m: f64) -> Option<WindSample> {
            self.queries.push((lat, lon, alt_m));
            let fl = alt_m / 0.3048 / 100.0;
            Some(WindSample { direction_true_deg: 200.0 + fl / 10.0, speed_kt: fl / 5.0, temperature_c: 15.0 - fl * 0.2 })
        }
    }

    fn route() -> WindRoute {
        let p = |id: &str, lon: f64, stage| RoutePoint { fix: WindFix::Ident(id.into()), latitude: 51.0, longitude: lon, stage };
        WindRoute {
            points: vec![
                p("DEP", 0.0, RouteStage::Climb),
                p("MID", 5.0, RouteStage::Cruise),
                RoutePoint { fix: WindFix::LatLon { lat: 52.0, long: 10.0 }, latitude: 52.0, longitude: 10.0, stage: RouteStage::Cruise },
                p("ARR", 13.0, RouteStage::Descent),
            ],
            alternate: Some((53.6, 10.0, 240.0)),
        }
    }

    #[test]
    fn metar_comes_from_the_sim_and_taf_is_absent() {
        let mut sim = FakeSim { queries: Vec::new() };
        let mut src = XPlaneWeather { host: &mut sim };
        assert!(src.metar("EGLL").unwrap().starts_with("EGLL"));
        assert_eq!(src.metar("LFPG"), None);
        assert_eq!(src.taf("EGLL"), None);
    }

    #[test]
    fn winds_are_sampled_along_the_route() {
        let mut sim = FakeSim { queries: Vec::new() };
        let req = WindRequestMessage {
            climb_wind_level: Some(Some(350.0)),
            cruise_winds: Some(CruiseWindRequest {
                flight_levels: vec![350.0, 390.0],
                waypoints: vec![WindFix::Ident("MID".into()), WindFix::LatLon { lat: 52.0, long: 10.0 }],
            }),
            descent_wind_level: Some(Some(350.0)),
            alternate_wind: Some(AlternateWindRequest { destination_icao: "EDDB".into(), alternate_icao: "EDDH".into() }),
        };
        let up = xplane_winds(&mut sim, &route(), &req);
        let climb = up.climb_winds.clone().unwrap();
        assert_eq!(climb.iter().map(|l| l.flight_level).collect::<Vec<_>>(), vec![350.0, 200.0, 150.0, 100.0, 50.0]);
        assert_eq!((climb[0].true_degrees, climb[0].magnitude), (235.0, 70.0));
        let cruise = up.cruise_winds.clone().unwrap();
        assert_eq!(cruise.len(), 2);
        assert_eq!(cruise[1].fixes.len(), 2);
        assert_eq!(cruise[1].fixes[0].true_degrees, 239.0);
        assert_eq!(up.descent_winds.as_ref().unwrap().len(), 5);
        assert_eq!(up.alternate_wind.unwrap().flight_level, 240.0);
        assert!((sim.queries[0].2 - 350.0 * 100.0 * 0.3048).abs() < 1e-6);
        assert!(format_wind_uplink_message(&up).starts_with("PWI/CB350235070."));
    }

    #[test]
    fn missing_route_or_sim_data_gives_empty_sets() {
        struct NoWx;
        impl XPlaneWeatherHost for NoWx {
            fn metar_for_airport(&mut self, _: &str) -> Option<String> {
                None
            }
            fn weather_at(&mut self, _: f64, _: f64, _: f64) -> Option<WindSample> {
                None
            }
        }
        let req = WindRequestMessage { climb_wind_level: Some(None), ..Default::default() };
        let up = xplane_winds(&mut NoWx, &WindRoute::default(), &req);
        assert_eq!(up.climb_winds, Some(Vec::new()));
        assert_eq!(up.cruise_winds, None);
    }
}
