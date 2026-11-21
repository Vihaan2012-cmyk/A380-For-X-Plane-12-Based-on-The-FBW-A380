//! Puts this aircraft's door block into openSAM's `planes.cfg`, ahead of its stock A388 entry, whose door 2 sits 3.7 m aft of
//! this model's M2L (issue #34). openSAM takes the first matching block and reads the file at every aircraft load.

use std::path::Path;

const STUDIO: &str = "A380 for X-Plane";
const BLOCK: &[&str] = &[
    "# A380 for X-Plane 12 (FlyByWire A380X port): M1L, M2L and U1L sills, metres from the CG reference",
    "icao\tA388",
    "studio\tA380 for X-Plane",
    "gpu_dref fbw/gse/connected/gpu eq 1",
    "pca_dref fbw/gse/connected/pca eq 1",
    "door_1 -2.92 0.15 -26.40",
    "door_2 -3.578 0.15 -16.21",
    "door_3 -2.85 2.89 -11.76",
    "door_1_psi 0",
    "door_3_psi 0",
    "door_1_xp 0",
    "door_2_xp 2",
    "door_3_xp 10",
    "no_auto_dock",
];

fn key_value(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let (k, v) = line.split_once(|c: char| c.is_whitespace())?;
    Some((k, v.trim()))
}

/// The file with this aircraft's block first among the aircraft blocks, or `None` when it already is.
pub fn merged(text: &str) -> Option<String> {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines: Vec<&str> = text.lines().collect();
    let ours = |l: &&str| key_value(l).is_some_and(|(k, v)| k == "studio" && v == STUDIO);
    let block_start = |i: usize| {
        let mut s = i;
        while s > 0 && lines[s - 1].trim().starts_with("icao") {
            s -= 1;
        }
        while s > 0 && lines[s - 1].trim_start().starts_with('#') && !lines[s - 1].trim_start().starts_with("##") {
            s -= 1;
        }
        s
    };
    let block_end = |i: usize| (i..lines.len()).find(|&j| lines[j].trim().is_empty() || lines[j].trim_start().starts_with('#')).unwrap_or(lines.len());
    let mut kept: Vec<&str> = lines.clone();
    if let Some(at) = lines.iter().position(ours) {
        let (s, e) = (block_start(at), block_end(at));
        let first_aircraft = lines.iter().position(|l| key_value(l).is_some_and(|(k, v)| k == "icao" && v != "fallback"));
        if lines[s..e].iter().copied().eq(BLOCK.iter().copied()) && first_aircraft.map(block_start) == Some(s) {
            return None;
        }
        kept.drain(s..e);
        while s < kept.len() && kept[s].trim().is_empty() && s > 0 && kept[s - 1].trim().is_empty() {
            kept.remove(s);
        }
    }
    let insert = kept
        .iter()
        .position(|l| key_value(l).is_some_and(|(k, v)| k == "icao" && v != "fallback"))
        .map(|i| {
            let mut s = i;
            while s > 0 && lines_comment(kept[s - 1]) {
                s -= 1;
            }
            s
        })
        .unwrap_or(kept.len());
    let mut out: Vec<&str> = kept[..insert].to_vec();
    if out.last().is_some_and(|l| !l.trim().is_empty()) {
        out.push("");
    }
    out.extend_from_slice(BLOCK);
    out.push("");
    out.extend_from_slice(&kept[insert..]);
    let mut s = out.join(nl);
    if text.ends_with('\n') && !s.ends_with('\n') {
        s.push_str(nl);
    }
    Some(s)
}

fn lines_comment(l: &str) -> bool {
    let t = l.trim_start();
    t.starts_with('#') && !t.starts_with("##")
}

/// Updates `<X-Plane>/Resources/plugins/openSAM/planes.cfg` when openSAM is installed.
pub fn ensure(xplane_root: &Path) {
    let path = xplane_root.join("Resources").join("plugins").join("openSAM").join("planes.cfg");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Some(new) = merged(&text) else { return };
    match state_snapshot::codec::write_atomic(&path, new.as_bytes()) {
        Ok(()) => crate::log("openSAM: this aircraft's door block written to planes.cfg (jetways use it from the next aircraft load)"),
        Err(e) => crate::log(&format!("openSAM: could not write planes.cfg: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STOCK: &str = "# header\n\n## Fallback\nicao    fallback\nstudio  openSAM\ngpu_dref x gt 20\n\n# Toliss A21N\nicao\tA21N\nstudio\tToLiss\n\nicao A388\ndoor_2 -3.50 0.83 -12.50\n";

    #[test]
    fn our_block_goes_ahead_of_the_stock_a388_entry_once() {
        let once = merged(STOCK).expect("inserted");
        let ours = once.find("studio\tA380 for X-Plane").unwrap();
        assert!(ours < once.find("icao A388").unwrap() && ours < once.find("A21N").unwrap());
        assert!(once.find("fallback").unwrap() < ours);
        assert!(once.contains("door_2 -3.578 0.15 -16.21"));
        assert_eq!(merged(&once), None, "a second pass changes nothing");
    }

    #[test]
    fn an_older_block_of_ours_is_replaced_and_kept_first() {
        let old = format!("{STOCK}\n# A380 for X-Plane 12\nicao\tA388\nstudio\tA380 for X-Plane\ndoor_2 -3.58 0.15 -16.21\n");
        let new = merged(&old).expect("replaced");
        assert_eq!(new.matches("studio\tA380 for X-Plane").count(), 1);
        assert!(new.find("studio\tA380 for X-Plane").unwrap() < new.find("icao A388").unwrap());
        assert_eq!(merged(&new), None);
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let new = merged(&STOCK.replace('\n', "\r\n")).unwrap();
        assert!(!new.replace("\r\n", "").contains('\n'));
    }
}
