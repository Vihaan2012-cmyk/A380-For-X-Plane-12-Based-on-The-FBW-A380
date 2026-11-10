#!/bin/sh
# Rebuild and copy only the plugin and its pages; tools/install.sh does the
# full install with FlyByWire's instruments.
set -e
AIRCRAFT="${1:-/d/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12}"
TARGET="${CARGO_TARGET_DIR:-/e/fbw-build/target}"
cd "$(dirname "$0")/.."
FEATURES="${FBW_FEATURES:-js}"
CARGO_TARGET_DIR="$TARGET" cargo +stable-x86_64-pc-windows-gnu build --release --features "$FEATURES"
cp "$TARGET/release/fbw_a380_systems.dll" "$AIRCRAFT/plugins/fbw_a380_systems/64/win.xpl"
case ",$FEATURES," in
  *,native-displays,*) cp "$TARGET/release/a380_displays.dll" "$AIRCRAFT/plugins/fbw_a380_systems/64/a380_displays.dll" ;;
esac
mkdir -p "$AIRCRAFT/plugins/fbw_a380_systems/web/ui" "$AIRCRAFT/plugins/fbw_a380_systems/web/js"
cp -r web/ui/* "$AIRCRAFT/plugins/fbw_a380_systems/web/ui/"
cp -r web/js/* "$AIRCRAFT/plugins/fbw_a380_systems/web/js/"
echo "fast install into $AIRCRAFT"
