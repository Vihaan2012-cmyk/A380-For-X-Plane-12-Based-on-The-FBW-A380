//! SourcePatches for ECAM/FWS and instrument gaps (top50 #9, 16 partial, 18-27,
//! 36-40; docs/analysis/ecam-instruments-coupling.md, systems.md) that are not
//! already covered by another module's own `source_patches()` (oans, wxr).
//!
//! Each patch below is a small, self-contained change to FlyByWire's own
//! development-build JS (esbuild output, comments mostly preserved; a
//! production/minified build does not match and the runtime says so, same
//! caveat as `js_bridge.rs`'s `native_ports`). The `find` text is exact,
//! copied from a fresh FlyByWire build
//! (`D:\fbw-aircraft\fbw-a380x\out\flybywire-aircraft-a380-842\html_ui`), the
//! same tree `boots_fbw_cockpit_views` loads.
//!
//! Real data sources only (docs/analysis/*.md and cited FBW source
//! file:line); where no real source exists (ECAM-001, ECAM-002, ECAM-007,
//! ECAM-008, INST-010's WBBC fallback, ECAM-017's CIDS chimes), nothing is
//! patched here and the report says why.

use crate::source_patch::SourcePatch;

/// The Electronic Checklist's own patches: the ECAM control panel's inputs and
/// the normal-checklist sensing (`docs/ecl.md`).
mod ecl;

const SYSTEMS_HOST: &str = "/Pages/VCockpit/Instruments/A380X/SystemsHost/SystemsHost.js";
const MFD: &str = "/Pages/VCockpit/Instruments/A380X/MFD/mfd.js";
const PFD: &str = "/Pages/VCockpit/Instruments/A380X/PFD/pfd.js";
const EWD: &str = "/Pages/VCockpit/Instruments/A380X/EWD/ewd.js";
const ND: &str = "/Pages/VCockpit/Instruments/A380X/ND/nd.js";

pub fn source_patches() -> Vec<SourcePatch> {
    let mut patches = vec![
        inst_005_takeoff_speeds_check(),
        ecam_010_reverser_inop(),
        ecam_010_gen_inop(),
        ecam_010_bleed_inop(),
        ecam_009_max_reverse_suppressed_on_reverser_inop(),
        ecam_003_pack_fault_door_and_ground(),
        ecam_011_012_elec_galley_pax_sys_off(),
        ecam_015_rudder_fault(),
        arpt_nav_needs_an_aligned_ir(),
        no_fws_fault_while_it_boots(),
        log_presented_alerts(),
        door_main_1l_latch_sensor(),
        ecam_014_engines_off_and_on_ground_uses_core_speed(),
        inst_009_pfd_spoiler_indication_max_of_all_panels(),
        inst_010_ewd_egt_not_clamped_to_850(),
        inst_010_ewd_egt_digits_not_clamped_to_850(),
        ewd_letter_width(),
        ewd_title_box_around_its_text(),
        ewd_underline_ends_with_its_text(),
        ewd_long_title_condensed(),
        ewd_title_line_width(),
        ewd_long_line_kept_on_one_row(),
        ewd_long_line_text_whole(),
        ewd_generated_line_text_whole(),
        ewd_fallback_line_kept_on_one_row(),
        oans_fms_button_label("origin", "ORIGIN"),
        oans_fms_button_label("destination", "DEST"),
        oans_fms_button_label("alternate", "ALTN"),
    ];
    patches.extend(ecl::source_patches());
    patches
}

/// EWD text advances 16.797 px a letter: the font's 15.047 at 24 px plus the
/// stylesheet's 1.75 px letter-spacing.
fn ewd_letter_width() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "const LETTER_WIDTH = 16;".to_string(),
        replace: r#"const LETTER_WIDTH = Math.min(16.797, 690 / Math.max(1, ...message.replace(/\x1B[^m]*m/g, "").split("\r").map((l) => l.length)));"#.to_string(),
        reason: "FormattedFwcText places boxes and underlines at 16 px a letter, so boxed titles \
                 such as ENG TWO ENG OUT ON SAME SIDE ran through the right side of their box"
            .to_string(),
    }
}

