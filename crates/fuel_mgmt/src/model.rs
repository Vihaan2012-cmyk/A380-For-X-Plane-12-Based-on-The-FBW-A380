pub const TANK_COUNT: usize = 11;

pub type Kg = [f64; TANK_COUNT];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tank {
    LeftOuter = 0,
    Feed1 = 1,
    LeftMid = 2,
    LeftInner = 3,
    Feed2 = 4,
    Feed3 = 5,
    RightInner = 6,
    RightMid = 7,
    Feed4 = 8,
    RightOuter = 9,
    Trim = 10,
}

impl Tank {
    pub const ALL: [Tank; TANK_COUNT] = [
        Tank::LeftOuter,
        Tank::Feed1,
        Tank::LeftMid,
        Tank::LeftInner,
        Tank::Feed2,
        Tank::Feed3,
        Tank::RightInner,
        Tank::RightMid,
        Tank::Feed4,
        Tank::RightOuter,
        Tank::Trim,
    ];
    pub const FEED: [Tank; 4] = [Tank::Feed1, Tank::Feed2, Tank::Feed3, Tank::Feed4];

    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn is_feed(self) -> bool {
        matches!(self, Tank::Feed1 | Tank::Feed2 | Tank::Feed3 | Tank::Feed4)
    }

    pub fn is_left(self) -> bool {
        matches!(self, Tank::LeftOuter | Tank::Feed1 | Tank::LeftMid | Tank::LeftInner | Tank::Feed2)
    }

    pub fn is_right(self) -> bool {
        matches!(self, Tank::RightOuter | Tank::Feed4 | Tank::RightMid | Tank::RightInner | Tank::Feed3)
    }
}

const CAPACITY_USG: [f64; TANK_COUNT] = [2731.5, 7299.6, 9632.0, 12189.4, 7753.2, 7753.2, 12189.4, 9632.0, 7299.6, 2731.5, 6260.3];
const L_PER_USG: f64 = 3.785_411_784;
pub const FUEL_DENSITY_KG_L: f64 = 0.8;

pub fn capacity_kg() -> Kg {
    let mut c = [0.0; TANK_COUNT];
    for (i, g) in CAPACITY_USG.iter().enumerate() {
        c[i] = g * L_PER_USG * FUEL_DENSITY_KG_L;
    }
    c
}

const CENTROID_X_FT_FWD: [f64; TANK_COUNT] = [-25.0, -7.45, 7.1, 16.5, 27.3, 27.3, 16.5, 7.1, -7.45, -25.0, -87.14];
const FT_TO_M: f64 = 0.3048;

#[derive(Clone, Debug)]
pub struct CgModel {
    pub mac_m: f64,
    pub arm_pct_mac: Kg,
}

impl Default for CgModel {
    fn default() -> Self {
        let mac_m = 12.36;
        let mut arm = [0.0; TANK_COUNT];
        for (i, x) in CENTROID_X_FT_FWD.iter().enumerate() {
            arm[i] = 40.0 - x * FT_TO_M / mac_m * 100.0;
        }
        Self { mac_m, arm_pct_mac: arm }
    }
}

impl CgModel {
    pub fn cg_after(&self, cg_pct: f64, gw_kg: f64, delta_kg: &Kg) -> (f64, f64) {
        let moment = gw_kg * cg_pct + delta_kg.iter().zip(self.arm_pct_mac.iter()).map(|(d, a)| d * a).sum::<f64>();
        let gw = gw_kg + delta_kg.iter().sum::<f64>();
        if gw <= 1.0 {
            return (cg_pct, gw.max(0.0));
        }
        (moment / gw, gw)
    }

    pub fn tank_arm(&self, tank: Tank) -> f64 {
        self.arm_pct_mac[tank.idx()]
    }
}

pub fn apply_delta(tanks: &mut Kg, delta: &Kg) -> Kg {
    let cap = capacity_kg();
    let mut applied = [0.0; TANK_COUNT];
    for i in 0..TANK_COUNT {
        let next = (tanks[i] + delta[i]).clamp(0.0, cap[i]);
        applied[i] = next - tanks[i];
        tanks[i] = next;
    }
    applied
}

pub fn total_kg(tanks: &Kg) -> f64 {
    tanks.iter().sum()
}

pub fn moved(v: f64, target: f64, step: f64) -> f64 {
    if (target - v).abs() <= step {
        target
    } else {
        v + step * (target - v).signum()
    }
}
