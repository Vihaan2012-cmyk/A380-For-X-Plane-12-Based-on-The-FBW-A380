use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::vorbis::Pcm;

const ID_BASE: u32 = 0xFB00_0000;
const COPILOT_BASE: u32 = 0xFB01_0000;

pub const MAPPING: &[(&str, &str)] = &[
    ("new_2500", "co_2500"),
    ("new_2_500", "co_2500"),
    ("new_2000", "co_2000"),
    ("new_1000", "co_1000"),
    ("new_500", "co_500"),
    ("new_400", "co_400"),
    ("new_300", "co_300"),
    ("new_200", "co_200"),
    ("new_100", "co_100"),
    ("50_380", "co_50"),
    ("40_380", "co_40"),
    ("30_380", "co_30"),
    ("20_380", "co_20"),
    ("10_380", "co_10"),
    ("new_retard", "co_retard"),
    ("aural_minimumnew", "co_minimum"),
    ("aural_100above", "voice_hundred"),
    ("V1", "voice_v1"),
    ("3click", "voice_triple"),
    ("aural_stall_new", "co_Stall"),
    ("aural_pullup_new", "co_pull"),
    ("aural_sink_rate_new", "co_sink"),
    ("aural_dontsink_new", "co_dntsink"),
    ("aural_too_low_gear", "co_toogear"),
    ("aural_too_low_flaps", "co_tooflaps"),
    ("aural_too_low_terrain", "co_tooterr"),
    ("aural_terrain_ahead", "aural_terrain_ahead"),
    ("aural_terrain_ahead_pull_up", "aural_terrain_ahead_pull_up"),
    ("aural_obstacle_ahead", "aural_obstacle_ahead"),
    ("aural_obstacle_ahead_pull_up", "aural_obstacle_ahead_pull_up"),
    ("runway_too_short", "voice_rwy_short"),
    ("brake_max_braking", "voice_max_brk"),
    ("set_max_reverse", "voice_set_rev"),
    ("keep_max_reverse", "voice_keep_rev"),
    ("autobrake_off", "voice_ab_off"),
    ("traffic_traffic", "tc_traffic"),
    ("clear_of_conflict", "tc_clear"),
    ("climb_climb", "tc_climb"),
    ("descend_descend", "tc_descend"),
    ("climb_climb_now", "tc_climb_now"),
    ("descend_descend_now", "tc_desc_now"),
    ("increase_climb", "tc_inc_climb"),
    ("increase_descent", "tc_inc_desc"),
    ("monitor_vs", "tc_monitor"),
    ("level_off_level_off", "tc_adjust"),
];

#[derive(Default)]
pub struct VoicePack {
    by_event: HashMap<String, u32>,
    pcm: HashMap<u32, Arc<Pcm>>,
}

impl VoicePack {
    pub fn load(dir: &Path) -> Self {
        let mut pack = VoicePack::default();
        let mut ids: HashMap<&str, u32> = HashMap::new();
        for &(event, stem) in MAPPING {
            let id = match ids.get(stem) {
                Some(&id) => id,
                None => {
                    let Some(pcm) = std::fs::read(dir.join(format!("{stem}.wav"))).ok().and_then(|b| parse_wav(&b)) else { continue };
                    let id = ID_BASE + ids.len() as u32;
                    pack.pcm.insert(id, Arc::new(pcm));
                    ids.insert(stem, id);
                    id
                }
            };
            pack.by_event.insert(event.to_ascii_lowercase(), id);
        }
        pack
    }

    pub fn load_copilot(&mut self, dir: &Path) -> usize {
        self.by_event.retain(|_, id| *id < COPILOT_BASE);
        self.pcm.retain(|id, _| *id < COPILOT_BASE);
        let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
        let mut names: Vec<String> = entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("fo_") && n.ends_with(".wav")).collect();
        names.sort();
        let mut n = 0;
        for name in names {
            let Some(pcm) = std::fs::read(dir.join(&name)).ok().and_then(|b| parse_wav(&b)) else { continue };
            let id = COPILOT_BASE + n;
            self.pcm.insert(id, Arc::new(pcm));
            self.by_event.insert(name.trim_end_matches(".wav").to_ascii_lowercase(), id);
            n += 1;
        }
        n as usize
    }

    pub fn media_for(&self, event: &str) -> Option<u32> {
        self.by_event.get(&event.trim().to_ascii_lowercase()).copied()
    }

    pub fn pcm(&self, id: u32) -> Option<Arc<Pcm>> {
        self.pcm.get(&id).cloned()
    }

    pub fn len(&self) -> usize {
        self.by_event.len()
    }
}