fn ewd_title_box_around_its_text() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "`M ${this.props.x + startCol * LETTER_WIDTH}\n                                ${this.props.y + yOffset - 22} h ${(col - startCol) * LETTER_WIDTH + 12} v 27 h ${-((col - startCol) * LETTER_WIDTH + 12)} v -27`".to_string(),
        replace: "`M ${this.props.x + startCol * LETTER_WIDTH - 4} ${this.props.y + yOffset - 22} h ${(col - startCol) * LETTER_WIDTH + 6} v 27 h ${-((col - startCol) * LETTER_WIDTH + 6)} v -27`".to_string(),
        reason: "with the true letter width the box needs the same margin either side of its text".to_string(),
    }
}

fn ewd_underline_ends_with_its_text() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "h ${(col - startCol) * LETTER_WIDTH + 5}`".to_string(),
        replace: "h ${(col - startCol) * LETTER_WIDTH + 2}`".to_string(),
        reason: "keeps underlines the length they were on short words with the true letter width".to_string(),
    }
}

/// A title too long for the line: letters closer together, so it still fits.
fn ewd_long_title_condensed() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "            UI.mount(s, e);\n          });\n          this.linesRef.target.appendChild(e);\n          yOffset += LINE_SPACING;\n        }\n      }, true);".to_string(),
        replace: "            UI.mount(s, e);\n          });\n          if (LETTER_WIDTH < 16.797) e.querySelectorAll(\"tspan\").forEach((t) => { t.style.letterSpacing = (LETTER_WIDTH - 15.047) + \"px\"; });\n          this.linesRef.target.appendChild(e);\n          yOffset += LINE_SPACING;\n        }\n      }, true);".to_string(),
        reason: "20 of FlyByWire's titles are longer than the line and ran off its right side".to_string(),
    }
}

fn ewd_title_line_width() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "width: \"680\",\n            height: \"33\",\n            style: {\n              display: this.props.data.derive(".to_string(),
        replace: "width: \"705\",\n            height: \"33\",\n            style: {\n              display: this.props.data.derive(".to_string(),
        reason: "a title's line was 680 px wide in a row of 706".to_string(),
    }
}

/// A procedure line longer than the row: smaller, on its one row.
fn ewd_long_line_kept_on_one_row() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "class: \"EclLineText\",\n            style: {\n              display:".to_string(),
        replace: "class: \"EclLineText\",\n            style: {\n              \"white-space\": \"nowrap\",\n              \"font-size\": this.props.data.derive((d) => d.text.length > 40 ? (26 * 40 / d.text.length).toFixed(1) + \"px\" : \"26px\"),\n              display:".to_string(),
        reason: "lines longer than the row were cut off, some losing the end of their instruction".to_string(),
    }
}

fn ewd_long_line_text_whole() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "this.props.data.derive((d) => d.text.substring(0, 39))".to_string(),
        replace: "this.props.data.derive((d) => d.text)".to_string(),
        reason: "the whole line, now that a long one shrinks to fit".to_string(),
    }
}

fn ewd_generated_line_text_whole() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "text: text.substring(0, WD_LINE_CHARACTERS - 1),".to_string(),
        replace: "text,".to_string(),
        reason: "the whole line, now that a long one shrinks to fit".to_string(),
    }
}

/// With no flight plan the store holds an empty string, not null, so FlyByWire's
/// `??` left the OANS airport buttons blank.
fn oans_fms_button_label(field: &str, label: &str) -> SourcePatch {
    SourcePatch {
        path: ND.to_string(),
        find: format!("this.fmsDataStore.{field}.derive((it) => it != null ? it : \"{label}\")"),
        replace: format!("this.fmsDataStore.{field}.derive((it) => it ? it : \"{label}\")"),
        reason: format!("the OANS {label} button was blank with no flight plan"),
    }
}

fn ewd_fallback_line_kept_on_one_row() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "UI.node(\"span\", { class: \"EclLineText smaller\" }, this.props.text)".to_string(),
        replace: "UI.node(\"span\", { class: \"EclLineText smaller\", style: \"white-space:nowrap;letter-spacing:-0.6px\" }, this.props.text)".to_string(),
        reason: "the FWS fault checklist's long lines wrapped onto the line below".to_string(),
    }
}

