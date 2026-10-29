//! The aircraft preset procedures: FlyByWire's ProcedureStep.hpp and
//! PresetProcedures.hpp, read from their own XML
//! (config/a380x/a380-842/aircraft_preset_procedures.xml, the file
//! Gauge_Extra_Backend.cpp:39 names).
//!
//! FlyByWire re-reads the file on every request so it can be edited while the
//! simulator runs (PresetProcedures.hpp:71-95). The plugin carries the file
//! from the FlyByWire checkout it is built against, and reads a copy next to
//! the plugin instead when one is there (see `AircraftPresets`).

/// The file FlyByWire ships, as built into the plugin.
pub const PROCEDURES_XML: &str = include_str!(
    "../../../fbw-aircraft/fbw-a380x/src/base/flybywire-aircraft-a380-842/config/a380x/a380-842/aircraft_preset_procedures.xml"
);

/// StepType flags (ProcedureStep.hpp:31-48).
pub const ACTION: u32 = 0b00001;
pub const CONDITION: u32 = 0b00010;
pub const NORMAL_MODE: u32 = 0b00100;
pub const EXPEDITED_MODE: u32 = 0b01000;
pub const EXPEDITED_DELAY: u32 = 0b10000;
pub const STEP: u32 = ACTION | NORMAL_MODE | EXPEDITED_MODE;
pub const PROC: u32 = ACTION | NORMAL_MODE;
pub const NOEX: u32 = STEP | EXPEDITED_DELAY;
pub const EXON: u32 = ACTION | EXPEDITED_MODE;
pub const COND: u32 = CONDITION | NORMAL_MODE | EXPEDITED_MODE;
pub const NCON: u32 = CONDITION | NORMAL_MODE;
pub const ECON: u32 = CONDITION | EXPEDITED_MODE;

