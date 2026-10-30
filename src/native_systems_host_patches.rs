//! SystemsHost.js and ExtrasHost index.js changes for
//! `native_systems_host.rs`: each module stands down while its bit in
//! `L:A380X_NATIVE_SYSHOST_MASK` is set. Applies after `wxr::source_patches`.

use crate::source_patch::SourcePatch;

pub const MASK_VAR: &str = "L:A380X_NATIVE_SYSHOST_MASK";
pub const TAKE_SYSTEMS_CALL: &str = "A380X_NATIVE_SYSHOST_TAKE_SYSTEMS";
pub const TAKE_EXTRAS_CALL: &str = "A380X_NATIVE_SYSHOST_TAKE_EXTRAS";
pub const KEY_CALL: &str = "A380X_NATIVE_SYSHOST_KEY";
pub const XPDR_AUTO_VAR: &str = "L:A380X_XPDR_MFD_AUTO";
pub const XPDR_ALT_RPTG_VAR: &str = "L:A380X_XPDR_MFD_ALT_RPTG";
pub const XPDR_MIRROR_VAR: &str = "L:A380X_XPDR_MFD_MIRROR";

pub const BIT_VHF: u32 = 1;
pub const BIT_XPDR: u32 = 2;
pub const BIT_AMU: u32 = 4;
pub const BIT_BTV: u32 = 8;
pub const BIT_EFIS_TAWS: u32 = 16;
pub const BIT_FWS: u32 = 32;
pub const BIT_LIGHT_SYNC: u32 = 64;
pub const BIT_KEY_INTERCEPTOR: u32 = 128;
pub const BIT_PILOT_SEAT: u32 = 256;
pub const BIT_PUSHBUTTON_CHECK: u32 = 512;
pub const BIT_BARO_UNIT: u32 = 1024;
pub const BIT_READY: u32 = 2048;
pub const BIT_TERRAIN_FEED: u32 = 4096;
pub const BIT_OIT_HEALTH: u32 = 8192;

const SYSTEMS_HOST: &str = "/Pages/VCockpit/Instruments/A380X/SystemsHost/SystemsHost.js";
const EXTRAS_HOST: &str = "/Pages/VCockpit/Instruments/A380X/ExtrasHost/index.js";

fn native(bit: u32) -> String {
    format!("(Vars.read(\"{MASK_VAR}\", \"number\") & {bit}) !== 0")
}

fn stand_down(head: &str, bit: u32, indent: &str) -> String {
    let (first, rest) = head.split_once('\n').unwrap_or((head, ""));
    format!("{first}\n{indent}if ({}) {{\n{indent}  return;\n{indent}}}\n{rest}", native(bit))
}

const VHF_FIND: &str = "    tick() {\n      const failed = this.failuresConsumer.isActive(this.failureKey);\n      if (this.isPowered.get() && !failed) {\n        const useBusA";

const XPDR_FIND: &str = "    tick() {\n      this.isFailed.set(this.failuresConsumer.isActive(this.failureKey));\n      const isMsfsTransponderOn";

const SIM_AUDIO_FIND: &str = "    tick() {\n      this.activeAmuIndex = !this.amu1.isHealthy.get()";

const BTV_FIND: &str = "    tick() {\n      const deltaTime = this.updateThrottler.canUpdate(this.instrument.frameDelta);\n      if (deltaTime <= 0) {\n        return;\n      }\n      this.detectLandingRunway();";

const TAWS_START_FIND: &str = "      const extremeLatitude = validIrMaintWord ? validIrMaintWord.bitValueOr(15, false) : false;\n";

const TAWS_SYNC_FIND: &str = "      this.efisDataSyncCapt.tick();\n      this.efisDataSyncFO.tick();\n";

const TAWS_POST_FIND: &str = "      if (this.aircraftStatusShouldBeUpdated && this.simBridgeClient.isConnected()) {\n";

const TAWS_END_FIND: &str = "this.aesu2ResetPulled.get() || !this.ac4Powered.get()\n      );\n      this.terrFailed.set(";

