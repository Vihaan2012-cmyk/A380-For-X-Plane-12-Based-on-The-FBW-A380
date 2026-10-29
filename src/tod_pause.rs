//! Pause at top of descent. FlyByWire's FMGC (`TodGuidance`, inside the MFD
//! bundle) raises its "TOP OF DESCENT" / "AP PROTECTION" popup on the event
//! bus, and in MSFS the popup instrument's `TodPauseManager` then pauses the
//! sim. That instrument is not loaded here, so the popup event is caught on
//! its way between views (`GENERIC_DATA`, the bus sync key) and X-Plane is
//! paused instead.

use std::sync::Mutex;
use std::time::{Duration, Instant};

const BUS_SYNC_KEY: &str = "xp-ui.bus";
const TOD_POPUP: &str = "tod-pause";
const REPEAT_GUARD: Duration = Duration::from_secs(20);

static LAST: Mutex<Option<Instant>> = Mutex::new(None);

pub fn tod_popup_title(key: &str, json: &str) -> Option<String> {
    if key != BUS_SYNC_KEY || !json.contains(TOD_POPUP) {
        return None;
    }
    let packet: serde_json::Value = serde_json::from_str(json).ok()?;
    packet.get("events")?.as_array()?.iter().find_map(|e| {
        let data = e.get("data")?;
        (e.get("topic")?.as_str()? == "popup_enqueue_popup" && data.get("uuid")?.as_str()? == TOD_POPUP)
            .then(|| data.get("title").and_then(|t| t.as_str()).unwrap_or("TOP OF DESCENT").to_owned())
    })
}

pub fn on_generic_data(key: &str, json: &str) {
    let Some(title) = tod_popup_title(key, json) else { return };
    let Ok(mut last) = LAST.lock() else { return };
    if last.is_some_and(|t| t.elapsed() < REPEAT_GUARD) {
        return;
    }
    *last = Some(Instant::now());
    crate::log(&format!("pause at T/D: {title}, pausing X-Plane"));
    crate::xp::command_once("sim/operation/pause_on");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_tod_popup_on_the_bus_sync_key_pauses() {
        let tod = r#"{"events":[{"origin":"bus1","topic":"fws_active_item","data":3,"cached":true},{"origin":"bus1","topic":"popup_enqueue_popup","data":{"uuid":"tod-pause","title":"TOP OF DESCENT","message":"x"},"cached":false}]}"#;
        assert_eq!(tod_popup_title("xp-ui.bus", tod).as_deref(), Some("TOP OF DESCENT"));
        assert_eq!(tod_popup_title("other", tod), None);
        let other = r#"{"events":[{"origin":"bus1","topic":"popup_enqueue_popup","data":{"uuid":"update-available"},"cached":false}]}"#;
        assert_eq!(tod_popup_title("xp-ui.bus", other), None);
        let dequeue = r#"{"events":[{"origin":"bus1","topic":"popup_dequeue_popup","data":"tod-pause","cached":false}]}"#;
        assert_eq!(tod_popup_title("xp-ui.bus", dequeue), None);
    }
}
