(() => {
  if (window.__TOUR) return;
  const hostView = /\/(SystemsHost|ExtrasHost)\//.test(location.href);
  const T = { frozen: false, frames: 0, loadedAt: Date.now() };
  window.__TOUR = T;
  if (window.customElements) {
    const realDefine = window.customElements.define.bind(window.customElements);
    window.customElements.define = (name, ctor, opts) => {
      const cc = ctor.prototype.connectedCallback;
      ctor.prototype.connectedCallback = function (...a) {
        if (T.connectedAt === undefined) T.connectedAt = { date: Date.now(), perf: performance.now() };
        return cc ? cc.apply(this, a) : undefined;
      };
      return realDefine(name, ctor, opts);
    };
  }
  const realRaf = window.requestAnimationFrame.bind(window);
  const realSetTimeout = window.setTimeout.bind(window);
  const realSetInterval = window.setInterval.bind(window);
  const heldRaf = [];
  const heldTimers = [];
  if (!hostView) {
    window.requestAnimationFrame = (cb) => (T.frozen ? (heldRaf.push(cb), 0) : realRaf(cb));
    window.setTimeout = (cb, ms, ...a) => (T.frozen ? (heldTimers.push(() => cb(...a)), 0) : realSetTimeout(cb, ms, ...a));
    window.setInterval = (cb, ms, ...a) => realSetInterval(() => { if (!T.frozen) cb(...a); }, ms);
  }
  const vars = new Map();
  const strs = new Map();
  const game = new Map();
  const provider = new Map();
  const bus = new Map();
  const generic = new Map();
  const fetched = new Map();
  const realFetch = window.fetch && window.fetch.bind(window);
  if (realFetch) {
    window.fetch = (resource, init) => {
      const url = typeof resource === 'string' ? resource : resource && resource.url !== undefined ? resource.url : String(resource);
      return realFetch(resource, init).then((r) => {
        try {
          r.clone().text().then((body) => fetched.set(url, { status: r.status, body: body.length > 4000000 ? body.slice(0, 4000000) : body, frame: T.frames }), () => {});
        } catch {}
        return r;
      });
    };
  }
  const calls = new Map();
  const stored = new Map();
  const files = new Map();
  const magvar = new Map();
  const writes = [];
  const clone = (v) => {
    try {
      return v === undefined ? null : JSON.parse(JSON.stringify(v));
    } catch {
      return String(v);
    }
  };
  T.freeze = () => {
    if (hostView) return false;
    T.frozen = true;
    clearTimeout(T.autoThaw);
    T.autoThaw = realSetTimeout(() => T.thaw(), 30000);
    T.frozenAt = { date: Date.now(), perf: performance.now(), frames: T.frames };
    for (const an of document.getAnimations()) {
      try {
        an.pause();
      } catch {}
    }
    return true;
  };
  T.thaw = () => {
    clearTimeout(T.autoThaw);
    if (!T.frozen) return;
    T.frozen = false;
    for (const an of document.getAnimations()) {
      try {
        an.play();
      } catch {}
    }
    for (const cb of heldTimers.splice(0)) realSetTimeout(cb, 0);
    for (const cb of heldRaf.splice(0)) realRaf(cb);
  };
  T.state = () =>
    JSON.stringify({
      href: location.href,
      env: Object.assign({}, T.env, { connectedAt: T.connectedAt }, T.extra || {}),
      frozenAt: T.frozenAt || null,
      frames: T.frames,
      vars: Object.fromEntries(vars),
      strings: Object.fromEntries(strs),
      game: Object.fromEntries(game),
      providerEvents: Object.fromEntries(provider),
      bus: Object.fromEntries(bus),
      genericData: Object.fromEntries(generic),
      fetched: Object.fromEntries(fetched),
      calls: [...calls.values()],
      stored: Object.fromEntries(stored),
      files: Object.fromEntries(files),
      magvar: [...magvar.values()],
      writes: writes.slice(-500),
      animations: document.getAnimations().map((a) => ({ name: a.animationName || (a.effect && a.effect.getKeyframes ? 'keyframes' : ''), time: a.currentTime })),
    });
  const wrap = (n) => {
    if (!n || n.__tour) return;
    const o = {};
    for (const k of ['snapshot', 'getVar', 'setVar', 'getString', 'setString', 'sendEvent', 'call', 'poll', 'gameString', 'storedData', 'readFile', 'magVar']) o[k] = n[k];
    T.env = { view: n.view, screen: n.screen, dpr: window.devicePixelRatio, w: window.innerWidth, h: window.innerHeight };
    const def = (k, f) => {
      if (typeof o[k] !== 'function') return;
      try {
        Object.defineProperty(n, k, { value: f, writable: true, configurable: true });
      } catch {
        n[k] = f;
      }
    };
    def('snapshot', (...a) => {
      if (!T.frozen) T.frames++;
      return o.snapshot.apply(n, a);
    });
    def('getVar', (name, unit) => {
      const v = o.getVar.call(n, name, unit);
      vars.set(name + '|' + unit, v);
      return v;
    });
    def('getString', (name) => {
      const v = o.getString.call(n, name);
      strs.set(name, v);
      return v;
    });
    def('gameString', (name) => {
      const v = o.gameString.call(n, name);
      game.set(name, v);
      return v;
    });
    def('poll', (...a) => {
      const items = o.poll.apply(n, a);
      if (Array.isArray(items)) {
        for (const it of items) {
          if (!Array.isArray(it) || it[0] !== 'provider') continue;
          const [, name, payload] = it;
          let key = String(name);
          try {
            const args = JSON.parse(payload);
            if (Array.isArray(args)) {
              if (typeof args[0] === 'string' && args[0].length < 120) key += '|' + args[0];
              if (typeof args[1] === 'string' && args[1].length < 120) key += '|' + args[1];
            }
          } catch {}
          provider.set(key, { name, payload, frame: T.frames });
          try {
            const args = JSON.parse(payload);
            if (name === 'EVENT_FROM_VIEW_LISTENER' && args[0] === 'JS_LISTENER_GENERICDATA' && typeof args[1] === 'string') {
              if (args[1] === 'xp-ui.bus') {
                const batch = typeof args[2] === 'string' ? JSON.parse(args[2]) : args[2];
                for (const e of (batch && batch.events) || []) bus.set(String(e.topic), { data: e.data, cached: !!e.cached, origin: e.origin, frame: T.frames });
              } else {
                generic.set(args[1], { data: args[2], frame: T.frames });
                if (args[1] === 'EB_EVENTS') {
                  const pack = typeof args[2] === 'string' ? JSON.parse(args[2]) : args[2];
                  for (const e of (pack && pack.data) || []) generic.set(String(e.topic), { data: e.data, frame: T.frames });
                }
              }
            }
          } catch {}
        }
      }
      return items;
    });
    def('call', (name, args) => {
      const key = name + '|' + (typeof args === 'string' ? args : JSON.stringify(args));
      const rec = { name, args: clone(args), ok: null, result: null, frame: T.frames };
      const p = o.call.call(n, name, args);
      return Promise.resolve(p).then(
        (r) => {
          rec.ok = true;
          rec.result = clone(r);
          calls.set(key, rec);
          return r;
        },
        (e) => {
          rec.ok = false;
          rec.result = String(e && e.message);
          calls.set(key, rec);
          throw e;
        },
      );
    });
    def('storedData', (op, key, value) => {
      const r = o.storedData.call(n, op, key, value);
      if (op === 'get' || op === 'set') stored.set(String(key), clone(op === 'set' ? value : r));
      return r;
    });
    def('readFile', (path) => {
      const r = o.readFile.call(n, path);
      files.set(String(path), r);
      return r;
    });
    def('magVar', (lat, lon) => {
      const r = o.magVar.call(n, lat, lon);
      magvar.set(lat + ',' + lon, [lat, lon, r]);
      return r;
    });
    for (const k of ['setVar', 'setString', 'sendEvent']) {
      def(k, (...a) => {
        writes.push({ frame: T.frames, k, a: clone(a) });
        if (writes.length > 2000) writes.splice(0, 1000);
        return o[k].apply(n, a);
      });
    }
    def('__tour', true);
  };
  if (window.__xphfbw) wrap(window.__xphfbw);
  else {
    let real;
    Object.defineProperty(window, '__xphfbw', {
      configurable: true,
      get: () => real,
      set: (v) => {
        wrap(v);
        real = v;
      },
    });
  }
})();
