// usage: node run-xpjs.cjs <out.json>
// Runs probe-xpjs.tsx (the probe converted onto JS Library for X-Plane) against this
// library and its DATA half, with a fake XPHFBW host that logs in the probe's old
// vocabulary so results compare directly with run.cjs.
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FORK = 'E:/fbw-int/fbw-aircraft-ts';
const PRIVATE = 'E:/fbw-int/xplane-js-private';
const forkRequire = require('module').createRequire(path.join(FORK, 'package.json'));
const esbuild = forkRequire('esbuild');
const { JSDOM } = forkRequire('jsdom');

const outFile = process.argv[2];
if (!outFile) {
  console.error('usage: node run-xpjs.cjs <out.json>');
  process.exit(2);
}

const root = path.resolve(__dirname, '..');
const libEntry = path.join(root, 'lib/src/index.ts');
const dataEntry = path.join(root, 'data/src/index.ts');

const xplaneJs = {
  name: 'xplane-js',
  setup(build) {
    build.onResolve({ filter: /^xplane-js\/(ui|data)$/ }, (a) => ({ path: a.path.endsWith('/ui') ? libEntry : dataEntry }));
  },
};

async function bundle(entry, jsx) {
  try {
    return await esbuild.build({
      entryPoints: [entry],
      absWorkingDir: root,
      bundle: true,
      write: false,
      metafile: true,
      format: 'iife',
      platform: 'browser',
      target: 'es2020',
      ...(jsx ? { jsxFactory: 'UI.node', jsxFragment: 'UI.Group' } : {}),
      logLevel: 'silent',
      plugins: [xplaneJs],
      define: { 'process.env.NODE_ENV': '"production"' },
    });
  } catch (e) {
    for (const m of e.errors || []) {
      console.error(`build error: ${m.text} (${m.location ? `${path.basename(m.location.file)}:${m.location.line}` : 'no location'})`);
    }
    process.exit(1);
  }
}

function checkInputs(meta, what) {
  const inputs = Object.keys(meta.inputs).map((p) => path.resolve(root, p).split(path.sep).join('/').toLowerCase());
  const microsoft = inputs.filter((p) => p.includes('@microsoft') || p.includes('/xp-kit/') || p.includes('/xp-avionics/')).length;
  console.log(`${what}: ${inputs.length} inputs, ${microsoft} forbidden`);
  if (microsoft > 0) process.exit(1);
}

function reverseNames() {
  const map = JSON.parse(fs.readFileSync(path.join(PRIVATE, 'api-map.json'), 'utf8'));
  const traffic = JSON.parse(fs.readFileSync(path.join(PRIVATE, 'traffic-map.json'), 'utf8'));
  const exact = new Map(Object.entries(traffic.exact));
  const heads = [...traffic.prefixes];
  for (const s of map.strings) {
    if (s.piece === 'full' && s.new !== s.old && !exact.has(s.new)) exact.set(s.new, s.old);
    if (s.piece === 'head' && s.new) heads.push([s.new, s.old]);
  }
  const wire = new Map(map.wire.map((w) => [w.new, w.old]));
  const wireNew = new Map(map.wire.map((w) => [w.old, w.new]));
  const toOld = (name) => {
    const n = String(name);
    if (exact.has(n)) return exact.get(n);
    for (const [p, o] of heads) if (n.startsWith(p) && n.length > p.length) return o + n.slice(p.length);
    return n;
  };
  return { toOld, wire: (n) => wire.get(n) ?? n, wireNew: (n) => wireNew.get(n) ?? n };
}

