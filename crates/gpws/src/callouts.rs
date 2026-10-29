pub mod flags {
    pub const TWO_THOUSAND_FIVE_HUNDRED: u32 = 1 << 0;
    pub const TWENTY_FIVE_HUNDRED: u32 = 1 << 1;
    pub const TWO_THOUSAND: u32 = 1 << 2;
    pub const ONE_THOUSAND: u32 = 1 << 3;
    pub const FIVE_HUNDRED: u32 = 1 << 4;
    pub const FIVE_HUNDRED_GLIDE: u32 = 1 << 5;
    pub const FOUR_HUNDRED: u32 = 1 << 6;
    pub const THREE_HUNDRED: u32 = 1 << 7;
    pub const TWO_HUNDRED: u32 = 1 << 8;
    pub const ONE_HUNDRED: u32 = 1 << 9;
    pub const NINETY: u32 = 1 << 10;
    pub const EIGHTY: u32 = 1 << 11;
    pub const SEVENTY: u32 = 1 << 12;
    pub const SIXTY: u32 = 1 << 13;
    pub const FIFTY: u32 = 1 << 14;
    pub const FORTY: u32 = 1 << 15;
    pub const THIRTY: u32 = 1 << 16;
    pub const TWENTY: u32 = 1 << 17;
    pub const TEN: u32 = 1 << 18;
    pub const FIVE: u32 = 1 << 19;
}

pub const DEFAULT_AUTO_CALL_OUTS: u32 = flags::TWO_THOUSAND_FIVE_HUNDRED
    | flags::ONE_THOUSAND
    | flags::FOUR_HUNDRED
    | flags::FIFTY
    | flags::FORTY
    | flags::THIRTY
    | flags::TWENTY
    | flags::TEN
    | flags::FIVE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Event {
    Enqueue(&'static str),
    Dequeue(&'static str),
}

struct Rung {
    up_above: f64,
    down_at_or_below: f64,
    flag: u32,
    sound: &'static str,
    skip_while_retard: bool,
}

const fn rung(up_above: f64, down_at_or_below: f64, flag: u32, sound: &'static str, skip_while_retard: bool) -> Rung {
    Rung { up_above, down_at_or_below, flag, sound, skip_while_retard }
}

// Index 0 is "ground", then over5, over10, over20 ... over2500. A rung holds the
// thresholds of the state it describes: leave upwards above `up_above`, downwards
// at or below `down_at_or_below` (calling out `sound` if its flag is in the pins).
const LADDER: [Rung; 19] = [
    rung(6., f64::NEG_INFINITY, 0, "", false),
    rung(12., 6., flags::FIVE, "alt_5", true),
    rung(22., 12., flags::TEN, "alt_10", true),
    rung(32., 22., flags::TWENTY, "alt_20", false),
    rung(42., 32., flags::THIRTY, "alt_30", false),
    rung(53., 42., flags::FORTY, "alt_40", false),
    rung(65., 53., flags::FIFTY, "alt_50", false),
    rung(75., 63., flags::SIXTY, "alt_60", false),
    rung(85., 73., flags::SEVENTY, "alt_70", false),
    rung(95., 83., flags::EIGHTY, "alt_80", false),
    rung(110., 93., flags::NINETY, "alt_90", false),
    rung(210., 110., flags::ONE_HUNDRED, "alt_100", false),
    rung(310., 210., flags::TWO_HUNDRED, "alt_200", false),
    rung(410., 310., flags::THREE_HUNDRED, "alt_300", false),
    rung(513., 410., flags::FOUR_HUNDRED, "alt_400", false),
    rung(1020., 513., flags::FIVE_HUNDRED, "alt_500", false),
    rung(2020., 1020., flags::ONE_THOUSAND, "alt_1000", false),
    rung(2530., 2020., flags::TWO_THOUSAND, "alt_2000", false),
    rung(f64::INFINITY, 2530., 0, "", false),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RetardState {
    OverRetard,
    RetardPlaying,
    Landed,
}

#[derive(Clone, Copy, Debug)]
pub struct CalloutContext {
    pub auto_call_out_pins: u32,
    pub autopilot_active: bool,
    pub tla: [f64; 4],
    pub go_around_phase: bool,
}

#[derive(Clone, Debug)]
pub struct Callouts {
    alt_state: usize,
    pub retard: RetardState,
}

impl Callouts {
    pub fn new() -> Callouts {
        Callouts { alt_state: 0, retard: RetardState::Landed }
    }

    pub fn alt_state_index(&self) -> usize {
        self.alt_state
    }

    pub fn update(&mut self, radio_alt: f64, ctx: &CalloutContext, events: &mut Vec<Event>) {
        if radio_alt.is_nan() {
            return;
        }

        let rung = &LADDER[self.alt_state];
        if self.alt_state < LADDER.len() - 1 && radio_alt > rung.up_above {
            self.alt_state += 1;
        } else if self.alt_state > 0 && radio_alt <= rung.down_at_or_below {
            let pins = ctx.auto_call_out_pins;
            if self.alt_state == LADDER.len() - 1 {
                if pins & flags::TWO_THOUSAND_FIVE_HUNDRED != 0 {
                    events.push(Event::Enqueue("alt_2500"));
                } else if pins & flags::TWENTY_FIVE_HUNDRED != 0 {
                    events.push(Event::Enqueue("alt_2500b"));
                }
            } else if pins & rung.flag != 0 && !(rung.skip_while_retard && self.retard == RetardState::RetardPlaying) {
                events.push(Event::Enqueue(rung.sound));
            }
            self.alt_state -= 1;
        }

        match self.retard {
            RetardState::OverRetard => {
                if radio_alt < 20. {
                    if !ctx.autopilot_active || radio_alt < 10. {
                        self.retard = RetardState::RetardPlaying;
                        events.push(Event::Enqueue("retard"));
                    }
                }
            }
            RetardState::RetardPlaying => {
                if ctx.tla.iter().any(|t| *t < 2.6) {
                    self.retard = RetardState::Landed;
                    events.push(Event::Dequeue("retard"));
                } else if ctx.go_around_phase || radio_alt > 20. {
                    self.retard = RetardState::OverRetard;
                    events.push(Event::Dequeue("retard"));
                }
            }
            RetardState::Landed => {
                if radio_alt > 20. {
                    self.retard = RetardState::OverRetard;
                }
            }
        }
    }
}

impl Default for Callouts {
    fn default() -> Self {
        Callouts::new()
    }
}
