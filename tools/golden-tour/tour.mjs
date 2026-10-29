import fs from 'node:fs';
import path from 'node:path';

const OUT = process.argv[2];
if (!OUT) {
  console.error('usage: node tour.mjs <out-dir> [sections]');
  process.exit(2);
}
const SECTIONS = (process.argv[3] || 'baseline,uplink,mfd,nd,sd,ewd,oit,final').split(',');
const PORT = process.env.TOUR_PORT || '9333';
const SIMBRIEF = process.env.TOUR_SIMBRIEF || '';
const OFP = process.env.TOUR_OFP ? JSON.parse(fs.readFileSync(process.env.TOUR_OFP, 'utf8')) : null;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const SPEED = Number(process.env.TOUR_SPEED || 0.5);
const nap = (ms) => sleep(Math.max(150, ms * SPEED));
const slug = (s) => String(s).replace(/<[^>]*>/g, ' ').replace(/[^0-9A-Za-z.+-]+/g, '_').replace(/^_+|_+$/g, '').slice(0, 48) || 'x';
const manifest = [];
let seq = 0;
fs.mkdirSync(OUT, { recursive: true });
const LOCK = path.join(path.dirname(path.resolve(OUT)), '.tour.lock');
try {
  fs.writeFileSync(LOCK, String(process.pid), { flag: 'wx' });
} catch {
  console.error(`another tour holds ${LOCK} (pid ${fs.readFileSync(LOCK, 'utf8')}); stop it or delete the lock`);
  process.exit(3);
}
process.on('exit', () => {
  try {
    fs.unlinkSync(LOCK);
  } catch {}
});
const LAYERS = process.env.TOUR_ND_LAYERS || '';
const log = (...a) => {
  const line = `[${new Date().toISOString().slice(11, 19)}] ${a.join(' ')}`;
  console.log(line);
  fs.appendFileSync(path.join(OUT, 'tour.log'), line + '\n');
};

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
    this.host = /\/(SystemsHost|ExtrasHost)\//.test(t.url);
    this.id = 0;
  }
  async open() {
    this.ws = new WebSocket(this.t.webSocketDebuggerUrl);
    await new Promise((r, j) => {
      this.ws.addEventListener('open', r);
      this.ws.addEventListener('error', j);
    });
    this.ws.addEventListener('close', () => log(this.name, 'socket closed'));
    await this.call('Runtime.enable');
    await this.call('Page.enable');
    await this.call('Emulation.setFocusEmulationEnabled', { enabled: true });
  }
  call(method, params = {}, timeoutMs = 30000) {
    return new Promise((resolve, reject) => {
      const my = ++this.id;
      const timer = setTimeout(() => {
        this.ws.removeEventListener('message', on);
        reject(new Error(`${this.name} ${method}: no answer in ${timeoutMs / 1000} s`));
      }, timeoutMs);
      const on = (ev) => {
        const m = JSON.parse(ev.data);
        if (m.id !== my) return;
        clearTimeout(timer);
        this.ws.removeEventListener('message', on);
        m.error ? reject(new Error(`${this.name} ${method}: ${m.error.message}`)) : resolve(m.result);
      };
      this.ws.addEventListener('message', on);
      this.ws.send(JSON.stringify({ id: my, method, params }));
    });
  }
  async ev(expression, commandLine = false) {
    const r = await this.call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, includeCommandLineAPI: commandLine });
    if (r.exceptionDetails) throw new Error(`${this.name}: ${JSON.stringify(r.exceptionDetails).slice(0, 500)}`);
    return r.result.value;
  }
  async big(expression) {
    const len = await this.ev(`(window.__TOURBIG = ${expression}).length`);
    let s = '';
    for (let at = 0; at < len; at += 1000000) s += await this.ev(`window.__TOURBIG.slice(${at}, ${at + 1000000})`);
    await this.ev('delete window.__TOURBIG');
    return s;
  }
  getVar(name, unit = 'number') {
    return this.ev(`SimVar.GetSimVarValue(${JSON.stringify(name)}, ${JSON.stringify(unit)})`);
  }
  setVar(name, value, unit = 'number') {
    return this.ev(`SimVar.SetSimVarValue(${JSON.stringify(name)}, ${JSON.stringify(unit)}, ${Number(value)})`);
  }
}

