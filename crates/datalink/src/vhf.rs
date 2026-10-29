use std::collections::HashSet;
use std::path::Path;

use crate::status::{AtsuFlightPhase, DatalinkModeCode, DatalinkStatusCode};

pub const PROVIDER_COUNT: usize = 2;
pub const ARINC: usize = 0;
pub const SITA: usize = 1;
pub const MAX_SEARCH_RANGE_NM: f64 = 400.0;
pub const VDL_MAX_DATARATE: f64 = 31500.0;
pub const DATALINK_FREQUENCIES_MHZ: [f64; PROVIDER_COUNT] = [137.275, 137.975];
pub const MAX_AIRPORTS_IN_RANGE: usize = 50;

const ADDITIVE_NOISE_OVERLAP_DB: f64 = 1.4;
const MAXIMUM_DAMPING_DB: f64 = -75.0;
const RECEIVER_ANTENNA_GAIN_DBI: f64 = 25.0;
const SIGNAL_STRENGTH_DBW: f64 = 39.1202;

const UPPER_SECTOR_ALTITUDE: f64 = 24000.0;
const MESSAGE_CHUNKS_PER_SECOND: f64 = 32.0;
const DATASLOTS_PER_SECOND: f64 = 24.0;
const BITS_OF_CHUNKS_PER_SECOND: f64 = MESSAGE_CHUNKS_PER_SECOND * 496.0;
const BYTES_PER_SLOT: f64 = 62.0;
pub const TRANSMISSION_TIME_PER_PACKET_MS: f64 = 40.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnAircraft {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
    pub altitude_above_ground: f64,
    pub pressure_altitude: f64,
}

#[derive(Clone, Debug, Default)]
pub struct VhfAirportList {
    icaos: HashSet<String>,
}

