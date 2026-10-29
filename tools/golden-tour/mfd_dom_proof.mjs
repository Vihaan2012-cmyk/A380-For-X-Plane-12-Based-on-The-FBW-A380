import fs from 'node:fs';
import path from 'node:path';

const OUT = path.resolve(process.argv[2] || 'captures');
const PORT = process.env.TOUR_PORT || '9333';
const SCRIPT = process.env.MFD_DOM_JS || path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '../../app/js/msfs/mfd-dom.js');
const PAGES = (process.env.MFD_PAGES || 'ACTIVE>F-PLN,ACTIVE>PERF,ACTIVE>INIT,ACTIVE>FUEL & LOAD,ACTIVE>WIND,SECONDARY>F-PLN,POSITION>MONITOR').split(',');
fs.mkdirSync(OUT, { recursive: true });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(`[${new Date().toISOString().slice(11, 19)}]`, ...a);

class View {
  constructor(t) {
    this.t = t;
    this.id = 0;
  }
  async open() {
    this.ws = new WebSocket(this.t.webSocketDebuggerUrl);
    await new Promise((r, j) => {
      this.ws.addEventListener('open', r);
      this.ws.addEventListener('error', j);
    });
    await this.call('Runtime.enable');
    await this.call('Page.enable');
  }
  call(method, params = {}, timeoutMs = 60000) {
    return new Promise((resolve, reject) => {
      const my = ++this.id;
      const timer = setTimeout(() => reject(new Error(`${method}: timeout`)), timeoutMs);
      const on = (ev) => {
        const m = JSON.parse(ev.data);
        if (m.id !== my) return;
        clearTimeout(timer);
        this.ws.removeEventListener('message', on);
        m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result);
      };
      this.ws.addEventListener('message', on);
      this.ws.send(JSON.stringify({ id: my, method, params }));
    });
  }
  async ev(expression) {
    const r = await this.call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 600));
    return r.result.value;
  }
  async big(expression) {
    const len = await this.ev(`(window.__PBIG = ${expression}).length`);
    let s = '';
    for (let at = 0; at < len; at += 1000000) s += await this.ev(`window.__PBIG.slice(${at}, ${at + 1000000})`);
    await this.ev('delete window.__PBIG');
    return s;
  }
}