/// #9 / INST-005 (FmcAircraftInterface.ts:635): the FMS's own V-speeds-too-low
/// check (`getToSpeedsTooLow`, fully implemented at ts:569-598: V1 < Vmcg, VR
/// < 1.05*Vmca or V2 < 1.1*Vmca) is computed but never used; `toSpeedsChecks`
/// hard-codes the result `false` instead of calling it.
fn inst_005_takeoff_speeds_check() -> SourcePatch {
    SourcePatch {
        path: MFD.to_string(),
        find: "      const toSpeedsTooLow = false;".to_string(),
        replace: "      const toSpeedsTooLow = this.getToSpeedsTooLow();".to_string(),
        reason: "FmcAircraftInterface.ts:635 disables the takeoff speeds check \
                  (`// FIXME revert once speeds are checked`) even though \
                  getToSpeedsTooLow() (ts:569-598) is fully implemented"
            .to_string(),
    }
}

/// #20 / ECAM-010 (FwsCore.ts:2286,2288, "TODO add power loss conditions,
/// hydraulic loss"): reverser 2/3 are hydraulically stowed and their EBHA
/// electrical locks stay engaged without power (A380ReverserAssembly,
/// fbw-common/.../engine/reverser.rs:17-120), fed from AC2/AC4
/// (A380Reversers::REVERSER_2/3_..._SUPPLY_POWER_BUS,
/// a380_systems/reverser/mod.rs:206-215; FBW's own "TODO use correct
/// electrical scheme" there is an upstream approximation this port doesn't
/// improve on, but the bus assignment is real, working data). FwsCore.ts
/// already tracks those buses as ac2BusPowered/ac4BusPowered.
fn ecam_010_reverser_inop() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.reverser2Inop = this.eng2Out;\n      \
               // TODO add power loss conditions, hydraulic loss\n      \
               this.reverser3Inop = this.eng3Out;\n      \
               // TODO add power loss conditions, hydraulic loss"
            .to_string(),
        replace: "      this.reverser2Inop = Derived.from(\n        \
                  ([engOut, powered]) => engOut || !powered,\n        \
                  this.eng2Out,\n        \
                  this.ac2BusPowered\n      \
                  );\n      \
                  this.reverser3Inop = Derived.from(\n        \
                  ([engOut, powered]) => engOut || !powered,\n        \
                  this.eng3Out,\n        \
                  this.ac4BusPowered\n      \
                  );"
            .to_string(),
        reason: "reverser2Inop/3Inop were engine-out only; add the reverser's own \
                  AC2/AC4 EBHA supply power loss (a380_systems/reverser/mod.rs)"
            .to_string(),
    }
}

/// #20 / ECAM-010 (FwsCore.ts:2261, "TODO add generator contactor open, idg
/// disconnect & fault conditions"): each ENG GEN pushbutton's own FAULT light
/// already reflects `gen_contactor_open(n) && gen.is_on()` (a380_systems
/// electrical/mod.rs:381-383, `OnOffFaultPushButton`, published as
/// `OVHD_ELEC_ENG_GEN_n_PB_HAS_FAULT`), and each IDG's disconnect state is
/// `!gen_drive_connected(n)` (mod.rs:385-387, `OVHD_ELEC_IDG_n_PB_IS_DISC`).
/// Both are real signals the generator/IDG pushbuttons already publish; they
/// were simply never read here.
fn ecam_010_gen_inop() -> SourcePatch {
    let gen = |n: u32| {
        format!(
            "      this.gen{n}Inop = Derived.from(\n        \
             ([phase121112, eng{n}Running]) => !phase121112 && !eng{n}Running || \
             Vars.read(\"L:A32NX_OVHD_ELEC_ENG_GEN_{n}_PB_HAS_FAULT\", \"bool\") === 1 || \
             Vars.read(\"L:A32NX_OVHD_ELEC_IDG_{n}_PB_IS_DISC\", \"bool\") === 1,\n        \
             this.flightPhase12Or1112,\n        \
             this.engine{n}Running\n      \
             );"
        )
    };
    let gen_old = |n: u32| {
        format!(
            "      this.gen{n}Inop = Derived.from(\n        \
             ([phase121112, eng{n}Running]) => !phase121112 && !eng{n}Running,\n        \
             this.flightPhase12Or1112,\n        \
             this.engine{n}Running\n      \
             );"
        )
    };
    let find = (1..=4).map(gen_old).collect::<Vec<_>>().join("\n");
    let replace = (1..=4).map(gen).collect::<Vec<_>>().join("\n");
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find,
        replace,
        reason: "gen1-4Inop were phase/engine-out only; add each generator \
                  pushbutton's own contactor-open fault and its IDG's \
                  disconnect state (a380_systems electrical/mod.rs:381-387), \
                  both already published, never read here. The mapper only \
                  re-runs when flightPhase12Or1112/engineNRunning change, so \
                  a fault appearing without those changing is not picked up \
                  until the next one does; a fully reactive version needs new \
                  tracked Subjects updated every tick, left for a larger change"
            .to_string(),
    }
}