function harness(names) {
  const dom = new JSDOM('<!doctype html><html><head></head><body></body></html>', { pretendToBeVisual: true });
  const w = dom.window;
  for (const k of ['window', 'document', 'Node', 'Element', 'HTMLElement', 'SVGElement', 'Text', 'Comment', 'DocumentFragment', 'Event', 'MouseEvent', 'KeyboardEvent', 'CustomEvent', 'DOMParser', 'getComputedStyle', 'customElements', 'HTMLDivElement', 'SVGSVGElement', 'NodeList', 'HTMLCollection', 'MutationObserver']) {
    if (w[k] !== undefined) globalThis[k] = k === 'getComputedStyle' ? w[k].bind(w) : w[k];
  }

  const realNow = Date.now.bind(Date);
  const realPerf = performance.now.bind(performance);
  const realSetTimeout = setTimeout;
  const clock = { real: Date.UTC(2026, 9, 21, 12, 0, 0), sim: 43200000, useReal: false };
  const now = () => (clock.useReal ? realNow() : clock.real);
  Date.now = now;
  w.Date.now = now;
  Object.defineProperty(performance, 'now', { configurable: true, value: () => (clock.useReal ? realPerf() : clock.real - Date.UTC(2026, 9, 21)) });
  globalThis.requestAnimationFrame = (cb) => realSetTimeout(() => cb(performance.now()), 16);
  globalThis.cancelAnimationFrame = (id) => clearTimeout(id);
  w.requestAnimationFrame = globalThis.requestAnimationFrame;

  const log = [];
  const sim = new Map();
  const reads = [];
  const queue = [];
  const shallow = (v) => {
    if (typeof v === 'string') return v;
    try {
      const s = JSON.stringify(v);
      return s && s.length > 160 ? `${s.slice(0, 160)}...` : s;
    } catch {
      return String(v);
    }
  };
  const simValue = (name, unit) => {
    reads.push([String(name), String(unit)]);
    const n = String(name);
    if (/^E:SIMULATION TIME/i.test(n)) return clock.sim / 1000;
    if (/^E:ABSOLUTE TIME/i.test(n)) return 63900000000 + clock.sim / 1000;
    if (/^E:ZULU TIME/i.test(n) || /^E:LOCAL TIME/i.test(n)) return (clock.sim / 1000) % 86400;
    if (/^E:ZULU (DAY|MONTH|YEAR)/i.test(n)) return { DAY: 21, MONTH: 10, YEAR: 2026 }[n.split(' ')[1].toUpperCase()];
    return sim.has(n) ? sim.get(n) : 0;
  };
  const write = (name, value) => {
    const old = names.toOld(name);
    log.push(['SimVar', 'SetSimVarValue', old, shallow(value)]);
    sim.set(old, value);
  };

  globalThis.__xphfbw = {
    view: 0,
    screen: 'PROBE',
    aircraftDir: '',
    snapshot() {},
    getVar: (name, unit) => {
      const v = simValue(names.toOld(name), unit);
      return typeof v === 'boolean' ? (v ? 1 : 0) : Number(v) || 0;
    },
    setVar: (name, unit, value) => write(name, value),
    getString: (name) => {
      const v = simValue(names.toOld(name), 'string');
      return typeof v === 'string' ? v : '';
    },
    setString: (name, value) => write(name, value),
    sendEvent: (name, values) => write(name, values[0]),
    call: (name, argsJson) => {
      let args = [];
      try {
        args = JSON.parse(argsJson);
      } catch {
        args = [argsJson];
      }
      const old = names.wire(name);
      if (old === 'GENERIC_DATA') log.push(['GenericDataListener', 'send', String(args[0]), shallow(args.slice(1))]);
      else log.push(['Coherent', 'call', String(old), shallow(args)]);
      return Promise.resolve('null');
    },
    poll: () => queue.splice(0, queue.length),
    gameString: () => '',
    storedData: () => '',
    readFile: () => null,
    log: () => {},
    magVar: () => 0,
    loaded: () => {},
  };

  const baseLog = [];
  const channelOf = { keyIntercepted: 'keys' };

  globalThis.__harness = {
    clock,
    log,
    sim,
    baseLog,
    realSetTimeout,
    reset() {
      clock.real = Date.UTC(2026, 9, 21, 12, 0, 0);
      clock.sim = 43200000;
      clock.useReal = false;
      log.length = 0;
      reads.length = 0;
    },
    resetSim(values) {
      sim.clear();
      reads.length = 0;
      for (const [k, v] of Object.entries(values)) sim.set(k, v);
    },
    readNames: () => Array.from(new Set(reads.map((r) => r[0]))).sort(),
    readUnits: () => Object.fromEntries(Array.from(new Set(reads.map((r) => `${r[0]}|${r[1]}`))).sort().map((k) => k.split('|'))),
    transport: () => log.filter((e) => e[0] !== 'SimVar').map((e) => e.slice(0, 3).join(':')),
    useRealTime(on) {
      clock.useReal = on;
    },
    emit(name, ...args) {
      const event = names.wireNew(name);
      if (channelOf[name]) queue.push(['provider', 'channel.event', JSON.stringify([channelOf[name], event, ...args])]);
      else queue.push(['provider', event, JSON.stringify(args)]);
    },
    frame: () => new Promise((r) => realSetTimeout(r, 60)),
  };
  return baseLog;
}

function mockBase(baseLog) {
  const data = globalThis.xplaneJsData;
  const labels = {
    onAttach: 'connectedCallback',
    onPrepare: 'Init',
    onFrame: 'Update',
    onPanelEvent: 'onInteractionEvent',
    onFlightBegin: 'onFlightStart',
    onSessionChange: 'onGameStateChanged',
    onSoundDone: 'onSoundEnd',
    onPowered: 'onPowerOn',
    onShutdown: 'onShutDown',
  };
  class PanelInstrument extends HTMLElement {
    constructor() {
      super();
      baseLog.push('base:ctor');
    }
    get templateId() {
      return '';
    }
    get interactive() {
      return false;
    }
  }
  for (const [m, label] of Object.entries(labels)) {
    PanelInstrument.prototype[m] = function () {
      baseLog.push(`base:${label}`);
    };
  }
  data.PanelInstrument = PanelInstrument;
}

(async () => {
  const names = reverseNames();
  const runtime = await bundle(path.join(root, 'data/src/runtime.ts'), false);
  const probe = await bundle(path.join(__dirname, 'probe-xpjs.tsx'), true);
  checkInputs(runtime.metafile, 'runtime');
  checkInputs(probe.metafile, 'probe');
  const baseLog = harness(names);
  globalThis.__harness.baseLog = baseLog;
  const out = { mode: 'xpjs', loadError: null, results: null };
  try {
    vm.runInThisContext(runtime.outputFiles[0].text, { filename: 'xplane-js-data.js' });
    mockBase(baseLog);
    vm.runInThisContext(probe.outputFiles[0].text, { filename: 'probe-xpjs.js' });
  } catch (e) {
    out.loadError = String(e && e.message ? e.message : e);
  }
  if (!out.loadError) out.results = await globalThis.__runProbe();
  fs.writeFileSync(outFile, JSON.stringify(out, null, 1));
  const ids = out.results ? Object.keys(out.results) : [];
  const errs = ids.filter((id) => out.results[id] && out.results[id].error);
  console.log(`xpjs: ${ids.length} items, ${errs.length} threw${out.loadError ? `, load error: ${out.loadError}` : ''}`);
  process.exit(0);
})();
