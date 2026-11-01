// paint_audit.mjs [port] [seconds]: per page, what painted and what asked for it.
const port = process.argv[2] ?? 9444;
const seconds = +(process.argv[3] ?? 5);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
const ws = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((r) => { ws.onopen = r; });
const events = [];
let done;
const finished = new Promise((r) => { done = r; });
ws.onmessage = (ev) => {
  const m = JSON.parse(ev.data);
  if (m.method === 'Tracing.dataCollected') events.push(...m.params.value);
  if (m.method === 'Tracing.tracingComplete') done();
};
const categories = 'devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-devtools.timeline.invalidationTracking';
ws.send(JSON.stringify({ id: 1, method: 'Tracing.start', params: { transferMode: 'ReportEvents', categories } }));
await sleep(seconds * 1000);
ws.send(JSON.stringify({ id: 2, method: 'Tracing.end' }));
await finished;
const pages = new Map();
for (const e of events) {
  const frames = e.args?.data?.frames;
  if (e.name === 'TracingStartedInBrowser' && frames) {
    for (const f of frames) if (f.url && f.processId && (!pages.has(f.processId) || f.url.includes('Instruments'))) pages.set(f.processId, f.url);
  }
}
const stats = new Map();
const of = (pid) => {
  if (!stats.has(pid)) stats.set(pid, { paints: 0, area: 0, raf: 0, script: 0, layout: 0, style: 0, paintMs: 0, reasons: new Map(), calls: new Map() });
  return stats.get(pid);
};
const bump = (map, key, by = 1) => map.set(key, (map.get(key) ?? 0) + by);
for (const e of events) {
  const s = of(e.pid);
  const d = e.args?.data ?? e.args?.beginData ?? {};
  const ms = (e.dur ?? 0) / 1000;
  if (e.name === 'Paint') {
    s.paints += 1;
    s.paintMs += ms;
    const c = d.clip;
    if (c) s.area += Math.abs(c[4] - c[0]) * Math.abs(c[5] - c[1]);
  }
  if (e.name === 'FireAnimationFrame') s.raf += 1;
  if (e.name === 'Layout') s.layout += ms;
  if (e.name === 'UpdateLayoutTree') s.style += ms;
  if (e.name === 'FunctionCall' || e.name === 'FireAnimationFrame' || e.name === 'TimerFire') {
    s.script += ms;
    if (e.name === 'FunctionCall' && d.functionName) bump(s.calls, `${d.functionName} @${(d.url ?? '').split('/').pop()}:${d.lineNumber}`, ms);
  }
  if (/InvalidationTracking$/.test(e.name)) bump(s.reasons, `${e.name.replace('InvalidationTracking', '')} ${d.nodeName ?? ''} ${d.reason ?? d.invalidationSet ?? ''}`.trim());
}
const top = (map, n) => [...map.entries()].sort((a, b) => b[1] - a[1]).slice(0, n).map(([k, v]) => `${k} (${Math.round(v)})`).join('; ');
for (const [pid, s] of [...stats.entries()].sort((a, b) => b[1].script - a[1].script)) {
  if (!s.paints && !s.raf && s.script < 5) continue;
  console.log(`${(pages.get(pid) ?? `pid ${pid}`).slice(-60)}: ${(s.paints / seconds).toFixed(1)} paints/s, ${(s.area / seconds / 1e6).toFixed(2)} Mpx/s, ${(s.raf / seconds).toFixed(1)} rAF/s, script ${(s.script / seconds).toFixed(0)} ms/s, style ${(s.style / seconds).toFixed(0)}, layout ${(s.layout / seconds).toFixed(0)}, paint ${(s.paintMs / seconds).toFixed(0)}`);
  console.log(`  why: ${top(s.reasons, 8)}`);
  console.log(`  script: ${top(s.calls, 5)}`);
}
process.exit(0);
