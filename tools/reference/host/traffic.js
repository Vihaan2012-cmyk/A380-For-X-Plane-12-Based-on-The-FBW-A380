// Variable traffic of a render: every read and write the screens make
// (name, unit, value, order), through either runtime, cut into frames at each
// __refStep. Names of the new runtime are mapped to the old ones first, so the
// two logs compare directly.
(function () {
  'use strict';
  const cfg = window.__REFERENCE__ || {};
  const back = cfg.trafficMap || { exact: {}, prefixes: [] };
  const toOld = (name) => {
    if (typeof name !== 'string') return name;
    if (Object.prototype.hasOwnProperty.call(back.exact, name)) return back.exact[name];
    for (const [p, o] of back.prefixes) {
      if (name.startsWith(p)) {
        const rest = name.slice(p.length);
        if (rest && /^\d+$/.test(rest)) return o + rest;
      }
    }
    return name;
  };
  const repr = (v) => {
    if (typeof v === 'number') return Number.isNaN(v) ? 'NaN' : String(v);
    if (v && typeof v === 'object') {
      try {
        return JSON.stringify(v);
      } catch {
        return String(v);
      }
    }
    return String(v);
  };
  const frames = [];
  let cur = [];
  let depth = 0;
  const keep = new Set((cfg.trafficKeep || [0, 1, 2]).map(Number));
  const kept = {};
  const hash = (lines) => {
    let h1 = 0x811c9dc5 | 0;
    let h2 = 0x01000193 | 0;
    for (const l of lines) {
      for (let i = 0; i < l.length; i++) {
        const c = l.charCodeAt(i);
        h1 = Math.imul(h1 ^ c, 16777619);
        h2 = Math.imul(h2 ^ c, 2246822519);
      }
      h1 = Math.imul(h1 ^ 10, 16777619);
    }
    return (h1 >>> 0).toString(16).padStart(8, '0') + (h2 >>> 0).toString(16).padStart(8, '0');
  };
  const close = () => {
    const i = frames.length;
    frames.push({ n: cur.length, hash: hash(cur) });
    if (keep.has(i) || keep.has(-1)) kept[i] = cur;
    cur = [];
  };
  const log = (op, name, unit, value) => cur.push(`${op}\t${toOld(name)}\t${unit}\t${repr(value)}`);

  const fromBundle = () => {
    const stack = new Error().stack || '';
    for (const line of stack.split(String.fromCharCode(10)).slice(1)) {
      const m = line.match(/https?:\/\/[^/\s)]+(\/[^\s):]*)/);
      if (!m) continue;
      return m[1].startsWith('/Pages/');
    }
    return false;
  };
  const ids = new Map();
  const wrap = (obj, key, op, kind) => {
    const f = obj && obj[key];
    if (typeof f !== 'function' || f.__traffic) return;
    const w = make(f, op, kind);
    let current = w;
    Object.defineProperty(obj, key, {
      configurable: true,
      enumerable: true,
      get: () => current,
      set: (v) => {
        current = typeof v === 'function' && !v.__traffic ? make(v, op, kind) : v;
      },
    });
  };
  const make = (f, op, kind) => {
    const w = function (...a) {
      depth++;
      let r;
      try {
        r = f.apply(this, a);
      } finally {
        depth--;
      }
      if (cfg.dbgName && new RegExp(cfg.dbgName).test(String(a[0])) && frames.length < 1) console.log('DBG', frames.length, depth, fromBundle(), (new Error().stack || '').split(String.fromCharCode(10)).slice(2, 7).map((s) => s.trim().replace(/https?:\/\/[^/]+/, '')).join(' | '));
      if (depth === 0 && fromBundle()) {
        if (kind === 'register') ids.set(r, [a[0], a[1]]);
        else if (kind === 'id') {
          const [n, u] = ids.get(a[0]) || ['#' + a[0], ''];
          log(op, n, op === 't' ? 'string' : u, r);
        } else if (op === 'w') log(op, a[0], a[1], a[2]);
        else log(op, a[0], a[1], r);
      }
      return r;
    };
    w.__traffic = true;
    return w;
  };
  const wrapOld = (s) => {
    wrap(s, 'GetSimVarValue', 'r');
    wrap(s, 'GetSimVarValueFast', 'r');
    wrap(s, 'GetGlobalVarValue', 'r');
    wrap(s, 'GetGameVarValue', 'h');
    wrap(s, 'SetSimVarValue', 'w');
    wrap(s, 'GetRegisteredId', '', 'register');
    wrap(s, 'GetSimVarValueFastReg', 'r', 'id');
    wrap(s, 'GetSimVarValueFastRegString', 't', 'id');
  };
  const wrapNew = (d) => {
    const v = d && d.Vars;
    wrap(v, 'read', 'r');
    wrap(v, 'readNow', 'r');
    wrap(v, 'readEnv', 'r');
    wrap(v, 'readHost', 'h');
    wrap(v, 'write', 'w');
    wrap(v, 'register', '', 'register');
    wrap(v, 'readById', 'r', 'id');
    wrap(v, 'readTextById', 't', 'id');
  };
  for (const [name, fn] of [['SimVar', wrapOld], ['xplaneJsData', wrapNew]]) {
    let held;
    Object.defineProperty(window, name, {
      configurable: true,
      get: () => held,
      set: (v) => {
        held = v;
        fn(v);
      },
    });
  }

  const hook = () => {
    const step = window.__refStep;
    if (!step || step.__traffic) return;
    const w = async (ms) => {
      close();
      return step(ms);
    };
    w.__traffic = true;
    window.__refStep = w;
  };
  hook();
  const rand = { bundle: 0, other: 0, firstOther: [] };
  const rnd = Math.random;
  Math.random = function () {
    const stack = new Error().stack || '';
    let who = 'other';
    for (const line of stack.split(String.fromCharCode(10)).slice(1)) {
      const m = line.match(/https?:\/\/[^/\s)]+(\/[^\s):]*)/);
      if (!m) continue;
      who = m[1].startsWith('/Pages/') ? 'bundle' : 'other';
      if (who === 'other' && rand.firstOther.length < 5) rand.firstOther.push(m[1] + ' @' + (rand.bundle + rand.other));
      if (cfg.dbgName === 'RANDOM' && rand.bundle + rand.other < 200) console.log('RND', frames.length, rand.bundle + rand.other, stack.split(String.fromCharCode(10)).slice(2, 5).map((s) => s.trim().replace(/https?:\/\/[^/]+\/Pages\/VCockpit\/Instruments\/A380X\//, '').replace(/:\d+:\d+\)?$/, '')).join(' < '));
      break;
    }
    rand[who]++;
    return rnd.call(Math);
  };
  window.__refRandom = () => rand;
  window.__refTrafficWrapOld = () => {
    try {
      wrapOld(SimVar);
    } catch {
      return;
    }
  };
  window.__refTraffic = () => {
    close();
    if (window.SimVar) wrapOld(window.SimVar);
    return { frames, kept };
  };
})();