/// #20 / ECAM-010 (FwsCore.ts:2289-2296, "TODO add bleed inop conditions"):
/// each engine bleed pushbutton's own FAULT light
/// (`OVHD_PNEU_ENG_n_BLEED_PB_HAS_FAULT`, a380_systems/pneumatic.rs
/// `OnOffFaultPushButton::new_on(context, "PNEU_ENG_n_BLEED")`) is a real,
/// already-published signal distinct from "engine out".
fn ecam_010_bleed_inop() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.eng1BleedInop = this.eng1Out;\n      \
               // TODO add bleed inop conditions\n      \
               this.eng2BleedInop = this.eng2Out;\n      \
               // TODO add bleed inop conditions\n      \
               this.eng3BleedInop = this.eng3Out;\n      \
               // TODO add bleed inop conditions\n      \
               this.eng4BleedInop = this.gen4Inop;\n      \
               // TODO add bleed inop conditions"
            .to_string(),
        replace: "      this.eng1BleedInop = Derived.from(\n        \
                  ([engOut]) => engOut || Vars.read(\"L:A32NX_OVHD_PNEU_ENG_1_BLEED_PB_HAS_FAULT\", \"bool\") === 1,\n        \
                  this.eng1Out\n      );\n      \
                  this.eng2BleedInop = Derived.from(\n        \
                  ([engOut]) => engOut || Vars.read(\"L:A32NX_OVHD_PNEU_ENG_2_BLEED_PB_HAS_FAULT\", \"bool\") === 1,\n        \
                  this.eng2Out\n      );\n      \
                  this.eng3BleedInop = Derived.from(\n        \
                  ([engOut]) => engOut || Vars.read(\"L:A32NX_OVHD_PNEU_ENG_3_BLEED_PB_HAS_FAULT\", \"bool\") === 1,\n        \
                  this.eng3Out\n      );\n      \
                  this.eng4BleedInop = Derived.from(\n        \
                  ([g4]) => g4 || Vars.read(\"L:A32NX_OVHD_PNEU_ENG_4_BLEED_PB_HAS_FAULT\", \"bool\") === 1,\n        \
                  this.gen4Inop\n      );"
            .to_string(),
        reason: "eng1-4BleedInop had no bleed-specific condition at all (eng4 even \
                  reused gen4Inop, kept as-is here, not this patch's call to fix); \
                  add each engine bleed pushbutton's own FAULT signal \
                  (a380_systems/pneumatic.rs)"
            .to_string(),
    }
}

/// #25 / ECAM-009 (FwsAutoCallouts.ts:56,59, "FIXME: Check reverser INOP"):
/// SET/KEEP MAX REVERSE should not call out max reverse on a reverser that is
/// INOP; uses the FwsCore reverser2Inop/3Inop this file's own ECAM-010 patch
/// makes real.
fn ecam_009_max_reverse_suppressed_on_reverser_inop() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      const maxReverseRequested = this.rowRopStatusWord.bitValueOr(12, false);\n      \
               this.setMaxReverse.set(maxReverseRequested && rolloutOrBouncedLanding && !brakeMaxBraking);\n      \
               const keepMaxReverse = this.rowRopStatusWord.bitValueOr(13, false) && !brakeMaxBraking && rolloutOrBouncedLanding;"
            .to_string(),
        replace: "      const reverserInop = this.fws.reverser2Inop.get() || this.fws.reverser3Inop.get();\n      \
                  const maxReverseRequested = this.rowRopStatusWord.bitValueOr(12, false);\n      \
                  this.setMaxReverse.set(maxReverseRequested && rolloutOrBouncedLanding && !brakeMaxBraking && !reverserInop);\n      \
                  const keepMaxReverse = this.rowRopStatusWord.bitValueOr(13, false) && !brakeMaxBraking && rolloutOrBouncedLanding && !reverserInop;"
            .to_string(),
        reason: "MAX REVERSE callouts ignored reverser INOP; suppress them when either \
                  reverser (reverser2Inop/3Inop) is inoperative"
            .to_string(),
    }
}

