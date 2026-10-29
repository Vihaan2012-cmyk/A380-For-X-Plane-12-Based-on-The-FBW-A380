# Laminar-style cockpit displays

## What Laminar does (read from the A330-300 in the user's X-Plane 12)

- The screens are one Plane Maker 2D panel (`cockpit_3d/-PANELS-/Panel.png`). X-Plane draws that panel texture on the GPU every frame, inside its own render pass, and maps it onto the cockpit screens.
- Each screen is a stack of generic instruments: bitmap sprites in `cockpit_3d/generic/` (176 for the PFD, 80 for the ECAM, plus ND, Standby and the 7-segment displays), moved, rotated, clipped or swapped by datarefs.
- Text comes from pre-drawn font sheets, one per colour (`generic/Text/airbus_display_18_{amber,cyan,green,magenta,red}.png`). There is no text shaping and no rasterising at run time.
- The ND map uses X-Plane's built-in EFIS map instruments.
- The logic is xlua (`plugins/xlua/scripts/A333.sys_displays` and others). It only writes datarefs.
- Result: no CPU drawing, no extra GL contexts, no copies, and every frame shows that frame's datarefs.

## What we do now

- Skia (Ganesh GL) draws each screen on 4 render threads. Their GL contexts share textures with X-Plane's. In the avionics draw callback the main thread copies each finished picture into the device texture.
- Measured offline (`a380disp_render --bench`, values changing every frame): PFD 1.4 ms, ND 1.6 ms, SD 1.1 ms, SND 2.0 ms. In the sim the PFD takes 8.9 ms a frame, so most of the in-sim cost is contention with X-Plane (threads, contexts, GPU queue), not drawing.

## Plan

### Phase 0: grey cockpit labels (separate problem)
1. Find the panel textures that hold "SIDESTICK PRIORITY" and the other grey words. Compare each label's ink brightness in the installed albedo with FBW's atlas.
2. Expected cause: lettering the 20 Oct `bake_fbw_ao.py --text-work` pass did not mask, so FBW's AO darkened it. Re-run the AO bake with the missing label masks, from the AO originals.
3. Check in the sim that SIDESTICK and PRIORITY match.

### Phase 1: draw in the screen callback, like Laminar (switch `FBW_NATIVE_INLINE=1`)
1. One Skia GrDirectContext on X-Plane's own GL context, made in the first avionics draw callback.
2. In each screen's draw callback, wrap the bound device FBO as a Skia render target and draw the display straight into it at the device size, with no thread, copy or fence. Call `resetContext()` before Skia draws, and restore the X-Plane state the plugin guidance lists afterwards.
3. Keep the per-screen dirty check: a screen whose inputs did not change re-shows its cached texture (one blit).
4. Measure at the same airport as the 33 fps reference flight: sim fps, plugin ms per frame, and per-screen ms. Keep whichever mode wins as the default.

### Phase 2: draw less, like generic instruments
1. Split each heavy screen into static and dynamic layers. Static parts (backgrounds, scale ticks, rose rings, frames) are drawn once into GPU textures. Each frame only draws the moving parts (tapes, pointers, values) over them.
2. PFD first (SND, ND, SD next): the speed and altitude tapes become one pre-drawn strip each, shifted by the value, as Laminar's `Tapes` sprites are.
3. Cache text blobs for repeated strings, so labels are not shaped every frame.

### Phase 3: Laminar-style text
1. Bake the Airbus display font into per-colour, per-size glyph sheets at the device texel size, as Laminar's `airbus_display_18_*` sheets do. Draw text as textured quads from them, so every glyph has the same weight and brightness at every mip.
2. Use them for the PFD, ND, EWD and SD. The MFD, OIT and EFB keep Skia text (they change rarely and are cached).

### Phase 4 (only if 1-3 fall short): full sprite renderer for the PFD and ND
- A small GL batcher (textured quads, rotation, clip rects) driven by the same inputs. Its art is pre-rendered from our own Skia displays, so it looks identical.

## Acceptance
- Same airport, time and weather as the 33 fps reference flight: at least 33 fps.
- The PFD and ND show a new picture every sim frame (no repeated frames in the "native displays" log).
- Display text the same brightness as now or better; panel labels uniform.
