#[derive(Clone, Debug)]
pub struct Throttler {
    interval_ms: f64,
    offset_ms: f64,
    current_time: f64,
    last_update_time: f64,
    refresh_number: f64,
}

impl Throttler {
    pub fn new(interval_ms: f64, offset_ms: f64) -> Throttler {
        Throttler { interval_ms, offset_ms, current_time: 0., last_update_time: 0., refresh_number: 0. }
    }

    pub fn can_update(&mut self, delta_ms: f64) -> f64 {
        self.current_time += delta_ms;
        let number = ((self.current_time + self.offset_ms) / self.interval_ms).floor();
        let update = number > self.refresh_number;
        self.refresh_number = number;
        if update {
            let accumulated = self.current_time - self.last_update_time;
            self.last_update_time = self.current_time;
            accumulated
        } else {
            -1.
        }
    }
}