/// #23 / ECAM-003 (FwsCore.ts:4151-4152, "TODO: Add fault when on ground,
/// with one engine running and one door open" / "TODO: Add pack overheat"):
/// the ground/engine/door condition uses FwsCore's own already-computed
/// aircraftOnGround, oneEngineRunning and cabinDoorOpen (ts:552,1876,
/// 4302-4311 — real cabin door state, not invented). Pack overheat has no
/// sensor anywhere in FBW's air conditioning model (no OVHT LVar for packs,
/// unlike the hydraulic reservoirs' OVHT LVars) and is left out.
fn ecam_003_pack_fault_door_and_ground() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.pack1And2Fault.set(\n        \
               (this.fdac1Channel1Failure.get() && this.fdac1Channel2Failure.get() && this.fdac2Channel1Failure.get() && this.fdac2Channel2Failure.get() || !this.pack1On.get() && !this.pack2On.get()) && this.phase8ConfirmationNode180.read()\n      \
               );"
            .to_string(),
        replace: "      this.pack1And2Fault.set(\n        \
                  ((this.fdac1Channel1Failure.get() && this.fdac1Channel2Failure.get() && this.fdac2Channel1Failure.get() && this.fdac2Channel2Failure.get() || !this.pack1On.get() && !this.pack2On.get()) && this.phase8ConfirmationNode180.read()) || \
                  (this.aircraftOnGround.get() && this.oneEngineRunning.get() && this.cabinDoorOpen.get())\n      \
                  );"
            .to_string(),
        reason: "PACK 1+2 FAULT never sensed ground+engine-running+door-open; add it \
                  from FwsCore's own real signals. Pack overheat has no sensor in \
                  FBW's model at all (checked air_conditioning/hydraulic OVHT LVars: \
                  only the hydraulic reservoirs have one) and is left out"
            .to_string(),
    }
}

/// #36 / ECAM-011/012 (FwsCore.ts:2991, "FIXME elecGalleyOff and
/// elecPaxSysOff currently use same simvar as buttons are linked"):
/// `ELEC_GALLEY_IS_SHED` (a380_systems electrical/mod.rs:227-228,
/// `main_galley.is_shed() || secondary_galley.is_shed()`) is the real
/// contactor/load-shed state — it already differs from the pushbutton
/// position under APU-gen-only/emer-gen-only ground ops
/// (electrical/mod.rs:2887-2924's own tests). FBW's A380 galley and cabin
/// (pax) systems share one pushbutton and one contactor in this model, so
/// both readouts share this same real signal (not the raw PB position
/// either used before).
fn ecam_011_012_elec_galley_pax_sys_off() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.elecGalleyOff.set(!Vars.read(\"L:A32NX_OVHD_ELEC_GALY_AND_CAB_PB_IS_AUTO\", \"bool\"));\n      \
               this.elecPaxSysOff.set(!Vars.read(\"L:A32NX_OVHD_ELEC_GALY_AND_CAB_PB_IS_AUTO\", \"bool\"));"
            .to_string(),
        replace: "      const elecGalleyShed = Vars.read(\"L:A32NX_ELEC_GALLEY_IS_SHED\", \"bool\") === 1;\n      \
                  this.elecGalleyOff.set(elecGalleyShed);\n      \
                  this.elecPaxSysOff.set(elecGalleyShed);"
            .to_string(),
        reason: "both readouts read the pushbutton position; read the real \
                  contactor/load-shed state (ELEC_GALLEY_IS_SHED) instead, which \
                  is independently computed and differs from the pushbutton under \
                  load-shedding"
            .to_string(),
    }
}