const targets = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((x) => x.type === 'page' && /Instruments\//.test(x.url));
const mfdT = targets.find((t) => /\/MFD\//.test(t.url));
const ewdT = targets.find((t) => /\/EWD\//.test(t.url));
const MFD = new View(mfdT);
await MFD.open();
const EWD = ewdT ? new View(ewdT) : null;
if (EWD) await EWD.open();

await MFD.ev(`window.__MFD_DOM_FORCE = 'manual'; window.__MFD_DOM_SINK = (b) => { window.__sinkBytes = b.length; };`);
await MFD.ev(fs.readFileSync(SCRIPT, 'utf8'));
log('script loaded:', await MFD.ev('!!window.__MFD_DOM'));

const HELPERS = `(() => {
  if (window.__TH) return 1;
  const shown = (el) => { if (getComputedStyle(el).visibility === 'hidden') return false; for (let n = el; n && n.nodeType === 1; n = n.parentElement) { if (getComputedStyle(n).display === 'none') return false; } const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  const text = (el) => (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim();
  const fire = (el) => { const r = el.getBoundingClientRect(); const o = { bubbles: true, cancelable: true, view: window, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2, button: 0 }; el.dispatchEvent(new MouseEvent('mousedown', o)); el.dispatchEvent(new MouseEvent('mouseup', o)); el.dispatchEvent(new MouseEvent('click', o)); };
  window.__TH = { shown, text, fire };
  return 1;
})()`;
await MFD.ev(HELPERS);

const clickText = (sel, text) =>
  MFD.ev(`(() => { const want = ${JSON.stringify(text.replace(/\s+/g, ''))}; const hits = [...document.querySelectorAll(${JSON.stringify(sel)})].filter((el) => __TH.shown(el) && el.getBoundingClientRect().x < 820 && __TH.text(el).replace(/\\s+/g, '').includes(want)); const el = hits.find((h) => !hits.some((o) => o !== h && h.contains(o))); if (!el) return false; __TH.fire(el); return true; })()`);

async function capture(label) {
  const dir = path.join(OUT, 'captures', label);
  fs.mkdirSync(dir, { recursive: true });
  await MFD.ev('window.__TOUR && window.__TOUR.freeze()');
  await sleep(200);
  const ours = await MFD.big('window.__MFD_DOM.snapshot(true)');
  const timing = await MFD.ev(`(() => { const D = window.__MFD_DOM; const full = []; for (let i = 0; i < 8; i++) { const t = performance.now(); const s = D.snapshot(true); full.push(performance.now() - t); } full.sort((a, b) => a - b); const t0 = performance.now(); const s2 = D.snapshot(false); const inc = performance.now() - t0; const t1 = performance.now(); const j = JSON.stringify(['xp-ui.bus', '{"events":[{"topic":"a380.mfd.dom","data":' + JSON.stringify(D.snapshot(false)) + '}]}']); const wrap = performance.now() - t1; return { fullMedian: full[4], fullMin: full[0], fullMax: full[7], incremental: inc, wrapMs: wrap, wireBytes: j.length, elements: document.body.querySelectorAll('*').length }; })()`);
  await MFD.ev(`(() => { const d = document.createElement('div'); d.id = '__tour_repaint'; d.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;opacity:0.004;background:#000;pointer-events:none'; document.documentElement.appendChild(d); return 1; })()`);
  await sleep(150);
  await MFD.ev(`(() => { const d = document.getElementById('__tour_repaint'); if (d) d.remove(); return 1; })()`);
  await sleep(150);
  const shot = await MFD.call('Page.captureScreenshot', { format: 'png', fromSurface: true }, 20000);
  fs.writeFileSync(path.join(dir, 'mfd.rgba.png'), Buffer.from(shot.data, 'base64'));
  fs.writeFileSync(path.join(dir, 'mfd.sparse.dom.html'), ours);
  fs.writeFileSync(path.join(dir, 'mfd.state.json'), await MFD.big('window.__TOUR.state()'));
  const oracle = await MFD.big("(() => { const props = ['display','position','top','left','right','bottom','width','height','min-width','min-height','max-width','max-height','margin-top','margin-right','margin-bottom','margin-left','padding-top','padding-right','padding-bottom','padding-left','border-top-width','border-right-width','border-bottom-width','border-left-width','border-top-style','border-top-color','border-left-color','border-right-color','border-bottom-color','box-sizing','flex-direction','flex-wrap','justify-content','align-items','align-self','align-content','flex-grow','flex-shrink','flex-basis','gap','row-gap','column-gap','grid-template-columns','grid-template-rows','grid-column','grid-row','order','font-family','font-size','font-weight','line-height','letter-spacing','text-align','vertical-align','white-space','text-transform','text-decoration-line','color','background-color','opacity','visibility','overflow','transform','z-index']; for (const el of document.body.querySelectorAll('*')) { const cs = getComputedStyle(el); el.setAttribute('data-cs', props.map((k) => k + ':' + cs.getPropertyValue(k)).join(';')); } return document.documentElement.outerHTML; })()");
  fs.writeFileSync(path.join(dir, 'mfd.dom.html'), oracle);
  fs.writeFileSync(path.join(dir, 'timing.json'), JSON.stringify({ ...timing, sparseBytes: ours.length, oracleBytes: oracle.length }, null, 1));
  await MFD.ev('window.__TOUR && window.__TOUR.thaw()');
  log(label, 'sparse', ours.length, 'oracle', oracle.length, JSON.stringify(timing));
}

await sleep(3000);
await capture('0001_start');
let n = 1;
for (const spec of PAGES) {
  const [sel, item] = spec.split('>');
  if (!(await clickText('.mfd-page-selector-outer', sel))) {
    log('no selector', sel);
    continue;
  }
  await sleep(500);
  if (!(await clickText('.mfd-dropdown-menu-element, .mfd-context-menu-element', item))) {
    log('no item', item);
    await MFD.ev(`(() => { const c = document.getElementById('MFD_CONTENT') || document.body; __TH.fire(c); return 1; })()`);
    continue;
  }
  await sleep(1500);
  await capture(`${String(++n).padStart(4, '0')}_${(sel + '-' + item).replace(/[^A-Za-z0-9]+/g, '_')}`);
}

if (process.env.MFD_AUTO) {
  await MFD.ev(`window.__MFD_DOM_FORCE = true; window.__MFD_DOM_SINK = undefined; 0`);
  await sleep(6000);
  log('auto stats', JSON.stringify(await MFD.ev('({ sends: __MFD_DOM.stats.sends, builds: __MFD_DOM.stats.builds, full: __MFD_DOM.stats.full, last: __MFD_DOM.stats.last, active: __MFD_DOM.active })')));
  if (EWD) {
    const bus = await EWD.ev("(() => { const s = JSON.parse(window.__TOUR.state()); const e = s.bus && s.bus['a380.mfd.dom']; return e ? { len: e.data.length, frame: e.frame } : null; })()");
    log('EWD saw bus a380.mfd.dom:', JSON.stringify(bus));
  }
  await MFD.ev(`window.__MFD_DOM_FORCE = false; 0`);
}
log('done');
process.exit(0);
