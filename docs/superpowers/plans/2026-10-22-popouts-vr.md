# Pop-out screens and panels, VR (A380 for X-Plane 12)

Goal: every cockpit screen and every cockpit panel can be opened in its own
X-Plane window (inside the sim, as an OS window on a second monitor, or as a
VR window), looks like the real panel, and every click works. Total native
display stack stays under 1 ms CPU per frame; a pop-out never adds a second
paint of a screen.

Status 22 Oct: audit + design + phase 1 logic module
(`src/native_displays/popout.rs`, 15 unit tests). Nothing wired into
X-Plane yet; the hook list (section 7) is what the owners of
`src/display/*` and `src/lib.rs` add.

## 1. Audit: what exists today

### Screens are already X-Plane avionics devices

- `src/display/mod.rs` `start()` creates one `XPLMCreateAvionicsEx` device
  per `display/screens.rs` `SCREENS` entry (PFD L/R, ND L/R, EWD, SD, MFD,
  FCU, ISIS 1/2, clock, RTPI, BAT, RMP 1-3, EFB, OIT L/R, IFE, FAP, HUD L/R).
  The cockpit OBJ binds them with `ATTR_cockpit_device <id>` +
  `ATTR_manip_device` (docs/screens.md).
- The native route (`FBW_NATIVE_SCREENS`, `src/native_displays/screen.rs`)
  draws on a render thread; `Displays::draw_native_source` uploads the newest
  frame into the device's texture inside the device draw callback. Chromium
  paint is switched off per screen once a native frame has landed.
- X-Plane shows a device's popup from the same device texture and scales it
  ("X-Plane takes care of scaling your screen and bezel if the user pops out
  the device's window", XPLMDisplay.h). So a popped-out screen costs no
  second paint. This is the property the brief asks for; a hand-made
  `XPLMCreateWindowEx` window per screen would have to re-blit the texture
  every frame and re-implement keyboard focus. **Decision: screens keep using
  the avionics popup, not XPLMCreateWindowEx.**
- `src/display/popout.rs` already registers `fbw/display/popout|popup|close/<id>`
  commands and an "A380X Displays" menu, pops a device out
  (`XPLMSetAvionicsPopupVisible` + `XPLMPopOutAvionics`), sizes the OS window
  to the screen's shape (X-Plane's default is a 198x1080 sliver) and keeps
  the aspect when the user drags an edge (`Watch`).
- Clicks: device `screen_touch`/`right_touch`/`scroll`/`cursor` callbacks
  receive device pixels (origin bottom-left) whether the screen is in the
  cockpit or popped out; `Displays::mouse` -> `dispatch_pointer` sends them
  to XPHFBW's input ring (Chromium) or the QuickJS `ScreenEvent` queue.
  Release-at-(0,0) is already corrected.
- Keyboard: `HAS_KEYBOARD` screens (MFD, OITs, EFB) take the keyboard on
  click (`aim_keyboard`), a key sniffer routes typing, Escape releases it
  (`key_sniffer`, `release_keyboard`). User decision (4) is therefore met for
  the cockpit. In a pop-out the left-click sniffer path is the same code;
  the right-click gesture (`toggle_keyboard`) shows the popup and calls
  `XPLMTakeAvionicsKeyboardFocus`. Unverified in sim: a left click in a
  popped-out MFD typing into the scratchpad when the OS window, not
  X-Plane's, has the OS keyboard focus. If the sniffer does not see those
  keys, H2 calls `take_keyboard_focus` on every click in a popped-out
  keyboard screen.

### Gaps found

1. **No VR mapping.** `XPLMSetAvionicsMappedToVR` / `XPLMIsAvionicsMappedToVR`
   (XPLM440) are not bound in `src/xp.rs` `AvionicsApi`.
2. **No remembered positions.** `Watch` sizes a fresh window every time;
   nothing is saved between sessions, and there are no named layouts.
3. **No monitor choice.** New windows open at (80,80) on the main monitor;
   `XPLMGetAllMonitorBoundsOS` is not bound.
4. **Native screens have no click path of their own.** `a380d_*` (C ABI)
   has no pointer or key entry; clicks still go to the Chromium views that
   keep running with paint off. When Chromium is removed (CEF removal plan)
   the native MFD/OIT/RMP/EFB need `a380d_pointer(display, kind, x, y,
   button, delta)` and `a380d_key(display, ch, vkey, flags)` in the ABI.
   Not part of this plan's phases, but every pop-out click lands in the same
   `dispatch_pointer`, so it is fixed once for cockpit and pop-out.