/// #37 / ECAM-015 rudder fault portion (FwsCore.ts:4774-4776, the FIXME and
/// two commented-out `this.lowerRudderFault.set()`/`this.upperRudderFault.set()`
/// lines that never ran at all): each rudder actuator's own hydraulic/electric
/// active-mode solenoids (prim.rs:1600-1610, `A32NX_UPPER/LOWER_RUDDER_*_EBHA_*_MODE_SOLENOID_ENERGIZED`,
/// ported from FlyByWireInterface.cpp:2833-2853) are real per-actuator control
/// state: a rudder section is faulted when none of its sources are active in
/// either mode. IR3-selection-from-CDS and SFCC1/2 switching (the rest of
/// ECAM-015) have no separate real source anywhere in FBW's model (attKnob
/// already is the real switching-knob input the CDS would relay; SFCC_2's own
/// flap/slat word is never published at all) and are left alone.
fn door_main_1l_latch_sensor() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "    this.main1LOpen.set(this.interactivePointZeroOpen.get());".to_string(),
        replace: "    this.main1LOpen.set(this.interactivePointZeroOpen.get() || Vars.read(\"L:A32NX_CABIN_DOOR_LATCHED:1\", \"number\") === 0);".to_string(),
        reason: "DOOR MAIN 1L NOT CLOSED read the door's true travel only; the real DSMS reads the door's not-latched sensor, which deep::cabin models with its stuck-sensor failure".to_string(),
    }
}

fn no_fws_fault_while_it_boots() -> SourcePatch {
    SourcePatch {
        path: "/Pages/VCockpit/Instruments/A380X/EWD/ewd.js".to_string(),
        find: "if (!(this.onGround.get() && this.groundspeed.get() >= 50)) {".to_string(),
        replace: "if (!(this.onGround.get() && this.groundspeed.get() >= 50) && Vars.read(\"L:A380X_FWS_CORE_STARTUP_COMPLETED\", \"bool\")) {".to_string(),
        reason: "FWS 1+2 FAULT filled the EWD for the minute the FWS takes to start on a cold aircraft; that is not a fault".to_string(),
    }
}

fn arpt_nav_needs_an_aligned_ir() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.oansPposLost.set(this.oansPposLostSimVar.get());".to_string(),
        replace: "      this.oansPposLost.set(this.oansPposLostSimVar.get() && [1, 2, 3].some((n) => Vars.read(`L:A32NX_ADIRS_ADIRU_${n}_STATE`, \"number\") === 2));".to_string(),
        reason: "ARPT NAV FAULT came up on every cold start: the OANS has no position until an IR has aligned, and that is not a fault".to_string(),
    }
}

fn log_presented_alerts() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.presentedFailures.push(...failureKeys);".to_string(),
        replace: "      this.presentedFailures.push(...failureKeys);\n      \
                  { const keys = failureKeys.join(\",\"); if (keys !== this.loggedFailureKeys) { this.loggedFailureKeys = keys; \
                  if (globalThis.__xphfbw && globalThis.__xphfbw.log) globalThis.__xphfbw.log(0, \"ecam shows: \" + \
                  (failureKeys.map((k) => k + \" \" + ((EcamAbnormalProcedures[k] && EcamAbnormalProcedures[k].title) || \"\").replace(/\\x1b<?\\d*m/g, \"\")).join(\"; \") || \"nothing\")); } }".to_string(),
        reason: "the log says which alerts the crew saw and when, for every flight".to_string(),
    }
}

