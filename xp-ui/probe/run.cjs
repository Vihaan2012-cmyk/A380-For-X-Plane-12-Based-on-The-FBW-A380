// usage: node run.cjs <new|old> <out.json>
// new: XP_UI (default ../lib); old: XP_KIT must point at the old kit's dist folder.
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FORK = 'E:/fbw-int/fbw-aircraft-ts';
const forkRequire = require('module').createRequire(path.join(FORK, 'package.json'));
const esbuild = forkRequire('esbuild');
const { JSDOM } = forkRequire('jsdom');

const [mode, outFile] = process.argv.slice(2);
if (!['new', 'old'].includes(mode) || !outFile) {
  console.error('usage: node run.cjs <new|old> <out.json>');
  process.exit(2);
}

const utils = forkRequire(path.join(FORK, 'build-utils.js'));
let plugin;
let libRoot;
if (mode === 'new') {
  delete process.env.XP_KIT;
  process.env.XP_UI = process.env.XP_UI || path.resolve(__dirname, '..', 'lib');
  libRoot = process.env.XP_UI;
  plugin = utils.xpUiPlugin();
} else {
  delete process.env.XP_UI;
  if (!process.env.XP_KIT) {
    console.error('run.cjs old: set XP_KIT');
    process.exit(2);
  }
  libRoot = path.resolve(process.env.XP_KIT, '..');
  plugin = utils.xpKitPlugin();
}

const probeDir = path.join(FORK, 'fbw-a380x/src/systems/instruments/src/KitProbe');
const probeFile = path.join(probeDir, 'probe.tsx');

async function build() {
  fs.mkdirSync(probeDir, { recursive: true });
  fs.copyFileSync(path.join(__dirname, 'probe.tsx'), probeFile);
  try {
    return await esbuild.build({
      entryPoints: [probeFile],
      absWorkingDir: FORK,
      bundle: true,
      write: false,
      metafile: true,
      format: 'iife',
      platform: 'browser',
      target: 'es2020',
      jsxFactory: 'FSComponent.buildComponent',
      jsxFragment: 'FSComponent.Fragment',
      logLevel: 'silent',
      plugins: [plugin],
      define: { 'process.env.NODE_ENV': '"production"' },
    });
  } catch (e) {
    for (const m of e.errors || []) {
      console.error(`build error: ${m.text} (${m.location ? `${path.basename(m.location.file)}:${m.location.line}` : 'no location'})`);
    }
    process.exit(1);
  } finally {
    fs.rmSync(probeDir, { recursive: true, force: true });
  }
}

function checkInputs(meta) {
  const inputs = Object.keys(meta.inputs).map((p) => path.resolve(FORK, p).split(path.sep).join('/').toLowerCase());
  const root = path.resolve(libRoot).split(path.sep).join('/').toLowerCase();
  const fromLib = inputs.filter((p) => p.startsWith(root)).length;
  const microsoft = inputs.filter((p) => p.includes('@microsoft')).length;
  console.log(`inputs: ${inputs.length} total, ${fromLib} from the ${mode} library, ${microsoft} from @microsoft`);
  if (microsoft > 0 || fromLib === 0) {
    console.error('run.cjs: the probe did not bundle the intended library; refusing to run it');
    process.exit(1);
  }
}