const TAWS_FLAGS: [(&str, &str); 6] = [
    ("terr1Failed", "L:A32NX_TERR_1_FAILED"),
    ("terr2Failed", "L:A32NX_TERR_2_FAILED"),
    ("gpws1Failed", "L:A32NX_GPWS_1_FAILED"),
    ("gpws2Failed", "L:A32NX_GPWS_2_FAILED"),
    ("wxr1Failed", "L:A32NX_WXR_1_FAILED"),
    ("wxr2Failed", "L:A32NX_WXR_2_FAILED"),
];

const FWS_FIND: &str = "        (_c = this.fwsCore) == null ? void 0 : _c.update(dt);\n";

const LIGHT_INIT_FIND: &str = "    initLighting() {\n      console.log(\"[LightSync] initializing lighting to defaults\");";

const LIGHT_UPDATE_FIND: &str = "    updateLighting() {\n      if (Vars.read(\"L:A32NX_CABIN_USING_AUTOBRIGHTNESS\", \"bool\")) {";

const KEY_FIND: &str = "      const config4 = this.keys.get(data2.key);\n      if (!config4) {\n        return;\n      }\n";

const SEAT_FIND: &str = "    tick() {\n      const cameraPos = Vars.readHost(\"fbw/CAMERA_POS_IN_PLANE\", \"xyz\");";

const PUSHBUTTON_FIND: &str = "    onToConfigPushbutton(pressed) {\n      if (pressed && !this.toConfTimer.waiting()";

const BARO_FIND: &str = "      this.baroUnitSelector.performSelection();\n";

const READY_FIND: &str = "          Vars.write(\"L:A32NX_IS_READY\", \"number\", 1);\n";

const EXTRAS_UPDATE_FIND: &str = "      this.backplane.tick();\n    }\n  };\n";

fn stand_down_before(find: &str, bit: u32, indent: &str) -> String {
    format!("{indent}if ({}) {{\n{indent}  return;\n{indent}}}\n{find}", native(bit))
}

fn taws_sync_replace() -> String {
    let body: String = TAWS_SYNC_FIND.lines().map(|l| format!("  {l}\n")).collect();
    format!("      if (!({})) {{\n{body}      }}\n", native(BIT_TERRAIN_FEED))
}

fn taws_start_replace() -> String {
    let readback: String = TAWS_FLAGS
        .iter()
        .map(|(subject, var)| format!("        this.{subject}.set(!!Vars.read(\"{var}\", \"bool\"));\n"))
        .collect();
    format!("{TAWS_START_FIND}      if ({}) {{\n{readback}      }} else {{\n", native(BIT_EFIS_TAWS))
}

fn fws_replace() -> String {
    format!(
        r#"        const nativeSyshostMask = Vars.read("{MASK_VAR}", "number");
        (_c = this.fwsCore) == null || (nativeSyshostMask & {BIT_FWS}) !== 0 ? void 0 : _c.update(dt);
        if (nativeSyshostMask !== 0 && !this.nativeSyshostTakePending) {{
          this.nativeSyshostTakePending = true;
          Bridge.call("{TAKE_SYSTEMS_CALL}").then((r) => {{
            this.nativeSyshostTakePending = false;
            const taken = typeof r === "string" ? JSON.parse(r) : r;
            const publisher = this.bus.outbox();
            for (const [topic, data] of (taken && taken.bus) || []) {{
              publisher.post(topic, data, true);
            }}
          }}).catch(() => {{
            this.nativeSyshostTakePending = false;
          }});
        }}
"#
    )
}

fn xpdr_replace() -> String {
    let head = format!(
        "    tick() {{\n      Vars.write(\"{XPDR_AUTO_VAR}\", \"number\", this.isAuto.get() ? 1 : 0);\n      Vars.write(\"{XPDR_ALT_RPTG_VAR}\", \"number\", this.isAltReportingOn.get() ? 1 : 0);\n      Vars.write(\"{XPDR_MIRROR_VAR}\", \"number\", 1);\n"
    );
    let rest = XPDR_FIND.split_once('\n').map_or("", |(_, r)| r);
    format!("{head}      if ({}) {{\n        return;\n      }}\n{rest}", native(BIT_XPDR))
}

fn key_replace() -> String {
    let arg = |n: u8| format!("data2.value{n} === void 0 ? null : data2.value{n}");
    format!(
        "{KEY_FIND}      if ({} && data2.key !== \"ENGINE_AUTO_START\" && data2.key !== \"ENGINE_AUTO_SHUTDOWN\") {{\n        Bridge.call(\"{KEY_CALL}\", data2.key, {}, {}, {});\n        return;\n      }}\n",
        native(BIT_KEY_INTERCEPTOR),
        arg(0),
        arg(1),
        arg(2)
    )
}