fn ecam_015_rudder_fault() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      this.rudderTrimNotToWarning.set(rudderTrimConfigTestInPhase129 || this.rudderTrimConfigInPhase3or4or5Sr.read());\n      \
               this.flapsLeverNotZero.set("
            .to_string(),
        replace: "      this.rudderTrimNotToWarning.set(rudderTrimConfigTestInPhase129 || this.rudderTrimConfigInPhase3or4or5Sr.read());\n      \
                  const upperRudderActive = Vars.read(\"L:A32NX_UPPER_RUDDER_YELLOW_EBHA_HYDRAULIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_UPPER_RUDDER_YELLOW_EBHA_ELECTRIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_UPPER_RUDDER_GREEN_EBHA_HYDRAULIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_UPPER_RUDDER_GREEN_EBHA_ELECTRIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1;\n      \
                  const rudderHydAvail = !this.aircraftOnGround.get() || \
                  (Vars.read(\"L:A32NX_HYD_GREEN_SYSTEM_1_SECTION_PRESSURE_SWITCH\", \"bool\") === 1 && \
                  Vars.read(\"L:A32NX_HYD_YELLOW_SYSTEM_1_SECTION_PRESSURE_SWITCH\", \"bool\") === 1);\n      \
                  this.upperRudderFault.set(rudderHydAvail && !upperRudderActive);\n      \
                  const lowerRudderActive = Vars.read(\"L:A32NX_LOWER_RUDDER_GREEN_EBHA_HYDRAULIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_LOWER_RUDDER_GREEN_EBHA_ELECTRIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_LOWER_RUDDER_YELLOW_EBHA_HYDRAULIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1 || \
                  Vars.read(\"L:A32NX_LOWER_RUDDER_YELLOW_EBHA_ELECTRIC_MODE_SOLENOID_ENERGIZED\", \"bool\") === 1;\n      \
                  this.lowerRudderFault.set(rudderHydAvail && !lowerRudderActive);\n      \
                  this.flapsLeverNotZero.set("
            .to_string(),
        reason: "lowerRudderFault/upperRudderFault were dead code (`// this.lowerRudderFault.set();`, \
                  never called at all); wire them from each actuator's real \
                  hydraulic/electric active-mode solenoids (prim.rs)"
            .to_string(),
    }
}

/// #19 / ECAM-014 partial (FwsCore.ts:4267, "FIXME eng running should use
/// core speed at above min idle"): `engineNCoreAtOrAboveMinIdle` (ts:1931-1936)
/// is already computed from real N2/core-speed data (HPNEngN); swap the
/// TLA/N1-threshold-based engineNRunning for it here, as FBW's own comment
/// asks. The other ECAM-014 sub-items (engine starting-state discrete from
/// the FADEC, ts:5266-5268) need a new discrete fadec.rs does not expose yet
/// — described in the report for workstream A rather than invented here.
fn ecam_014_engines_off_and_on_ground_uses_core_speed() -> SourcePatch {
    SourcePatch {
        path: SYSTEMS_HOST.to_string(),
        find: "      const engNotRunning = !this.engine1Running.get() && !this.engine2Running.get() && !this.engine3Running.get() && !this.engine4Running.get();\n      \
               this.enginesOffAndOnGroundSignal.write(this.aircraftOnGround.get() && engNotRunning, deltaTime);"
            .to_string(),
        replace: "      const engNotRunning = !this.engine1CoreAtOrAboveMinIdle.get() && !this.engine2CoreAtOrAboveMinIdle.get() && !this.engine3CoreAtOrAboveMinIdle.get() && !this.engine4CoreAtOrAboveMinIdle.get();\n      \
                  this.enginesOffAndOnGroundSignal.write(this.aircraftOnGround.get() && engNotRunning, deltaTime);"
            .to_string(),
        reason: "enginesOffAndOnGroundSignal used the TLA-based engineNRunning flags; \
                  use the already-real core-speed-based engineNCoreAtOrAboveMinIdle \
                  instead, per FBW's own FIXME"
            .to_string(),
    }
}

