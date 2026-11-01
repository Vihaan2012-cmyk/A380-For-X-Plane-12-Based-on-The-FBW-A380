#!/bin/sh
# paint_probe.sh <label> "<chromium switches>"
cd /e/fbw-int/plugin
export CARGO_TARGET_DIR=/e/fbw-build/target TMP='E:\fbw-build\tmp' TEMP='E:\fbw-build\tmp'
export FBW_BROWSER_ROUTE=own FBW_BENCH_CEF="E:/fbw-build/cef117" FBW_BENCH_CACHE="E:\\fbw-build\\tmp\\probe-cache-$1"
export FBW_BENCH_URL="file:///E:/fbw-int/plugin/tools/browser-screens/paint_page.html" FBW_CHROMIUM_SWITCHES="$2"
rm -rf "/e/fbw-build/tmp/probe-cache-$1"; mkdir -p "/e/fbw-build/tmp/probe-cache-$1"
(sleep 22; powershell.exe -NoProfile -ExecutionPolicy Bypass -File 'E:\fbw-int\plugin\tools\browser-screens\chromium_vram.ps1' > "/e/fbw-build/tmp/probe-vram-$1.txt" 2>&1) &
timeout 120 cargo +stable-x86_64-pc-windows-gnu test --release --features js --lib paint_cost_probe -- --ignored --nocapture > "/e/fbw-build/tmp/probe-$1.txt" 2>&1
wait
echo "== $1 [$2]"
grep -aE "^PROBE|^error|panicked" "/e/fbw-build/tmp/probe-$1.txt" | cut -c1-170
cut -c1-140 "/e/fbw-build/tmp/probe-vram-$1.txt"
