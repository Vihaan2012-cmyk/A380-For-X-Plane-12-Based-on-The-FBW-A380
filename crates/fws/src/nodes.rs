#[derive(Debug, Clone)]
pub struct Monostable {
    t: f64,
    rising: bool,
    retrigger: bool,
    timer: f64,
    prev: Option<bool>,
    out: bool,
}

impl Monostable {
    pub fn new(t: f64, rising: bool, retrigger: bool) -> Self {
        Self { t, rising, retrigger, timer: 0., prev: None, out: false }
    }

    pub fn rising(t: f64) -> Self {
        Self::new(t, true, false)
    }

    pub fn falling(t: f64) -> Self {
        Self::new(t, false, false)
    }

    pub fn write(&mut self, value: bool, dt_ms: f64, skip_startup: bool) -> bool {
        if self.prev.is_none() && skip_startup {
            self.prev = Some(value);
        }
        if self.timer > 0. {
            self.timer = (self.timer - dt_ms / 1000.).max(0.);
        }
        let prev = self.prev.unwrap_or(false);
        if (self.retrigger || self.timer == 0.) && ((self.rising && value && !prev) || (!self.rising && !value && prev)) {
            self.timer = self.t;
        }
        self.prev = Some(value);
        self.out = self.timer > 0.;
        self.out
    }

    pub fn read(&self) -> bool {
        self.out
    }
}

#[derive(Debug, Clone)]
pub struct Confirm {
    t: f64,
    rising: bool,
    timer: f64,
    prev_in: Option<bool>,
    prev_out: Option<bool>,
}

impl Confirm {
    pub fn new(t: f64, rising: bool) -> Self {
        Self { t, rising, timer: 0., prev_in: None, prev_out: None }
    }

    pub fn rising(t: f64) -> Self {
        Self::new(t, true)
    }

    pub fn write(&mut self, value: bool, dt_ms: f64, skip_startup: bool) -> bool {
        if self.prev_in.is_none() && skip_startup {
            self.prev_in = Some(value);
            self.prev_out = Some(value);
        }
        let prev_in = self.prev_in.unwrap_or(false);
        if self.rising {
            if !value {
                self.timer = 0.;
            } else if self.timer > 0. {
                self.timer = (self.timer - dt_ms / 1000.).max(0.);
                self.prev_in = Some(value);
                self.prev_out = Some(!value);
                return !value;
            } else if !prev_in && value {
                self.timer = self.t;
                self.prev_in = Some(value);
                self.prev_out = Some(!value);
                return !value;
            }
        } else if value {
            self.timer = 0.;
        } else if self.timer > 0. {
            self.timer = (self.timer - dt_ms / 1000.).max(0.);
            self.prev_in = Some(value);
            self.prev_out = Some(!value);
            return !value;
        } else if prev_in && !value {
            self.timer = self.t;
            self.prev_in = Some(value);
            self.prev_out = Some(!value);
            return !value;
        }
        self.prev_in = Some(value);
        self.prev_out = Some(value);
        value
    }

    pub fn read(&self) -> bool {
        self.prev_out.unwrap_or(false)
    }

    pub fn previous_input(&self) -> bool {
        self.prev_in.unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    set_star: bool,
    value: bool,
}

impl Memory {
    pub fn new(set_star: bool) -> Self {
        Self { set_star, value: false }
    }

    pub fn write(&mut self, set: bool, reset: bool) -> bool {
        if set && reset {
            self.value = self.set_star;
        } else if set && !self.value {
            self.value = true;
        } else if reset && self.value {
            self.value = false;
        }
        self.value
    }

    pub fn read(&self) -> bool {
        self.value
    }
}

#[derive(Debug, Clone)]
pub struct Pulse {
    rising: bool,
    out: bool,
    last: bool,
}

impl Pulse {
    pub fn new(rising: bool) -> Self {
        Self { rising, out: false, last: false }
    }

    pub fn write(&mut self, input: bool) -> bool {
        if self.out {
            self.out = false;
        }
        if (self.rising && input && !self.last) || (!self.rising && !input && self.last) {
            self.out = true;
        }
        self.last = input;
        self.out
    }

    pub fn read(&self) -> bool {
        self.out
    }
}

#[derive(Debug, Clone)]
pub struct Clock {
    from: f64,
    to: f64,
    timer: f64,
    flag: bool,
    output: f64,
}

impl Clock {
    pub fn new(from: f64, to: f64) -> Self {
        Self { from, to, timer: 0., flag: false, output: 0. }
    }

    pub fn write(&mut self, value: bool, dt_ms: f64) -> f64 {
        if !value {
            self.timer = 0.;
            self.flag = false;
            self.output = 0.;
        }
        if !self.flag {
            self.flag = true;
            self.timer = self.from;
        } else {
            self.timer = (self.timer - dt_ms / 1000.).max(0.);
            self.output = if self.timer == self.to { 2. } else { 1. };
        }
        self.output
    }

    pub fn read(&self) -> f64 {
        self.output
    }
}

#[derive(Debug, Clone)]
pub struct Throttler {
    interval_ms: f64,
    offset_ms: f64,
    current: f64,
    last_update: f64,
    refresh_number: i64,
}

impl Throttler {
    pub fn new(interval_ms: f64, offset_ms: f64) -> Self {
        Self { interval_ms, offset_ms, current: 0., last_update: 0., refresh_number: 0 }
    }

    pub fn can_update(&mut self, dt_ms: f64) -> f64 {
        self.current += dt_ms;
        let number = ((self.current + self.offset_ms) / self.interval_ms).floor() as i64;
        let update = number > self.refresh_number;
        self.refresh_number = number;
        if update {
            let acc = self.current - self.last_update;
            self.last_update = self.current;
            acc
        } else {
            -1.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_passes_a_rising_signal_after_its_time() {
        let mut n = Confirm::rising(1.);
        let mut outs = Vec::new();
        for _ in 0..12 {
            outs.push(n.write(true, 125., false));
        }
        assert_eq!(&outs[..8], &[false; 8]);
        assert!(outs[9]);
        assert!(!n.write(false, 125., false));
    }

    #[test]
    fn monostable_holds_for_its_time_after_a_rising_edge() {
        let mut n = Monostable::rising(0.3);
        assert!(!n.write(false, 100., false));
        assert!(n.write(true, 100., false));
        assert!(n.write(true, 100., false));
        assert!(!n.write(true, 200., false));
    }

    #[test]
    fn memory_set_star_wins_a_tie() {
        let mut a = Memory::new(true);
        let mut b = Memory::new(false);
        assert!(a.write(true, true));
        assert!(!b.write(true, true));
    }

    #[test]
    fn pulse_lasts_one_write() {
        let mut p = Pulse::new(true);
        assert!(p.write(true));
        assert!(!p.write(true));
        assert!(!p.write(false));
        assert!(p.write(true));
    }

    #[test]
    fn throttler_spaces_updates_by_the_interval() {
        let mut t = Throttler::new(125., 80.);
        let mut times = Vec::new();
        for i in 0..10 {
            if t.can_update(50.) >= 0. {
                times.push((i + 1) * 50);
            }
        }
        assert_eq!(times, vec![50, 200, 300, 450]);
    }
}
