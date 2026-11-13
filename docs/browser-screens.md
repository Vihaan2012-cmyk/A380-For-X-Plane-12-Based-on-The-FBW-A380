# Screens in X-Plane's browser (experimental)

Setting `browserDisplays` (EFB Settings, "Screens in X-Plane's browser"), off by default. Reload the aircraft to apply. `FBW_BROWSER_DISPLAYS=1` or `0` overrides it.

With it on, XPHFBW.exe is not started:

| Job | Where it goes |
|---|---|
| FlyByWire's Rust systems | A thread of the plugin, in lockstep with a 100 ms limit; `FBW_SYSTEMS_IN_PROCESS=0` uses `64/fbw_a380_systems_server.exe` |
| Screen pages | One browser device per screen (`xplm_WindowContentTypeBrowser`), same device IDs |
| `SystemsHost`, `ExtrasHost` | Browser devices no mesh uses; moved onto a shown screen if they have not connected after 45 s |
| `window.__xphfbw` | `src/browser_screens/shim.js` |
| `coui://` files, patches, EFB routes | `src/browser_screens/server.rs`, on a free loopback port |
| ND terrain and weather radar | Canvas under the ND page, polled from the server |
| Screen dimming | Black overlay per dimming region |
| Typing | The key sniffer sends keystrokes to the page whose text field has focus |

`XphfbwHost` is unchanged: the server uses the same shared-memory `Session` as XPHFBW's renderers, so none of this adds work to the main thread beyond making the devices.

Each device has its own host name (`s<n>.localhost`), because Chromium allows six requests per host and pages block on synchronous requests. `requestAnimationFrame` falls back to published frames when the browser stops calling it, so page logic keeps running on an undrawn device.

Log lines start `browser screens:`. Every 30 s there is one with each page's frame rate and bytes sent.

## Testing without X-Plane

`tools/browser-screens/run.sh` runs the ignored test `serve_the_real_pages_for_a_browser` as a stand-in for the plugin, opens every real page in a copy of X-Plane's Chromium 117 (`Resources/dlls/64/cef/win/cefclient.exe`, copied to `E:/A380/fbw-build/cef117`) and reports load state, errors and screenshots. It has no systems data, so pages show failure flags.

It cannot check what depends on X-Plane itself: whether devices no mesh uses keep running, how clicks reach a page, and frame rate.
