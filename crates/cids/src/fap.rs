use crate::types::{Config, Event, Page, ZONES};

pub struct FapUnit {
    pub page: Page,
    pub boot_s: f64,
    pub off: bool,
    pub lock_s: f64,
    pub lav_maint: bool,
    pub was_powered: bool,
    pub msg: Option<(String, f64)>,
    pub emer_light: bool,
    pub pax_sys_off: bool,
    pub seat_power_off: bool,
    pub heaters_off: bool,
    pub galley_off: [bool; ZONES],
    pub scene: Option<u8>,
    pub level_pct: Option<u8>,
}

impl FapUnit {
    pub fn new() -> Self {
        Self {
            page: Page::Status,
            boot_s: 0.,
            off: false,
            lock_s: 0.,
            lav_maint: false,
            was_powered: false,
            msg: None,
            emer_light: false,
            pax_sys_off: false,
            seat_power_off: false,
            heaters_off: false,
            galley_off: [false; ZONES],
            scene: None,
            level_pct: None,
        }
    }

    pub fn say(&mut self, text: &str, cfg: &Config) {
        self.msg = Some((text.to_string(), cfg.msg_s));
    }

    pub fn tick(&mut self, powered: bool, cfg: &Config, dt: f64) {
        if powered && !self.was_powered {
            self.boot_s = cfg.boot_s;
            self.page = Page::Status;
            self.off = false;
        }
        if !powered {
            self.boot_s = 0.;
            self.msg = None;
            self.lock_s = 0.;
        }
        self.was_powered = powered;
        self.boot_s = (self.boot_s - dt).max(0.);
        self.lock_s = (self.lock_s - dt).max(0.);
        if let Some((_, left)) = &mut self.msg {
            *left -= dt;
        }
        if self.msg.as_ref().is_some_and(|m| m.1 <= 0.) {
            self.msg = None;
        }
    }

    pub fn live(&self, powered: bool) -> bool {
        powered && self.boot_s <= 0.
    }

    pub fn accepts(&self, ev: &Event) -> bool {
        ev.is_hard_key() || (!self.off && self.lock_s <= 0.)
    }

    pub fn navigate(&mut self, ev: &Event) {
        match ev {
            Event::SelectPage(p) => self.page = *p,
            Event::NextPage => self.page = Page::ALL[(self.page.index() + 1) % Page::ALL.len()],
            Event::PrevPage => self.page = Page::ALL[(self.page.index() + Page::ALL.len() - 1) % Page::ALL.len()],
            _ => {}
        }
    }

    pub fn apply_light_event(&mut self, ev: &Event) {
        match ev {
            Event::Scene(0) => self.scene = None,
            Event::Scene(n) => self.scene = Some(if *n >= 12 { 0 } else { (*n).clamp(1, 11) }),
            Event::Level(p) if *p > 100 => self.level_pct = None,
            Event::Level(p) => self.level_pct = Some(*p),
            Event::LightsMain => self.level_pct = if self.level_pct == Some(0) { Some(100) } else { Some(0) },
            _ => {}
        }
    }
}

impl Default for FapUnit {
    fn default() -> Self {
        Self::new()
    }
}