/// #27 / INST-009 (PFD/LowerArea.tsx:59, "FIXME don't use commanded position
/// from just one spoiler + figure out whether it's averaged, or max-ed"): all
/// 8 real per-panel commanded-position LVars per side exist
/// (a380_systems/hydraulic/mod.rs:6796-6807,
/// `LEFT/RIGHT_SPOILER_n_COMMANDED_POSITION`, n=1..8) but only panel 1 was
/// read. FBW's own comment leaves averaged-vs-maxed undecided upstream too;
/// this uses the greatest (most extended) commanded position across all 8
/// panels per side, the conservative choice for a position indicator. The
/// ground-spoiler LGCIS indication (LowerArea.tsx:540, GearIndicator) already
/// reads the real LGCIU discrete word (lgciuDiscreteWord1 bits 23-25) despite
/// its own stale "once LGCIS is implemented" comment — no change needed
/// there.
fn inst_009_pfd_spoiler_indication_max_of_all_panels() -> SourcePatch {
    SourcePatch {
        path: PFD.to_string(),
        find: "      this.spoilersCommandedPosition = BusSignal.from(\n        \
               this.sub.on(\"spoilersCommanded\").distinct(),\n        0\n      \
               );"
            .to_string(),
        replace: "      this.spoilersCommandedPosition = BusSignal.from(\n        \
                  this.sub.on(\"spoilersCommanded\").distinct(),\n        0\n      \
                  ).derive((p) => {\n        \
                  let m = p;\n        \
                  for (const side of [\"LEFT\", \"RIGHT\"]) {\n          \
                  for (let i = 1; i <= 8; i++) {\n            \
                  const v = Vars.read(`L:A32NX_${side}_SPOILER_${i}_COMMANDED_POSITION`, \"number\");\n            \
                  if (v > m) m = v;\n          \
                  }\n        \
                  }\n        \
                  return m;\n      \
                  });"
            .to_string(),
        reason: "the PFD spoiler tape showed only panel 1's commanded position; show \
                  the greatest deflection across all 8 real spoiler panels per side \
                  instead"
            .to_string(),
    }
}

/// The EWD's EGT never reads above 850 C, whatever the engine is doing.
///
/// `EGT.tsx` caps the displayed value:
///
/// ```js
/// Math.min([3, 4].includes(throttleMode) ? 900 : 850, egt)
/// ```
///
/// `throttleMode` there is `throttle_position`, which `EwdSimvarPublisher`
/// binds to `L:A32NX_AUTOTHRUST_TLA:n` -- the thrust lever *angle in
/// degrees* (idle 0, CLB 25, FLX/MCT 35, TOGA 45, `throttle.rs`).
/// `[3, 4].includes(...)` is asking whether that angle is exactly 3 or 4
/// degrees: a thrust-limit-mode test applied to a number of degrees. It is
/// never true in normal operation, so the 850 branch always wins and the
/// gauge pins there at every power setting. The same file reads the same
/// variable as a percentage two lines earlier (`tm < 33`), which is the
/// other half of the same mix-up.
///
/// The cap is dropped rather than corrected, for two reasons. This port
/// already applies the EEC's own TGT trim upstream (`engine_commands.rs`:
/// `egt_displayed = phys.egt_c - tgt_trim_c(...)`, EASA.E.012 Note 16), so
/// capping here trims a trimmed value twice. And a cap at 900 would still
/// be wrong: the certified over-temperature limit is **920 C trimmed** for
/// 20 s (Note 14), so the cockpit has to be able to show a number above
/// 900 -- a gauge that cannot is hiding the exceedance the crew is meant to
/// act on.
///
/// Colour is untouched: `warningEGTColor` still turns the reading red at
/// 900 and amber above 850 below the take-off detent.
fn inst_010_ewd_egt_not_clamped_to_850() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "Math.min([3, 4].includes(throttleMode) ? 900 : 850, egt),".to_string(),
        replace: "egt,".to_string(),
        reason: "EWD EGT is capped at 850 C because [3,4].includes() tests a thrust-limit mode \
                 against A32NX_AUTOTHRUST_TLA, which is the lever angle in degrees; the cap is \
                 removed rather than fixed because engine_commands.rs already applies the EEC's \
                 TGT trim, and the 920 C over-temperature limit has to be displayable"
            .to_string(),
    }
}

/// The same cap on the digital readout beside the gauge -- the same
/// expression, applied to a rounded value.
fn inst_010_ewd_egt_digits_not_clamped_to_850() -> SourcePatch {
    SourcePatch {
        path: EWD.to_string(),
        find: "Math.min([3, 4].includes(this.throttlePosition.get()) ? 900 : 850, Math.round(egt))".to_string(),
        replace: "Math.round(egt)".to_string(),
        reason: "the EGT digits carry the same 850 C cap as the gauge (inst_010), from the same \
                 thrust-limit-mode test applied to a lever angle in degrees"
            .to_string(),
    }
}
