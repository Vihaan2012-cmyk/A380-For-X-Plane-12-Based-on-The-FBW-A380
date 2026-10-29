# MFD pages D: ATCCOM, SURV, FCU BKUP page trees (crates/mfd_pages_d)

Goal: each page is a pure function from a page model struct to a `mfd_tree::Node` tree that reproduces the DOM FlyByWire's TSX renders (tags, ids, classes, text, inline style). Shell, header and shared widgets belong to `crates/mfd_pages_a`; this crate carries only the minimal widget helpers it needs (`src/w.rs`) and the page bodies.

## FBW sources translated

- `fbw-a380x/src/systems/instruments/src/MFD/pages/ATCCOM/`: `MfdAtccomConnect.tsx`, `MfdAtccomRequest.tsx`, `MfdAtccomMsgRecord.tsx`, `MfdAtccomDAtis.tsx`, `DAtisBlock.tsx`, `MfdAtccomFooter.tsx`, `Elements/{NewAtisIcon,AutoUpdateIcon,AutoPrintIcon}.tsx` (icons read from the capture).
- `.../MFD/pages/SURV/`: `MfdSurvControls.tsx`, `MfdSurvStatusSwitching.tsx`.
- `.../MFD/pages/FCUBKUP/`: `MfdFcuBkupAfs.tsx`, `MfdFcuBkupEfis.tsx`, `FcuBkupWidgets.tsx`.
- Widgets: `MsfsAvionicsCommon/UiWidgets/{Button,DropdownMenu,InputField,AdscButton,SurvButton,SurvStatusButton,SurvStatusItem,RadioButtonGroup,ConfirmationDialog}.tsx`, `MFD/pages/common/{ActivePageTitleBar,Footer,RequestMenuButton}.tsx`.
- Routing: `MfdPageDirectory.tsx`. REPORT & NOTIFY and EMER have no page of their own in FBW. REPORT renders the REQUEST page (same tree, title CONNECT); EMER renders D-ATIS/LIST.

## Crate shape

- `w`: `button` (plain and menu buttons), `dropdown`, `input_field_text`, `toggle_button` (ADS-C and SURV two-label buttons), `radio_group`, `title_bar`, `footer` / `atccom_footer`, `navigator`, `inop_overlay`.
- `atccom`: `connect_page`, `request_page`, `msg_record_page`, `datis_page` (+ `*_body`), models `ConnectModel`, `RequestModel`, `MsgRecordModel`, `DAtisModel`.
- `surv`: `controls_page`, `status_page`, models `SurvControlsModel`, `SurvStatusModel`.
- `fcubkup`: `afs_page`, `efis_page`, models `FcuBkupAfsModel`, `FcuBkupEfisModel`.
- `*_page` = navigator container (title bar + page body + overlays + footer). `*_body` = the `mfd-page-container` element only.

## Model fields and where they come from

- datalink (atsu26 `crates/datalink`): D-ATIS `icao`, `type_dropdown.selected` (DEP/ARR), `atis_code`, `timestamp` (mailboxTimestamp), `new_atis` (code != lastReadAtis), `auto_update`, `message` (truncated, upper case), `status_label`; Connect `notify_to_atc.values` (available ATC centres, empty today), `active_atc`/`next_atc` (static `XXXX` in FBW), `adsc_armed`/`adsc_emer_armed` (UI state only today), `connected_stations`; Request `frames` (MessageTable visualisations, not translated); `footer.message` (ATC error queue, `AtccomFooter`).
- surveillance / systems: SQWK (`TRANSPONDER CODE`), `xpdr_failed`, `alt_rptg_on/disabled`, XPDR/TCAS radio selection (`xpdrState`, tcas alert level), `tcas_failed`, WXR flags (`wxr1/2Failed`, `wxrOff`, `wxrPwsOff`, `wxrTurbOff`, tilt/elevation/gain values, `wxrGainMan`, `wxrModeMap`, `wxrVdOff`), TAWS flags (`terr/gpws Failed`, `GPWS_*_OFF`), Status page `wxr_taws[*]` / `xpdr_tcas[*]` active and failed flags (`wxrTawsSysSelected`, `xpdrTcas*`).
- FCU: all `FcuBkup*Model` fields (modes `trk_fpa_mode`, `metric`, `mach_mode`, `true_mode`, window texts from `AUTOPILOT_*` / FCU_* vars, lights from `FCU_*` / `AP`/`ATHR` discrete words, EFIS `baro_*`, `ls`, `vv`, `cstr`, `filter`, `arpt`, `overlay`, `traf`).
- UI state: dropdown open flags, `update_menu_open`, `open_menu`, `confirm_visible`/`confirm_label`, `footer`.

## Proof

- `proof/extract_models.py` reads each v15 capture (read only), parses the model from the capture text (UI state and values that have no other source), writes `E:/fbw-build/mfd-pages-d/models/<capture>.model.json` and `manifest.tsv`.
- `examples/proof.rs` builds the page tree from each model and writes `E:/fbw-build/mfd-pages-d/<capture>.json`.
- `python tools/pagetree/compare.py <json> <capture>/mfd.dom.html cls=mfd-navigator-container`. The default page root class `mfd-page-container` matches the first such element, which in the captures is the "FMS PAGE NOT AVAIL" dialog, so the navigator container (title bar, page body, overlay, footer) is the root compared.
- `E:/fbw-build/mfd-pages-d/run_compare.sh` repeats the comparisons; results in `compare-results.txt`.

## Not done

- MSG RECORD sub-pages (`MfdAtccomMsgRecordAll/Monitored/Expand.tsx`), D-ATIS RECEIVED (`MfdAtccomDAtisReceived.tsx`), the request message frames (`Messages/*`, `Elements/MessageElement*.tsx`): no capture, not translated.
- Radio `checked` / `disabled` attributes and `input` state are properties, not in the tree.