5. **Panels cannot be popped out at all.** Only screens are devices.
6. **Click OBJ.** `a380_cockpit_click.obj` (installed build, 22 Oct) holds
   1,050 manipulators: 420 command, 217 toggle, 197 push, 72 drag_axis,
   71 command_knob, 52 wheel, 11+2 command_switch, 8 device. Tooltips name
   the panel (`PUSH OVHD APU START`, `KNOB RMP 1 ...`, `PUSH EFIS CS PFD`).
   The SmartCopilot list (tools/smartcopilot/sync_list.json, 611 commands,
   480 datarefs over all OBJs) is the same set of bindings.

## 2. Screen pop-outs (phase 1)

- Modes per screen: Cockpit, Float (popup in X-Plane), PopOut (OS window),
  VR. `popout::transition(now, want, vr_running, vr_api, rect)` returns the
  avionics calls in order: PopOut = popup visible, `XPLMPopOutAvionics`,
  `XPLMSetAvionicsGeometryOS`; VR = `XPLMSetAvionicsMappedToVR(1)`, popup
  visible; leaving VR unmaps first. VR asked without a headset (dataref
  `sim/graphics/VR/enabled` = 0) or without the XPLM440 symbol floats it.
- Placement: `popout::place(saved, monitors, sim_monitor, content, nth)`.
  A saved rectangle whose centre is still on a monitor is reused (pulled
  fully onto that monitor); otherwise the window opens on the first monitor
  X-Plane is not on (user flies 1440p on DISPLAY2, pop-outs go to the other
  one), 60 % of it in the screen's shape, cascaded 40 px per window
  (GENERIC sizes).
- Memory: `popout::Store` in `Output/preferences/fbw_a380x_popouts.json`:
  `windows` (id -> mode + last OS rect) and `layouts` (name -> windows).
  The rectangle is recorded when `Watch` sees the window settle and when a
  window is closed, so the next pop-out returns to the same spot. Written
  with a temp file + rename.
- Named layouts (user decision 3): "Save layout..." stores every open window
  under a name (e.g. "Captain 2nd monitor"); restoring one opens its windows
  at their saved rectangles and puts every other open window away
  (`Store::restore_layout`). Offered as menu items, `fbw/popout/layout/<n>`
  commands (n = 1..8, bindable to a joystick) and an EFB list.
- Opening: existing commands + menu stay. New: a hot-spot on each screen
  (right-click on the bezel edge 24 px band opens the pop-out menu for that
  screen; GENERIC width), and the EFB Settings > Pop-outs page listing every
  screen and panel with Float / Window / VR buttons and the layout list.
- Size: a window may be resized freely; the device texture is not
  re-rendered at the window size (X-Plane scales it). Only if the user picks
  "sharp" for a window larger than 1.5x the screen does the native route
  draw that screen at the window's size (one paint, bigger DPR), never twice.

## 3. Panel pop-outs (phase 2)

Look (user decision 1): like the Hot Start CL650 "Captain's Glareshield
Panel" window: a modern X-Plane window titled with the panel's name, the
panel shown straight-on and photo-real, lamps lit live, any screen on the
panel shown live, every knob/button/switch working.

### Window

- `XPLMCreateWindowEx` modern window (decoration round rect, layer floating),
  positioning `xplm_WindowPositionFree` inside X-Plane, `xplm_WindowPopOut`
  for the OS window, `xplm_WindowVR` in VR (`XPLMSetWindowGeometryVR` in
  boxels = panel mm, GENERIC). Same `Store`/`place` as screens, ids
  `panel/<panel id>`.
