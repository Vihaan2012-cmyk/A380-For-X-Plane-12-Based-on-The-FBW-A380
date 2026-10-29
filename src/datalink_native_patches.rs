//! SystemsHost.js changes for `datalink_native.rs`: the datalink inputs are
//! forwarded to the native DatalinkSystem, and Acr, AtcDatalink, AtsuSystem,
//! SimVarHandling and the ACARS activation stand down while bit 1 of
//! `L:A380X_NATIVE_DATALINK_MASK` is set. Applies after the systems host patches.

use crate::source_patch::SourcePatch;

pub const MASK_VAR: &str = "L:A380X_NATIVE_DATALINK_MASK";
pub const BIT_DATALINK: u32 = 1;
pub const INPUT_CALL: &str = "A380X_NATIVE_DATALINK_INPUT";
pub const TAKE_CALL: &str = "A380X_NATIVE_DATALINK_TAKE";

/// Topics the FMS, MFD and SD publish to the datalink (`DatalinkSystem::bus_input`).
pub const INPUT_TOPICS: &[&str] = &[
    "atcRouteData",
    "atcLogon",
    "atcLogoff",
    "atcRequestAtis",
    "atcActivateAtisAutoUpdate",
    "atcDeactivateAtisAutoUpdate",
    "atcTogglePrintAtisReportsPrint",
    "atcSetMaxUplinkDelay",
    "atcToggleAutomaticPositionReport",
    "atcRequestPositionReport",
    "atcRegisterCpdlcMessages",
    "atcRegisterDclMessages",
    "atcRegisterOclMessages",
    "atcUpdateMessage",
    "atcMessageRead",
    "atcRemoveMessage",
    "atcCleanupMessages",
    "atcResetAtisAutoUpdate",
    "uplinkResponse",
    "downlinkTransmit",
    "modifyMessage",
    "printMessage",
    "closeMessage",
    "updateMessageMonitoring",
    "stopMessageMonitoring",
    "recallMessage",
    "readMessage",
    "invertSemanticResponse",
    "visibleMessage",
    "aocSendFreetextMessage",
    "aocRequestAtis",
    "aocRequestWeather",
    "aocRegisterWeatherMessages",
    "aocRegisterFreetextMessages",
    "aocMessageRead",
    "aocRemoveMessage",
    "aocRequestWinds",
    "wind_uplink_request",
    "routerConnect",
    "routerDisconnect",
    "routerRequestStationAvailable",
];

const SYSTEMS_HOST: &str = "/Pages/VCockpit/Instruments/A380X/SystemsHost/SystemsHost.js";

const INIT_FIND: &str = "      this.acr.init();\n      this.atc.init();\n      this.atsu.init();\n      this.simVarHandling.initialize();\n";

const ROUTER_UPDATE_FIND: &str = "    update(currentTimestamp) {\n      this.digitalInputs.setVhf3Datamode(this.vhfRadios.isDataModeActive());\n";

const ACTIVATE_FIND: &str = "    static activateAcars() {\n      _AcarsConnector.stopActivation();\n";

const ATTEMPT_FIND: &str = "    static async attemptActivation() {\n      _AcarsConnector.connectionAttemptCounter += 1;\n";

const SIMVAR_UPDATE_FIND: &str = "        this.powerPublisher.tick();\n        this.simVarHandling.tick();\n";

const GAME_STATE_FIND: &str = "          this.simVarHandling.begin();\n        }\n        this.gameState = gamestate;\n      }\n";

fn native() -> String {
    format!("(Vars.read(\"{MASK_VAR}\", \"number\") & {BIT_DATALINK}) !== 0")
}

fn init_replace() -> String {
    let topics = INPUT_TOPICS.iter().map(|t| format!("\"{t}\"")).collect::<Vec<_>>().join(", ");
    let init: String = INIT_FIND.lines().map(|l| format!("  {l}\n")).collect();
    format!(
        r#"      for (const topic of [{topics}]) {{
        this.bus.inbox().topic(topic).listen((data) => {{
          Bridge.call("{INPUT_CALL}", topic, JSON.stringify(data === void 0 ? null : data)).catch(() => {{}});
        }});
      }}
      if ({native}) {{
        this.nativeDatalink = true;
      }} else {{
{init}      }}
"#,
        native = native()
    )
}

fn stand_down_after_head(find: &str, body: &str) -> String {
    let (first, rest) = find.split_once('\n').unwrap_or((find, ""));
    format!("{first}\n      if ({}) {{\n{body}        return;\n      }}\n{rest}", native())
}

fn simvar_update_replace() -> String {
    format!("        this.powerPublisher.tick();\n        if (!({})) {{\n          this.simVarHandling.tick();\n        }}\n", native())
}

