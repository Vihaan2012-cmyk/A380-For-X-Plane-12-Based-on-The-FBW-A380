//! Bus payloads for the native ND and VD (a380-displays displays/nd/terrain.cpp):
//! `a380.terrain.nd.<L|R>` and `a380.terrain.vd.<L|R>`.

use crate::bands::{BandImage, DisplayMode};
use crate::profile::ProfilePoint;

pub fn nd_topic(side: char) -> String {
    format!("a380.terrain.nd.{side}")
}

pub fn vd_topic(side: char) -> String {
    format!("a380.terrain.vd.{side}")
}

pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let v = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                s.push(T[(v >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v}")
    } else {
        "null".into()
    }
}

pub fn nd_json(img: &BandImage, generation: u64) -> String {
    format!(
        "{{\"gen\":{generation},\"n\":{},\"lat0\":{},\"lon0\":{},\"dlat\":{},\"dlon\":{},\"mode\":{},\"min\":{},\"minMode\":{},\"max\":{},\"maxMode\":{},\"bands\":\"{}\"}}",
        img.n,
        num(img.lat0),
        num(img.lon0),
        num(img.dlat),
        num(img.dlon),
        if img.mode == DisplayMode::Peaks { 1 } else { 0 },
        num(img.min_ft),
        img.min_mode,
        num(img.max_ft),
        img.max_mode,
        base64(&img.bands)
    )
}

/// `grey_from_nm`: distance beyond which the cut no longer lies ahead of the aircraft (FCOM
/// DSC-31-20-40-20 grey area), or none.
pub fn vd_json(points: &[ProfilePoint], step_nm: f64, grey_from_nm: Option<f64>, generation: u64) -> String {
    let mut elev = String::with_capacity(points.len() * 6);
    let mut kind = String::with_capacity(points.len());
    for (k, p) in points.iter().enumerate() {
        if k > 0 {
            elev.push(',');
        }
        match *p {
            ProfilePoint::Unknown => {
                elev.push_str("null");
                kind.push('U');
            }
            ProfilePoint::Water(h) => {
                elev.push_str(&format!("{}", h.round()));
                kind.push('W');
            }
            ProfilePoint::Land(h) => {
                elev.push_str(&format!("{}", h.round()));
                kind.push('L');
            }
        }
    }
    format!("{{\"gen\":{generation},\"stepNm\":{},\"grey\":{},\"elev\":[{elev}],\"kind\":\"{kind}\"}}", num(step_nm), grey_from_nm.map_or("null".into(), num))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(&[0, 1, 2, 9, 10]), "AAECCQo=");
    }

    #[test]
    fn vd_payload_lists_heights_and_kinds() {
        let j = vd_json(&[ProfilePoint::Land(1234.4), ProfilePoint::Water(0.), ProfilePoint::Unknown], 0.5, Some(12.), 7);
        assert_eq!(j, "{\"gen\":7,\"stepNm\":0.5,\"grey\":12,\"elev\":[1234,0,null],\"kind\":\"LWU\"}");
    }
}
