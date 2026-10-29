#!/usr/bin/env bash
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
SNAP=${SNAP:-E:/fbw-build/emu-snapshots}
OUT=${1:-E:/golden-tour/flight-v1}
SCENARIOS=${2:-emu-06-taxi emu-07-takeoff-roll emu-08-initial-climb emu-09-climb emu-10-cruise emu-11-descent emu-12-approach emu-15-eng2-fire emu-17-elec-emer}
mkdir -p "$OUT/seeds"
for s in $SCENARIOS; do
  python -I - "$SNAP/$s.json" "$OUT/seeds/$s.json" <<'PY'
import json, sys
snap = json.load(open(sys.argv[1], encoding="utf-8"))
vars_out = {name: value for _named, name, value in snap["vars"] if isinstance(value, (int, float))}
vars_out.update({"A32NX_OVHD_INTLT_ANN": 1, "A380X_RMP_1_BRIGHTNESS_KNOB": 60, "A380X_RMP_2_BRIGHTNESS_KNOB": 60, "A380X_RMP_3_BRIGHTNESS_KNOB": 60})
json.dump({"vars": vars_out, "name": snap.get("name"), "description": snap.get("description")}, open(sys.argv[2], "w", encoding="utf-8"))
print(f"{sys.argv[2]}: {len(vars_out)} vars")
PY
  if [ -f "$OUT/$s/captures/manifest.json" ] && grep -q '"final' "$OUT/$s/captures/manifest.json"; then
    echo "$s already recorded"
    continue
  fi
  TOUR_SEED="$OUT/seeds/$s.json" bash "$HERE/run.sh" "$OUT/$s" baseline,sd,ewd,afs,fma,final > "$OUT/$s.run.log" 2>&1
  echo "$s: $(tail -1 "$OUT/$s.run.log")"
done
echo "flight states done"