fn game_state_replace() -> String {
    format!(
        r#"          if (!({native})) {{
            this.simVarHandling.begin();
          }}
        }}
        this.gameState = gamestate;
      }}
      if ({native} && !this.nativeDatalinkTakePending) {{
        this.nativeDatalinkTakePending = true;
        Bridge.call("{TAKE_CALL}").then((r) => {{
          this.nativeDatalinkTakePending = false;
          const taken = typeof r === "string" ? JSON.parse(r) : r;
          const publisher = this.bus.outbox();
          for (const [topic, data] of (taken && taken.bus) || []) {{
            publisher.post(topic, data, true);
          }}
        }}).catch(() => {{
          this.nativeDatalinkTakePending = false;
        }});
      }}
"#,
        native = native()
    )
}

pub fn source_patches() -> Vec<SourcePatch> {
    let patch = |find: &str, replace: String, reason: &str| SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: find.to_string(),
        replace,
        reason: reason.to_string(),
    };
    let stop = "        _AcarsConnector.stopActivation();\n";
    vec![
        patch(INIT_FIND, init_replace(), "datalink inputs go to the native DatalinkSystem; Acr/AtcDatalink/AtsuSystem/SimVarHandling stay uninitialised while native datalink is authoritative"),
        patch(ROUTER_UPDATE_FIND, stand_down_after_head(ROUTER_UPDATE_FIND, ""), "the JS datalink Router stands down while native datalink is authoritative"),
        patch(ACTIVATE_FIND, stand_down_after_head(ACTIVATE_FIND, stop), "ACARS activation (and its L:A32NX_ACARS_ACTIVE write) stands down while native datalink is authoritative"),
        patch(ATTEMPT_FIND, stand_down_after_head(ATTEMPT_FIND, stop), "no JS ACARS ping while native datalink is authoritative"),
        patch(SIMVAR_UPDATE_FIND, simvar_update_replace(), "SimVarHandling stands down while native datalink is authoritative"),
        patch(GAME_STATE_FIND, game_state_replace(), "native datalink bus events are taken each frame while native datalink is authoritative"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_patch_applies_once_and_keeps_brackets_balanced() {
        let text = source_patches().iter().map(|p| p.find.clone()).collect::<Vec<_>>().join("\n...\n");
        let mut applied = Vec::new();
        let out = crate::source_patch::apply(&source_patches(), SYSTEMS_HOST, text.clone(), |ok, _| applied.push(ok));
        assert_eq!(applied, vec![true; source_patches().len()], "{out}");
        for (open, close) in [('{', '}'), ('(', ')'), ('[', ']')] {
            assert_eq!(out.matches(open).count() - text.matches(open).count(), out.matches(close).count() - text.matches(close).count(), "{open}{close}\n{out}");
        }
        assert_eq!(out.matches(INPUT_CALL).count(), 1);
        assert_eq!(out.matches(TAKE_CALL).count(), 1);
        assert_eq!(out.matches(&format!("& {BIT_DATALINK}) !== 0")).count(), 7);
    }

    #[test]
    fn inputs_are_not_datalink_outputs() {
        for group in [datalink::tables::ATC_TO_FMS, datalink::tables::AOC_TO_FMS, datalink::tables::ROUTER_TO_FMS] {
            for t in group {
                assert!(!INPUT_TOPICS.contains(t), "{t} is an output");
            }
        }
        for t in datalink::tables::FMS_TO_ATC.iter().chain(datalink::tables::FMS_TO_AOC).chain(datalink::tables::FMS_TO_ROUTER) {
            assert!(INPUT_TOPICS.contains(t) || *t == "routerManagementResponse", "{t} is not forwarded");
        }
        assert!(!INPUT_TOPICS.contains(&"routerManagementResponse"));
    }

    #[test]
    fn the_patches_match_the_built_host_once_after_the_other_patches() {
        let Some(root) = std::env::var_os("FBW_HTML_UI") else { return };
        let Ok(text) = std::fs::read_to_string(std::path::Path::new(&root).join(SYSTEMS_HOST.trim_start_matches('/'))) else { return };
        let text = crate::source_patch::apply(&crate::wxr::source_patches(), SYSTEMS_HOST, text, |_, _| {});
        let text = crate::source_patch::apply(&crate::native_systems_host_patches::source_patches(), SYSTEMS_HOST, text, |_, _| {});
        let mut failed = Vec::new();
        crate::source_patch::apply(&source_patches(), SYSTEMS_HOST, text, |ok, why| {
            if !ok {
                failed.push(why.to_string());
            }
        });
        assert!(failed.is_empty(), "{failed:#?}");
    }
}
