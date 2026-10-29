# MFD pages A: shell and FMS ACTIVE F-PLN

Crate: `crates/mfd_pages_a` (path dependency on `crates/mfd_tree`). Every page is a pure function from a model struct to a `Node` tree reproducing the DOM FBW's TSX renders.

## Sources (FBW GPL, `fbw-a380x/src/systems/instruments/src`)

- `MFD/MFD.tsx` render (display unit, mfd-main, page container)
- `MFD/pages/common/{AbstractHeader,FmsHeader,Footer,FmsPage,ActivePageTitleBar}.tsx`
- `MsfsAvionicsCommon/UiWidgets/{Button,DropdownMenu,PageSelectorDropdownMenu,InputField,IconButton,ContextMenu,ConditionalComponent,MouseCursor,shapes}.tsx`
- `MFD/pages/FMS/{MfdMsgList,MfdFmsPageNotAvail}.tsx`, `F-PLN/{MfdFmsFpln,DestinationWindow,InsertNextWptFrom,FplnRevisionsMenu,MfdFmsFplnDuplicateNames}.tsx`

## Modules

- `widgets`: shared by all page crates. `button`, `dropdown_menu`, `page_selector`, `input_field`, `icon_button`, `context_menu`, `conditional`, `title_bar`, `footer`, `triangle_*`, `mouse_cursor`.
- `shell`: `display_unit(ShellModel, page)` = svg overlays + mfd-main + header (sys select row, page select row) + page-not-avail + message list + duplicate names + navigator container + cursor.
- `fpln`: `fpln_page(FplnModel)` = title bar, page container (revisions context menu, NEW DEST box, INSERT NEXT WPT box, header, lines, TMPY buttons, destination row, footer, message footer).

## Model

`ShellModel`: side (CAPT/FO), fms source, callsign, active category, sys dropdown state, message list, footer messages, EO/TMPY/penalty flags.
`FplnModel`: lines (waypoint / special / hold, with flags), line colour (loaded plan), flight phase, EFOB/WIND toggle, true track, weight unit, destination row values, scroll button state, open dropdown, revisions menu items + selected row, NEW DEST box, INSERT NEXT WPT box.

## Captures (v15, left MFD subtree `id=MFD_LEFT_PARENT_DIV`)

0008 system-menu, 0018 F-PLN, 0023 SPD/ALT dropdown, 0027 F-PLN INFO dropdown, 0030 revisions YSSY, 0035 revisions KADOM, 0050/0053/0057/0061 INSERT NEXT WPT, 0077/0080/0083 OVERFLY, 0087/0090/0092 NEW DEST, 0163/0165/0168 revisions PKS/APOMA/AS.

Out of scope here (other agents): captures where the page is DIRECT-TO, HOLD, AIRWAYS, VERT REV, WIND (0040-0044, 0065-0072, 0096-0160).

## Model source

Models are parsed from the capture text by `tools/capture_to_model.py` (in the output folder `E:/fbw-build/mfd-pages-a`), because the fms-oracle goldens hold procedures and secondary plans, not the displayed F-PLN rows. The Rust example `build_capture` reads the model JSON, builds the tree, and writes `<capture>.json`.