const targets = (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((x) => x.type === 'page' && /Instruments\//.test(x.url));
const views = targets.map((t) => new View(t));
for (const v of views) await v.open();
const byName = Object.fromEntries(views.map((v) => [v.name, v]));
const instruments = views.filter((v) => !v.host);
log(`${views.length} views:`, views.map((v) => v.name).join(' '));
const find = (re) => views.find((v) => re.test(v.t.url));
const MFD = find(/\/MFD\//);
const ND1 = find(/\/ND\/.*Index=1/);
const ND2 = find(/\/ND\/.*Index=2/);
const SDV = find(/\/SDv2\//) || find(/\/SD\//);
const SDS = views.filter((v) => /\/SD(v2)?\//.test(v.t.url));
const EWD = find(/\/EWD\//);
const OITS = views.filter((v) => /\/OIT\//.test(v.t.url) && !/legacy/i.test(v.t.url));
const ANY = find(/\/PFD\//) || instruments[0];

for (let i = 0; i < 300; i++) {
  const ready = await Promise.all(instruments.map((v) => v.ev('!!(window.__TOUR && window.__TOUR.frames > 5)').catch(() => false)));
  if (ready.every(Boolean)) break;
  if (i === 299) {
    log('not every view reports frames:', instruments.filter((_, k) => !ready[k]).map((v) => v.name).join(' '));
  }
  await sleep(1000);
}
const noTour = [];
for (const v of instruments) if (!(await v.ev('!!window.__TOUR'))) noTour.push(v.name);
if (noTour.length) {
  log('recorder missing in', noTour.join(' '), '- is XPHFBW_INJECT_JS set?');
  process.exit(1);
}

const LAYOUT_DUMP = `JSON.stringify((() => {
  const out = [];
  const walk = (el) => {
    if (out.length > 4000) return;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden') return;
    const own = [...el.childNodes].filter((n) => n.nodeType === 3 && n.textContent.trim()).map((n) => n.textContent.trim()).join(' ');
    if (own) {
      const r = el.getBoundingClientRect();
      const rec = { tag: el.tagName.toLowerCase(), cls: String(el.getAttribute('class') || ''), text: own.slice(0, 80), x: r.x, y: r.y, w: r.width, h: r.height, font: cs.fontFamily, size: cs.fontSize, lh: cs.lineHeight, ls: cs.letterSpacing, color: cs.color };
      if (el instanceof SVGTextElement || el instanceof SVGTSpanElement) { try { const b = el.getBBox(); rec.bbox = [b.x, b.y, b.width, b.height]; rec.anchor = cs.textAnchor; rec.baseline = cs.dominantBaseline || el.getAttribute('alignment-baseline'); } catch (e) {} }
      for (const n of el.childNodes) if (n.nodeType === 3 && n.textContent.trim()) { const rg = document.createRange(); rg.selectNodeContents(n); const rr = rg.getClientRects(); rec.lines = [...rr].map((q) => [q.x, q.y, q.width, q.height]); break; }
      out.push(rec);
    }
    for (const c of el.children) walk(c);
  };
  walk(document.body);
  return out;
})())`;

async function capture(label, which, note = '') {
  const list = (which === 'all' ? instruments : which).filter(Boolean);
  const dir = path.join(OUT, `${String(++seq).padStart(4, '0')}_${slug(label)}`);
  fs.mkdirSync(dir, { recursive: true });
  const settle = Number(process.env.TOUR_SETTLE_FRAMES || 40);
  const start = await Promise.all(list.map((v) => v.ev('window.__TOUR.frames').catch(() => 0)));
  for (let i = 0; i < 40; i++) {
    const now = await Promise.all(list.map((v) => v.ev('window.__TOUR.frames').catch(() => Infinity)));
    if (now.every((f, k) => f - start[k] >= settle)) break;
    await sleep(100);
  }
  await Promise.all(list.map((v) => v.ev('window.__TOUR.freeze()').catch(() => {})));
  await sleep(150);
  const shots = [];
  try {
  for (const v of list) {
    try {
      await v.ev(`(() => { const d = document.createElement('div'); d.id = '__tour_repaint'; d.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;opacity:0.004;background:#000;pointer-events:none'; document.documentElement.appendChild(d); return 1; })()`);
      await sleep(120);
      await v.ev(`(() => { const d = document.getElementById('__tour_repaint'); if (d) d.remove(); return 1; })()`);
      await sleep(120);
      const shot = await v.call('Page.captureScreenshot', { format: 'png', fromSurface: true }, 20000);
      fs.writeFileSync(path.join(dir, `${v.name}.rgba.png`), Buffer.from(shot.data, 'base64'));
      fs.writeFileSync(path.join(dir, `${v.name}.state.json`), await v.big('window.__TOUR.state()'));
      fs.writeFileSync(path.join(dir, `${v.name}.layout.json`), await v.big(LAYOUT_DUMP).catch(() => '[]'));
      if (v === MFD || v === ND1 || v === ND2 || OITS.includes(v)) fs.writeFileSync(path.join(dir, `${v.name}.dom.html`), await v.big("(() => { for (const i of document.querySelectorAll('input, textarea, select')) { if (i.type === 'checkbox' || i.type === 'radio') { if (i.checked) i.setAttribute('checked', ''); else i.removeAttribute('checked'); } else if ('value' in i) i.setAttribute('value', i.value); } if (window.__TOUR_CS !== false) { const props = ['display','position','top','left','right','bottom','width','height','min-width','min-height','max-width','max-height','margin-top','margin-right','margin-bottom','margin-left','padding-top','padding-right','padding-bottom','padding-left','border-top-width','border-right-width','border-bottom-width','border-left-width','border-top-style','border-top-color','border-left-color','border-right-color','border-bottom-color','box-sizing','flex-direction','flex-wrap','justify-content','align-items','align-self','align-content','flex-grow','flex-shrink','flex-basis','gap','row-gap','column-gap','grid-template-columns','grid-template-rows','grid-column','grid-row','order','font-family','font-size','font-weight','line-height','letter-spacing','text-align','vertical-align','white-space','text-transform','text-decoration-line','color','background-color','opacity','visibility','overflow','transform','z-index']; for (const el of document.body.querySelectorAll('*')) { const cs = getComputedStyle(el); el.setAttribute('data-cs', props.map((k) => k + ':' + cs.getPropertyValue(k)).join(';')); } } return document.documentElement.outerHTML; })()"));
      const side = v === ND1 ? 'L' : v === ND2 ? 'R' : '';
      if (side && LAYERS && fs.existsSync(path.join(LAYERS, `latest-${side}.txt`))) {
        const layer = fs.readFileSync(path.join(LAYERS, `latest-${side}.txt`), 'utf8').trim();
        fs.copyFileSync(path.join(LAYERS, layer), path.join(dir, `${v.name}.layer.png`));
        fs.writeFileSync(path.join(dir, `${v.name}.layer.txt`), layer);
      }
      shots.push(v.name);
    } catch (e) {
      log('capture', label, v.name, 'FAILED', e.message);
    }
  }
  } finally {
    await Promise.all(list.map((v) => v.ev('window.__TOUR.thaw()').catch(() => {})));
  }
  manifest.push({ seq, dir: path.basename(dir), label, note, views: shots });
  fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 1));
  log('captured', path.basename(dir), shots.join(','));
}

const HELPERS = `(() => {
  if (window.__TH) return 1;
  const shown = (el) => {
    if (getComputedStyle(el).visibility === 'hidden') return false;
    for (let n = el; n && n.nodeType === 1; n = n.parentElement) {
      if (getComputedStyle(n).display === 'none') return false;
    }
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  };
  const text = (el) => (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim();
  const fire = (el) => {
    const r = el.getBoundingClientRect();
    const o = { bubbles: true, cancelable: true, view: window, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2, button: 0 };
    el.dispatchEvent(new MouseEvent('mousedown', o));
    el.dispatchEvent(new MouseEvent('mouseup', o));
    el.dispatchEvent(new MouseEvent('click', o));
  };
  window.__TH = { shown, text, fire };
  return 1;
})()`;

const LEFT = 'el.getBoundingClientRect().x < 820';

async function mfdList(sel, extra = 'true') {
  return MFD.ev(`(() => { ${HELPERS}; return [...document.querySelectorAll(${JSON.stringify(sel)})].filter((el) => __TH.shown(el) && ${LEFT} && (${extra})).map((el, i) => ({ i, text: __TH.text(el).slice(0, 60), cls: el.className && el.className.baseVal === undefined ? String(el.className) : '' })); })()`);
}

async function mfdClick(sel, index, extra = 'true') {
  return MFD.ev(`(() => { ${HELPERS}; const els = [...document.querySelectorAll(${JSON.stringify(sel)})].filter((el) => __TH.shown(el) && ${LEFT} && (${extra})); const el = els[${index}]; if (!el) return false; __TH.fire(el); return true; })()`);
}

async function mfdClickText(sel, text) {
  return MFD.ev(`(() => { ${HELPERS}; const want = ${JSON.stringify(text.replace(/\s+/g, ''))}; const hits = [...document.querySelectorAll(${JSON.stringify(sel)})].filter((el) => __TH.shown(el) && ${LEFT} && __TH.text(el).replace(/\\s+/g, '').includes(want)); const el = hits.find((h) => !hits.some((o) => o !== h && h.contains(o))); if (!el) return false; __TH.fire(el); return true; })()`);
}

async function mfdTexts() {
  return MFD.ev(`(() => { ${HELPERS}; return [...document.querySelectorAll('*')].filter((el) => !el.children.length && __TH.shown(el) && ${LEFT}).map((el) => __TH.text(el)).filter(Boolean); })()`);
}

const MENU = '.mfd-dropdown-menu';
const MENU_EL = '.mfd-dropdown-menu-element, .mfd-context-menu-element';
const OPEN_MENU_EXTRA = "!!el.closest('.mfd-dropdown-menu, .mfd-context-menu')";
const SELECTOR = '.mfd-page-selector-outer';
const SYS_DROPDOWN = '.mfd-header-sys-select-row .mfd-dropdown-container > :first-child';
const PAGE_DROPDOWN = ".mfd-dropdown-outer, .mfd-button:has(.mfd-fms-fpln-button-dropdown)";
const PAGE_DROPDOWN_EXTRA = "!el.closest('.mfd-header-page-select-row, .mfd-header-sys-select-row') && !el.classList.contains('mfd-dropdown-menu')";
const TAB = '.mfd-top-tab-navigator-bar-element-outer';

async function openMenusOf(prefix) {
  const dds = await mfdList(PAGE_DROPDOWN, PAGE_DROPDOWN_EXTRA);
  for (const dd of dds) {
    if (!(await mfdClick(PAGE_DROPDOWN, dd.i, PAGE_DROPDOWN_EXTRA))) continue;
    await nap(500);
    const open = await mfdList(MENU_EL, OPEN_MENU_EXTRA);
    if (open.length) await capture(`${prefix}/dropdown-${dd.i}-${dd.text}`, [MFD], `${open.length} items: ${open.map((o) => o.text).join(' | ')}`);
    await mfdClick(PAGE_DROPDOWN, dd.i, PAGE_DROPDOWN_EXTRA);
    await nap(400);
    if ((await mfdList(MENU_EL, OPEN_MENU_EXTRA)).length) {
      await MFD.ev(`(() => { ${HELPERS}; const c = document.getElementById('MFD_CONTENT') || document.body; __TH.fire(c); return 1; })()`);
      await nap(300);
    }
  }
}

async function tourPage(prefix) {
  await capture(prefix, [MFD]);
  await openMenusOf(prefix);
  const tabs = await mfdList(TAB);
  for (const tab of tabs) {
    if (tab.i === 0) continue;
    await mfdClick(TAB, tab.i);
    await nap(700);
    await capture(`${prefix}/tab-${tab.i}-${tab.text}`, [MFD]);
    await openMenusOf(`${prefix}/tab-${tab.i}`);
  }
  if (tabs.length) {
    await mfdClick(TAB, 0);
    await nap(500);
  }
}

async function selectPage(selectorText, itemText) {
  if (!(await mfdClickText(SELECTOR, selectorText))) return false;
  await nap(400);
  const ok = await mfdClickText(MENU_EL, itemText);
  await nap(900);
  return ok;
}

async function eraseTmpy() {
  if (await mfdClickText('.mfd-fms-fpln-line-erase-temporary, .mfd-button, .mfd-fms-fpln-line', 'ERASE TMPY')) {
    await nap(800);
    log('erased a temporary flight plan the tour opened');
  }
}

async function fplnRevisions(prefix) {
  const idents = await mfdList('.mfd-fms-fpln-line-ident');
  const picks = [...new Set([0, 1, 2, Math.floor(idents.length / 2), idents.length - 1].filter((i) => i >= 0 && i < idents.length))];
  for (const i of picks) {
    const ident = idents[i];
    if (!(await mfdClick('.mfd-fms-fpln-line-ident', i))) continue;
    await nap(500);
    const items = await mfdList('.mfd-context-menu-element');
    if (!items.length) continue;
    await capture(`${prefix}/revisions-${i}-${ident.text}`, [MFD], items.map((x) => x.text).join(' | '));
    if (i !== picks[1] && picks.length > 1) {
      await MFD.ev(`(() => { ${HELPERS}; const c = document.getElementById('MFD_CONTENT') || document.body; __TH.fire(c); return 1; })()`);
      await nap(300);
      continue;
    }
    const labels = items.filter((x) => !/disabled/.test(x.cls)).map((x) => x.text);
    await MFD.ev(`(() => { ${HELPERS}; const c = document.getElementById('MFD_CONTENT') || document.body; __TH.fire(c); return 1; })()`);
    await nap(300);
    for (const label of labels) {
      if (!label || /^(DELETE|ERASE)/i.test(label)) continue;
      await selectPage('ACTIVE', 'F-PLN');
      if (!(await mfdClick('.mfd-fms-fpln-line-ident', i))) continue;
      await nap(500);
      if (!(await mfdClickText('.mfd-context-menu-element', label))) continue;
      await nap(1000);
      await tourPage(`${prefix}/rev-${ident.text}/${label}`);
      await selectPage('ACTIVE', 'F-PLN');
      await eraseTmpy();
    }
  }
}

async function currentSystem() {
  return MFD.ev(`(() => { ${HELPERS}; const s = [...document.querySelectorAll('.mfd-header-sys-select-row .mfd-dropdown-outer')].find((el) => __TH.shown(el) && ${LEFT}); return s ? __TH.text(s) : ''; })()`);
}

async function closeMenus() {
  for (let i = 0; i < 3 && (await mfdList(MENU_EL, OPEN_MENU_EXTRA)).length; i++) {
    await MFD.ev(`(() => { ${HELPERS}; const c = document.getElementById('MFD_CONTENT') || document.body; __TH.fire(c); return 1; })()`);
    await nap(300);
  }
}

async function switchSystem(sys) {
  for (let attempt = 0; attempt < 4; attempt++) {
    if ((await currentSystem()) === sys) return true;
    await closeMenus();
    await mfdClick(SYS_DROPDOWN, 0);
    await nap(500);
    await mfdClickLabel(MENU_EL, sys);
    await nap(1500);
  }
  if ((await currentSystem()) === sys) return true;
  log('could not switch the MFD to', sys, '- it shows', await currentSystem());
  return false;
}

async function mfdTour() {
  await MFD.ev(HELPERS);
  const systems = [];
  if (await MFD.ev(`(() => { ${HELPERS}; return !!document.querySelector(${JSON.stringify(SYS_DROPDOWN)}); })()`)) {
    await mfdClick(SYS_DROPDOWN, 0);
    await nap(500);
    for (const s of await mfdList(MENU_EL, OPEN_MENU_EXTRA)) systems.push(s.text);
    await capture('mfd/system-menu', [MFD], systems.join(' | '));
    await mfdClick(SYS_DROPDOWN, 0);
    await nap(300);
  }
  log('MFD systems:', systems.join(', ') || '(no system menu found)');
  for (const sys of systems.length ? systems : ['FMS']) {
    if (systems.length && !(await switchSystem(sys))) continue;
    const selectors = await mfdList(SELECTOR);
    log(`MFD ${sys} page selectors:`, selectors.map((s) => s.text).join(', '));
    if (!selectors.length) {
      await tourPage(`mfd/${sys}`);
      continue;
    }
    for (const s of selectors) {
      await mfdClick(SELECTOR, s.i);
      await nap(500);
      const items = await mfdList(MENU_EL, OPEN_MENU_EXTRA);
      if (!items.length) {
        await nap(700);
        await tourPage(`mfd/${sys}/${s.text}`);
        continue;
      }
      await capture(`mfd/${sys}/${s.text}-menu`, [MFD], items.map((x) => x.text).join(' | '));
      await mfdClick(SELECTOR, s.i);
      await nap(300);
      for (const item of items) {
        if (/disabled/.test(item.cls)) {
          log('skip disabled', sys, s.text, item.text);
          continue;
        }
        await mfdClick(SELECTOR, s.i);
        await nap(400);
        if (!(await mfdClickText(MENU_EL, item.text))) continue;
        await nap(1000);
        const prefix = `mfd/${sys}/${s.text}/${item.text}`;
        await tourPage(prefix);
        if (/^FMS/.test(sys) && /F-PLN/.test(item.text) && s.text === 'ACTIVE') await fplnRevisions(prefix);
      }
    }
  }
  if (systems.length) await switchSystem(systems[0]);
  await selectPage('ACTIVE', 'F-PLN');
}

async function uplink() {
  if (SIMBRIEF) {
    const stored = await MFD.ev(`(() => { try { SetStoredData('A380X_CONFIG_OVERRIDE_SIMBRIEF_USERID', ${JSON.stringify(SIMBRIEF)}); return GetStoredData('A380X_CONFIG_OVERRIDE_SIMBRIEF_USERID'); } catch (e) { return 'error: ' + e.message; } })()`);
    log('SimBrief id stored:', stored);
  }
  await MFD.ev(HELPERS);
  await selectPage('ACTIVE', 'INIT');
  await capture('uplink/init-before', [MFD]);
  let received = false;
  for (let attempt = 1; attempt <= 3 && !received; attempt++) {
    if (!(await mfdClickText('*', 'CPNY F-PLN REQUEST'))) log('no CPNY F-PLN REQUEST button');
    for (let i = 0; i < 60; i++) {
      await sleep(1000);
      const shown = (await mfdTexts()).join(' ');
      if (/RECEIVED\s*CPNY\s*F-PLN/.test(shown)) {
        received = true;
        break;
      }
      if (/NOT\s*VALID/.test(shown)) {
        log(`uplink attempt ${attempt}: company flight plan not valid, retrying`);
        await mfdClickText('*', 'CLEAR INFO');
        await sleep(5000);
        break;
      }
    }
  }
  if (!received) log('the SimBrief uplink never arrived');
  await capture('uplink/received', [MFD]);
  await mfdClickText('*', 'RECEIVED CPNY F-PLN');
  await nap(800);
  await capture('uplink/uplink-plan', [MFD]);
  if (!(await mfdClickText('*', 'INSERT*'))) await mfdClickText('*', 'INSERT');
  await sleep(5000);
  await selectPage('ACTIVE', 'F-PLN');
  await nap(3000);
  await capture('uplink/inserted', 'all');
  if (OFP) await procedures();
  if (OFP) {
    const kg = OFP.params && /lb/i.test(OFP.params.units) ? 0.45359237 : 1;
    const tonnes = (v) => ((Number(v) * kg) / 1000).toFixed(1);
    await selectPage('ACTIVE', 'FUEL&LOAD');
    await enterField('ZFW', tonnes(OFP.weights.est_zfw));
    await enterField('ZFWCG', process.env.TOUR_ZFWCG || '37.0');
    await enterField('BLOCK', tonnes(OFP.fuel.plan_ramp));
    await nap(3000);
    await capture('uplink/fuel-load-entered', [MFD]);
    await selectPage('ACTIVE', 'F-PLN');
    await nap(8000);
    await capture('uplink/predictions', 'all');
  }
}

async function mfdClickLabel(sel, label) {
  return MFD.ev(`(() => { ${HELPERS}; const want = ${JSON.stringify(label.replace(/\s+/g, ''))}; const hits = [...document.querySelectorAll(${JSON.stringify(sel)})].filter((el) => __TH.shown(el) && ${LEFT} && __TH.text(el).replace(/[\\s\\u25b2\\u25bc*]+/g, '') === want); const el = hits.find((h) => !hits.some((o) => o !== h && h.contains(o))); if (!el) return false; __TH.fire(el); return true; })()`);
}

async function pickFromMenu(button, match) {
  let items = [];
  for (let attempt = 0; attempt < 6 && !items.length; attempt++) {
    if (attempt) {
      await closeMenus();
      await sleep(1000);
    }
    if (!(await mfdClickLabel('.mfd-button, .mfd-button *', button))) {
      if (attempt === 5) {
        log('no button', button);
        return false;
      }
      continue;
    }
    await sleep(600);
    items = await mfdList(MENU_EL, OPEN_MENU_EXTRA);
  }
  const pick = items.find((x) => match(x.text.replace(/\s+/g, ' ').trim()));
  if (!pick) {
    log(`${button}: nothing matched in ${items.map((x) => x.text).join(' | ')}`);
    await mfdClickLabel('.mfd-button, .mfd-button *', button);
    await nap(300);
    return false;
  }
  await mfdClick(MENU_EL, pick.i, OPEN_MENU_EXTRA);
  await nap(1500);
  log(`${button}: ${pick.text}`);
  return true;
}

async function insertTmpy(label) {
  if (!(await mfdClickLabel('.mfd-button, .mfd-button *', 'TMPY F-PLN'))) log(label, 'no TMPY F-PLN button');
  await nap(1500);
  await capture(`procedures/${label}-tmpy`, [MFD]);
  if (!(await mfdClickText('*', 'INSERT TMPY'))) {
    if (!(await mfdClickText('*', 'TMPY INSERT'))) log(label, 'no INSERT TMPY button');
  }
  await nap(3000);
}

async function revise(identSel, ident, item) {
  if (!(await mfdClickLabel(identSel, ident))) {
    log(`no ${ident} line to revise`);
    return false;
  }
  await nap(600);
  if (item && !(await mfdClickText('.mfd-context-menu-element, .mfd-dropdown-menu-element', item))) {
    log(`no ${item} in the ${ident} revision menu`);
    return false;
  }
  await nap(1500);
  return true;
}

async function procedures() {
  const g = OFP.general || {};
  const dep = OFP.origin || {};
  const arr = OFP.destination || {};
  await selectPage('ACTIVE', 'F-PLN');
  if (await revise('.mfd-fms-fpln-line-ident, .mfd-fms-fpln-line-ident *', dep.icao_code, 'DEPARTURE')) {
    await pickFromMenu('RWY', (t) => t.replace(/\s+/g, '').startsWith(dep.icao_code + dep.plan_rwy) || t.split(' ')[0] === dep.plan_rwy || t.includes(dep.plan_rwy));
    if (g.sid_ident) await pickFromMenu('SID', (t) => t.split(' ')[0] === g.sid_ident);
    if (g.sid_trans) await pickFromMenu('TRANS', (t) => t.split(' ')[0] === g.sid_trans);
    await capture('procedures/departure', [MFD]);
    await insertTmpy('departure');
  }
  await selectPage('ACTIVE', 'F-PLN');
  if (await revise('.mfd-fms-fpln-line-destination *', arr.icao_code, null)) {
    const rwy = arr.plan_rwy;
    const ok = (await pickFromMenu('APPR', (t) => /^ILS/.test(t) && t.includes(rwy))) || (await pickFromMenu('APPR', (t) => t.includes(rwy)));
    if (!ok) log('no approach for', rwy);
    if (g.star_ident) await pickFromMenu('STAR', (t) => t.split(' ')[0] === g.star_ident);
    if (g.star_trans) await pickFromMenu('TRANS', (t) => t.split(' ')[0] === g.star_trans);
    await capture('procedures/arrival', [MFD]);
    await insertTmpy('arrival');
  }
  await selectPage('ACTIVE', 'F-PLN');
  await nap(4000);
  await capture('procedures/f-pln', 'all');
}

async function typeKeys(text) {
  for (const ch of text) {
    const code = ch.toUpperCase().charCodeAt(0);
    await MFD.call('Input.dispatchKeyEvent', { type: 'keyDown', key: ch, text: ch, unmodifiedText: ch, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
    await MFD.call('Input.dispatchKeyEvent', { type: 'keyUp', key: ch, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
  }
  await MFD.call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', text: '\r', unmodifiedText: '\r', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
  await MFD.call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
}

async function enterField(label, value) {
  const ok = await MFD.ev(
    `(() => { ${HELPERS}; const lab = [...document.querySelectorAll('.mfd-label')].find((el) => __TH.shown(el) && ${LEFT} && __TH.text(el) === ${JSON.stringify(label)}); if (!lab) return false; let n = lab.nextElementSibling; while (n && !n.classList.contains('mfd-input-field-root')) n = n.nextElementSibling; const box = n && n.querySelector('.mfd-input-field-text-input-container'); if (!box) return false; __TH.fire(box); return true; })()`,
  );
  if (!ok) {
    log('no input field labelled', label);
    return;
  }
  await nap(300);
  await typeKeys(value);
  await nap(1200);
  log(`entered ${label} ${value}`);
}

const MODES = ['LS', 'VOR', 'NAV', 'ARC', 'PLAN'];
const RANGES = ['ZOOM', '10', '20', '40', '80', '160', '320', '640'];
const OANS_RANGES = ['0', '1', '2', '3', '4'];
const OPTION_BITS = { CSTR: 1, VORD: 2, WPT: 4, NDB: 8, ARPT: 16 };
const BUTTONS = ['CSTR', 'WPT', 'VORD', 'NDB', 'ARPT', 'TRAF', 'WX', 'TERR', 'LS', 'VV'];
const efis = { L: { option: 0, overlay: 0 }, R: { option: 0, overlay: 0 } };

async function setEfis(view, side, mode, range, oansRange = 5) {
  await view.setVar(`L:A32NX_EFIS_${side}_ND_MODE`, mode);
  await view.setVar(`L:A380X_EFIS_${side}_ND_MODE`, mode);
  await view.setVar(`L:A32NX_EFIS_${side}_ND_RANGE`, range);
  await view.setVar(`L:A380X_EFIS_${side}_ND_RANGE`, range);
  await view.setVar(`L:A32NX_EFIS_${side}_OANS_RANGE`, range === 0 ? oansRange : 5);
}

async function efisButton(view, side, btn, on) {
  const s = efis[side];
  if (btn in OPTION_BITS) {
    s.option = on ? s.option | OPTION_BITS[btn] : s.option & ~OPTION_BITS[btn];
    await view.setVar(`L:A32NX_EFIS_${side}_OPTION`, s.option);
    if (btn === 'CSTR' || btn === 'ARPT') await view.setVar(`L:A380X_EFIS_${side}_${btn}_BUTTON_IS_ON`, on ? 1 : 0);
  } else if (btn === 'WX' || btn === 'TERR') {
    s.overlay = on ? (btn === 'WX' ? 1 : 2) : 0;
    await view.setVar(`L:A380X_EFIS_${side}_ACTIVE_OVERLAY`, s.overlay);
    await view.setVar(`L:A32NX_EFIS_TERR_${side}_ACTIVE`, s.overlay === 2 ? 1 : 0);
  } else {
    await view.setVar(`L:A380X_EFIS_${side}_${btn}_BUTTON_IS_ON`, on ? 1 : 0);
  }
  await view.setVar(`L:A32NX_FCU_EFIS_${side}_${btn}_LIGHT_ON`, on ? 1 : 0);
}

async function viewClickables(v) {
  await v.ev(HELPERS);
  return v.ev(
    `(() => { ${HELPERS}; const out = []; let i = 0; for (const el of document.querySelectorAll('body *')) { if (!__TH.shown(el)) continue; const l = getEventListeners(el); if (!(l.click || l.mousedown || l.mouseup)) continue; el.dataset.tourId = String(i); out.push({ i: i++, text: __TH.text(el).slice(0, 40), cls: String(el.getAttribute('class') || '').slice(0, 60) }); } return out; })()`,
    true,
  );
}

async function viewClick(v, i) {
  return v.ev(`(() => { ${HELPERS}; const el = document.querySelector('[data-tour-id="${i}"]'); if (!el) return false; __TH.fire(el); return true; })()`);
}

async function oansTour(nd, side) {
  await efisButton(nd, side, 'ARPT', true);
  for (const m of [3, 2]) {
    for (let o = 0; o < OANS_RANGES.length; o++) {
      await setEfis(nd, side, m, 0, o);
      await nap(o === 0 && m === 3 ? 15000 : 3000);
      await capture(`nd/${side}/${MODES[m]}/ZOOM-${OANS_RANGES[o]}/OANS`, [nd]);
    }
  }
  await setEfis(nd, side, 3, 0, 2);
  await nap(4000);
  const controls = (await viewClickables(nd)).filter((c) => !/DELETE|CLEAR|ERASE|RESET/i.test(c.text));
  log(`OANS ${side}: ${controls.length} clickable elements`);
  for (const c of controls.slice(0, 30)) {
    if (!(await viewClick(nd, c.i))) continue;
    await nap(1200);
    await capture(`nd/${side}/OANS-control-${c.i}-${c.text || c.cls}`, [nd]);
    await nd.call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }).catch(() => {});
    await nd.call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27, nativeVirtualKeyCode: 27 }).catch(() => {});
    await nap(500);
    await viewClickables(nd);
  }
  await efisButton(nd, side, 'ARPT', false);
}

async function fplnScrollButtons() {
  return mfdList('.mfd-button', "!__TH.text(el).trim() && !!el.querySelector('svg')");
}

async function planTour(nd, side) {
  await selectPage('ACTIVE', 'F-PLN');
  await efisButton(nd, side, 'CSTR', true);
  for (let r = 1; r < RANGES.length; r++) {
    await setEfis(nd, side, 4, r);
    await nap(2000);
    await capture(`nd/${side}/PLAN/origin/${RANGES[r]}`, [nd, MFD]);
  }
  for (let k = 1; k <= 12; k++) {
    const arrows = await fplnScrollButtons();
    if (!arrows.length) break;
    await mfdClick('.mfd-button', arrows[0].i, "!__TH.text(el).trim() && !!el.querySelector('svg')");
    await nap(900);
    await setEfis(nd, side, 4, 3);
    await nap(1200);
    await capture(`nd/${side}/PLAN/down-${k}/40`, [nd, MFD]);
  }
  if (await mfdClickLabel('.mfd-button, .mfd-button *', 'DEST')) {
    await nap(1500);
    for (let r = 1; r <= 5; r++) {
      await setEfis(nd, side, 4, r);
      await nap(2000);
      await capture(`nd/${side}/PLAN/destination/${RANGES[r]}`, [nd, MFD]);
    }
    for (let k = 1; k <= 6; k++) {
      const arrows = await fplnScrollButtons();
      if (arrows.length < 2) break;
      await mfdClick('.mfd-button', arrows[1].i, "!__TH.text(el).trim() && !!el.querySelector('svg')");
      await nap(900);
      await setEfis(nd, side, 4, 2);
      await nap(1200);
      await capture(`nd/${side}/PLAN/destination-up-${k}/20`, [nd, MFD]);
    }
  } else {
    log('no DEST button on the F-PLN page');
  }
  await efisButton(nd, side, 'CSTR', false);
  await selectPage('ACTIVE', 'F-PLN');
}

async function waitLayer(side, kind, maxMs = 15000) {
  const file = path.join(LAYERS, `latest-${side}.txt`);
  const read = () => {
    try {
      return fs.readFileSync(file, 'utf8');
    } catch {
      return '';
    }
  };
  const before = read();
  const until = Date.now() + maxMs;
  await sleep(1000);
  while (Date.now() < until && (read() === before || !read().includes(kind))) await sleep(250);
  if (!read().includes(kind)) {
    log(`no ${kind} layer on ${side} after ${maxMs / 1000} s (latest ${read()})`);
    return false;
  }
  await sleep(kind === 'wxr' ? 12000 : 1200);
  return true;
}

async function overlayOn(nd, side, b) {
  const kind = b === 'WX' ? 'wxr' : 'terrain';
  for (let attempt = 0; attempt < 3; attempt++) {
    await efisButton(nd, side, b === 'WX' ? 'TERR' : 'WX', false);
    await efisButton(nd, side, b, true);
    if (await waitLayer(side, kind)) return true;
  }
  return false;
}

async function ndTour() {
  for (const [side, nd, full] of [['L', ND1, true], ['R', ND2, false]]) {
    if (!nd) continue;
    for (const b of BUTTONS) await efisButton(nd, side, b, false);
    for (let m = 0; m < MODES.length; m++) {
      for (let r = 1; r < RANGES.length; r++) {
        if (!full && r !== 3) continue;
        await setEfis(nd, side, m, r);
        await nap(1500);
        await capture(`nd/${side}/${MODES[m]}/${RANGES[r]}`, [nd]);
      }
    }
    if (!full) continue;
    await oansTour(nd, side);
    for (const [m, r] of [[3, 2], [2, 1], [4, 3]]) {
      await setEfis(nd, side, m, r);
      for (const b of BUTTONS) {
        if ((b === 'TERR' || b === 'WX') && m !== 4) await overlayOn(nd, side, b);
        else {
          await efisButton(nd, side, b, true);
          await nap(1500);
        }
        await capture(`nd/${side}/${MODES[m]}/${RANGES[r]}/${b}`, [nd]);
        await efisButton(nd, side, b, false);
      }
    }
    for (const r of [2, 3, 4, 5]) {
      for (const b of ['WX', 'TERR']) {
        await setEfis(nd, side, 3, r);
        await overlayOn(nd, side, b);
        await capture(`nd/${side}/ARC/${RANGES[r]}/${b}-range`, [nd]);
        await efisButton(nd, side, b, false);
      }
    }
    await efisButton(nd, side, 'CSTR', true);
    await efisButton(nd, side, 'WPT', true);
    await efisButton(nd, side, 'TERR', true);
    await efisButton(nd, side, 'TRAF', true);
    await setEfis(nd, side, 3, 3);
    await overlayOn(nd, side, 'TERR');
    await capture(`nd/${side}/ARC/40/CSTR+WPT+TERR+TRAF`, [nd]);
    for (const b of ['CSTR', 'WPT', 'TERR', 'TRAF']) await efisButton(nd, side, b, false);
    await setEfis(nd, side, 3, 3);
  }
}


const SD_PAGES = [['ENG', 0], ['APU', 1], ['BLEED', 2], ['COND', 3], ['PRESS', 4], ['DOOR', 5], ['ELEC_AC', 6], ['ELEC_DC', 7], ['FUEL', 8], ['WHEEL', 9], ['HYD', 10], ['F_CTL', 11], ['CB', 12], ['STS', 14], ['VIDEO', 15]];

async function sdTour() {
  if (!SDV) return;
  for (const [name, index] of SD_PAGES) {
    await SDV.setVar('L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX', index);
    await nap(2000);
    await capture(`sd/${name}`, SDS);
  }
  await SDV.setVar('L:A32NX_ECAM_SD_CURRENT_PAGE_INDEX', -1);
  await nap(2000);
  await capture('sd/auto', SDS);
}

async function press(btn) {
  await ANY.setVar(`L:A32NX_BTN_${btn}`, 1);
  await nap(250);
  await ANY.setVar(`L:A32NX_BTN_${btn}`, 0);
  await nap(1500);
}

async function keyEvent(name) {
  await ANY.setVar(`K:${name}`, 0);
  await nap(1500);
}

async function afsTour() {
  const shots = [find(/\/PFD\/.*Index=1/), find(/\/PFD\/.*Index=2/), ND1, ND2, find(/\/FCU\//), EWD].filter(Boolean);
  await capture('afs/initial', shots);
  await keyEvent('A32NX.FCU_AP_1_PUSH');
  await capture('afs/ap1', shots);
  await keyEvent('A32NX.FCU_ATHR_PUSH');
  await capture('afs/ap1-athr', shots);
  for (const side of ['L', 'R']) await efisButton(side === 'L' ? ND1 : ND2, side, 'LS', true);
  await nap(1500);
  await capture('afs/ls', shots);
  await keyEvent('A32NX.FCU_LOC_PUSH');
  await capture('afs/loc', shots);
  await keyEvent('A32NX.FCU_APPR_PUSH');
  await capture('afs/appr', shots);
  await keyEvent('A32NX.FCU_EXPED_PUSH');
  await capture('afs/exped', shots);
  await keyEvent('A32NX.FCU_EFIS_L_FD_PUSH');
  await keyEvent('A32NX.FCU_EFIS_R_FD_PUSH');
  await capture('afs/fd-off', shots);
  await keyEvent('A32NX.FCU_EFIS_L_FD_PUSH');
  await keyEvent('A32NX.FCU_EFIS_R_FD_PUSH');
  await keyEvent('A32NX.FCU_AP_DISC_PUSH');
  await capture('afs/ap-off', shots);
}

function word(bits) {
  const f = new Float32Array([bits.reduce((v, b) => v + 2 ** (b - 1), 0)]);
  return 3 * 2 ** 32 + new Uint32Array(f.buffer)[0];
}
const FMA_DEFAULTS = { FG_DISCRETE_WORD_1: [15, 16, 17, 18], FG_DISCRETE_WORD_2: [12], FG_DISCRETE_WORD_3: [], FG_DISCRETE_WORD_4: [], FG_DISCRETE_WORD_5: [18], FG_DISCRETE_WORD_6: [], FG_ATS_DISCRETE_WORD: [13], FG_ATS_FMA_DISCRETE_WORD: [] };
const FMA_STATES = [
  ['clb-nav-ap1-athr', { FG_DISCRETE_WORD_1: [11, 13, 15, 16, 17, 18], FG_DISCRETE_WORD_2: [11, 12], FG_DISCRETE_WORD_3: [11], FG_DISCRETE_WORD_4: [12], FG_ATS_DISCRETE_WORD: [11, 12, 13], FG_ATS_FMA_DISCRETE_WORD: [19] }],
  ['alt-crz-nav', { FG_DISCRETE_WORD_1: [11, 13, 15, 16, 17, 18], FG_DISCRETE_WORD_2: [12], FG_DISCRETE_WORD_3: [20, 29], FG_DISCRETE_WORD_4: [12], FG_ATS_DISCRETE_WORD: [11, 12, 13], FG_ATS_FMA_DISCRETE_WORD: [19] }],
  ['gs-loc-armed-appr1', { FG_DISCRETE_WORD_1: [11, 13, 15, 16, 17, 18], FG_DISCRETE_WORD_2: [12, 13, 23, 28], FG_DISCRETE_WORD_3: [20], FG_DISCRETE_WORD_4: [16], FG_ATS_DISCRETE_WORD: [11, 12, 13], FG_ATS_FMA_DISCRETE_WORD: [19] }],
  ['land3-dual', { FG_DISCRETE_WORD_1: [11, 12, 13, 14, 15, 16, 17, 18, 23], FG_DISCRETE_WORD_3: [22], FG_DISCRETE_WORD_4: [14], FG_ATS_DISCRETE_WORD: [11, 12, 13], FG_ATS_FMA_DISCRETE_WORD: [19] }, { FCDC_1_FG_DISCRETE_WORD_1: [26], FCDC_2_FG_DISCRETE_WORD_1: [26] }],
  ['srs-rwy-man-toga', { FG_DISCRETE_WORD_1: [13, 15, 16, 17, 18], FG_DISCRETE_WORD_2: [12, 22], FG_DISCRETE_WORD_3: [15], FG_DISCRETE_WORD_4: [11, 18], FG_ATS_DISCRETE_WORD: [11, 13], FG_ATS_FMA_DISCRETE_WORD: [11] }],
];

function writePins(vars) {
  if (!process.env.TOUR_PIN_FILE) return false;
  fs.writeFileSync(process.env.TOUR_PIN_FILE, JSON.stringify({ vars }));
  return true;
}

async function statesTour() {
  const file = process.env.TOUR_STATES;
  if (!file) { log('states: no TOUR_STATES'); return; }
  for (const line of fs.readFileSync(file, 'utf8').split('\n')) {
    if (!line.trim()) continue;
    const { name, vars } = JSON.parse(line);
    if (!writePins(vars)) { log('states: no TOUR_PIN_FILE'); return; }
    await nap(6000);
    await capture(`flight/${name}`, 'all');
  }
  writePins({});
}

async function fmaTour() {
  const shots = [find(/\/PFD\/.*Index=1/), find(/\/PFD\/.*Index=2/), find(/\/FCU\//)].filter(Boolean);
  let prevVars = {};
  for (const n of [1, 2, 3]) for (const [k, v] of Object.entries(FMA_DEFAULTS)) prevVars[`L:A32NX_PRIM_${n}_${k}`] = word(v);
  for (const n of [1, 2]) prevVars[`L:A32NX_FCDC_${n}_FG_DISCRETE_WORD_1`] = word([]);
  for (const [name, words, extra = {}] of FMA_STATES) {
    const vars = {};
    for (const n of [1, 2, 3]) {
      for (const [k, v] of Object.entries({ ...FMA_DEFAULTS, ...words })) vars[`A32NX_PRIM_${n}_${k}`] = word(v);
      for (const k of ['ROLL_FD_COMMAND', 'PITCH_FD_COMMAND', 'YAW_FD_COMMAND']) for (const s of [1, 2]) vars[`A32NX_PRIM_${n}_${k}_${s}`] = 12884901888;
    }
    for (const [k, v] of Object.entries(extra)) vars[`A32NX_${k}`] = word(v);
    if (!writePins(vars)) { log('fma: no TOUR_PIN_FILE'); return; }
    for (const v of shots) await v.ev(`(() => { window.__TOUR.extra = { fmaChangedAt: performance.now(), fmaPrev: ${JSON.stringify(prevVars)} }; return 1; })()`).catch(() => {});
    prevVars = Object.fromEntries(Object.entries(vars).filter(([k]) => k.startsWith('A32NX_PRIM_') || k.startsWith('A32NX_FCDC_')).map(([k, v]) => [`L:${k}`, v]));
    await nap(1500);
    await capture(`fma/${name}/change`, shots);
    await nap(11000);
    await capture(`fma/${name}/steady`, shots);
  }
  writePins({});
}

async function ewdTour() {
  if (!EWD) return;
  await capture('ewd/normal', [EWD]);
  await press('CL');
  await capture('ewd/checklist-menu', [EWD]);
  for (let i = 0; i < 3; i++) {
    await press('DOWN');
    await capture(`ewd/checklist-menu-down-${i + 1}`, [EWD]);
  }
  await press('CL');
  await press('ABNPROC');
  await capture('ewd/abnormal-procedures', [EWD]);
  await press('ABNPROC');
  await press('RCL');
  await capture('ewd/recall', [EWD]);
}

const OIT_UNSAFE = /LOG ?OUT|DELETE|ERASE|CLEAR|RESET|SEND|CONFIRM|SHUT|POWER|REBOOT|DISCARD|REMOVE|SIGN/i;

async function oitClickables(v) {
  return v.ev(
    `(() => { ${HELPERS}; const out = []; let i = 0; for (const el of document.querySelectorAll('body *')) { if (!__TH.shown(el)) continue; const l = getEventListeners(el); if (!(l.click || l.mousedown || l.mouseup)) continue; const t = __TH.text(el); if (!t || t.length > 40) continue; out.push({ i: i, text: t }); el.dataset.tourId = String(i++); } return out; })()`,
    true,
  );
}

async function oitTour() {
  for (const v of OITS) {
    await v.ev(HELPERS);
    await capture(`oit/${v.name}/home`, [v]);
    await oitClickables(v);
    const loggedIn = await v.ev(`(() => { ${HELPERS}; const el = [...document.querySelectorAll('[data-tour-id]')].find((e) => __TH.text(e) === 'PILOT' && __TH.shown(e)); if (!el) return false; __TH.fire(el); return true; })()`);
    log(`OIT ${v.name}: login as PILOT ${loggedIn ? 'done' : 'not found'}`);
    await nap(1500);
    await capture(`oit/${v.name}/logged-in`, [v]);
    const top = (await oitClickables(v)).filter((c) => !OIT_UNSAFE.test(c.text));
    const seen = new Set(top.map((c) => c.text));
    log(`OIT ${v.name}: ${top.length} top-level controls`);
    for (const c of top) {
      const clicked = await v.ev(`(() => { ${HELPERS}; const el = [...document.querySelectorAll('[data-tour-id]')].find((e) => __TH.text(e) === ${JSON.stringify(c.text)} && __TH.shown(e)); if (!el) return false; __TH.fire(el); return true; })()`);
      if (!clicked) continue;
      await nap(1200);
      await capture(`oit/${v.name}/${c.text}`, [v]);
      const inner = (await oitClickables(v)).filter((x) => !OIT_UNSAFE.test(x.text) && !seen.has(x.text));
      for (const x of inner.slice(0, 25)) {
        const ok = await v.ev(`(() => { ${HELPERS}; const el = [...document.querySelectorAll('[data-tour-id]')].find((e) => __TH.text(e) === ${JSON.stringify(x.text)} && __TH.shown(e)); if (!el) return false; __TH.fire(el); return true; })()`);
        if (!ok) continue;
        await nap(1000);
        await capture(`oit/${v.name}/${c.text}/${x.text}`, [v]);
        await v.ev(`(() => { ${HELPERS}; const el = [...document.querySelectorAll('[data-tour-id]')].find((e) => __TH.text(e) === ${JSON.stringify(c.text)} && __TH.shown(e)); if (el) __TH.fire(el); return 1; })()`);
        await nap(800);
        await oitClickables(v);
      }
    }
  }
}

const started = Date.now();
try {
  await ANY.setVar('L:A380X_SWITCH_LAPTOP_POWER_LEFT', 1);
  await ANY.setVar('L:A380X_SWITCH_LAPTOP_POWER_RIGHT', 1);
  await ANY.setVar('L:A32NX_CONFIG_ADIRS_IR_ALIGN_TIME', 1);
  await ANY.setVar('L:A32NX_WXR_TAWS_SYS_SELECTED', 1);
  await ANY.setVar('L:A380X_WXR_OFF', 0);
  for (const side of ['L', 'R']) await ANY.setVar(`L:A380X_WXR_${side}_MODE_MAP`, 0);
  for (const n of [1, 2, 3]) {
    await ANY.setVar(`L:A32NX_OVHD_ADIRS_IR_${n}_MODE_SELECTOR_KNOB`, 1);
    await ANY.setVar(`L:A32NX_OVHD_ADIRS_IR_${n}_PB_IS_ON`, 1);
    await ANY.setVar(`L:A32NX_OVHD_ADIRS_ADR_${n}_PB_IS_ON`, 1);
  }
  await sleep(8000);
  log('WXR: system selected', await ANY.getVar('L:A32NX_WXR_TAWS_SYS_SELECTED'), 'failed 1/2', await ANY.getVar('L:A32NX_WXR_1_FAILED'), await ANY.getVar('L:A32NX_WXR_2_FAILED'));
  if (SECTIONS.includes('baseline')) await capture('baseline', 'all');
  if (SECTIONS.includes('uplink')) await uplink();
  const run = (name, f) => f().catch((e) => log(`${name} stopped:`, e.stack || e.message));
  const tasks = [];
  if (SECTIONS.includes('mfd')) tasks.push(run('mfd', mfdTour));
  if (SECTIONS.includes('revisions'))
    tasks.push(
      run('revisions', async () => {
        await MFD.ev(HELPERS);
        await selectPage('ACTIVE', 'F-PLN');
        await fplnRevisions('mfd/FMS/ACTIVE/F-PLN');
      }),
    );
  if (SECTIONS.includes('extra'))
    tasks.push(run('extra', async () => (await import('./extra.mjs')).default({ MFD, OITS, ANY, capture, log, nap, sleep, HELPERS, selectPage, mfdClickText, mfdClickLabel, mfdList, mfdClick, mfdTexts, enterField, typeKeys, revise, closeMenus, eraseTmpy, MENU_EL, OPEN_MENU_EXTRA, TAB })));
  if (SECTIONS.includes('nd')) tasks.push(run('nd', ndTour));
  if (SECTIONS.includes('sd')) tasks.push(run('sd', sdTour));
  if (SECTIONS.includes('ewd')) tasks.push(run('ewd', ewdTour));
  if (SECTIONS.includes('oit')) tasks.push(run('oit', oitTour));
  await Promise.all(tasks);
  if (SECTIONS.includes('nd') && ND1) await run('plan', () => planTour(ND1, 'L'));
  if (SECTIONS.includes('afs')) await run('afs', afsTour);
  if (SECTIONS.includes('fma')) await run('fma', fmaTour);
  if (SECTIONS.includes('states')) await run('states', statesTour);
  if (SECTIONS.includes('final')) await capture('final', 'all');
} catch (e) {
  log('tour stopped:', e.stack || e.message);
}
log(`done: ${manifest.length} captures in ${((Date.now() - started) / 60000).toFixed(1)} min`);
if (process.env.TOUR_STOP_FILE) fs.writeFileSync(process.env.TOUR_STOP_FILE, 'done');
process.exit(0);
