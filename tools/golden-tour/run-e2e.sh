#!/usr/bin/env bash
set -u
OUT=${1:?out dir}
PAINT_OFF=${2:-0}
HERE=E:/fbw-int/plugin/tools/golden-tour
XP="D:/Steam Games/steamapps/common/X-Plane 12"
AC="$XP/Aircraft/A380 for X-Plane 12"
if tasklist //FI "IMAGENAME eq XPHFBW.exe" 2>/dev/null | grep -q XPHFBW; then
  echo "XPHFBW.exe is already running; not starting"; exit 1
fi
mkdir -p "$OUT"
rm -f "$OUT/stop"
cd E:/fbw-int/plugin
CARGO_TARGET_DIR=E:/fbw-build/target A380_DISPLAYS_BUILD_DIR="${DLLDIR:-E:/fbw-build/a380-displays/dll21}" \
A380_DISPLAYS_DLL="${DLLDIR:-E:/fbw-build/a380-displays/dll21}/a380_displays.dll" A380_HTML_UI="$AC/html_ui" \
FBW_NATIVE_SCREENS="${NATIVE:-all}" E2E_PAINT_OFF="$PAINT_OFF" E2E_SHOT_TRIGGER="$OUT/shoot" \
XPHFBW_EXE="${E2E_XPHFBW_EXE:-E:/fbw-build/e2e21/XPHFBW/XPHFBW.exe}" XPHFBW_AIRCRAFT="$AC" XPHFBW_XP_ROOT="E:/golden-tour/cpu-root" \
XPHFBW_OUT="$OUT/harness" XPHFBW_SECONDS=14400 XPHFBW_STOP="$OUT/stop" XPHFBW_ND_LAYERS="$OUT/nd-layers" \
XPHFBW_STORMS="storm-day" XPHFBW_PIN_FILE="$OUT/pins.json" \
FBW_AMDB_DIR="$XP/Output/fbw-a380x/amdb" \
XPHFBW_DEVTOOLS_PORT="${E2E_DEVTOOLS_PORT:-9333}" XPHFBW_INJECT_JS="$HERE/rec.js" XPHFBW_INJECT_MATCH="Instruments/" \
XPHFBW_SEED_VARS="$HERE/seed-yssy.json" XPHFBW_SET="LIGHT POTENTIOMETER:78=1,LIGHT POTENTIOMETER:79=1" \
cargo +stable-x86_64-pc-windows-msvc test --release --target x86_64-pc-windows-msvc --features js,native-displays --lib xphfbw_harness::xphfbw_harness -- --ignored --nocapture > "$OUT/harness.log" 2>&1 &
wait $!
