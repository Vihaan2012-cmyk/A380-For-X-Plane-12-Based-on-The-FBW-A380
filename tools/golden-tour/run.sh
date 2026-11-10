#!/usr/bin/env bash
set -u
OUT=${1:?out dir}
SECTIONS=${2:-baseline,uplink,mfd,nd,sd,ewd,oit,final}
HERE=$(cd "$(dirname "$0")" && pwd)
XP="D:/Steam Games/steamapps/common/X-Plane 12"
AC="$XP/Aircraft/A380 for X-Plane 12"
if tasklist //FI "IMAGENAME eq XPHFBW.exe" 2>/dev/null | grep -q XPHFBW; then
  echo "XPHFBW.exe is already running (X-Plane or another harness); not starting"
  exit 1
fi
mkdir -p "$OUT"
rm -f "$OUT/stop"
cd "$HERE/../.."
CARGO_TARGET_DIR=E:/fbw-build/target \
XPHFBW_EXE="${TOUR_XPHFBW_EXE:-$AC/plugins/fbw_a380_systems/XPHFBW/XPHFBW.exe}" XPHFBW_AIRCRAFT="$AC" XPHFBW_XP_ROOT="${TOUR_XP_ROOT:-$XP}" \
XPHFBW_OUT="$OUT/harness" XPHFBW_SECONDS=14400 XPHFBW_STOP="$OUT/stop" XPHFBW_ND_LAYERS="$OUT/nd-layers" \
XPHFBW_STORMS="${TOUR_STORMS:-storm-day}" XPHFBW_PIN_FILE="$OUT/pins.json" \
FBW_AMDB_DIR="$XP/Output/fbw-a380x/amdb" \
XPHFBW_SEED_VARS="${TOUR_SEED:-$HERE/seed-yssy.json}" XPHFBW_SET="LIGHT POTENTIOMETER:78=1,LIGHT POTENTIOMETER:79=1" \
XPHFBW_DEVTOOLS_PORT=9333 XPHFBW_INJECT_JS="$HERE/rec.js" XPHFBW_INJECT_MATCH="Instruments/" \
cargo test --release --features js --lib xphfbw_harness::xphfbw_harness -- --ignored --nocapture > "$OUT/harness.log" 2>&1 &
HARNESS=$!
until grep -qE "loaded 22/22|test result" "$OUT/harness.log" 2>/dev/null; do
  kill -0 $HARNESS 2>/dev/null || break
  sleep 2
done
if ! grep -q "loaded 22/22" "$OUT/harness.log"; then
  echo "harness failed to start"
  tail -20 "$OUT/harness.log"
  touch "$OUT/stop"
  exit 1
fi
sleep 20
if [ "$SECTIONS" = "hold" ]; then echo "harness up; touch $OUT/stop to end it"; wait $HARNESS; exit 0; fi
TOUR_PIN_FILE="$OUT/pins.json" TOUR_STOP_FILE="$OUT/stop" TOUR_ND_LAYERS="$OUT/nd-layers" node "$HERE/tour.mjs" "$OUT/captures" "$SECTIONS"
touch "$OUT/stop"
wait $HARNESS
python -I "$HERE/flatten.py" "$OUT/captures"
echo "tour finished: $(ls "$OUT/captures" | grep -c '^[0-9]') capture folders"
