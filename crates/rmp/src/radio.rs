#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrequencyMode {
    Frequency = 0,
    Data = 1,
    Emergency = 2,
}

impl FrequencyMode {
    pub fn from_f64(v: f64) -> Self {
        match v as i64 {
            1 => FrequencyMode::Data,
            2 => FrequencyMode::Emergency,
            _ => FrequencyMode::Frequency,
        }
    }
}

pub struct Channels {
    pub channel_mask: u32,
    pub channels: &'static [u32],
    pub min: u32,
    pub max: u32,
    pub carry_mask: u32,
}

pub const VHF_COM_8_33_25: Channels = Channels {
    channel_mask: 0xfff,
    channels: &[0, 0x50, 0x100, 0x150, 0x250, 0x300, 0x350, 0x400, 0x500, 0x550, 0x600, 0x650, 0x750, 0x800, 0x850, 0x900],
    min: 0x1180000,
    max: 0x1369750,
    carry_mask: 0xffff,
};

pub const DATA_FREQUENCY: u32 = 0x1315500;
pub const EMERGENCY_FREQUENCY: u32 = 0x1215000;

pub fn is_valid_range(bcd32: u32) -> bool {
    bcd32 >= VHF_COM_8_33_25.min && bcd32 <= VHF_COM_8_33_25.max
}

pub fn is_valid_spacing(bcd32: u32) -> bool {
    VHF_COM_8_33_25.channels.contains(&(bcd32 & VHF_COM_8_33_25.channel_mask))
}

pub fn is_valid_frequency(bcd32: u32) -> bool {
    is_valid_range(bcd32) && is_valid_spacing(bcd32)
}

pub fn format_bcd32(bcd32: u32, separator: &str) -> String {
    let digits = format!("{:06x}", (bcd32 >> 4) & 0xffffff);
    format!("{}{}{}", &digits[..3], separator, &digits[3..])
}

pub fn pack_bcd32_vhf_to_arinc(bcd32: u32) -> u32 {
    (bcd32 >> 4) & 0x7ffff
}

pub fn unpack_vhf_arinc_to_hz(arinc: u32) -> u32 {
    100_000_000 + 10_000_000 * ((arinc >> 16) & 0x7) + 1_000_000 * ((arinc >> 12) & 0xf) + 100_000 * ((arinc >> 8) & 0xf) + 10_000 * ((arinc >> 4) & 0xf) + 1_000 * (arinc & 0xf)
}

pub fn pack_bcd32_hz(hz: f64) -> u32 {
    let freq = (hz / 1000.).round() as i64;
    let d = |div: i64| ((freq / div) % 10) as u32;
    (d(1) << 4) + (d(10) << 8) + (d(100) << 12) + (d(1000) << 16) + (d(10000) << 20) + (d(100000) << 24)
}

pub fn bcd16_to_bcd32(bcd16: u32) -> u32 {
    0x1000000 | (bcd16 << 8)
}

pub fn increment_bcd32(bcd32: u32, nibble: u32, direction: i32) -> u32 {
    let mut ret = bcd32;
    for i in nibble..8 {
        let current = ((ret >> (i * 4)) & 0xf) as i32;
        let new = current + direction;
        let clear = !(0xfu32 << (i * 4));
        if new < 0 {
            ret = (ret & clear) | (9 << (i * 4));
        } else if new > 9 {
            ret &= clear;
        } else {
            ret = (ret & clear) | ((new as u32) << (i * 4));
            break;
        }
    }
    ret
}

fn closest_channel(bcd32: u32) -> u32 {
    let channel = bcd32 & VHF_COM_8_33_25.channel_mask;
    let mut best = VHF_COM_8_33_25.channels[0];
    let mut best_d = (best as i64 - channel as i64).abs();
    for &c in VHF_COM_8_33_25.channels {
        let d = (c as i64 - channel as i64).abs();
        if d < best_d {
            best = c;
            best_d = d;
        }
    }
    best
}

pub fn get_next_valid_channel(bcd32: u32, direction: i32, carry: bool) -> u32 {
    let info = &VHF_COM_8_33_25;
    let current = closest_channel(bcd32);
    let idx = info.channels.iter().position(|&c| c == current).unwrap() as i32;
    let new_idx = idx + direction;
    let ret;
    if new_idx >= 0 && (new_idx as usize) < info.channels.len() {
        ret = (bcd32 & !info.channel_mask) | info.channels[new_idx as usize];
    } else {
        let wrap = if new_idx < 0 { info.channels[info.channels.len() - 1] } else { info.channels[0] };
        let mut r = (bcd32 & !info.channel_mask) | wrap;
        let first_nibble = (32 - info.channel_mask.leading_zeros()) >> 2;
        r = increment_bcd32(r, first_nibble, direction);
        if !carry {
            let min = (bcd32 & !info.carry_mask) | info.channels[0];
            let max = (bcd32 & !info.carry_mask) | (0x99999999 & info.carry_mask & !info.channel_mask) | info.channels[info.channels.len() - 1];
            r = r.clamp(min, max);
        }
        ret = r;
    }
    ret.clamp(info.min, info.max)
}

pub fn get_closest_valid_frequency(bcd32: u32) -> u32 {
    let info = &VHF_COM_8_33_25;
    let ch = closest_channel(bcd32);
    ((bcd32 & !info.channel_mask) | ch).clamp(info.min, info.max)
}