function harness() {
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
  const ids = new Map();
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
  const simvarTarget = {
    GetSimVarValue: (name, unit) => simValue(name, unit),
    SetSimVarValue: (name, unit, value) => {
      log.push(['SimVar', 'SetSimVarValue', String(name), shallow(value)]);
      sim.set(String(name), value);
      return Promise.resolve();
    },
    GetGlobalVarValue: (name, unit) => simValue(name, unit),
    GetGameVarValue: (name, unit) => simValue(name, unit),
    GetRegisteredId: (name, unit) => {
      const key = `${name}|${unit}`;
      if (!ids.has(key)) ids.set(key, ids.size + 1);
      return ids.get(key);
    },
  };
  const byId = () => new Map(Array.from(ids.entries()).map(([k, v]) => [v, k.split('|')]));
  const simvar = new Proxy(simvarTarget, {
    get(target, prop) {
      if (prop in target) return target[prop];
      if (typeof prop !== 'string') return undefined;
      return (...args) => {
        if (/^Get.*Reg/.test(prop)) {
          const [name, unit] = byId().get(args[0]) || [`#${args[0]}`, ''];
          return simValue(name, unit);
        }
        if (prop.startsWith('Get')) return simValue(args[0], args[1]);
        log.push(['SimVar', prop, shallow(args[0])]);
        return Promise.resolve();
      };
    },
  });
  globalThis.SimVar = simvar;
  globalThis.simvar = simvar;

  const coherentHandlers = new Map();
  globalThis.Coherent = {
    call: (name, ...args) => {
      log.push(['Coherent', 'call', String(name), shallow(args)]);
      return Promise.resolve(undefined);
    },
    trigger: (name, ...args) => {
      log.push(['Coherent', 'trigger', String(name), shallow(args)]);
    },
    on: (name, fn) => {
      log.push(['Coherent', 'on', String(name)]);
      if (!coherentHandlers.has(name)) coherentHandlers.set(name, []);
      coherentHandlers.get(name).push(fn);
      return { clear: () => log.push(['Coherent', 'clear', String(name)]) };
    },
    off: (name) => log.push(['Coherent', 'off', String(name)]),
  };
  class MockBroadcastChannel {
    constructor(name) {
      this.name = name;
      log.push(['BroadcastChannel', 'new', String(name)]);
    }
    postMessage(m) {
      log.push(['BroadcastChannel', 'postMessage', this.name, shallow(m)]);
    }
    addEventListener() {}
    removeEventListener() {}
    close() {}
  }
  globalThis.BroadcastChannel = MockBroadcastChannel;
  globalThis.RegisterViewListener = (name, cb) => {
    log.push(['RegisterViewListener', 'register', String(name)]);
    if (cb) realSetTimeout(() => cb(), 0);
    return {
      on: (ev, fn) => {
        log.push(['ViewListener', 'on', String(ev)]);
        if (!coherentHandlers.has(ev)) coherentHandlers.set(ev, []);
        if (fn) coherentHandlers.get(ev).push(fn);
      },
      off: () => {},
      call: (ev, ...args) => {
        log.push(['ViewListener', 'call', String(ev), shallow(args)]);
        return Promise.resolve(undefined);
      },
      trigger: (ev, ...args) => log.push(['ViewListener', 'trigger', String(ev), shallow(args)]),
      unregister: () => {},
    };
  };
  globalThis.RegisterGenericDataListener = (cb) => {
    log.push(['GenericDataListener', 'register', '']);
    if (cb) realSetTimeout(() => cb(), 0);
    return {
      on: (ev) => log.push(['GenericDataListener', 'on', String(ev)]),
      off: () => {},
      send: (ev, ...args) => log.push(['GenericDataListener', 'send', String(ev), shallow(args)]),
      call: (ev, ...args) => {
        log.push(['GenericDataListener', 'call', String(ev), shallow(args)]);
        return Promise.resolve(undefined);
      },
      trigger: (ev, ...args) => log.push(['GenericDataListener', 'trigger', String(ev), shallow(args)]),
      unregister: () => {},
    };
  };
  globalThis.GameState = { mainmenu: 0, loading: 1, briefing: 2, ingame: 3 };
  globalThis.LaunchFlowEventToGlobalFlow = (...a) => log.push(['global', 'LaunchFlowEventToGlobalFlow', shallow(a)]);
  globalThis.Name_Z = class {
    constructor(s) {
      this.str = s;
    }
  };

  const baseLog = [];
  globalThis.BaseInstrument = class {
    constructor() {
      baseLog.push('base:ctor');
    }
    connectedCallback() {
      baseLog.push('base:connectedCallback');
    }
    disconnectedCallback() {
      baseLog.push('base:disconnectedCallback');
    }
    Init() {
      baseLog.push('base:Init');
    }
    Update() {
      baseLog.push('base:Update');
    }
    onInteractionEvent() {
      baseLog.push('base:onInteractionEvent');
    }
    onFlightStart() {
      baseLog.push('base:onFlightStart');
    }
    onGameStateChanged() {
      baseLog.push('base:onGameStateChanged');
    }
    onSoundEnd() {
      baseLog.push('base:onSoundEnd');
    }
    onPowerOn() {
      baseLog.push('base:onPowerOn');
    }
    onShutDown() {
      baseLog.push('base:onShutDown');
    }
    getChildById() {
      return null;
    }
    get instrumentIdentifier() {
      return 'PROBE';
    }
    get isInteractive() {
      return false;
    }
    get IsGlassCockpit() {
      return true;
    }
  };
  globalThis.registerInstrument = (name) => log.push(['global', 'registerInstrument', String(name)]);

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
      for (const fn of coherentHandlers.get(name) || []) fn(...args);
    },
  };
}

(async () => {
  const result = await build();
  checkInputs(result.metafile);
  harness();
  const code = result.outputFiles[0].text;
  const out = { mode, loadError: null, results: null };
  try {
    vm.runInThisContext(code, { filename: `probe-${mode}.js` });
  } catch (e) {
    out.loadError = String(e && e.message ? e.message : e);
  }
  if (!out.loadError) {
    out.results = await globalThis.__runProbe();
  }
  fs.writeFileSync(outFile, JSON.stringify(out, null, 1));
  const ids = out.results ? Object.keys(out.results) : [];
  const errs = ids.filter((id) => out.results[id] && out.results[id].error);
  console.log(`${mode}: ${ids.length} items, ${errs.length} threw${out.loadError ? `, load error: ${out.loadError}` : ''}`);
  process.exit(0);
})();
