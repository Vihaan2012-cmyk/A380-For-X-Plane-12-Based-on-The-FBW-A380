// Opens every host page the bench lists as a tab in cefclient over raw DevTools, and reports what the pages did.
// node bs_probe.mjs <devtools port> <portfile> <shots dir> [settle seconds]
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
const [dt, portfile, shots, settle] = process.argv.slice(2);
mkdirSync(shots, { recursive: true });
const lines = readFileSync(portfile, 'utf8').trim().split(/\r?\n/);
const port = lines[0];
const hosts = lines.slice(1).concat(['SCREEN_EFB']);
const SIZE = { SCREEN_DU_MFD: [2469, 1536], FCU: [2560, 1280], SCREEN_EFB: [1430, 1000], SCREEN_DU_RMP_1: [1664, 1024], SCREEN_OIT_LEFT: [1333, 1000], SCREEN_OIT_RIGHT: [1333, 1000], SCREEN_ISIS_1: [768, 768], SCREEN_ISIS_2: [768, 768], CLOCK: [256, 256], BAT: [256, 128], RTPI: [338, 128] };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const problems = new Map();
const note = (host, text) => {
  const key = text.slice(0, 260);
  const m = problems.get(host) || new Map();
  m.set(key, (m.get(key) || 0) + 1);
  problems.set(host, m);
};
class Tab {
  constructor(host, url) { this.host = host; this.url = url; this.id = 0; this.waiting = new Map(); }
  async open() {
    await opener.send('Runtime.evaluate', { expression: `window.open(${JSON.stringify(this.url)},'_blank','noopener')`, userGesture: true });
    let t;
    for (let i = 0; i < 100 && !t; i++) {
      await sleep(100);
      t = (await (await fetch(`http://127.0.0.1:${dt}/json/list`)).json()).find((x) => x.url === this.url);
    }
    if (!t) throw new Error('no target for ' + this.url);
    this.ws = new WebSocket(t.webSocketDebuggerUrl);
    await new Promise((res, rej) => { this.ws.onopen = res; this.ws.onerror = rej; });
    this.ws.onmessage = (ev) => {
      const m = JSON.parse(ev.data);
      if (m.id && this.waiting.has(m.id)) { this.waiting.get(m.id)(m); this.waiting.delete(m.id); return; }
      if (m.method === 'Runtime.exceptionThrown') { const d = m.params.exceptionDetails; note(this.host, 'exception: ' + ((d.exception && d.exception.description) || d.text)); }
      if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') note(this.host, 'log: ' + m.params.entry.text + ' ' + (m.params.entry.url || '').replace(/^https?:\/\/[^/]+/, ''));
      if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error') note(this.host, 'console: ' + m.params.args.map((a) => a.value || a.description || '').join(' '));
      if (m.method === 'Network.responseReceived' && m.params.response.status >= 400) note(this.host, m.params.response.status + ' ' + m.params.response.url.replace(/^https?:\/\/[^/]+/, ''));
    };
    for (const d of ['Runtime', 'Log', 'Network', 'Page']) await this.send(d + '.enable');
    const [w, h] = SIZE[this.host] || (this.host.startsWith('SCREEN_DU_RMP') ? [1664, 1024] : this.host.startsWith('@') ? [64, 64] : [1152, 1536]);
    this.size = [w, h];
    await this.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
    await this.send('Page.reload', { ignoreCache: true });
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res) => {
      this.waiting.set(id, res);
      this.ws.send(JSON.stringify({ id, method, params }));
      setTimeout(() => { if (this.waiting.has(id)) { this.waiting.delete(id); res({ timeout: true }); } }, 25000);
    });
  }
  async eval(expr) {
    const r = await this.send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
    if (r.timeout) return { timeout: true };
    if (r.result.exceptionDetails) return { threw: r.result.exceptionDetails.exception?.description || r.result.exceptionDetails.text };
    return r.result.result.value;
  }
}
const blank = (await (await fetch(`http://127.0.0.1:${dt}/json/list`)).json())[0];
const opener = new Tab('opener', '');
opener.ws = new WebSocket(blank.webSocketDebuggerUrl);
await new Promise((res) => { opener.ws.onopen = res; });
opener.ws.onmessage = (ev) => { const m = JSON.parse(ev.data); if (m.id && opener.waiting.has(m.id)) { opener.waiting.get(m.id)(m); opener.waiting.delete(m.id); } };
const tabs = [];
for (const [i, host] of hosts.entries()) {
  const t = new Tab(host, `http://s${i}.localhost:${port}/__b/host/${encodeURIComponent(host)}`);
  await t.open();
  tabs.push(t);
  await sleep(300);
}
console.log('opened', tabs.length, 'tabs, settling');
await sleep((+settle || 40) * 1000);
const SUMMARY = `(() => [...document.querySelectorAll('iframe')].map((f) => { const w = f.contentWindow; const b = w.__xphfbw; const d = w.document; return { name: f.name, url: w.location.pathname + w.location.search, bridge: !!b, trip: b ? b.getVar('L:A380X_BENCH_ROUNDTRIP', 'number') : null, panel: !!d.querySelector('vcockpit-panel'), nodes: d.getElementsByTagName('*').length, focus: d.hasFocus(), bg: w.getComputedStyle(d.documentElement).backgroundColor }; }))()`;
for (const t of tabs) {
  console.log(t.host, JSON.stringify(await t.eval(SUMMARY)));
  if (!t.host.startsWith('@')) {
    const s = await t.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    if (s.result?.data) writeFileSync(`${shots}/${t.host}.png`, Buffer.from(s.result.data, 'base64'));
    else console.log('  no screenshot', t.host);
  }
}
const pfd = tabs.find((t) => t.host === 'SCREEN_DU_PFDL');
if (pfd) {
  const B = `document.querySelector('iframe').contentWindow.__xphfbw`;
  console.log('echo', await pfd.eval(`(${B}.setVar('L:A380X_BENCH_ECHO','number',7), ${B}.getVar('L:A380X_BENCH_ECHO','number'))`));
  console.log('call', await pfd.eval(`Promise.race([${B}.call('GET_NOTHING_THAT_EXISTS','[]').then((t)=>'replied '+t,(e)=>'rejected '+e), new Promise((r)=>setTimeout(()=>r('no reply in 3 s'),3000))])`));
  console.log('stored', await pfd.eval(`(${B}.storedData('set','BENCH_KEY','abc'), ${B}.storedData('get','BENCH_KEY',''))`));
  console.log('file bytes', await pfd.eval(`(${B}.readFile('/VFS/panel/panel.xml')||'').length`), 'mag', await pfd.eval(`${B}.magVar(51.5,-0.1)`), 'game', await pfd.eval(`${B}.gameString('fbw/FLIGHT_NAVDATA_DATE_RANGE')`));
}
const inspect = async (name, expr) => { const t = tabs.find((x) => x.host === name); if (t) console.log(name, JSON.stringify(await t.eval(expr))); };
await inspect('SCREEN_DU_PFDL', `(() => { const d = document.querySelector('iframe').contentDocument; return { text: d.body.innerText.slice(0, 200), svg: d.querySelectorAll('svg').length, hidden: [...d.querySelectorAll('svg')].slice(0, 4).map((s) => getComputedStyle(s).display), panel: d.querySelector('vcockpit-panel').getBoundingClientRect().toJSON() }; })()`);
await inspect('FCU', `(() => { const f = document.querySelector('iframe'); const d = f.contentDocument; const bg = [...d.querySelectorAll('*')].map((e) => [e.tagName, e.id, f.contentWindow.getComputedStyle(e).backgroundColor]).filter((r) => r[2] !== 'rgba(0, 0, 0, 0)').slice(0, 6); return bg; })()`);
await inspect('SCREEN_DU_EWD', `(() => { const f = document.querySelector('iframe'); const w = f.contentWindow; const d = f.contentDocument; const col = d.querySelector('.WarningsColumn'); const rows = [...d.querySelectorAll('.EclLine')].filter((l) => l.offsetParent).slice(0, 18).map((l) => { const s = [...l.querySelectorAll('.EclLineText')].find((x) => x.offsetParent); const r = l.getBoundingClientRect(); const q = s ? s.getBoundingClientRect() : {width:0,height:0}; return [s ? s.textContent : '', Math.round(r.width), Math.round(r.height), Math.round(q.width), Math.round(q.height)]; }); const s = d.querySelector('.EclLineText'); const cs = w.getComputedStyle(s); const lc = w.getComputedStyle(d.querySelector('.EclLineContainer')); const c = d.querySelector('.EclLineCheckboxArea'); return { col: col && col.getBoundingClientRect().toJSON(), font: cs.fontFamily, size: cs.fontSize, spacing: cs.letterSpacing, lh: cs.lineHeight, ws: cs.whiteSpace, ctnH: lc.height, box: c && w.getComputedStyle(c).display, boxW: c && c.getBoundingClientRect().width, soft: (d.querySelector('.EclSoftKeysContainer')||{}).offsetWidth, rows }; })()`);
await inspect('SCREEN_DU_NDL', `(() => { const w = document.querySelector('iframe').contentWindow; w.document.dispatchEvent(new PointerEvent('pointermove', {clientX: 300, clientY: 400})); const k = document.getElementById('kccu'); return { shown: k.style.display, left: k.style.left, top: k.style.top, stroke: k.firstChild.getAttribute('stroke') }; })()`);
for (const [host, m] of problems) {
  console.log('--', host);
  for (const [text, n] of [...m].slice(0, 7)) console.log('   ', n, text);
}
await sleep(12000);
process.exit(0);