/// ProcedureStep::StepTypeMap (ProcedureStep.hpp:114-122).
pub fn step_type(name: &str) -> Option<u32> {
    Some(match name {
        "STEP" => STEP,
        "PROC" => PROC,
        "NOEX" => NOEX,
        "EXON" => EXON,
        "COND" => COND,
        "NCON" => NCON,
        "ECON" => ECON,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcedureStep {
    pub description: String,
    pub step_type: u32,
    /// Milliseconds.
    pub delay_after: f64,
    pub expected_state_check_code: String,
    pub action_code: String,
}

/// The procedure names, in PresetProcedures::initializeProcedureListMap's
/// order (PresetProcedures.hpp:181-192).
const PROCEDURES: [&str; 11] = [
    "POWERUP_CONFIG_ON",
    "POWERUP_CONFIG_OFF",
    "PUSHBACK_CONFIG_ON",
    "PUSHBACK_CONFIG_OFF",
    "TAXI_CONFIG_ON",
    "TAXI_CONFIG_OFF",
    "TAKEOFF_CONFIG_ON",
    "TAKEOFF_CONFIG_OFF",
    "FO_AFTER_LANDING",
    "FO_AFTER_START",
    "FO_TAXI",
];

/// Which procedures each preset runs (PresetProcedures.hpp:198-213): 1 cold
/// and dark, 2 powered, 3 ready for pushback, 4 ready for taxi, 5 ready for
/// takeoff.
const PRESETS: [[&str; 4]; 5] = [
    ["TAKEOFF_CONFIG_OFF", "TAXI_CONFIG_OFF", "PUSHBACK_CONFIG_OFF", "POWERUP_CONFIG_OFF"],
    ["TAKEOFF_CONFIG_OFF", "TAXI_CONFIG_OFF", "PUSHBACK_CONFIG_OFF", "POWERUP_CONFIG_ON"],
    ["TAKEOFF_CONFIG_OFF", "TAXI_CONFIG_OFF", "POWERUP_CONFIG_ON", "PUSHBACK_CONFIG_ON"],
    ["TAKEOFF_CONFIG_OFF", "POWERUP_CONFIG_ON", "PUSHBACK_CONFIG_ON", "TAXI_CONFIG_ON"],
    ["POWERUP_CONFIG_ON", "PUSHBACK_CONFIG_ON", "TAXI_CONFIG_ON", "TAKEOFF_CONFIG_ON"],
];

/// The first officer's share of the A380 AFTER LANDING procedure, preset 6
/// (FCOM PRO-NOR-SOP AFTER LANDING: flaps, APU, exterior lights are FO
/// items; ground spoilers DISARM is the captain's). Every step is one of
/// FlyByWire's own from aircraft_preset_procedures.xml; the APU master
/// condition is the switch alone, since FlyByWire's also skips it with
/// engines 1 and 2 running.
pub const AFTER_LANDING_PRESET: i64 = 6;
pub const AFTER_LANDING_XML: &str = r#"<Procedures>
    <Procedure Name="FO_AFTER_LANDING">
        <Step Name="Landing Lights Off" Type="STEP" Delay="500">
            <Condition>(A:LIGHT LANDING:2, Number) 0 ==</Condition>
            <Action>2 (&gt;K:LANDING_LIGHTS_OFF)</Action>
        </Step>
        <Step Name="NOSE Lt Takeoff Off" Type="STEP" Delay="500">
            <Condition>(A:LIGHT LANDING:1, Number) 0 ==</Condition>
            <Action>1 (&gt;K:LANDING_LIGHTS_OFF)</Action>
        </Step>
        <Step Name="NOSE Lt Taxi" Type="STEP" Delay="1000">
            <Condition>(A:LIGHT TAXI, Number) 1 ==</Condition>
            <Action>1 (&gt;K:TAXI_LIGHTS_ON)</Action>
        </Step>
        <Step Name="Strobe Auto" Type="STEP" Delay="1000">
            <Condition>(L:LIGHTING_STROBE_0) 1 ==</Condition>
            <Action>1 (&gt;L:LIGHTING_STROBE_0)</Action>
        </Step>
        <Step Name="Flaps Retract" Type="STEP" Delay="3000">
            <Condition>(L:A32NX_FLAPS_HANDLE_INDEX) 0 ==</Condition>
            <Action>0 (&gt;L:A32NX_FLAPS_HANDLE_INDEX)</Action>
        </Step>
        <Step Name="APU Master On" Type="STEP" Delay="3000">
            <Condition>(L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON) 1 ==</Condition>
            <Action>1 (&gt;L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON)</Action>
        </Step>
        <Step Name="APU Start On" Type="STEP" Delay="1000">
            <Condition>(L:A32NX_OVHD_APU_MASTER_SW_PB_IS_ON) ! (L:A32NX_OVHD_APU_START_PB_IS_AVAILABLE) ||</Condition>
            <Action>1 (&gt;L:A32NX_OVHD_APU_START_PB_IS_ON)</Action>
        </Step>
    </Procedure>
</Procedures>"#;

/// The first officer's share of the A380 AFTER START procedure, preset 7
/// (FCOM PRO-NOR-SOP AFTER START: ground spoilers, rudder trim and flaps are
/// FO items; ENG START selector, APU bleed, anti-ice and APU master are the
/// captain's). FlyByWire's own TAXI_CONFIG_ON steps; the flaps go to the
/// take-off configuration the EFB computed, and stay with the crew when
/// none was.
pub const AFTER_START_PRESET: i64 = 7;
pub const AFTER_START_XML: &str = r#"<Procedures>
    <Procedure Name="FO_AFTER_START">
        <Step Name="Spoiler Arm" Type="STEP" Delay="2000">
            <Condition>(L:A32NX_SPOILERS_ARMED) 1 ==</Condition>
            <Action>1 (&gt;K:SPOILERS_ARM_SET)</Action>
        </Step>
        <Step Name="Rudder Trim Reset" Type="STEP" Delay="2000">
            <Condition>(A:RUDDER TRIM, Radians) 0 ==</Condition>
            <Action>0 (&gt;K:RUDDER_TRIM_SET)</Action>
        </Step>
        <Step Name="Flaps Set" Type="STEP" Delay="3000">
            <Condition>(L:A32NX_FLAPS_HANDLE_INDEX) TOFLAPS ==</Condition>
            <Action>TOFLAPS (&gt;L:A32NX_FLAPS_HANDLE_INDEX)</Action>
        </Step>
    </Procedure>
</Procedures>"#;

/// The first officer's share of the A380 TAXI procedure, preset 8 (FCOM
/// PRO-NOR-SOP TAXI: RTO pb ARM and TO CONFIG pb TEST are FO items), from
/// FlyByWire's own TAXI_CONFIG_ON steps.
pub const TAXI_FLOW_PRESET: i64 = 8;
pub const TAXI_FLOW_XML: &str = r#"<Procedures>
    <Procedure Name="FO_TAXI">
        <Step Name="Autobrake RTO" Type="STEP" Delay="1000">
            <Condition>(L:A32NX_OVHD_AUTOBRK_RTO_ARM_IS_PRESSED) 0 ==</Condition>
            <Action>1 (&gt;L:A32NX_OVHD_AUTOBRK_RTO_ARM_IS_PRESSED)</Action>
        </Step>
        <Step Name="T.O. Config Push" Type="STEP" Delay="200">
            <Action>1 (&gt;L:A32NX_BTN_TOCONFIG)</Action>
        </Step>
        <Step Name="T.O. Config Release" Type="STEP" Delay="2000">
            <Action>0 (&gt;L:A32NX_BTN_TOCONFIG)</Action>
        </Step>
    </Procedure>
</Procedures>"#;
pub const LAST_PRESET: i64 = TAXI_FLOW_PRESET;

/// The after-start flow with the take-off flap lever position in place, or
/// without its flaps step when no take-off configuration is known.
pub fn after_start_xml(takeoff_flaps: Option<u8>) -> String {
    match takeoff_flaps {
        Some(f) => AFTER_START_XML.replace("TOFLAPS", &f.to_string()),
        None => {
            let start = AFTER_START_XML.find("        <Step Name=\"Flaps Set\"").unwrap_or(AFTER_START_XML.len());
            let end = AFTER_START_XML[start..].find("</Step>\n").map_or(start, |e| start + e + "</Step>\n".len());
            format!("{}{}", &AFTER_START_XML[..start], &AFTER_START_XML[end..])
        }
    }
}

/// PresetProcedures::getProcedure: the steps of preset `id` (1-5, then 6 and
/// 7 for the first officer's flows) from the XML text, or why not.
pub fn preset(xml: &str, id: i64) -> Result<Vec<ProcedureStep>, String> {
    let flow = |text: &str, name: &str| -> Result<Vec<ProcedureStep>, String> {
        Ok(parse(text)?.into_iter().find(|(n, _)| n == name).map(|(_, s)| s).unwrap_or_default())
    };
    if id == AFTER_LANDING_PRESET {
        return flow(AFTER_LANDING_XML, "FO_AFTER_LANDING");
    }
    if id == AFTER_START_PRESET {
        return flow(&after_start_xml(crate::copilot::takeoff_flaps()), "FO_AFTER_START");
    }
    if id == TAXI_FLOW_PRESET {
        return flow(TAXI_FLOW_XML, "FO_TAXI");
    }
    if !(1..=5).contains(&id) {
        return Err(format!("AircraftPresets: The procedure ID {id} is not valid. Valid IDs are 1-{LAST_PRESET}."));
    }
    let procedures = parse(xml)?;
    let mut steps = Vec::new();
    for name in PRESETS[(id - 1) as usize] {
        if let Some((_, list)) = procedures.iter().find(|(n, _)| n == name) {
            steps.extend(list.iter().cloned());
        }
    }
    Ok(steps)
}

/// PresetProcedures::processProcedures: every known procedure's valid steps.
pub fn parse(xml: &str) -> Result<Vec<(String, Vec<ProcedureStep>)>, String> {
    let root = xml::parse(xml)?;
    let mut out: Vec<(String, Vec<ProcedureStep>)> = PROCEDURES.iter().map(|n| (n.to_string(), Vec::new())).collect();
    for procedure in &root.children {
        let name = procedure.attr("Name").unwrap_or_default();
        // "The procedure {} is not valid. Skipping the whole procedure."
        let Some((_, steps)) = out.iter_mut().find(|(n, _)| *n == name) else { continue };
        for step in &procedure.children {
            // Invalid step type or a negative delay skip the step.
            let Some(step_type) = step.attr("Type").and_then(|t| step_type(&t)) else { continue };
            // tinyxml2's IntAttribute: 0 when missing or not a number.
            let delay = step.attr("Delay").and_then(|d| d.trim().parse::<i64>().ok()).unwrap_or(0);
            if delay < 0 {
                continue;
            }
            let text = |tag: &str| step.children.iter().find(|c| c.name == tag).map(|c| c.text.clone()).unwrap_or_default();
            steps.push(ProcedureStep {
                description: step.attr("Name").unwrap_or_default(),
                step_type,
                delay_after: delay as f64,
                expected_state_check_code: text("Condition"),
                action_code: text("Action"),
            });
        }
    }
    Ok(out)
}

/// Just enough XML for this file: elements, attributes, text, comments and
/// the five predefined entities. Also reads the package's sound.xml
/// (sound/triggers.rs).
pub(crate) mod xml {
    #[derive(Debug, Default)]
    pub struct Element {
        pub name: String,
        pub attrs: Vec<(String, String)>,
        pub children: Vec<Element>,
        pub text: String,
    }

    impl Element {
        pub fn attr(&self, name: &str) -> Option<String> {
            self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
        }
    }

    pub fn unescape(s: &str) -> String {
        s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
    }

    /// The root element.
    pub fn parse(text: &str) -> Result<Element, String> {
        let mut stack: Vec<Element> = vec![Element::default()];
        let mut rest = text;
        while let Some(lt) = rest.find('<') {
            let before = &rest[..lt];
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&unescape(before));
            }
            rest = &rest[lt..];
            if let Some(after) = rest.strip_prefix("<!--") {
                let end = after.find("-->").ok_or("unterminated comment")?;
                rest = &after[end + 3..];
            } else if rest.starts_with("<?") {
                let end = rest.find("?>").ok_or("unterminated declaration")?;
                rest = &rest[end + 2..];
            } else if let Some(after) = rest.strip_prefix("</") {
                let end = after.find('>').ok_or("unterminated end tag")?;
                let name = after[..end].trim();
                rest = &after[end + 1..];
                let done = stack.pop().ok_or("unbalanced end tag")?;
                if done.name != name {
                    return Err(format!("</{name}> closes <{}>", done.name));
                }
                stack.last_mut().ok_or("unbalanced end tag")?.children.push(done);
            } else {
                let end = tag_end(rest).ok_or("unterminated tag")?;
                let inner = &rest[1..end];
                rest = &rest[end + 1..];
                let (inner, empty) = match inner.strip_suffix('/') {
                    Some(i) => (i, true),
                    None => (inner, false),
                };
                let element = start_tag(inner)?;
                if empty {
                    stack.last_mut().ok_or("no parent")?.children.push(element);
                } else {
                    stack.push(element);
                }
            }
        }
        if stack.len() != 1 {
            return Err("unclosed elements".into());
        }
        let mut document = stack.pop().unwrap_or_default();
        document.children.pop().ok_or_else(|| "no root element".to_string())
    }

    /// The `>` ending a start tag, outside quoted attribute values.
    fn tag_end(s: &str) -> Option<usize> {
        let mut quote = None;
        for (i, c) in s.char_indices() {
            match (quote, c) {
                (None, '"' | '\'') => quote = Some(c),
                (Some(q), c) if c == q => quote = None,
                (None, '>') => return Some(i),
                _ => {}
            }
        }
        None
    }

    fn start_tag(inner: &str) -> Result<Element, String> {
        let inner = inner.trim();
        let name_end = inner.find(char::is_whitespace).unwrap_or(inner.len());
        let mut element = Element { name: inner[..name_end].to_string(), ..Default::default() };
        let mut rest = inner[name_end..].trim_start();
        while !rest.is_empty() {
            let eq = rest.find('=').ok_or_else(|| format!("attribute without value in <{}>", element.name))?;
            let key = rest[..eq].trim().to_string();
            let after = rest[eq + 1..].trim_start();
            let quote = after.chars().next().ok_or("missing attribute value")?;
            let close = after[1..].find(quote).ok_or("unterminated attribute value")?;
            element.attrs.push((key, unescape(&after[1..1 + close])));
            rest = after[close + 2..].trim_start();
        }
        Ok(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_after_landing_flow_only_works_switches_flybywires_own_presets_work() {
        let steps = preset(PROCEDURES_XML, AFTER_LANDING_PRESET).unwrap();
        assert_eq!(steps.len(), 7);
        assert!(steps.iter().all(|s| !s.action_code.contains("SPOILERS_ARM_SET")), "the captain disarms the spoilers");
        assert_eq!(steps[4].expected_state_check_code, "(L:A32NX_FLAPS_HANDLE_INDEX) 0 ==");
        for step in &steps {
            assert_eq!(step.step_type, STEP);
            let target = step.action_code.split("(>").nth(1).and_then(|t| t.split(')').next()).unwrap();
            assert!(PROCEDURES_XML.contains(&format!("(&gt;{target})")), "{target} is not a switch FlyByWire's presets use");
        }
    }

    #[test]
    fn the_after_start_flow_is_the_first_officers_share_with_the_planned_flaps() {
        let with = parse(&after_start_xml(Some(2))).unwrap().into_iter().find(|(n, _)| n == "FO_AFTER_START").unwrap().1;
        assert_eq!(with.len(), 3);
        assert_eq!(with[2].action_code, "2 (>L:A32NX_FLAPS_HANDLE_INDEX)");
        assert_eq!(with[2].expected_state_check_code, "(L:A32NX_FLAPS_HANDLE_INDEX) 2 ==");
        let without = parse(&after_start_xml(None)).unwrap().into_iter().find(|(n, _)| n == "FO_AFTER_START").unwrap().1;
        assert_eq!(without.len(), 2, "no take-off configuration known: the flaps stay with the crew");
        assert!(with.iter().all(|s| !s.action_code.contains("APU")), "APU bleed and master are the captain's");
        let taxi_flow = preset(PROCEDURES_XML, TAXI_FLOW_PRESET).unwrap();
        assert_eq!(taxi_flow.len(), 3);
        let taxi = parse(PROCEDURES_XML).unwrap().into_iter().find(|(n, _)| n == "TAXI_CONFIG_ON").unwrap().1;
        for step in without.iter().chain(&taxi_flow) {
            assert!(
                taxi.iter().any(|t| t.expected_state_check_code == step.expected_state_check_code && t.action_code == step.action_code),
                "{} is not a TAXI_CONFIG_ON step",
                step.description
            );
        }
    }

    #[test]
    fn flybywire_procedures_parse_into_their_presets() {
        let procedures = parse(PROCEDURES_XML).unwrap();
        for (name, steps) in procedures.iter().filter(|(n, _)| !n.starts_with("FO_")) {
            assert!(!steps.is_empty(), "{name} has no steps");
        }
        let power_on = &procedures.iter().find(|(n, _)| n == "POWERUP_CONFIG_ON").unwrap().1;
        // aircraft_preset_procedures.xml:50-53.
        assert_eq!(power_on[0].description, "BAT1 On");
        assert_eq!(power_on[0].step_type, STEP);
        assert_eq!(power_on[0].delay_after, 1000.);
        assert_eq!(power_on[0].expected_state_check_code, "(L:A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO)");
        assert_eq!(power_on[0].action_code, "1 (>L:A32NX_OVHD_ELEC_BAT_1_PB_IS_AUTO)");
        // Ready for takeoff is the four ON procedures in order.
        let takeoff = preset(PROCEDURES_XML, 5).unwrap();
        let count = |n: &str| procedures.iter().find(|(p, _)| p == n).unwrap().1.len();
        assert_eq!(
            takeoff.len(),
            count("POWERUP_CONFIG_ON") + count("PUSHBACK_CONFIG_ON") + count("TAXI_CONFIG_ON") + count("TAKEOFF_CONFIG_ON")
        );
        assert!(preset(PROCEDURES_XML, LAST_PRESET + 1).is_err());
        // Every step type in the file is one FlyByWire knows.
        let types: Vec<u32> = procedures.iter().flat_map(|(_, s)| s.iter().map(|s| s.step_type)).collect();
        assert!(types.contains(&COND) && types.contains(&NCON) && types.contains(&PROC));
    }
}
