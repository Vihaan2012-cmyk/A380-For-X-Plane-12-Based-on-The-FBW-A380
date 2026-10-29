pub const TO_CONFIG_MAX_PRESS_TIME_MS: u64 = 30_000;
const CRUISE_FWC_PHASE: i32 = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub title: &'static str,
    pub message: &'static str,
}

const HELD_NOTIFICATION: Notification = Notification {
    title: "ECP Pushbutton Held",
    message: "The TO CONF pushbutton has been held for a long time!\n\nIf you have external hardware or software controlling this variable (L:A32NX_BTN_TOCONFIG), please check that it is setup to write the variable to 0 when the button is released.",
};

pub struct PushbuttonCheck {
    pressed_in_cruise: bool,
    due_at_ms: Option<u64>,
    message_shown: bool,
}

impl PushbuttonCheck {
    pub fn new() -> Self {
        Self { pressed_in_cruise: false, due_at_ms: None, message_shown: false }
    }

    pub fn update(&mut self, now_ms: u64, to_config_button: bool, fwc_flight_phase: i32) -> Option<Notification> {
        let pressed = to_config_button && fwc_flight_phase == CRUISE_FWC_PHASE;
        if pressed != self.pressed_in_cruise {
            self.pressed_in_cruise = pressed;
            if pressed && self.due_at_ms.is_none() && !self.message_shown {
                self.due_at_ms = Some(now_ms + TO_CONFIG_MAX_PRESS_TIME_MS);
            } else if !pressed {
                self.due_at_ms = None;
            }
        }
        if let Some(due) = self.due_at_ms {
            if now_ms >= due {
                self.due_at_ms = None;
                self.message_shown = true;
                return Some(HELD_NOTIFICATION);
            }
        }
        None
    }
}

impl Default for PushbuttonCheck {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_for_30_s_in_cruise_notifies_once() {
        let mut p = PushbuttonCheck::new();
        assert!(p.update(0, true, 8).is_none());
        assert!(p.update(29_999, true, 8).is_none());
        let n = p.update(30_000, true, 8).expect("notification");
        assert_eq!(n.title, "ECP Pushbutton Held");
        assert!(p.update(31_000, true, 8).is_none());
        assert!(p.update(32_000, false, 8).is_none());
        assert!(p.update(33_000, true, 8).is_none());
        assert!(p.update(90_000, true, 8).is_none());
    }

    #[test]
    fn release_or_phase_change_cancels_the_timer() {
        let mut p = PushbuttonCheck::new();
        p.update(0, true, 8);
        p.update(10_000, false, 8);
        assert!(p.update(40_000, false, 8).is_none());
        p.update(50_000, true, 8);
        p.update(60_000, true, 9);
        assert!(p.update(95_000, true, 9).is_none());
    }

    #[test]
    fn pressed_outside_cruise_never_counts() {
        let mut p = PushbuttonCheck::new();
        assert!(p.update(0, true, 1).is_none());
        assert!(p.update(100_000, true, 1).is_none());
    }
}
