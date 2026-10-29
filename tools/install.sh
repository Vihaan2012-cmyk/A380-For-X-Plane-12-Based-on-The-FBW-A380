#!/bin/sh
# Install the plugin, XPHFBW (the companion app) and FlyByWire's instruments
# into the converted aircraft.
# X-Plane must be closed (it locks win.xpl and XPHFBW.exe).
#   tools/install.sh [aircraft folder]
# Everything is assembled in a staging folder on E: first and the release gates
# (tools/release_gates.py) run on it; the aircraft folder is only written when
# every gate passes. XPJS_STAGE_ONLY=1 stops after the gates.
set -e
AIRCRAFT="${1:-/d/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12}"
# One shared build folder for every build (the user's preference,
# 2026-09-27). The msvc app is built for an explicit --target, so its outputs
# land in their own subfolder: without it, the app's msvc build of the
# plugin library (a dependency of the app) overwrote release/
# fbw_a380_systems.dll -- the gnu plugin with the js feature -- and the
# wrong 11.8 MB DLL was installed as win.xpl (2026-09-27).
TARGET="${CARGO_TARGET_DIR:-/e/fbw-build/target}"
TARGET_APP="${CARGO_TARGET_DIR_APP:-$TARGET}"
APP_OUT="$TARGET_APP/x86_64-pc-windows-msvc/release"
CEF_PATH="${CEF_PATH:-/e/fbw-build/cef}"
# FlyByWire's instruments converted onto JS Library for X-Plane (the fork's
# xp-own-api branch), built by `node xplane/build.mjs --out <XPJS_BUILD>`, and
# FlyByWire's fonts and images from the same fork.
FORK="${XPJS_FORK:-/e/fbw-int/fbw-own-api}"
XPJS_BUILD="${XPJS_BUILD:-/e/fbw-build/xpjs/out}"
BUILD="$XPJS_BUILD/html_ui"
ASSETS="$FORK/fbw-a380x/src/base/flybywire-aircraft-a380-842/html_ui"
STAGE="${XPJS_STAGE:-/e/fbw-build/xpjs/stage}"
GATES="${XPJS_GATES:-/e/fbw-build/xpjs/gates-install}"
PANEL="/d/Microsoft Flight Simulator 2020/Microsoft Flight Simulator 2020 Packages/Community/flybywire-aircraft-a380-842/SimObjects/AirPlanes/FlyByWire_A380_842/panel"

cd "$(dirname "$0")/.."
FEATURES="${FBW_FEATURES:-js}"
CARGO_TARGET_DIR="$TARGET" cargo +stable-x86_64-pc-windows-gnu build --release --features "$FEATURES"
# XPHFBW: the companion app (docs/briefs/xphfbw-app.md), MSVC + CEF, its own
# target dir (a different host toolchain from the plugin above).
( cd app && CEF_PATH="$CEF_PATH" CARGO_TARGET_DIR="$TARGET_APP" cargo +stable-x86_64-pc-windows-msvc build --release --target x86_64-pc-windows-msvc )
case "$(echo "$STAGE" | tr 'A-Z' 'a-z')" in
  *x-plane\ 12/aircraft*) echo "install.sh: FAIL: the staging folder must not be inside X-Plane: $STAGE" >&2; exit 1 ;;
esac
rm -rf "$STAGE"
mkdir -p "$STAGE/plugins/fbw_a380_systems/64" "$STAGE/panel" "$STAGE/html_ui"
cp "$TARGET/release/fbw_a380_systems.dll" "$STAGE/plugins/fbw_a380_systems/64/win.xpl"
case ",$FEATURES," in
  *,native-displays,*) cp "$TARGET/release/a380_displays.dll" "$STAGE/plugins/fbw_a380_systems/64/a380_displays.dll" ;;
esac
# FlyByWire's systems in their own process (src/remote); the plugin falls
# back to running them itself without it.
cp "$TARGET/release/fbw_a380_systems_server.exe" "$STAGE/plugins/fbw_a380_systems/64/fbw_a380_systems_server.exe"
# XPHFBW.exe, its CEF runtime and settings UI, next to the plugin so it can
# find it (src/lib.rs start_systems -> js_bridge::plugin_dir()/XPHFBW), the
# plugin's first choice before fbw_a380_systems_server.exe and running the
# systems in-process.
XPHFBW="$STAGE/plugins/fbw_a380_systems/XPHFBW"
mkdir -p "$XPHFBW/ui" "$XPHFBW/locales" "$XPHFBW/js"
cp "$APP_OUT/XPHFBW.exe" "$XPHFBW/"
for dll in "$APP_OUT/"*.dll; do
  # The shared build folder also holds the plugin's own DLL; XPHFBW only
  # needs CEF's.
  [ "$(basename "$dll")" = "fbw_a380_systems.dll" ] || cp "$dll" "$XPHFBW/"
