import fs from 'node:fs';
import path from 'node:path';

const OUT = process.argv[2];
const LABEL = process.argv[3] || 'shot';
if (!OUT) {
  console.error('usage: node e2e-shoot.mjs <run out dir> [label]');
  process.exit(2);
}
const PORT = process.env.E2E_DEVTOOLS_PORT || '9333';
const IDS = { ewd: 'SCREEN_DU_EWD', 'pfd-1': 'SCREEN_DU_PFDL', 'pfd-2': 'SCREEN_DU_PFDR', 'nd-1': 'SCREEN_DU_NDL', 'nd-2': 'SCREEN_DU_NDR' };
const WANT = (process.env.E2E_STATE_VIEWS || Object.keys(IDS).join(',')).split(',');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nameOf = (url) => {
  const u = new URL(url);
  const parts = u.pathname.split('/').filter(Boolean);
  const page = parts[parts.length - 1].replace(/\.html$/, '').toLowerCase();
  const index = (u.searchParams.get('Index') || '').replace(/[^0-9A-Za-z]+.*$/, '');
  return (index ? `${page}-${index}` : page).replace(/[^0-9A-Za-z_-]/g, '_');
};

class View {
  constructor(t) {
    this.t = t;
    this.name = nameOf(t.url);
    this.id = 0;
  }
  async open() {
    this.ws = new WebSocket(this.t.webSocketDebuggerUrl);
    await new Promise((r, j) => {
      this.ws.addEventListener('open', r);
      this.ws.addEventListener('error', j);
    });
  }
  call(method, params = {}) {
    return new Promise((resolve, reject) => {
      const my = ++this.id;
      const timer = setTimeout(() => reject(new Error(`${this.name} ${method} timed out`)), 20000);
      const on = (ev) => {
        const m = JSON.parse(ev.data);
        if (m.id !== my) return;
        clearTimeout(timer);
        this.ws.removeEventListener('message', on);
        m.error ? reject(new Error(m.error.message)) : resolve(m.result);
      };
      this.ws.addEventListener('message', on);
      this.ws.send(JSON.stringify({ id: my, method, params }));
    });
  }
  async ev(expression) {
    const r = await this.call('Runtime.evaluate', { expression, returnByValue: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 300));
    return r.result.value;
  }
  async big(expression) {
    const len = await this.ev(`(window.__E2EBIG = ${expression}).length`);
    let s = '';
    for (let at = 0; at < len; at += 1000000) s += await this.ev(`window.__E2EBIG.slice(${at}, ${at + 1000000})`);
    await this.ev('delete window.__E2EBIG');
    return s;
  }
}

const targets = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((x) => x.type === 'page' && /Instruments\//.test(x.url));
const views = targets.map((t) => new View(t)).filter((v) => WANT.includes(v.name));
for (const v of views) await v.open();
const dir = path.join(OUT, 'harness', 'native-shots', LABEL);
fs.mkdirSync(dir, { recursive: true });
const states = new Map();
for (const v of views) states.set(v.name, await v.big('window.__TOUR.state()'));
const cdp = new Map();
if (process.env.E2E_CDP_SHOT) for (const v of views) cdp.set(v.name, (await v.call('Page.captureScreenshot', { format: 'png', fromSurface: true })).data);
fs.writeFileSync(path.join(OUT, 'shoot'), LABEL + '\n');
for (let i = 0; i < 100 && fs.existsSync(path.join(OUT, 'shoot')); i++) await sleep(100);
for (const v of views) {
  fs.writeFileSync(path.join(dir, `${IDS[v.name]}.state.json`), states.get(v.name));
  if (cdp.has(v.name)) fs.writeFileSync(path.join(dir, `${IDS[v.name]}.cdp.png`), Buffer.from(cdp.get(v.name), 'base64'));
  v.ws.close();
}
console.log('saved', views.map((v) => v.name).join(' '), 'into', dir);