- Draw callback: one textured quad of the panel art, lamp overlays (each a
  pre-cut lit sprite drawn with additive blend when its dataref is on, at the
  dataref's brightness), knob/switch sprites rotated or swapped by the bound
  dataref, and each live screen drawn by binding that device's existing GL
  texture (needs hook H4) into its rectangle. No offscreen render, no paint
  of the screen: the panel window samples the texture the cockpit uses.
- Clicks: `popout::window_to_panel` maps the window's boxels to panel pixels
  (letterboxed); `Panel::hit` finds the control, `popout::act` turns the
  gesture into a command begin/end, a one-shot command (knob halves, wheel)
  or a dataref write, exactly as the 3D manipulator would. A click inside a
  live screen rectangle (`Panel::screen_at`) goes to that screen's
  `dispatch_pointer` in its own device pixels, so the FCU display and RMP
  screens work inside their panels.
- Hover: cursor turns to X-Plane's hand/rotate cursor over controls;
  tooltip = the manipulator tooltip.

### Panel description

`Panel` JSON per panel in `<aircraft>/plugins/.../panels/<id>.json`:
`{id, title, size, art, controls: [{id, rect, kind, binding, guard, lamp}],
screens: [[device id, rect]]}`. Kinds: button, switch, guarded switch, knob,
rotary. Bindings: `command`, `knob` (dec/inc commands), `dataref` (value
list, cycles or steps), `axis` (min/max/step). Generated, not hand-written:

1. `tools/panels/extract.py` parses `a380_cockpit_click.obj` (approach in
   `E:/fbw-build/popout34/manips.py`: manipulator line + the `TRIS` range
   after it -> vertex bounds; applies static `ANIM_trans` offsets, and must
   be extended to apply the animation's rest-pose rotations too), groups by
   tooltip and panel plane, fits the panel plane (SVD), and writes each
   control's rectangle in that plane. Kind and binding come from the
   manipulator type and its command/dataref, the lamp from the visual OBJ's
   `ATTR_light_level`/`ANIM_show` dataref on the same button mesh.
2. `tools/panels/render.py` runs Blender 5.2 (installed,
   `C:/Program Files/Blender Foundation/Blender 5.2`) headless: imports the
   cockpit OBJs through an OBJ8 reader (the aircraft converter
   `D:/A380/msfs2xp-aircraft/src/model/obj8.rs` knows the format; a small
   Python OBJ8 importer is written for this), places an orthographic camera
   on the panel plane, renders Cycles with even studio light at 4 px/mm
   (GENERIC), twice: lamps off (base) and lamps on (each lamp's emissive
   material at full; cut into sprites by the lamp's control rectangle).
   Knob caps are rendered separately at 0 deg for rotation.
3. The art is PNG, power-of-two padded (DDS rule), and loaded once.

### Panel division (from the click OBJ, 22 Oct; sizes approximate, rest-pose
rotations not yet applied)

| Panel id | Title | Controls | Approx. size |
|---|---|---|---|
| glareshield_capt | Captain's glareshield (master warn/caution, sidestick prio, chrono, lighting) | 9 | 560 x 220 mm |
| efis_cp_capt | Captain's EFIS control panel | 24 | 300 x 110 mm |
| fcu | FCU (with live FCU display) | 11 push + FCU knobs | 700 x 110 mm |
| efis_cp_fo | First officer's EFIS control panel | 26 | 300 x 110 mm |
| glareshield_fo | First officer's glareshield | 8 | 560 x 220 mm |
| ovhd_left | Overhead left column (ADIRS, evac, oxygen, ent, jettison, flt ctl L) | 50 | 500 x 160 mm |
| ovhd_center_fwd | Overhead centre forward (ext lt, anti-ice, cabin press, wipers) | 66 | 380 x 320 mm |
| ovhd_center_mid | Overhead centre (elec, air cond, fuel) | 49 | 380 x 300 mm |
| ovhd_center_aft | Overhead centre aft (hyd, fire, eng/FADEC, APU) | 18 | 380 x 330 mm |
| ovhd_right | Overhead right column (eng man start, ventilation, cargo smoke) | 28 | 310 x 160 mm |
| ovhd_aft | Aft overhead (reset panel / FMC, NSS, system resets) | 84 | 600 x 235 mm |
| lateral_capt | Captain's lateral (ACD, OIS transfer, laptop) | 9 | GENERIC |
| lateral_fo | First officer's lateral | 9 | GENERIC |
| instr_center | Centre instrument panel (gear, autobrake, ISIS keys, clock, brake fan) | 35 | 460 x 170 mm |
| rmp1 / rmp2 / rmp3 | RMP 1, 2, 3 (live RMP screen) | 58 each | 180 x 130 mm |
| ecam_cp | ECAM control panel | 29 | 180 x 55 mm |
| kccu_capt / kccu_fo | KCCU (keyboard + cursor control) | 70 each + keyboard | 210 x 155 mm |
| eng_master | Engine masters + start selector + ATHR disconnect | 6 | 180 x 150 mm |
| surv | SURV panel | 9 | 80 x 50 mm |
| flaps_speedbrake | Flaps / speedbrake / parking brake / thrust levers (drag axes) | levers | GENERIC |
| lighting_handling | Lighting knobs, wipers | 8 | split per side |

Screens stay their own windows too (PFD, ND, EWD, SD, MFD, OIT, EFB, RMP,
ISIS, FCU display, clock, RTPI, BAT, HUD); a panel that contains a screen
shows it live inside the panel as well.

## 4. VR (user decision 2: OpenXR, Quest/Pico over Link)

- Screens: `XPLMSetAvionicsMappedToVR` (XPLM440, optional symbol; absent on
  older X-Plane = menu item greyed). The VR window shows the device at its
  intrinsic size and is not user-resizable (header), so `CreateAvionics`
  bezel sizes are the VR size.
- Panels: `xplm_WindowVR` positioning mode; `XPLMSetWindowGeometryVR` to the
  panel's mm size. Laser and hand clicks on a VR window arrive as normal
  window mouse callbacks; nothing extra.
- 3D cockpit screens: `ATTR_manip_device` is a manipulator, so the
  controller laser/hand pointer clicks it like any other manipulator (OBJ8,
  12.1). To verify in the headset: MFD, OIT, EFB, RMP, ND (range/mode
  touch) on every screen with `ATTR_manip_device`. If X-Plane only sends the
  press: the release fix in `Displays::mouse` already handles it.
- Watching VR state: `sim/graphics/VR/enabled`; when VR stops, X-Plane
  unmaps VR windows; `Store` keeps their last mode so "VR" layouts restore
  on the next VR start.
- Keyboard in VR: the KCCU panel window (or the KCCU in the cockpit) types
  into the MFD; the physical keyboard path still works.

## 5. Performance budget (user decision: whole native stack < 1 ms/frame)

- Screens: zero added paint; X-Plane composites the device texture.
- Panels: per window per frame one quad + up to ~40 lamp/knob sprites in one
  vertex buffer (one draw call per texture) + one quad per live screen: about
  0.02 ms CPU each (GENERIC estimate), measured with the existing per-screen
  stats logging. Panel windows that are hidden do not draw.
- Datarefs read for lamps/knobs: cached `XPLMDataRef` handles, read once per
  frame per window; ~3,000 reads/frame worst case if every panel is open
  (GENERIC), well under 0.1 ms.
- No `XPLMAvionicsNeedsDrawing` from pop-outs; the native route's 30 Hz
  frame limiter stays the only paint clock.

## 6. Phases

1. Screens: VR binding, monitor placement, `Store` + layouts, EFB page,
   bezel hot-spot. Logic done (`popout.rs`), hooks H1-H6 left.
2. Panel tooling: `tools/panels/extract.py` + `render.py`, art for the
   glareshield (CL650 look check against the user's reference screenshot
   first), then the rest of the division table.
3. Panel windows: drawing, clicks, lamps, live screens (H7-H9).
4. VR pass in the headset (user, OpenXR on Quest/Pico over Link); in-sim
   check of every panel's controls against the 3D cockpit.
5. Native pointer/key ABI (gap 4) together with the CEF removal.

## 7. Hook list (for the owners of the shared files)

| Hook | File | Change |
|---|---|---|
| H1 | `src/xp.rs` `AvionicsApi` | optional `set_mapped_to_vr`, `is_mapped_to_vr` (`XPLMSetAvionicsMappedToVR`, `XPLMIsAvionicsMappedToVR`); `Xplm`: `XPLMGetAllMonitorBoundsOS`, `XPLMGetWindowGeometryVR`/`SetWindowGeometryVR`, `XPLMWindowIsInVR` |
| H2 | `src/display/popout.rs` `apply` | run `native_displays::popout::transition` calls; add `Act::Vr`; place with `popout::place` from the store |
| H3 | `src/display/popout.rs` `tend` | `Store::remember` when a window settles or closes; save on change (debounced) and at `stop` |
| H4 | `src/display/mod.rs` | `pub fn screen_texture(id) -> Option<c_int>` (the device's GL texture) for panel windows |
| H5 | `src/display/mod.rs` `start` menu | "VR" item per screen, "Save layout...", layout list, `fbw/popout/layout/<n>` commands |
| H6 | EFB Settings page | Pop-outs page (screens, panels, layouts) |
| H7 | new `src/panels/` module | panel windows (`XPLMCreateWindowEx`), drawing, clicks via `popout::act` |
| H8 | `src/lib.rs` | start/stop the panel module (one line each) |
| H9 | `src/display/mod.rs` | `pub fn pointer(id, kind, x, y, button, delta)` so a panel's live screen forwards clicks to `dispatch_pointer` |
