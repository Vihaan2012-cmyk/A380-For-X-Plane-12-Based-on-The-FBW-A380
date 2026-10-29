//! SystemsHost.js changes for `native_shadow.rs`: each module FlyByWire's
//! SystemsHost runs (LegacyGpws, LegacyTcasComputer, the FWS health block)
//! stands down while its bit in `L:A380X_NATIVE_SYSTEMS_MASK` is set, and
//! the native computers' bus events and ND traffic come back in through
//! `Coherent.call("A380X_NATIVE_SYSTEMS_TAKE")`. Must apply after
//! `surveillance.rs`'s patch, whose anchor ends where `TCAS_MODE_FIND` starts.

use crate::source_patch::SourcePatch;

pub const MASK_VAR: &str = "L:A380X_NATIVE_SYSTEMS_MASK";
pub const TAKE_CALL: &str = "A380X_NATIVE_SYSTEMS_TAKE";
pub const TCAS_ALERT_LEVEL_VAR: &str = "L:A380X_TCAS_MFD_ALERT_LEVEL";
pub const TCAS_ALT_SELECT_VAR: &str = "L:A380X_TCAS_MFD_ALT_SELECT";
pub const TCAS_MIRROR_VAR: &str = "L:A380X_TCAS_MFD_MIRROR";
pub const FWS_STARTUP_VAR: &str = "L:A380X_FWS_CORE_STARTUP_COMPLETED";

const SYSTEMS_HOST: &str = "/Pages/VCockpit/Instruments/A380X/SystemsHost/SystemsHost.js";

const GPWS_FIND: &str = "        (_b = this.gpws) == null ? void 0 : _b.update(dt);\n";

const GPWS_REPLACE: &str = r#"        const nativeMask = Vars.read("L:A380X_NATIVE_SYSTEMS_MASK", "number");
        (_b = this.gpws) == null || (nativeMask & 1) !== 0 ? void 0 : _b.update(dt);
        if ((nativeMask & 3) !== 0 && !this.nativeTakePending) {
          this.nativeTakePending = true;
          Bridge.call("A380X_NATIVE_SYSTEMS_TAKE").then((r) => {
            this.nativeTakePending = false;
            const taken = typeof r === "string" ? JSON.parse(r) : r;
            if (!taken) {
              return;
            }
            const publisher = this.bus.outbox();
            for (const [topic, data] of taken.bus || []) {
              publisher.post(topic, data);
            }
            if (taken.traffic && this.nativeTcasComputer) {
              this.nativeTcasComputer.syncer.sendEvent("A32NX_TCAS_L_TRAFFIC", taken.traffic.l);
              this.nativeTcasComputer.syncer.sendEvent("A32NX_TCAS_R_TRAFFIC", taken.traffic.r);
            }
          }).catch(() => {
            this.nativeTakePending = false;
          });
        }
"#;

const TCAS_ADD_FIND: &str = "this.backplane.attachDevice(\"TcasComputer\", new LegacyTcasComputer(this.bus, this.soundManager));";
const TCAS_ADD_REPLACE: &str =
    "this.backplane.attachDevice(\"TcasComputer\", this.nativeTcasComputer = new LegacyTcasComputer(this.bus, this.soundManager));";

const TCAS_UPDATE_FIND: &str = "    update(_deltaTime) {\n      this.updateVars();\n      this.updateInhibitions();\n";

const TCAS_UPDATE_REPLACE: &str = r#"    update(_deltaTime) {
      this.nativeOwned = (Vars.read("L:A380X_NATIVE_SYSTEMS_MASK", "number") & 2) !== 0;
      this.updateVars();
      Vars.write("L:A380X_TCAS_MFD_ALERT_LEVEL", "number", this.tcasAlertLevel.get());
      Vars.write("L:A380X_TCAS_MFD_ALT_SELECT", "number", this.tcasAltSelect.get());
      Vars.write("L:A380X_TCAS_MFD_MIRROR", "number", 1);
      if (this.nativeOwned) {
        return;
      }
      this.updateInhibitions();
"#;

const TCAS_MODE_FIND: &str = "      this.tcasMode.setVar(\n        this.xpdrStatus === 1 /* STBY */";
const TCAS_MODE_REPLACE: &str = "      this.nativeOwned || this.tcasMode.setVar(\n        this.xpdrStatus === 1 /* STBY */";

const FWS_START_FIND: &str = "      this.fws1Healthy.set(\n        (_b = !this.fws1Failed.get()";

const FWS_START_REPLACE: &str = r#"      Vars.write("L:A380X_FWS_CORE_STARTUP_COMPLETED", "bool", this.fwsCore !== void 0 && this.fwsCore.startupCompleted.get() ? 1 : 0);
      if ((Vars.read("L:A380X_NATIVE_SYSTEMS_MASK", "number") & 4) !== 0) {
        this.fws1Healthy.set(!!Vars.read("L:A32NX_FWS1_IS_HEALTHY", "bool"));
        this.fws2Healthy.set(!!Vars.read("L:A32NX_FWS2_IS_HEALTHY", "bool"));
        this.fwsEcpFailed.set(!!Vars.read("L:A32NX_FWS_ECP_FAILED", "bool"));
      } else {
      this.fws1Healthy.set(
        (_b = !this.fws1Failed.get()"#;

const FWS_END_FIND: &str = "|| !this.dcEssBusPowered.get() || ecpNotReachable\n      );\n";
const FWS_END_REPLACE: &str = "|| !this.dcEssBusPowered.get() || ecpNotReachable\n      );\n      }\n";

pub fn source_patches() -> Vec<SourcePatch> {
    let patch = |find: &str, replace: &str, reason: &str| SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: find.to_string(),
        replace: replace.to_string(),
        reason: reason.to_string(),
    };
    vec![
        patch(GPWS_FIND, GPWS_REPLACE, "LegacyGpws stands down while native GPWS is authoritative; native GPWS/TCAS events are taken each frame"),
        patch(TCAS_ADD_FIND, TCAS_ADD_REPLACE, "the TCAS computer is kept reachable for native ND traffic"),
        patch(TCAS_UPDATE_FIND, TCAS_UPDATE_REPLACE, "LegacyTcasComputer mirrors the MFD SURV selections and stands down while native TCAS is authoritative"),
        patch(TCAS_MODE_FIND, TCAS_MODE_REPLACE, "LegacyTcasComputer leaves L:A32NX_TCAS_MODE to native TCAS while it is authoritative"),
        patch(FWS_START_FIND, FWS_START_REPLACE, "FWS health is read back from native hosts_misc while it is authoritative"),
        patch(FWS_END_FIND, FWS_END_REPLACE, "FWS health is read back from native hosts_misc while it is authoritative"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_patch_applies_once_to_a_systems_host_excerpt() {
        let excerpt = [GPWS_FIND, TCAS_ADD_FIND, TCAS_UPDATE_FIND, TCAS_MODE_FIND, FWS_START_FIND, FWS_END_FIND].join("\n...\n");
        let mut applied = Vec::new();
        let out = crate::source_patch::apply(&source_patches(), SYSTEMS_HOST, excerpt.to_string(), |ok, _| applied.push(ok));
        assert_eq!(applied, vec![true; 6], "{out}");
        assert!(out.contains("& 1) !== 0 ? void 0 : _b.update(dt)"));
        assert!(out.contains("this.nativeTcasComputer = new LegacyTcasComputer"));
        assert!(out.contains("this.nativeOwned || this.tcasMode.setVar("));
        assert_eq!(out.matches('{').count() - excerpt.matches('{').count(), out.matches('}').count() - excerpt.matches('}').count());
    }
}
