// Translated from FlyByWire's fmgc guidance/vnav/EngineModel.ts (GPL-3.0-or-later).

use crate::atmosphere::{delta as pressure_ratio, interpolate};
use crate::config::{EngineModelParameters, Table};

pub fn table_interpolation(table: &Table, i: f64, j: f64) -> f64 {
    let num_rows = table.len();
    let num_cols = table[0].len();
    let mut r = 1;
    while r < num_rows {
        if table[r][0] > i {
            break;
        }
        r += 1;
    }
    let r1 = 1.max(r.saturating_sub(1));
    let r2 = (num_rows - 1).min(r);
    let mut c = 1;
    while c < num_cols {
        if table[0][c] > j {
            break;
        }
        c += 1;
    }
    let c1 = 1.max(c.saturating_sub(1));
    let c2 = (num_cols - 1).min(c);

    let at_c1 = if r1 == r2 {
        table[r1][c1]
    } else {
        interpolate(i, table[r1][0], table[r2][0], table[r1][c1], table[r2][c1])
    };
    let at_c2 = if r1 == r2 {
        table[r1][c2]
    } else {
        interpolate(i, table[r1][0], table[r2][0], table[r1][c2], table[r2][c2])
    };
    if c1 == c2 {
        return at_c1;
    }
    interpolate(j, table[0][c1], table[0][c2], at_c1, at_c2)
}

pub fn reverse_table_interpolation(table: &Table, j: f64, result: f64) -> f64 {
    let num_rows = table.len();
    let num_cols = table[0].len();

    let mut c = 1;
    while c < num_cols {
        if table[0][c] > j {
            break;
        }
        c += 1;
    }
    let c1 = 1.max(c.saturating_sub(1));
    let c2 = (num_cols - 1).min(c);

    let mut r = 1;
    while r < num_rows {
        if table[r][c1] > result {
            break;
        }
        r += 1;
    }
    let r1 = 1.max(r.saturating_sub(1));
    let r2 = (num_rows - 1).min(r);
    r = 1;
    while r < num_rows {
        if table[r][c2] > result {
            break;
        }
        r += 1;
    }
    let r3 = 1.max(r.saturating_sub(1));
    let r4 = (num_rows - 1).min(r);

    let at_c1 = if r1 == r2 {
        table[r1][0]
    } else {
        interpolate(result, table[r1][c1], table[r2][c1], table[r1][0], table[r2][0])
    };
    let at_c2 = if r3 == r4 {
        table[r3][0]
    } else {
        interpolate(result, table[r3][c2], table[r4][c2], table[r3][0], table[r4][0])
    };
    interpolate(j, table[0][c1], table[0][c2], at_c1, at_c2)
}

pub fn corrected_fuel_flow(p: &EngineModelParameters, cn1: f64, mach: f64, alt: f64) -> f64 {
    let c = [
        -1.763e2, -2.1542e-1, 4.7119e1, 6.1519e2, 1.8047e-3, -4.4554e-1, -4.394e1, 4.0459e-5, -3.2912e1,
        -6.2894e-3, -1.2544e-7, 1.0938e-2, 4.0936e-1, -5.5841e-6, -2.3829e1, 9.3269e-4, 2.0273e-11, -2.41e2,
        1.4171e-2, -9.5581e-7, 1.2728e-11,
    ];
    let flow = c[0]
        + c[1]
        + c[2] * cn1
        + c[3] * mach
        + c[4] * alt
        + c[5] * cn1.powi(2)
        + c[6] * cn1 * mach
        + c[7] * cn1 * alt
        + c[8] * mach.powi(2)
        + c[9] * mach * alt
        + c[10] * alt.powi(2)
        + c[11] * cn1.powi(3)
        + c[12] * cn1.powi(2) * mach
        + c[13] * cn1.powi(2) * alt
        + c[14] * cn1 * mach.powi(2)
        + c[15] * cn1 * mach * alt
        + c[16] * cn1 * alt.powi(2)
        + c[17] * mach.powi(3)
        + c[18] * mach.powi(2) * alt
        + c[19] * mach * alt.powi(2)
        + c[20] * alt.powi(3);
    p.fuel_burn_factor * flow
}

pub fn corrected_n1(n1: f64, theta2: f64) -> f64 {
    n1 / theta2.sqrt()
}

pub fn uncorrected_n1(cn1: f64, theta2: f64) -> f64 {
    cn1 * theta2.sqrt()
}

pub fn uncorrected_n2(cn2: f64, theta2: f64) -> f64 {
    cn2 * theta2.sqrt()
}

pub fn uncorrected_thrust(corrected: f64, delta2: f64) -> f64 {
    corrected * delta2
}

pub fn uncorrected_fuel_flow(corrected: f64, delta2: f64, theta2: f64) -> f64 {
    corrected * delta2 * theta2.sqrt()
}

pub fn corrected_thrust(uncorrected: f64, delta2: f64) -> f64 {
    uncorrected / delta2
}

pub fn idle_corrected_n1(p: &EngineModelParameters, altitude: f64, mach: f64, tropo_altitude: f64) -> f64 {
    let delta = pressure_ratio(altitude, altitude > tropo_altitude);
    let iap = 1.0 / delta;
    let low_mach_cn2 = table_interpolation(&p.table1503, 0.0, iap);
    let high_mach_cn2 = table_interpolation(&p.table1504, 0.0, iap);
    let cn2 = interpolate(mach, 0.0, 0.9, low_mach_cn2, high_mach_cn2);
    table_interpolation(&p.table1502, cn2, mach)
}

pub fn climb_thrust_corrected_n1(p: &EngineModelParameters, pressure_altitude: f64, outside_temperature: f64) -> f64 {
    let t = &p.cn1_climb_limit;
    let (lo, hi) = if pressure_altitude >= t[t.len() - 1][0] {
        (t.len() - 1, t.len() - 1)
    } else {
        let mut hi = 0;
        for (idx, row) in t.iter().enumerate() {
            if row[0] <= pressure_altitude {
                hi = idx + 1;
            }
        }
        (hi.saturating_sub(1), hi)
    };
    let lerp = |col: usize| {
        if lo == hi {
            t[lo][col]
        } else {
            interpolate(pressure_altitude, t[lo][0], t[hi][0], t[lo][col], t[hi][col])
        }
    };
    let cp = lerp(1);
    let lp = lerp(2);
    let cn1_flat = lerp(3);
    let cn1_last = lerp(4);
    if outside_temperature <= cp {
        return cn1_flat;
    }
    let m = (cn1_last - cn1_flat) / (lp - cp);
    let b = cn1_last - m * lp;
    m * outside_temperature + b
}