impl VhfAirportList {
    pub fn from_text(text: &str) -> VhfAirportList {
        let icaos = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| l.to_ascii_uppercase())
            .collect();
        VhfAirportList { icaos }
    }

    pub fn load(path: &Path) -> std::io::Result<VhfAirportList> {
        Ok(VhfAirportList::from_text(&std::fs::read_to_string(path)?))
    }

    pub fn contains(&self, icao: &str) -> bool {
        self.icaos.contains(icao)
    }

    pub fn len(&self) -> usize {
        self.icaos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.icaos.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NearbyAirport {
    pub icao: String,
    pub elevation_ft: f64,
    pub distance_nm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceStation {
    pub callsign: String,
    pub frequency_mhz: f64,
    pub distance_nm: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Traffic {
    pub uid: u32,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RelevantAirport {
    pub icao: String,
    pub elevation: f64,
    pub distance: f64,
    pub datarates: [Option<f64>; PROVIDER_COUNT],
}

pub fn free_space_path_loss(frequency_mhz: f64, distance_nm: f64) -> f64 {
    let meters = distance_nm * 1852.0;
    10.0 * ((4.0 * std::f64::consts::PI * meters * (frequency_mhz * 1_000_000.0)) / 299_792_458.0)
        .powi(2)
        .log10()
}

pub fn line_of_sight_nm(altitude0: f64, altitude1: f64) -> f64 {
    1.23 * (altitude0 - altitude1).abs().sqrt()
}

pub fn distance_3d_nm(lat0: f64, lon0: f64, alt0: f64, lat1: f64, lon1: f64, alt1: f64) -> f64 {
    let earth_radius = 3440.065;
    let d = std::f64::consts::PI / 180.0;
    let r1 = alt0 / 6076.0 + earth_radius;
    let r2 = alt1 / 6076.0 + earth_radius;
    let p = |r: f64, lat: f64, lon: f64| {
        (
            r * (d * (lat + 90.0)).sin() * (d * (lon + 180.0)).cos(),
            r * (d * (lat + 90.0)).sin() * (d * (lon + 180.0)).sin(),
            r * (d * (lat + 90.0)).cos(),
        )
    };
    let (x1, y1, z1) = p(r1, lat0, lon0);
    let (x2, y2, z2) = p(r2, lat1, lon1);
    ((x1 - x2).powi(2) + (y1 - y2).powi(2) + (z1 - z2).powi(2)).sqrt()
}

#[derive(Clone, Debug)]
pub struct Vhf {
    pub stations_upper_airspace: usize,
    pub datarates: [f64; PROVIDER_COUNT],
    pub frequency_overlap: [u32; PROVIDER_COUNT],
    pub relevant_airports: Vec<RelevantAirport>,
    pub datalink_status: DatalinkStatusCode,
    pub datalink_mode: DatalinkModeCode,
}

impl Default for Vhf {
    fn default() -> Self {
        Vhf {
            stations_upper_airspace: 0,
            datarates: [0.0; PROVIDER_COUNT],
            frequency_overlap: [0; PROVIDER_COUNT],
            relevant_airports: Vec::new(),
            datalink_status: DatalinkStatusCode::Inop,
            datalink_mode: DatalinkModeCode::None,
        }
    }
}

impl Vhf {
    pub fn estimate_datarate(&self, provider: usize, distance_nm: f64, phase: AtsuFlightPhase) -> Option<f64> {
        let maximum_loss = SIGNAL_STRENGTH_DBW + RECEIVER_ANTENNA_GAIN_DBI
            - ADDITIVE_NOISE_OVERLAP_DB * self.frequency_overlap[provider] as f64
            - MAXIMUM_DAMPING_DB;
        let mut loss = free_space_path_loss(DATALINK_FREQUENCIES_MHZ[provider], distance_nm);
        match phase {
            AtsuFlightPhase::Preflight | AtsuFlightPhase::Done => loss += 30.0,
            AtsuFlightPhase::Takeoff | AtsuFlightPhase::GoAround | AtsuFlightPhase::Approach => loss += 15.0,
            _ => {}
        }
        if maximum_loss < loss {
            return None;
        }
        let quality = ((maximum_loss - loss) / MAXIMUM_DAMPING_DB.abs()).min(1.0);
        let scaling = (1.0 / ((9.0 * (1.0 - quality) - 5.0).exp() + 1.0)).max(0.1);
        Some(VDL_MAX_DATARATE * scaling)
    }

    pub fn update_used_voice_frequencies(&mut self, controllers: &[VoiceStation]) {
        self.frequency_overlap = [0; PROVIDER_COUNT];
        for c in controllers {
            if c.callsign.contains("_OBS") || c.frequency_mhz > 136.975 || c.distance_nm > MAX_SEARCH_RANGE_NM {
                continue;
            }
            for (i, f) in DATALINK_FREQUENCIES_MHZ.iter().enumerate() {
                if (c.frequency_mhz - f).abs() <= 0.025 {
                    self.frequency_overlap[i] += 1;
                }
            }
        }
    }

    pub fn update_relevant_airports(
        &mut self,
        own: &OwnAircraft,
        nearby: &[NearbyAirport],
        list: &VhfAirportList,
        phase: AtsuFlightPhase,
    ) {
        self.stations_upper_airspace = 0;
        self.relevant_airports.clear();
        for apt in nearby.iter().take(MAX_AIRPORTS_IN_RANGE) {
            let icao = apt.icao.trim();
            if !list.contains(icao) {
                continue;
            }
            if apt.distance_nm <= line_of_sight_nm(own.pressure_altitude, apt.elevation_ft) {
                let mut datarates = [None; PROVIDER_COUNT];
                for (i, rate) in datarates.iter_mut().enumerate() {
                    *rate = self.estimate_datarate(i, apt.distance_nm, phase);
                }
                if datarates.iter().any(Option::is_some) {
                    self.relevant_airports.push(RelevantAirport {
                        icao: icao.to_string(),
                        elevation: apt.elevation_ft,
                        distance: apt.distance_nm,
                        datarates,
                    });
                }
            }
            if apt.distance_nm <= MAX_SEARCH_RANGE_NM {
                self.stations_upper_airspace += 1;
            }
        }
    }

    pub fn simulate_datarates(
        &mut self,
        own: &OwnAircraft,
        phase: AtsuFlightPhase,
        nearby: &[NearbyAirport],
        list: &VhfAirportList,
        controllers: &[VoiceStation],
    ) {
        self.update_used_voice_frequencies(controllers);
        self.update_relevant_airports(own, nearby, list, phase);
        self.datarates = [0.0; PROVIDER_COUNT];
        let mut count = [0usize; PROVIDER_COUNT];
        for apt in &self.relevant_airports {
            for ((sum, n), rate) in self.datarates.iter_mut().zip(count.iter_mut()).zip(apt.datarates.iter()) {
                if let Some(rate) = rate {
                    *sum += rate;
                    *n += 1;
                }
            }
        }
        for (sum, n) in self.datarates.iter_mut().zip(count.iter()) {
            if *n != 0 {
                *sum /= *n as f64;
            }
        }
    }

    pub fn update_datalink_states(&mut self, interface_powered: bool, data_mode: bool, acars_active: bool) {
        if !interface_powered {
            self.datalink_status = DatalinkStatusCode::Inop;
            self.datalink_mode = DatalinkModeCode::None;
        } else if !data_mode {
            self.datalink_status = DatalinkStatusCode::DlkNotAvail;
            self.datalink_mode = DatalinkModeCode::None;
        } else {
            self.datalink_status = DatalinkStatusCode::DlkAvail;
            self.datalink_mode = if acars_active { DatalinkModeCode::AtcAoc } else { DatalinkModeCode::Aoc };
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct QueueDelay {
    update_time: f64,
    messages: i32,
    delay: f64,
}

#[derive(Clone, Debug)]
pub struct Vdl {
    pub vhf3: Vhf,
    inbound: QueueDelay,
    outbound: QueueDelay,
    upper_airspace_traffic: usize,
    lower_airspace_traffic: usize,
    pub per_packet_delay: [f64; PROVIDER_COUNT],
}

impl Default for Vdl {
    fn default() -> Self {
        Vdl {
            vhf3: Vhf::default(),
            inbound: QueueDelay::default(),
            outbound: QueueDelay::default(),
            upper_airspace_traffic: 0,
            lower_airspace_traffic: 0,
            per_packet_delay: [500.0; PROVIDER_COUNT],
        }
    }
}

pub struct VdlInputs<'a> {
    pub own: OwnAircraft,
    pub phase: AtsuFlightPhase,
    pub nearby_airports: &'a [NearbyAirport],
    pub airport_list: &'a VhfAirportList,
    pub voice_stations: &'a [VoiceStation],
    pub traffic: &'a [Traffic],
}

impl Vdl {
    fn update_remote_aircraft(&mut self, own: &OwnAircraft, traffic: &[Traffic]) {
        self.lower_airspace_traffic = 0;
        self.upper_airspace_traffic = 0;
        for t in traffic {
            if t.latitude == 0.0 || t.longitude == 0.0 || t.altitude == 0.0 || t.uid == 0 {
                continue;
            }
            let distance = distance_3d_nm(
                t.latitude,
                t.longitude,
                t.altitude,
                own.latitude,
                own.longitude,
                own.pressure_altitude,
            );
            if distance <= MAX_SEARCH_RANGE_NM {
                if t.altitude < UPPER_SECTOR_ALTITUDE {
                    self.lower_airspace_traffic += 1;
                } else {
                    self.upper_airspace_traffic += 1;
                }
            }
        }
    }

    pub fn simulate_transmission_times(&mut self, inputs: &VdlInputs) {
        self.vhf3.simulate_datarates(
            &inputs.own,
            inputs.phase,
            inputs.nearby_airports,
            inputs.airport_list,
            inputs.voice_stations,
        );
        self.update_remote_aircraft(&inputs.own, inputs.traffic);

        if self.vhf3.datarates.iter().all(|r| *r == 0.0) {
            self.per_packet_delay = [10000.0; PROVIDER_COUNT];
            return;
        }

        let relevant = self.vhf3.relevant_airports.len() as f64;
        let upper = self.upper_airspace_traffic as f64;
        let lower = self.lower_airspace_traffic as f64;
        let mut stations = if inputs.own.pressure_altitude < UPPER_SECTOR_ALTITUDE {
            let ratio = if self.vhf3.stations_upper_airspace != 0 {
                relevant / self.vhf3.stations_upper_airspace as f64
            } else {
                1.0
            };
            lower + ratio * upper
        } else {
            let ratio = if self.vhf3.stations_upper_airspace != 0 { 1.0 / relevant } else { 1.0 };
            upper + ratio * lower
        };
        stations += 1.0 + relevant;

        for i in 0..PROVIDER_COUNT {
            let mut count = (DATASLOTS_PER_SECOND * (self.vhf3.datarates[i] / BITS_OF_CHUNKS_PER_SECOND).min(1.0)).floor();
            count *= relevant;
            let per_station = count / stations;
            self.per_packet_delay[i] = (1000.0 / per_station + 0.5).round();
        }
    }

    fn min_packet_delay(&self) -> f64 {
        self.per_packet_delay.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn transmission_time_ms(&self, network_text_len: usize) -> f64 {
        let blocks = (network_text_len as f64 / BYTES_PER_SLOT + 0.5).round();
        blocks * TRANSMISSION_TIME_PER_PACKET_MS + (blocks - 1.0) * self.min_packet_delay()
    }

    pub fn reinitialize(&mut self) {
        self.inbound = QueueDelay::default();
        self.outbound = QueueDelay::default();
    }

    pub fn enqueue_inbound_message(&mut self, now_ms: f64, network_text_len: usize) -> f64 {
        let mut time = self.transmission_time_ms(network_text_len);
        if self.inbound.messages != 0 {
            time += self.min_packet_delay();
        } else {
            self.inbound.update_time = now_ms;
        }
        self.inbound.messages += 1;
        self.inbound.delay = time;
        time - (now_ms - self.inbound.update_time)
    }

    pub fn dequeue_inbound_message(&mut self, now_ms: f64, delay: f64) {
        Self::dequeue(&mut self.inbound, now_ms, delay);
    }

    fn enqueue_outbound(&mut self, now_ms: f64, base: f64) -> f64 {
        let mut time = base;
        if self.outbound.messages != 0 {
            time += self.min_packet_delay();
        } else {
            time += TRANSMISSION_TIME_PER_PACKET_MS * self.vhf3.relevant_airports.len() as f64;
            self.outbound.update_time = now_ms;
        }
        self.outbound.messages += 1;
        self.outbound.delay = time;
        time - (now_ms - self.outbound.update_time)
    }

    pub fn enqueue_outbound_message(&mut self, now_ms: f64, network_text_len: usize) -> f64 {
        let base = self.transmission_time_ms(network_text_len);
        self.enqueue_outbound(now_ms, base)
    }

    pub fn enqueue_outbound_packet(&mut self, now_ms: f64) -> f64 {
        self.enqueue_outbound(now_ms, TRANSMISSION_TIME_PER_PACKET_MS)
    }

    pub fn dequeue_outbound_message(&mut self, now_ms: f64, delay: f64) {
        Self::dequeue(&mut self.outbound, now_ms, delay);
    }

    fn dequeue(q: &mut QueueDelay, now_ms: f64, delay: f64) {
        q.delay = (q.delay - delay).max(0.0);
        q.update_time = now_ms;
        q.messages -= 1;
        if q.messages <= 0 {
            q.messages = 0;
            q.delay = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> VhfAirportList {
        VhfAirportList::from_text(include_str!("../data/vhf_airports.txt"))
    }

    #[test]
    fn bundled_list_loads() {
        let l = list();
        assert_eq!(l.len(), 1379);
        assert!(l.contains("EDDF"));
        assert!(l.contains("NLWW"));
        assert!(!l.contains("ZZZZ"));
    }

    #[test]
    fn path_loss_matches_formula() {
        let loss = free_space_path_loss(137.275, 10.0);
        assert!((loss - 100.54).abs() < 0.05, "{loss}");
    }

    #[test]
    fn close_station_gives_full_rate_and_buildings_reduce_it() {
        let vhf = Vhf::default();
        let cruise = vhf.estimate_datarate(ARINC, 20.0, AtsuFlightPhase::Cruise).unwrap();
        let ground = vhf.estimate_datarate(ARINC, 20.0, AtsuFlightPhase::Preflight).unwrap();
        assert!(cruise > ground);
        assert!(cruise <= VDL_MAX_DATARATE);
        assert!(ground >= VDL_MAX_DATARATE * 0.1);
        assert!(vhf.estimate_datarate(ARINC, 2000.0, AtsuFlightPhase::Cruise).is_none());
    }

    #[test]
    fn voice_overlap_counts_neighbours_only() {
        let mut vhf = Vhf::default();
        let s = |cs: &str, f: f64, d: f64| VoiceStation { callsign: cs.into(), frequency_mhz: f, distance_nm: d };
        vhf.update_used_voice_frequencies(&[
            s("TEST_CTR", 137.27, 10.0),
            s("TEST_OBS", 137.27, 10.0),
            s("FAR_CTR", 137.27, 500.0),
            s("OTHER_APP", 118.5, 10.0),
        ]);
        assert_eq!(vhf.frequency_overlap, [0, 0]);
        vhf.update_used_voice_frequencies(&[s("LOW_CTR", 136.96, 10.0)]);
        assert_eq!(vhf.frequency_overlap, [0, 0]);
    }

    fn cruise_inputs<'a>(nearby: &'a [NearbyAirport], l: &'a VhfAirportList, traffic: &'a [Traffic]) -> VdlInputs<'a> {
        VdlInputs {
            own: OwnAircraft { latitude: 50.0, longitude: 8.5, altitude: 35000.0, altitude_above_ground: 34000.0, pressure_altitude: 35000.0 },
            phase: AtsuFlightPhase::Cruise,
            nearby_airports: nearby,
            airport_list: l,
            voice_stations: &[],
            traffic,
        }
    }

    #[test]
    fn vdl_without_stations_is_slow() {
        let l = list();
        let mut vdl = Vdl::default();
        vdl.simulate_transmission_times(&cruise_inputs(&[], &l, &[]));
        assert_eq!(vdl.per_packet_delay, [10000.0, 10000.0]);
    }

    #[test]
    fn vdl_with_stations_and_traffic() {
        let l = list();
        let nearby = vec![
            NearbyAirport { icao: "EDDF".into(), elevation_ft: 364.0, distance_nm: 15.0 },
            NearbyAirport { icao: "EDDS".into(), elevation_ft: 1300.0, distance_nm: 90.0 },
            NearbyAirport { icao: "XXXX".into(), elevation_ft: 0.0, distance_nm: 5.0 },
        ];
        let traffic = vec![
            Traffic { uid: 1, latitude: 50.1, longitude: 8.6, altitude: 36000.0 },
            Traffic { uid: 2, latitude: 50.1, longitude: 8.6, altitude: 3000.0 },
            Traffic { uid: 0, latitude: 50.1, longitude: 8.6, altitude: 3000.0 },
        ];
        let mut vdl = Vdl::default();
        vdl.simulate_transmission_times(&cruise_inputs(&nearby, &l, &traffic));
        assert_eq!(vdl.vhf3.relevant_airports.len(), 2);
        assert_eq!(vdl.vhf3.stations_upper_airspace, 2);
        assert!(vdl.per_packet_delay[ARINC] > 0.0 && vdl.per_packet_delay[ARINC] < 10000.0);

        let t1 = vdl.enqueue_outbound_message(1000.0, 100);
        let blocks = 2.0;
        let expected = blocks * 40.0 + vdl.per_packet_delay.iter().cloned().fold(f64::INFINITY, f64::min) + 80.0;
        assert_eq!(t1, expected);
        let t2 = vdl.enqueue_outbound_packet(1010.0);
        assert!(t2 > 0.0);
        vdl.dequeue_outbound_message(1100.0, t1);
        vdl.dequeue_outbound_message(1200.0, t2);
        let t3 = vdl.enqueue_outbound_packet(2000.0);
        assert_eq!(t3, 40.0 + 80.0);
    }

    #[test]
    fn datalink_states() {
        let mut vhf = Vhf::default();
        vhf.update_datalink_states(false, true, true);
        assert_eq!((vhf.datalink_status, vhf.datalink_mode), (DatalinkStatusCode::Inop, DatalinkModeCode::None));
        vhf.update_datalink_states(true, false, true);
        assert_eq!(vhf.datalink_status, DatalinkStatusCode::DlkNotAvail);
        vhf.update_datalink_states(true, true, false);
        assert_eq!((vhf.datalink_status, vhf.datalink_mode), (DatalinkStatusCode::DlkAvail, DatalinkModeCode::Aoc));
        vhf.update_datalink_states(true, true, true);
        assert_eq!(vhf.datalink_mode, DatalinkModeCode::AtcAoc);
    }
}