done
cp "$APP_OUT/"*.pak "$XPHFBW/"
cp "$APP_OUT/"*.bin "$XPHFBW/"
cp "$APP_OUT/"*.dat "$XPHFBW/"
cp "$APP_OUT/vk_swiftshader_icd.json" "$XPHFBW/"
cp "$APP_OUT/locales/"*.pak "$XPHFBW/locales/"
cp -r app/ui/* "$XPHFBW/ui/"
# The DATA half of JS Library for X-Plane (app/js), served by the app's own
# xphfbw://runtime/ scheme handler (app/src/scheme.rs) from XPHFBW/js next to
# the exe.
( cd app/js && find . -type f -print ) | while read -r f; do
  mkdir -p "$XPHFBW/js/$(dirname "$f")"
  cp "app/js/$f" "$XPHFBW/js/$f"
done
cp "$PANEL/panel.cfg" "$PANEL/panel.xml" "$STAGE/panel/"
# ISIS 2 is our Standby Navigation Display (html_ui/, copied after FlyByWire's).
sed -i 's|^;htmlgauge00=A380X/ISISlegacy/isislegacy.html, 0,0,512,512|htmlgauge00=A380X/SND/snd.html, 0,0,512,512|' "$STAGE/panel/panel.cfg"
grep -q '^htmlgauge00=A380X/SND/snd.html' "$STAGE/panel/panel.cfg" || { echo "install.sh: FAIL: panel.cfg has no ISIS 2 line to point at the SND" >&2; exit 1; }
grep -q 'A380X/IFE/ife.html' "$STAGE/panel/panel.cfg" || printf '
[VCockpit25]
size_mm=512,256
pixel_size=512,256
texture=$SCREEN_IFE

htmlgauge00=A380X/IFE/ife.html, 0,0,512,256
' >> "$STAGE/panel/panel.cfg"
if [ ! -d "$BUILD" ] || [ ! -d "$ASSETS" ]; then
  echo "install.sh: FAIL: FlyByWire's instruments not found at $BUILD or its fonts and images at $ASSETS -- build them first (node $FORK/xplane/build.mjs --out $XPJS_BUILD)." >&2
  exit 1
fi
# FlyByWire's instruments (GPL-3.0), without the EFB, the OITs' superseded
# legacy page and the popup, and FlyByWire's fonts and images. Fonts/fbw-a380x/EFB
# is installed, EFB or not: the OIT's own faces load from it (oit.css).
( cd "$BUILD" && find . -type f \
    ! -path "./Pages/VCockpit/Instruments/A380X/EFB/*" \
    ! -path "./Pages/VCockpit/Instruments/A380X/OITlegacy/*" \
    ! -path "./Pages/VCockpit/Instruments/A380X/popup/*" -print ) | while read -r f; do
  mkdir -p "$STAGE/html_ui/$(dirname "$f")"
  cp "$BUILD/$f" "$STAGE/html_ui/$f"
done
mkdir -p "$STAGE/html_ui/Fonts" "$STAGE/html_ui/Images"
cp -r "$ASSETS/Fonts/." "$STAGE/html_ui/Fonts/"
cp -r "$ASSETS/Images/." "$STAGE/html_ui/Images/"
echo "install.sh: FlyByWire instruments from $BUILD"
cp -r html_ui/. "$STAGE/html_ui/"
# FBW reads its build info from the package root (/VFS/a380x_build_info.json).
for j in "$XPJS_BUILD"/*.json; do
  [ -f "$j" ] && cp "$j" "$STAGE/"
done
PFD_HTML="$STAGE/html_ui/Pages/VCockpit/Instruments/A380X/PFD/pfd.html"
if [ ! -f "$PFD_HTML" ]; then
  echo "install.sh: FAIL: $PFD_HTML is missing after the html_ui merge -- every cockpit screen will be BLACK." >&2
  exit 1
fi
if grep -rqE '@microsoft|msfssdk' "$STAGE/html_ui" "$XPHFBW/js" --include='*.js' 2>/dev/null; then
  echo "install.sh: FAIL: a Microsoft package is in the staged pages:" >&2
  grep -rlE '@microsoft|msfssdk' "$STAGE/html_ui" "$XPHFBW/js" --include='*.js' >&2
  exit 1
fi
python -I tools/third_party_licenses.py "$STAGE" || { echo "install.sh: FAIL: could not write THIRD_PARTY_LICENSES.txt" >&2; exit 1; }
cp distribution/THIRD_PARTY_NOTICES.txt "$STAGE/THIRD_PARTY_NOTICES.txt"
SOUND_SRC="${FBW_SOUND_SRC:-$(dirname "$PANEL")/sound}"
SOUND_DST="$STAGE/plugins/fbw_a380_systems/sound"
if [ -f "$SOUND_SRC/sound.xml" ]; then
  mkdir -p "$SOUND_DST"
  for f in "$SOUND_SRC"/sound.xml "$SOUND_SRC"/*.[Pp][Cc][Kk]; do
    [ -f "$f" ] && cp "$f" "$SOUND_DST/"
  done
else
  echo "install.sh: no FlyByWire sound banks at $SOUND_SRC; the aircraft will use an MSFS install's at run time" >&2
fi
if ! python -I tools/release_gates.py "$STAGE" --out "$GATES"; then
  echo "install.sh: FAIL: release gates failed (reports in $GATES/gates); nothing was installed" >&2
  exit 1
fi
cp -r "$GATES/gates" "$STAGE/"
cp "$GATES/PROVENANCE.csv" "$STAGE/"
if [ "${XPJS_STAGE_ONLY:-0}" = 1 ]; then
  echo "staged in $STAGE (gates passed); not installed"
  exit 0
fi
if tasklist 2>/dev/null | grep -qi '^X-Plane.exe'; then
  echo "install.sh: FAIL: X-Plane is running" >&2
  exit 1
fi
/e/fbw-build/backup-plugin.sh
# The pages, the page runtime and the gate reports are replaced whole, so
# nothing from an earlier install (the old page shims, html_ui/JS) survives.
rm -rf "$AIRCRAFT/html_ui" "$AIRCRAFT/plugins/fbw_a380_systems/XPHFBW/js" "$AIRCRAFT/gates"
mkdir -p "$AIRCRAFT"
cp -r "$STAGE/." "$AIRCRAFT/"
echo "installed into $AIRCRAFT (XPHFBW: $AIRCRAFT/plugins/fbw_a380_systems/XPHFBW)"
