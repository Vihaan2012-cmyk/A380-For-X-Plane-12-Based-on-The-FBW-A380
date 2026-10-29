use crate::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotSeatConfig {
    Auto,
    Left,
    Right,
}

impl PilotSeatConfig {
    pub fn parse(value: &str) -> Self {
        match value {
            "Auto" => PilotSeatConfig::Auto,
            "Right" => PilotSeatConfig::Right,
            _ => PilotSeatConfig::Left,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotSeat {
    Left = 0,
    Right = 1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightDeckBounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub min_z: f64,
    pub max_z: f64,
}

pub const A380X_FLIGHT_DECK: FlightDeckBounds =
    FlightDeckBounds { min_x: -0.92, max_x: 0.92, min_y: 2., max_y: 3.5, min_z: 30.8, max_z: 32.85 };

pub struct PilotSeatManager {
    bounds: FlightDeckBounds,
    config: PilotSeatConfig,
    actual_seat: PilotSeat,
    in_flight_deck: bool,
}

impl PilotSeatManager {
    pub fn new(bounds: FlightDeckBounds) -> Self {
        Self { bounds, config: PilotSeatConfig::Left, actual_seat: PilotSeat::Left, in_flight_deck: false }
    }

    pub fn init(&mut self, config: PilotSeatConfig) -> Vec<Command> {
        self.config = config;
        vec![Command::lvar("FBW_PILOT_SEAT", self.actual_seat as u8 as f64), Command::flag("FBW_IN_FLIGHT_DECK", self.in_flight_deck)]
    }

    pub fn set_config(&mut self, config: PilotSeatConfig) {
        self.config = config;
    }

    pub fn seat(&self) -> PilotSeat {
        self.actual_seat
    }

    pub fn update(&mut self, camera_pos: (f64, f64, f64), camera_state: i32) -> Vec<Command> {
        let mut out = Vec::new();
        let in_deck = self.is_in_flight_deck_bounds(camera_pos, camera_state);
        if in_deck != self.in_flight_deck {
            self.in_flight_deck = in_deck;
            out.push(Command::flag("FBW_IN_FLIGHT_DECK", in_deck));
        }
        let seat = if self.config == PilotSeatConfig::Auto {
            if self.in_flight_deck {
                Some(if camera_pos.0 < 0. { PilotSeat::Right } else { PilotSeat::Left })
            } else {
                None
            }
        } else {
            Some(if self.config == PilotSeatConfig::Right { PilotSeat::Right } else { PilotSeat::Left })
        };
        if let Some(seat) = seat {
            if seat != self.actual_seat {
                self.actual_seat = seat;
                out.push(Command::lvar("FBW_PILOT_SEAT", seat as u8 as f64));
            }
        }
        out
    }

    fn is_in_flight_deck_bounds(&self, pos: (f64, f64, f64), camera_state: i32) -> bool {
        let b = &self.bounds;
        pos.0 >= b.min_x
            && pos.0 <= b.max_x
            && pos.1 >= b.min_y
            && pos.1 <= b.max_y
            && pos.2 >= b.min_z
            && pos.2 <= b.max_z
            && (camera_state == 2 || camera_state == 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT_SEAT: (f64, f64, f64) = (0.5, 2.5, 31.5);
    const RIGHT_SEAT: (f64, f64, f64) = (-0.5, 2.5, 31.5);
    const OUTSIDE: (f64, f64, f64) = (0.5, 2.5, 20.);

    #[test]
    fn init_writes_left_seat_not_in_flight_deck() {
        let mut m = PilotSeatManager::new(A380X_FLIGHT_DECK);
        assert_eq!(m.init(PilotSeatConfig::Left), vec![Command::lvar("FBW_PILOT_SEAT", 0.), Command::flag("FBW_IN_FLIGHT_DECK", false)]);
    }

    #[test]
    fn fixed_right_seat_is_set_on_first_update_regardless_of_camera() {
        let mut m = PilotSeatManager::new(A380X_FLIGHT_DECK);
        m.init(PilotSeatConfig::Right);
        assert_eq!(m.update(OUTSIDE, 2), vec![Command::lvar("FBW_PILOT_SEAT", 1.)]);
        assert!(m.update(OUTSIDE, 2).is_empty());
    }

    #[test]
    fn auto_follows_camera_side_inside_the_flight_deck_only() {
        let mut m = PilotSeatManager::new(A380X_FLIGHT_DECK);
        m.init(PilotSeatConfig::Auto);
        let out = m.update(RIGHT_SEAT, 2);
        assert_eq!(out, vec![Command::flag("FBW_IN_FLIGHT_DECK", true), Command::lvar("FBW_PILOT_SEAT", 1.)]);
        let out = m.update(OUTSIDE, 2);
        assert_eq!(out, vec![Command::flag("FBW_IN_FLIGHT_DECK", false)]);
        assert_eq!(m.seat(), PilotSeat::Right);
        let out = m.update(LEFT_SEAT, 3);
        assert_eq!(out, vec![Command::flag("FBW_IN_FLIGHT_DECK", true), Command::lvar("FBW_PILOT_SEAT", 0.)]);
    }

    #[test]
    fn external_camera_states_are_never_in_the_flight_deck() {
        let mut m = PilotSeatManager::new(A380X_FLIGHT_DECK);
        m.init(PilotSeatConfig::Auto);
        assert!(m.update(LEFT_SEAT, 4).is_empty());
        assert!(m.update(LEFT_SEAT, 1).is_empty());
    }

    #[test]
    fn config_parse_defaults_to_left() {
        assert_eq!(PilotSeatConfig::parse("Auto"), PilotSeatConfig::Auto);
        assert_eq!(PilotSeatConfig::parse("Right"), PilotSeatConfig::Right);
        assert_eq!(PilotSeatConfig::parse("junk"), PilotSeatConfig::Left);
    }
}