fn extras_update_replace() -> String {
    format!(
        r#"      if (Vars.read("{MASK_VAR}", "number") !== 0 && !this.nativeSyshostTakePending) {{
        this.nativeSyshostTakePending = true;
        Bridge.call("{TAKE_EXTRAS_CALL}").then((r) => {{
          this.nativeSyshostTakePending = false;
          const taken = typeof r === "string" ? JSON.parse(r) : r;
          for (const n of (taken && taken.notifications) || []) {{
            this.notificationManager.showNotification({{ title: n.title, message: n.message, theme: "TIPS" }});
          }}
        }}).catch(() => {{
          this.nativeSyshostTakePending = false;
        }});
      }}
{EXTRAS_UPDATE_FIND}"#
    )
}

pub fn source_patches() -> Vec<SourcePatch> {
    let patch = |path: &str, find: &str, replace: String, reason: &str| SourcePatch {
        path: path.to_string(),
        find: find.to_string(),
        replace,
        reason: reason.to_string(),
    };
    vec![
        patch(SYSTEMS_HOST, VHF_FIND, stand_down(VHF_FIND, BIT_VHF, "      "), "VhfRadio stands down while native vhf is authoritative"),
        patch(SYSTEMS_HOST, XPDR_FIND, xpdr_replace(), "Transponder mirrors the MFD AUTO/ALT RPTG selections and stands down while native xpdr is authoritative"),
        patch(SYSTEMS_HOST, SIM_AUDIO_FIND, stand_down(SIM_AUDIO_FIND, BIT_AMU, "      "), "SimAudioManager stands down while native amu is authoritative"),
        patch(SYSTEMS_HOST, BTV_FIND, stand_down(BTV_FIND, BIT_BTV, "      "), "BrakeToVacate's periodic update stands down while native btv is authoritative"),
        patch(SYSTEMS_HOST, TAWS_START_FIND, taws_start_replace(), "EfisTawsBridge reads the TERR/GPWS/WXR failure flags back from native efis_taws while it is authoritative"),
        patch(SYSTEMS_HOST, TAWS_END_FIND, TAWS_END_FIND.replace("\n      this.terrFailed.set(", "\n      }\n      this.terrFailed.set("), "EfisTawsBridge reads the TERR/GPWS/WXR failure flags back from native efis_taws while it is authoritative"),
        patch(SYSTEMS_HOST, TAWS_SYNC_FIND, taws_sync_replace(), "EfisTawsBridge's EfisDataSync stands down while native terrain_feed is authoritative"),
        patch(SYSTEMS_HOST, TAWS_POST_FIND, stand_down_before(TAWS_POST_FIND, BIT_TERRAIN_FEED, "      "), "EfisTawsBridge stops posting aircraft status and the VD path to SimBridge while native terrain_feed is authoritative"),
        patch(SYSTEMS_HOST, FWS_FIND, fws_replace(), "FwsCore stands down while native fws is authoritative; native bus events are taken each frame"),
        patch(EXTRAS_HOST, LIGHT_INIT_FIND, stand_down(LIGHT_INIT_FIND, BIT_LIGHT_SYNC, "      "), "LightSync's initial lighting stands down while native light_sync is authoritative"),
        patch(EXTRAS_HOST, LIGHT_UPDATE_FIND, stand_down(LIGHT_UPDATE_FIND, BIT_LIGHT_SYNC, "      "), "LightSync's cabin lighting stands down while native light_sync is authoritative"),
        patch(EXTRAS_HOST, KEY_FIND, key_replace(), "intercepted keys go to native key_interceptor while it is authoritative (engine auto start/shutdown dialogs stay)"),
        patch(EXTRAS_HOST, SEAT_FIND, stand_down(SEAT_FIND, BIT_PILOT_SEAT, "      "), "PilotSeatManager stands down while native pilot_seat is authoritative"),
        patch(EXTRAS_HOST, PUSHBUTTON_FIND, stand_down(PUSHBUTTON_FIND, BIT_PUSHBUTTON_CHECK, "      "), "PushbuttonCheck stands down while native pushbutton_check is authoritative"),
        patch(EXTRAS_HOST, BARO_FIND, format!("      if (!({})) {{\n  {BARO_FIND}      }}\n", native(BIT_BARO_UNIT)), "the baro unit is chosen natively while native baro_unit is authoritative"),
        patch(EXTRAS_HOST, READY_FIND, format!("          if (!({})) {{\n  {READY_FIND}          }}\n", native(BIT_READY)), "L:A32NX_IS_READY is written natively while native ready is authoritative"),
        patch(EXTRAS_HOST, EXTRAS_UPDATE_FIND, extras_update_replace(), "native notifications are taken each frame while any native ExtrasHost module is authoritative"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAWS_EXCERPT: &str = "      this.efisDataSyncCapt.tick();\n      this.efisDataSyncFO.tick();\n      if (deltaTime < 0) {\n        return;\n      }\n      const extremeLatitude = validIrMaintWord ? validIrMaintWord.bitValueOr(15, false) : false;\n      this.terr1Failed.set(\n        x\n      );\n      this.gpws2Failed.set(\n        this.failuresConsumer.isActive(A380Failure.Gpws2) || this.aesu2ResetPulled.get() || !this.ac4Powered.get()\n      );\n      this.terrFailed.set(\n        x\n      );\n      if (this.aircraftStatusShouldBeUpdated && this.simBridgeClient.isConnected()) {\n        y;\n      }\n";

    fn excerpt(path: &str) -> String {
        let mut parts: Vec<String> =
            source_patches().into_iter().filter(|p| p.path == path && ![TAWS_START_FIND, TAWS_END_FIND, TAWS_SYNC_FIND, TAWS_POST_FIND].contains(&p.find.as_str())).map(|p| p.find).collect();
        if path == SYSTEMS_HOST {
            parts.push(TAWS_EXCERPT.to_string());
        }
        parts.join("\n...\n")
    }

    #[test]
    fn every_patch_applies_once_and_keeps_braces_balanced() {
        for path in [SYSTEMS_HOST, EXTRAS_HOST] {
            let text = excerpt(path);
            let mut applied = Vec::new();
            let out = crate::source_patch::apply(&source_patches(), path, text.clone(), |ok, _| applied.push(ok));
            assert!(applied.iter().all(|ok| *ok), "{path}: {applied:?}\n{out}");
            assert_eq!(applied.len(), source_patches().iter().filter(|p| p.path == path).count());
            assert_eq!(out.matches('{').count() - text.matches('{').count(), out.matches('}').count() - text.matches('}').count(), "{out}");
            assert_eq!(out.matches('(').count() - text.matches('(').count(), out.matches(')').count() - text.matches(')').count(), "{out}");
        }
    }

    #[test]
    fn every_bit_is_distinct_and_tested_by_its_patch() {
        let bits = [
            BIT_VHF,
            BIT_XPDR,
            BIT_AMU,
            BIT_BTV,
            BIT_EFIS_TAWS,
            BIT_FWS,
            BIT_LIGHT_SYNC,
            BIT_KEY_INTERCEPTOR,
            BIT_PILOT_SEAT,
            BIT_PUSHBUTTON_CHECK,
            BIT_BARO_UNIT,
            BIT_READY,
            BIT_TERRAIN_FEED,
        ];
        assert_eq!(bits.iter().fold(0, |a, b| a | b).count_ones() as usize, bits.len());
        let all: String = source_patches().iter().map(|p| p.replace.clone()).collect();
        for bit in bits {
            assert!(all.contains(&format!("& {bit})")), "bit {bit} gates nothing");
        }
    }

    #[test]
    fn the_patches_match_the_built_hosts_once_after_the_wxr_patch() {
        let Some(root) = std::env::var_os("FBW_HTML_UI") else { return };
        for path in [SYSTEMS_HOST, EXTRAS_HOST] {
            let Ok(text) = std::fs::read_to_string(std::path::Path::new(&root).join(path.trim_start_matches('/'))) else { return };
            let text = crate::source_patch::apply(&crate::wxr::source_patches(), path, text, |_, _| {});
            let mut failed = Vec::new();
            crate::source_patch::apply(&source_patches(), path, text, |ok, why| {
                if !ok {
                    failed.push(why.to_string());
                }
            });
            assert!(failed.is_empty(), "{failed:#?}");
        }
    }
}
