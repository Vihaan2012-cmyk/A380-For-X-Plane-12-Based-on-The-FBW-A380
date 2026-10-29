// The host object XPHFBW installs in every view (window.__xphfbw), served from
// a reference state instead of the plugin, for rendering JS Library for X-Plane
// builds in the harness.
(function () {
  'use strict';
  const cfg = window.__REFERENCE__ || {};
  const state = (cfg.state && cfg.state.vars) || {};
  const table = cfg.names || { exact: {}, prefixes: [], wire: {} };
  const convert = (v, from, to) => window.__refUnits.convert(v, from, to).value;

  const norm = (name) => {
    let s = String(name).trim();
    const m = s.match(/^([A-Za-z]{1,4}):(.*)$/);
    let prefix = 'A';
    if (m && (m[1].length === 1 || m[1].toUpperCase() === 'GAME')) {
      prefix = m[1].toUpperCase();
      s = m[2];
    }
    if (prefix === 'A' || prefix === 'E') s = s.toUpperCase().replace(/\s+/g, ' ');
    return `${prefix}:${s}`;
  };
  const internal = (name) => {
    if (Object.prototype.hasOwnProperty.call(table.exact, name)) return table.exact[name];
    for (const [p, i] of table.prefixes) {
      if (name.startsWith(p)) {
        const rest = name.slice(p.length);
        if (rest && /^\d+$/.test(rest)) return i + rest;
      }
    }
    return name;
  };

  const CLOCKS = new Set(['E:SIMULATION TIME', 'E:ZULU TIME', 'E:LOCAL TIME', 'E:ABSOLUTE TIME']);
  const values = new Map();
  for (const [k, v] of Object.entries(state)) values.set(k, { value: v.value, unit: v.unit });
  for (const pair of (cfg.setVars || '').split(',').filter(Boolean)) {
    const [k, v] = pair.split('=');
    values.set(norm(k), { value: Number(v), unit: 'number' });
  }
  const strings = new Map();
  const reads = new Map();
  const errors = [];
  const events = [];
  const queue = [];
  let loadedCount = 0;

  const lookup = (name) => {
    const key = norm(internal(name));
    return { key, entry: values.get(key) };
  };

  window.__xphfbw = {
    view: 0,
    screen: (cfg.screen && cfg.screen.name) || '',
    aircraftDir: '',
    snapshot() {},
    getVar(name, unit) {
      const { key, entry } = lookup(name);
      const r = reads.get(key + '|' + unit) || { key, units: unit, found: !!entry, count: 0 };
      r.count++;
      reads.set(key + '|' + unit, r);
      if (!entry) return 0;
      let v = Number(entry.value);
      if (CLOCKS.has(key) && cfg.state.time && typeof cfg.state.time.epochMs === 'number') v += (Date.now() - cfg.state.time.epochMs) / 1000;
      return convert(Number.isFinite(v) ? v : 0, entry.unit, unit);
    },
    setVar(name, unit, value) {
      const { key, entry } = lookup(name);
      const stored = entry ? convert(value, unit, entry.unit) : value;
      values.set(key, { value: stored, unit: entry ? entry.unit : unit });
    },
    getString(name) {
      const { key, entry } = lookup(name);
      if (strings.has(key)) return strings.get(key);
      return entry && typeof entry.value === 'string' ? entry.value : '';
    },
    setString(name, value) {
      const { key } = lookup(name);
      const text = String(value);
      strings.set(key, text);
      if (key.startsWith('L:')) {
        const t = text.trim();
        const n = t === '' ? 0 : Number(t);
        if (t === '' || (Number.isFinite(n) && /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(t))) values.set(key, { value: n, unit: 'number' });
      }
    },
    sendEvent(name, list) {
      events.push([name, list]);
    },
    call(name, argsJson) {
      const wire = table.wire[name] || name;
      if (wire === 'GENERIC_DATA') {
        let args = [];
        try {
          args = JSON.parse(argsJson);
        } catch {
          args = [];
        }
        queue.push(['provider', 'channel.event', JSON.stringify(['relay', args[0], args[1]])]);
        if (args[0] === 'xp-ui.bus' && String(args[1]).startsWith('{"events":[{"topic":"a380.mfd.dom"')) window.__refMfdDom = JSON.parse(args[1]).events[0].data;
      }
      // Unanswered, as in the VCockpit host (no navdata, weather or traffic in the reference).
      return Promise.resolve(undefined);
    },
    poll() {
      return queue.splice(0, queue.length);
    },
    gameString(name) {
      const { entry } = lookup(internal(name).startsWith('GAME:') ? internal(name) : 'GAME:' + internal(name));
      return entry && entry.value !== undefined ? String(entry.value) : '';
    },
    storedData(op, key, value) {
      const store = (window.__xpjsStore = window.__xpjsStore || new Map(Object.entries((cfg.state && cfg.state.storage) || {}).map(([k, v]) => [k, String(v)])));
      if (op === 'get') return store.has(key) ? store.get(key) : '';
      if (op === 'set') store.set(key, String(value));
      if (op === 'delete') store.delete(key);
      return '';
    },
    readFile() {
      return null;
    },
    log(level, text) {
      if (level >= 2) errors.push(String(text));
    },
    magVar() {
      return 0;
    },
    loaded(ok, text) {
      if (!ok) errors.push(`load failed: ${text}`);
      loadedCount++;
      const want = (window.__xpjsGauges || []).length;
      if (want && loadedCount >= want) window.__refPanelLoaded = true;
    },
  };

  window.__refAccessLog = () => ({ reads: [...reads.values()], errors, events: events.length, storage: [], storageWrites: [] });
})();
