use crate::{bx, key, row, ttl, Ctx};
use cids::maint::TestResult;
use cids::view::PageBody;
use cids::TestKind;
use mfd_tree::Node;

const AMBER: &str = "#ffa21f";
const RED: &str = "#ff5a4a";
const GREEN: &str = "#7cf04a";
pub const CALL_ROWS: usize = 12;

fn panel(x: i32, y: i32, w: i32, h: i32, title: &str) -> Node {
    bx(x, y, w, h, "panel").child(ttl(title))
}

fn age(s: f64) -> String {
    let s = s.max(0.) as u64;
    format!("{:02}:{:02} AGO", s / 60, s % 60)
}

pub fn fault() -> Vec<Node> {
    vec![bx(200, 220, 400, 120, "inset").child(Node::el("div", "status").style("text-align:center;padding-top:30px;color:#ffa21f").text("CIDS FAULT"))]
}

pub fn calls(c: &Ctx) -> Vec<Node> {
    let (log, pending, emer) = match &c.view.body {
        PageBody::Calls { log, pending, emer } => (log.as_slice(), *pending, *emer),
        _ => (&[][..], 0, false),
    };
    let mut v = vec![panel(20, 44, 520, 474, "CALL LOG")];
    let mut st = Node::el("div", "status");
    if log.is_empty() {
        st = st.text("NO CALLS");
    }
    for l in log.iter().rev().take(CALL_ROWS) {
        let state = if l.answered { "ANSWERED" } else { "OPEN" };
        let colour = if l.text.starts_with("EMERGENCY") {
            RED
        } else if l.answered {
            ""
        } else {
            "#6fe3ff"
        };
        st = row(st, &l.text, &format!("{} {state}", age(l.age_s)), colour);
    }
    v.push(bx(30, 68, 500, 440, "inset").child(st));
    v.push(panel(556, 44, 224, 250, "CALL STATUS"));
    let mut st = Node::el("div", "status");
    st = row(st, "PENDING CALLS", &pending.to_string(), if pending > 0 { "#6fe3ff" } else { "" });
    st = row(st, "EMERGENCY CALL", if emer { "ACTIVE" } else { "NONE" }, if emer { RED } else { "" });
    v.push(bx(566, 68, 204, 140, "inset").child(st));
    v.push(key(588, 222, 160, 50, "callreset", "CALL RESET", "blue"));
    v
}

pub fn evac(c: &Ctx) -> Vec<Node> {
    let (active, cockpit, cabin, capt_only, guard, horn) = match &c.view.body {
        PageBody::Evac { active, cockpit_cmd, cabin_cmd, capt_only, guard_open, horn_cockpit } => {
            (*active, *cockpit_cmd, *cabin_cmd, *capt_only, *guard_open, *horn_cockpit)
        }
        _ => (false, false, false, false, false, false),
    };
    let on_off = |b: bool| if b { "ON" } else { "OFF" };
    let mut v = vec![panel(20, 44, 370, 330, "EVACUATION COMMAND")];
    let mut st = Node::el("div", "status");
    st = row(st, "EVACUATION", if active { "ALERT" } else { "NORMAL" }, if active { RED } else { "" });
    st = row(st, "COCKPIT COMMAND", on_off(cockpit), if cockpit { RED } else { "" });
    st = row(st, "CABIN COMMAND", on_off(cabin), if cabin { RED } else { "" });
    st = row(st, "COMMAND AUTHORITY", if capt_only { "CAPT" } else { "CAPT & PURS" }, "");
    st = row(st, "COCKPIT HORN", on_off(horn), if horn { AMBER } else { "" });
    v.push(bx(30, 68, 350, 296, "inset").child(st));
    v.push(panel(410, 44, 370, 330, "CABIN EVACUATION").child(
        Node::el("div", "note")
            .style("padding:4px 14px")
            .text("EVAC CMD needs a second press within the guard time. EVAC RESET clears a command started in the cabin only."),
    ));
    v.push(key(450, 160, 130, 64, "evac", "EVAC<br>CMD", if active { "red" } else { "" }).class_if("on", guard && !active));
    v.push(key(610, 160, 130, 64, "evacreset", "EVAC<br>RESET", if cabin && !cockpit { "" } else { "off" }));
    if guard {
        v.push(bx(430, 250, 330, 50, "inset").child(Node::el("div", "status w").text("PRESS EVAC CMD AGAIN TO CONFIRM")));
    }
    v
}

fn test_name(k: TestKind) -> &'static str {
    match k {
        TestKind::Lamp => "LAMP TEST",
        TestKind::Pa => "PA TEST",
        TestKind::Directors => "DIRECTOR TEST",
    }
}

pub fn maintenance(c: &Ctx) -> Vec<Node> {
    let (result, allowed, lamp) = match &c.view.body {
        PageBody::Maintenance { result, test_allowed, lamp_test } => (result.clone(), *test_allowed, *lamp_test),
        _ => (TestResult::None, false, false),
    };
    let running = match &result {
        TestResult::Running(k) => Some(*k),
        _ => None,
    };
    let mut v = vec![panel(20, 44, 470, 330, "SYSTEM TESTS")];
    for (i, (k, act)) in [(TestKind::Lamp, "test:lamp"), (TestKind::Pa, "test:pa"), (TestKind::Directors, "test:directors")].iter().enumerate() {
        let cls = if running == Some(*k) || (*k == TestKind::Lamp && lamp) {
            "on"
        } else if allowed {
            ""
        } else {
            "off"
        };
        v.push(key(40 + i as i32 * 148, 90, 132, 64, act, &test_name(*k).replacen(' ', "<br>", 1), cls));
    }
    let (test, res, colour) = match &result {
        TestResult::None => ("NONE".to_string(), "--".to_string(), ""),
        TestResult::Running(k) => (test_name(*k).into(), "IN PROGRESS".into(), "#6fe3ff"),
        TestResult::Pass(k) => (test_name(*k).into(), "PASSED".into(), GREEN),
        TestResult::Fail(k, why) => (test_name(*k).into(), format!("FAILED: {why}"), AMBER),
        TestResult::NotAllowed(k) => (test_name(*k).into(), "NOT ALLOWED".into(), AMBER),
    };
    let mut st = Node::el("div", "status");
    st = row(st, "TEST", &test, "");
    st = row(st, "RESULT", &res, colour);
    v.push(bx(20, 390, 470, 128, "inset").child(Node::el("div", "otitle").style("margin-top:6px").text("TEST RESULT")).child(st));
    v.push(panel(506, 44, 274, 230, "CONDITIONS").child(
        Node::el("div", "note").style("padding:4px 14px").text("Tests run on the ground with the parking brake set and all engines shut down."),
    ));
    let mut st = Node::el("div", "status");
    st = row(st, "TESTS", if allowed { "AVAILABLE" } else { "NOT AVAILABLE" }, if allowed { GREEN } else { AMBER });
    v.push(bx(516, 150, 254, 80, "inset").child(st));
    v
}
