#!/bin/sh
# One offline run: the bench standing in for the plugin, X-Plane's Chromium 117 (or BROWSER) as the browser.
cd /e/fbw-int/plugin
AC="/d/Steam Games/steamapps/common/X-Plane 12/Aircraft/A380 for X-Plane 12"
export CARGO_TARGET_DIR=/e/fbw-build/target TMP='E:\fbw-build\tmp' TEMP='E:\fbw-build\tmp'
export FBW_BENCH_AIRCRAFT="$AC" FBW_BENCH_XPROOT='E:\fbw-build\tmp\bs-xproot' FBW_BENCH_UI="/e/fbw-int/plugin/app/ui" FBW_BENCH_PORTFILE='E:\fbw-build\tmp\bs-port.txt' FBW_BENCH_SECONDS=${1:-170}
rm -f /e/fbw-build/tmp/bs-port.txt
(cargo +stable-x86_64-pc-windows-gnu test --release --features js --lib serve_the_real_pages -- --ignored --nocapture > /e/fbw-build/tmp/bs-bench.log 2>&1 &)
for i in $(seq 1 280); do [ -s /e/fbw-build/tmp/bs-port.txt ] && break; sleep 1; done
if [ -n "$BROWSER" ]; then
  "$BROWSER" --remote-debugging-port=9333 --user-data-dir='E:\fbw-build\tmp\bs-profile' --no-first-run --headless=new about:blank > /dev/null 2>&1 &
else
  /e/fbw-build/cef117/cefclient.exe --url=about:blank --remote-debugging-port=9333 --off-screen-rendering-enabled --root-cache-path='E:\fbw-build\tmp\bs-cache' --cache-path='E:\fbw-build\tmp\bs-cache\c' --log-file='E:\fbw-build\tmp\bs-cef.log' --hide-controls > /dev/null 2>&1 &
fi
PID=$!
sleep 5
cd /e/fbw-build/tmp
timeout $(( ${1:-170} - 5 )) node /e/fbw-int/plugin/tools/browser-screens/probe.mjs 9333 E:/fbw-build/tmp/bs-port.txt E:/fbw-build/tmp/bs-shots ${2:-45} 2>&1 | cut -c1-430 | head -100
taskkill //F //T //PID "$(cat /proc/$PID/winpid)" > /dev/null 2>&1
grep -a -E "^t=" /e/fbw-build/tmp/bs-bench.log | tail -3 | cut -c1-700