pub fn parse_wav(b: &[u8]) -> Option<Pcm> {
    if b.len() < 12 || &b[..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data: Option<&[u8]> = None;
    let mut i = 12;
    while i + 8 <= b.len() {
        let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
        let body = b.get(i + 8..(i + 8 + size).min(b.len()))?;
        match &b[i..i + 4] {
            b"fmt " if body.len() >= 16 => {
                let u16_at = |o: usize| u16::from_le_bytes([body[o], body[o + 1]]);
                fmt = Some((u16_at(0), u16_at(2), u32::from_le_bytes(body[4..8].try_into().ok()?), u16_at(14)));
            }
            b"data" => data = Some(body),
            _ => {}
        }
        i += 8 + size + (size & 1);
    }
    let (format, channels, sample_rate, bits) = fmt?;
    if format != 1 || bits != 16 || channels == 0 {
        return None;
    }
    let samples = data?.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]])).collect();
    Some(Pcm { sample_rate, channels, samples, loop_frames: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[i16]) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        for v in [1u16, 1] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&48_000u32.to_le_bytes());
        b.extend_from_slice(&96_000u32.to_le_bytes());
        for v in [2u16, 16] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        b
    }

    #[test]
    fn a_16_bit_pcm_wav_reads_as_its_samples() {
        let pcm = parse_wav(&wav(&[0, 1000, -1000, i16::MAX])).unwrap();
        assert_eq!((pcm.sample_rate, pcm.channels), (48_000, 1));
        assert_eq!(pcm.samples, [0, 1000, -1000, i16::MAX]);
        assert!(parse_wav(b"RIFF....WAVEnope").is_none());
    }

    #[test]
    fn every_fbw_event_has_one_recording_and_a_missing_file_leaves_fbw_s_own() {
        let mut seen = std::collections::HashSet::new();
        for (event, _) in MAPPING {
            assert!(seen.insert(event.to_ascii_lowercase()), "{event} mapped twice");
        }
        let dir = std::env::temp_dir().join(format!("fbw-voice-pack-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("co_2500.wav"), wav(&[1, 2, 3])).unwrap();
        std::fs::write(dir.join("tc_traffic.wav"), wav(&[4])).unwrap();
        let pack = VoicePack::load(&dir);
        let a = pack.media_for("new_2500").unwrap();
        assert_eq!(pack.media_for("NEW_2_500"), Some(a), "both 2500 events share the one recording");
        assert_eq!(pack.pcm(a).unwrap().samples, [1, 2, 3]);
        assert!(pack.media_for("traffic_traffic").is_some());
        assert!(pack.media_for("new_1000").is_none(), "no file, FlyByWire's own sound stays");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_copilot_voice_replaces_the_last_one_and_leaves_the_callouts() {
        let base = std::env::temp_dir().join(format!("fbw-copilot-test-{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(base.join("co_2500.wav"), wav(&[1])).unwrap();
        std::fs::write(a.join("fo_rotate.wav"), wav(&[7])).unwrap();
        std::fs::write(a.join("fo_100kt.wav"), wav(&[8])).unwrap();
        std::fs::write(b.join("fo_rotate.wav"), wav(&[9])).unwrap();
        let mut pack = VoicePack::load(&base);
        assert_eq!(pack.load_copilot(&a), 2);
        assert_eq!(pack.pcm(pack.media_for("fo_rotate").unwrap()).unwrap().samples, [7]);
        assert_eq!(pack.load_copilot(&b), 1);
        assert_eq!(pack.pcm(pack.media_for("fo_rotate").unwrap()).unwrap().samples, [9]);
        assert!(pack.media_for("fo_100kt").is_none());
        assert!(pack.media_for("new_2500").is_some());
        let _ = std::fs::remove_dir_all(&base);
    }
}
