"use strict";
(() => {
  // ../clean-avionics/data/src/host.ts
  function globalHost() {
    const h = globalThis.__xphfbw;
    return h == null ? void 0 : h;
  }
  function describeError(e) {
    if (e instanceof Error) {
      return e.stack ? e.message + "\n" + e.stack : e.message;
    }
    if (e && typeof e === "object" && "message" in e) {
      const o = e;
      return typeof o.stack === "string" ? String(o.message) + "\n" + o.stack : String(o.message);
    }
    return String(e);
  }
  function errorMessage(e) {
    if (e instanceof Error) {
      return e.message;
    }
    if (e && typeof e === "object" && "message" in e) {
      return String(e.message);
    }
    return String(e);
  }
  function requestFrame(cb) {
    const g2 = globalThis;
    if (typeof g2.requestAnimationFrame !== "function") {
      return false;
    }
    g2.requestAnimationFrame(cb);
    return true;
  }
  function frameTime(time) {
    if (typeof time === "number" && Number.isFinite(time)) {
      return time;
    }
    return clockNow();
  }
  function clockNow() {
    const g2 = globalThis;
    return g2.performance && typeof g2.performance.now === "function" ? g2.performance.now() : Date.now();
  }

  // ../clean-avionics/data/src/types.ts
  function makeEnum(values) {
    const out = {};
    for (const key of Object.keys(values)) {
      const value = values[key];
      out[key] = value;
      out[String(value)] = key;
    }
    return out;
  }
  var SessionPhase = makeEnum({
    mainmenu: 0,
    loading: 1,
    briefing: 2,
    flying: 3
  });
  function sessionPhase() {
    return SessionPhase.flying;
  }
  var AircraftModel = makeEnum({
    CJ4: 0,
    A320: 1,
    B747_8: 2,
    AS01B: 3,
    AS02A: 4
  });
  var KeyCodes = makeEnum({
    KEY_CANCEL: 3,
    KEY_HELP: 6,
    KEY_BACK_SPACE: 8,
    KEY_TAB: 9,
    KEY_CLEAR: 12,
    KEY_RETURN: 13,
    KEY_ENTER: 13,
    KEY_SHIFT: 16,
    KEY_CONTROL: 17,
    KEY_ALT: 18,
    KEY_PAUSE: 19,
    KEY_CAPS_LOCK: 20,
    KEY_ESCAPE: 27,
    KEY_SPACE: 32,
    KEY_PAGE_UP: 33,
    KEY_PAGE_DOWN: 34,
    KEY_END: 35,
    KEY_HOME: 36,
    KEY_LEFT: 37,
    KEY_UP: 38,
    KEY_RIGHT: 39,
    KEY_DOWN: 40,
    KEY_DELETE: 46,
    KEY_0: 48,
    KEY_9: 57,
    KEY_A: 65,
    KEY_P: 80,
    KEY_Z: 90
  });
  var InputButtons = { MENU_A: "MENU_VALID" };
  var SoundId = class {
    constructor(value = "") {
      this.str = value;
      this.value = 0;
    }
  };
  var Noop = class {
  };
  Noop.Void = () => {
  };
  Noop.Boolean = (_b) => {
  };
  var GeoPosData = class {
    constructor(lat = 0, long = 0) {
      this.lat = lat;
      this.long = long;
    }
  };
  var GeoPos = class extends GeoPosData {
    constructor(lat, long) {
      if (lat !== null && typeof lat === "object") {
        super(lat.lat, lat.long);
      } else {
        super(lat, long);
      }
    }
  };
  var GeoPos3 = class {
    constructor(lat, long, alt = 0) {
      if (lat !== null && typeof lat === "object") {
        this.lat = lat.lat;
        this.long = lat.long;
        this.alt = lat.alt !== void 0 ? lat.alt : 0;
      } else {
        this.lat = lat === void 0 ? 0 : lat;
        this.long = long === void 0 ? 0 : long;
        this.alt = alt;
      }
    }
    toLatLong() {
      return new GeoPos(this.lat, this.long);
    }
  };
  var Vec3Data = class {
    constructor() {
      this.x = 0;
      this.y = 0;
      this.z = 0;
    }
  };
  var MapConfig = class {
    load(_path, callback) {
      Promise.resolve().then(() => callback());
    }
    applyTo(_map) {
    }
  };

  // ../clean-avionics/data/src/vars.ts
  function sameValue(a, b) {
    return a === b || a !== a && b !== b;
  }
  function createVars(ctx) {
    const pairs = [];
    const pairIds = /* @__PURE__ */ new Map();
    const watches = [];
    const batches = [];
    function lower(unit) {
      return typeof unit === "string" ? unit.toLowerCase() : String(unit).toLowerCase();
    }
    function readXyz(name) {
      const h = ctx.host();
      const v = new Vec3Data();
      if (h) {
        v.x = h.getVar(name + "_X", "number");
        v.y = h.getVar(name + "_Y", "number");
        v.z = h.getVar(name + "_Z", "number");
      }
      return v;
    }
    function read(name, unit, _source) {
      const h = ctx.host();
      const u = lower(unit);
      if (u === "string") {
        return h ? h.getString(name) : "";
      }
      if (u === "latlonalt") {
        if (!h) {
          return new GeoPos3(0, 0, 0);
        }
        return new GeoPos3(h.getVar(name + "_LAT", "degrees"), h.getVar(name + "_LON", "degrees"), h.getVar(name + "_ALT", "feet"));
      }
      if (u === "xyz") {
        return readXyz(name);
      }
      return h ? h.getVar(name, unit) : 0;
    }
    function write(name, unit, value, _source) {
      const h = ctx.host();
      if (h) {
        if (typeof name === "string" && (name.startsWith("K:") || name.startsWith("H:"))) {
          h.sendEvent(name, [Number(value) || 0]);
        } else if (lower(unit) === "string") {
          h.setString(name, String(value));
        } else {
          h.setVar(name, unit, typeof value === "boolean" ? value ? 1 : 0 : Number(value));
        }
      }
      return Promise.resolve(void 0);
    }
    function readNow(name, unit, source) {
      return read(name, unit, source);
    }
    function readEnv(name, unit) {
      return read(name, unit);
    }
    function readHost(name, unit, _param1, _param2) {
      const h = ctx.host();
      const u = lower(unit);
      if (u === "string") {
        return h ? h.gameString(name) : "";
      }
      if (u === "xyz") {
        return readXyz(name);
      }
      return h ? h.getVar(name, unit) : 0;
    }
    function register(name, unit, _source) {
      const n = String(name);
      const u = String(unit);
      const key = n + "\0" + u;
      const existing = pairIds.get(key);
      if (existing !== void 0) {
        return existing;
      }
      const id = pairs.length;
      pairs.push([n, u]);
      pairIds.set(key, id);
      return id;
    }
    function readById(id) {
      const pair = pairs[id];
      return pair ? read(pair[0], pair[1]) : 0;
    }
    function readTextById(id) {
      const pair = pairs[id];
      if (!pair) {
        return "";
      }
      const h = ctx.host();
      return h ? h.getString(pair[0]) : "";
    }
    class VarBatch {
      constructor(countName, idName) {
        this.columns = [];
        this.countName = countName;
        this.idName = idName;
      }
      add(name, unit, type) {
        this.columns.push({ name, unit, type });
      }
    }
    function readBatch(_batch, callback, _source) {
      batches.push({ startAfter: ctx.frameIndex(), callback });
    }
    function watch(name, unit, listener) {
      const entry = { name, unit, listener, startAfter: ctx.frameIndex(), seen: false, last: void 0 };
      watches.push(entry);
      return {
        dispose() {
          const i = watches.indexOf(entry);
          if (i >= 0) {
            watches.splice(i, 1);
          }
        }
      };
    }
    function tick() {
      const frame = ctx.frameIndex();
      for (const w of watches.slice()) {
        if (frame <= w.startAfter || watches.indexOf(w) < 0) {
          continue;
        }
        const value = read(w.name, w.unit);
        if (!w.seen || !sameValue(value, w.last)) {
          w.seen = true;
          w.last = value;
          try {
            w.listener(value);
          } catch (e) {
            ctx.log(2, describeError(e));
          }
        }
      }
      if (batches.length > 0) {
        const due = [];
        for (let i = 0; i < batches.length; ) {
          if (frame > batches[i].startAfter) {
            due.push(batches[i]);
            batches.splice(i, 1);
          } else {
            i++;
          }
        }
        for (const b of due) {
          try {
            b.callback([]);
          } catch (e) {
            ctx.log(2, describeError(e));
          }
        }
      }
    }
    const Vars = {
      read,
      write,
      readHost,
      readEnv,
      register,
      readNow,
      readById,
      readTextById,
      readBatch,
      watch,
      VarBatch
    };
    function pairOf(id) {
      return typeof id === "number" ? pairs[id] : void 0;
    }
    return { Vars, tick, pairOf };
  }

  // ../clean-avionics/data/src/bridge.ts
  function parseResult(text) {
    if (typeof text !== "string") {
      return text;
    }
    try {
      return JSON.parse(text);
    } catch (e) {
      return text;
    }
  }
  function swallow() {
  }
  function createBridge(ctx, local = () => false) {
    const listeners = /* @__PURE__ */ new Map();
    const channels = /* @__PURE__ */ new Set();
    const relayReceivers = [];
    function invoke(fn, args) {
      try {
        fn(...args);
      } catch (e) {
        ctx.log(2, describeError(e));
      }
    }
    function call(name, ...args) {
      try {
        if (local(name, args)) {
          return Promise.resolve(void 0);
        }
      } catch (e) {
        return Promise.reject(e);
      }
      const h = ctx.host();
      if (!h) {
        return new Promise(() => {
        });
      }
      let pending;
      try {
        pending = Promise.resolve(h.call(name, JSON.stringify(args)));
      } catch (e) {
        return Promise.reject(e);
      }
      return pending.then(parseResult);
    }
    function signal(name, ...args) {
      try {
        if (local(name, args)) {
          return;
        }
      } catch (e) {
        ctx.log(2, describeError(e));
        return;
      }
      const h = ctx.host();
      if (!h) {
        return;
      }
      try {
        Promise.resolve(h.call(name, JSON.stringify(args))).then(swallow, swallow);
      } catch (e) {
        return;
      }
    }
    function on(name, listener) {
      const reg = { fn: listener };
      let list = listeners.get(name);
      if (!list) {
        list = [];
        listeners.set(name, list);
      }
      list.push(reg);
      return {
        clear() {
          const current = listeners.get(name);
          if (!current) {
            return;
          }
          const i = current.indexOf(reg);
          if (i >= 0) {
            current.splice(i, 1);
          }
          if (current.length === 0) {
            listeners.delete(name);
          }
        }
      };
    }
    function off(name, listener) {
      const list = listeners.get(name);
      if (!list) {
        return;
      }
      for (let i = list.length - 1; i >= 0; i--) {
        if (list[i].fn === listener) {
          list.splice(i, 1);
        }
      }
      if (list.length === 0) {
        listeners.delete(name);
      }
    }
    function emit(name, args) {
      const list = listeners.get(name);
      if (!list) {
        return;
      }
      for (const reg of list.slice()) {
        if (list.indexOf(reg) < 0) {
          continue;
        }
        invoke(reg.fn, args);
      }
    }
    class Channel {
      constructor(name = "") {
        this.handlers = /* @__PURE__ */ new Map();
        this.closed = false;
        this.close = () => {
          this.closed = true;
          channels.delete(this);
        };
        this.name = name;
      }
      on(name, listener) {
        let list = this.handlers.get(name);
        if (!list) {
          list = [];
          this.handlers.set(name, list);
        }
        list.push(listener);
      }
      off(name, listener) {
        if (listener === void 0) {
          this.handlers.delete(name);
          return;
        }
        const list = this.handlers.get(name);
        if (!list) {
          return;
        }
        for (let i = list.length - 1; i >= 0; i--) {
          if (list[i] === listener) {
            list.splice(i, 1);
          }
        }
        if (list.length === 0) {
          this.handlers.delete(name);
        }
      }
      call(name, ...args) {
        return call(name, ...args);
      }
      signal(name, ...args) {
        signal(name, ...args);
      }
      broadcast(event, ...args) {
        signal("channel.broadcast", this.name, event, ...args);
      }
      dispatch(event, args) {
        if (this.closed) {
          return;
        }
        const list = this.handlers.get(event);
        if (!list) {
          return;
        }
        for (const fn of list.slice()) {
          if (this.closed) {
            return;
          }
          invoke(fn, args);
        }
      }
    }
    function openChannel(name, onReady, _single) {
      const channel = new Channel(name);
      channels.add(channel);
      if (onReady) {
        Promise.resolve().then(() => onReady());
      }
      return channel;
    }
    function openRelay(onReady) {
      const relay = {
        send(key, data) {
          signal("relay.send", key, typeof data === "string" ? data : JSON.stringify(data));
        },
        receive(key, listener) {
          relayReceivers.push({ key, fn: listener });
        }
      };
      if (onReady) {
        Promise.resolve().then(() => onReady());
      }
      return relay;
    }
    function sendFlowEvent(name, ...args) {
      signal(name, ...args);
    }
    function channelEvent(args) {
      if (args.length < 2) {
        return;
      }
      const channelName = args[0];
      const event = args[1];
      const rest = args.slice(2);
      let realName = event;
      let realArgs = rest;
      if (event === "channel.broadcasted") {
        realName = rest[0];
        realArgs = rest.slice(1);
      }
      for (const channel of Array.from(channels)) {
        if (channel.name === channelName) {
          channel.dispatch(String(realName), realArgs);
        }
      }
      if (channelName === "relay") {
        const key = args[1];
        const data = args[2];
        let value = data;
        if (typeof data === "string") {
          try {
            value = JSON.parse(data);
          } catch (e) {
            value = data;
          }
        }
        for (const r of relayReceivers.slice()) {
          if (r.key === key) {
            invoke(r.fn, [value]);
          }
        }
      }
    }
    function providerEvent(name, payload) {
      let args;
      try {
        args = JSON.parse(payload);
      } catch (e) {
        args = [payload];
      }
      const list = Array.isArray(args) ? args : [args];
      emit(name, list);
      if (name === "channel.event") {
        channelEvent(list);
      }
    }
    function handlePoll(entries, panelEvent) {
      for (const entry of entries) {
        const kind = entry[0];
        const name = entry[1];
        if (kind === "h") {
          panelEvent(name);
          emit("panel.event", [name]);
        } else if (kind === "provider") {
          providerEvent(name, entry[2]);
        }
      }
    }
    const Bridge = { call, on, off, signal };
    const Channels = { Channel };
    return { Bridge, Channels, openChannel, openRelay, sendFlowEvent, handlePoll };
  }

  // ../clean-avionics/data/src/text.ts
  function randomBytes(count) {
    const bytes = new Uint8Array(count);
    const c = globalThis.crypto;
    if (c && typeof c.getRandomValues === "function") {
      c.getRandomValues(bytes);
    } else {
      for (let i = 0; i < count; i++) {
        bytes[i] = Math.floor(Math.random() * 256);
      }
    }
    return bytes;
  }
  function newGuid() {
    const b = randomBytes(16);
    b[6] = b[6] & 15 | 64;
    b[8] = b[8] & 63 | 128;
    let hex = "";
    for (let i = 0; i < 16; i++) {
      hex += (b[i] + 256).toString(16).slice(1);
    }
    return hex.slice(0, 8) + "-" + hex.slice(8, 12) + "-" + hex.slice(12, 16) + "-" + hex.slice(16, 20) + "-" + hex.slice(20);
  }
  function localize(key) {
    return key;
  }
  var Text = { localize, newGuid };

  // ../clean-avionics/data/src/panelHost.ts
  var FallbackElement = class {
    appendChild(node) {
      return node;
    }
  };
  function baseElement() {
    const g2 = globalThis;
    return g2.HTMLElement ? g2.HTMLElement : FallbackElement;
  }
  function pageDocument() {
    const g2 = globalThis;
    return g2.document;
  }
  function whenReady(fn) {
    const doc = pageDocument();
    if (!doc) {
      return;
    }
    if (doc.readyState !== "loading") {
      fn();
    } else {
      doc.addEventListener("DOMContentLoaded", () => fn(), { once: true });
    }
  }
  function createInstrumentHost(ctx, signal) {
    const Base = baseElement();
    const states = /* @__PURE__ */ new WeakMap();
    const tags = /* @__PURE__ */ new WeakMap();
    const attached = [];
    function stateOf(el) {
      let st = states.get(el);
      if (!st) {
        st = { guid: Text.newGuid(), started: false, attached: false, templated: false, failed: false, flown: false, frameDelta: 0, previousTime: 0 };
        states.set(el, st);
      }
      return st;
    }
    function tagOf(el) {
      const local = el.localName;
      if (local) {
        return local;
      }
      const byCtor = tags.get(el.constructor);
      if (byCtor) {
        return byCtor;
      }
      const tagName = el.tagName;
      return tagName ? tagName.toLowerCase() : "";
    }
    function hooks(el) {
      return el;
    }
    class PanelInstrument extends Base {
      get templateId() {
        return "";
      }
      get interactive() {
        return true;
      }
      get screenIndex() {
        const g2 = globalThis;
        const search = g2.location && typeof g2.location.search === "string" ? g2.location.search : "";
        const params = new URLSearchParams(search);
        let result = 1;
        let found = false;
        params.forEach((value, key) => {
          if (!found && key.toLowerCase() === "index") {
            found = true;
            const n = parseInt(value, 10);
            if (Number.isFinite(n)) {
              result = n;
            }
          }
        });
        return result;
      }
      get frameDelta() {
        return stateOf(this).frameDelta;
      }
      connectedCallback() {
        const st = stateOf(this);
        if (st.started) {
          return;
        }
        st.started = true;
        const chain = (time) => {
          if (st.failed) {
            return;
          }
          requestFrame(chain);
          if (st.attached) {
            instrumentFrame(this, st, frameTime(time));
          }
        };
        st.previousTime = clockNow();
        requestFrame(chain);
        try {
          this.onAttach();
        } catch (e) {
          st.failed = true;
          const h2 = ctx.host();
          if (h2) {
            h2.log(2, describeError(e));
            h2.loaded(false, errorMessage(e));
          }
          return;
        }
        st.attached = true;
        attached.push(this);
        const h = ctx.host();
        if (h) {
          h.loaded(true, "");
        }
        signal("instrument.ready", st.guid, tagOf(this));
      }
      onAttach() {
        const st = stateOf(this);
        if (!st.templated) {
          st.templated = true;
          const id = this.templateId;
          const doc = pageDocument();
          if (id && doc) {
            const tpl = doc.getElementById(id);
            if (tpl && tpl.content) {
              this.appendChild(tpl.content.cloneNode(true));
            }
          }
        }
        this.readPanelConfig();
        this.onPrepare();
      }
      onPrepare() {
      }
      onFrame() {
      }
      onPanelEvent(_args) {
      }
      onSessionChange(_from, _to) {
      }
      onFlightBegin() {
      }
      onSoundDone(_id) {
      }
      onPowered() {
      }
      onShutdown() {
      }
      sessionState() {
        return sessionPhase();
      }
      readPanelConfig() {
        const h = ctx.host();
        const text = h ? h.readFile("/VFS/panel/panel.xml") : null;
        const xml = text === null || text === void 0 ? "<PanelInfo/>" : text;
        this.panelConfig = new DOMParser().parseFromString(xml, "text/xml");
      }
    }
    class MapElement extends Base {
      constructor() {
        super(...arguments);
        this.configId = 0;
        this.configs = [];
      }
      setMapId(_id) {
      }
      setConfig(_id) {
      }
      setParams(_params) {
      }
      setVisible(_visible) {
      }
    }
    function defineInstrument(tag, type) {
      tags.set(type, tag);
      const g2 = globalThis;
      const registry = g2.customElements;
      if (registry) {
        try {
          if (!registry.get(tag)) {
            registry.define(tag, type);
          }
        } catch (e) {
        }
      }
      whenReady(() => {
        var _a, _b;
        const doc = pageDocument();
        if (doc && doc.body) {
          const panel = doc.createElement("vcockpit-panel");
          panel.style.cssText = "display:block;position:absolute;left:0;top:0;width:100%;height:100%";
          const el = doc.createElement(tag);
          el.setAttribute("url", String((_b = (_a = globalThis.location) == null ? void 0 : _a.href) != null ? _b : ""));
          panel.appendChild(el);
          doc.body.appendChild(panel);
        }
      });
    }
    function panelEvent(name) {
      for (const el of attached.slice()) {
        try {
          hooks(el).onPanelEvent([name]);
        } catch (e) {
          ctx.log(2, describeError(e));
        }
      }
    }
    function instrumentFrame(el, st, time) {
      st.frameDelta = time - st.previousTime;
      st.previousTime = time;
      try {
        if (!st.flown) {
          st.flown = true;
          hooks(el).onSessionChange(SessionPhase.briefing, SessionPhase.flying);
          hooks(el).onFlightBegin();
        }
        hooks(el).onFrame();
      } catch (e) {
        ctx.log(2, describeError(e));
      }
    }
    return { PanelInstrument, MapElement, defineInstrument, panelEvent };
  }

  // ../clean-avionics/data/src/planestate.ts
  function createPlaneState(read) {
    function altitude() {
      return read("fbw/INDICATED_ALTITUDE", "feet");
    }
    function heightAboveGround(_forceRead) {
      return read("fbw/PLANE_ALT_ABOVE_GROUND_MINUS_CG", "feet");
    }
    function outsideAirTemp() {
      return read("sim/weather/aircraft/temperature_ambient_deg_c", "celsius");
    }
    function groundSpeed() {
      return read("fbw/GPS_GROUND_SPEED", "knots");
    }
    function indicatedSpeed() {
      return read("sim/flightmodel/position/indicated_airspeed", "knots");
    }
    function verticalSpeed() {
      return read("fbw/VERTICAL_SPEED", "feet per minute");
    }
    function onGround() {
      return read("fbw/SIM_ON_GROUND", "bool") !== 0;
    }
    function baroSetting(units = "inches of mercury") {
      return read("fbw/KOHLSMAN_SETTING_HG_1", units);
    }
    function baroMode(_aircraft) {
      return read("fbw/KOHLSMAN_SETTING_STD_1", "bool") !== 0 ? "STD" : "QNH";
    }
    function baroUnits() {
      return read("L:XMLVAR_Baro_Selector_HPA_1", "bool") !== 0 ? "millibar" : "inches of mercury";
    }
    function apSpeedTarget() {
      return read("fbw/AUTOPILOT_AIRSPEED_HOLD_VAR", "knots");
    }
    function apSpeedSelected() {
      return read("fbw/AUTOPILOT_SPEED_SLOT_INDEX", "number") === 1;
    }
    function apSpeedManaged() {
      return read("fbw/AUTOPILOT_SPEED_SLOT_INDEX", "number") === 2;
    }
    function apMachTarget() {
      return read("fbw/AUTOPILOT_MACH_HOLD_VAR", "number");
    }
    function apMachMode() {
      return read("fbw/AUTOPILOT_MANAGED_SPEED_IN_MACH", "bool") !== 0;
    }
    function apHeadingTarget(radians = false) {
      return read("fbw/AUTOPILOT_HEADING_LOCK_DIR", radians ? "radians" : "degrees");
    }
    function apAltitudeTarget(units = "feet") {
      return read("fbw/AUTOPILOT_ALTITUDE_LOCK_VAR_3", units);
    }
    function apAltitudeManaged() {
      return read("fbw/AUTOPILOT_ALTITUDE_SLOT_INDEX", "number") === 2;
    }
    function greenDotSpeed() {
      return read("L:A32NX_SPEEDS_GD", "number");
    }
    return {
      altitude,
      heightAboveGround,
      outsideAirTemp,
      apSpeedTarget,
      apSpeedManaged,
      apSpeedSelected,
      apAltitudeManaged,
      apAltitudeTarget,
      apMachTarget,
      apMachMode,
      apHeadingTarget,
      greenDotSpeed,
      groundSpeed,
      indicatedSpeed,
      onGround,
      baroMode,
      baroUnits,
      baroSetting,
      verticalSpeed
    };
  }

  // ../clean-avionics/data/src/units.ts
  var table = /* @__PURE__ */ new Map();
  function linear(dimension2, factor, names) {
    const def = {
      dimension: dimension2,
      toBase: (v) => v * factor,
      fromBase: (v) => v / factor
    };
    for (const n of names) {
      table.set(n, def);
    }
  }
  function custom(dimension2, toBase, fromBase, names) {
    const def = { dimension: dimension2, toBase, fromBase };
    for (const n of names) {
      table.set(n, def);
    }
  }
  linear("length", 1, ["meter", "meters", "m"]);
  linear("length", 0.3048, ["foot", "feet", "ft"]);
  linear("length", 1852, ["nautical mile", "nm"]);
  linear("length", 1e3, ["kilometer", "km"]);
  linear("length", 1609.344, ["statute mile", "mile"]);
  linear("length", 0.0254, ["inch", "in"]);
  linear("length", 0.01, ["centimeter", "cm"]);
  linear("length", 1e-3, ["millimeter", "mm"]);
  linear("speed", 1, ["meter per second", "m/s"]);
  linear("speed", 1852 / 3600, ["knot", "knots", "kt"]);
  linear("speed", 1 / 3.6, ["kilometer per hour", "km/h"]);
  linear("speed", 0.3048, ["feet per second"]);
  linear("speed", 508e-5, ["feet per minute", "fpm"]);
  linear("angle", 1, ["radian"]);
  linear("angle", Math.PI / 180, ["degree", "degrees", "deg", "degree latitude", "degree longitude"]);
  linear("mass", 1, ["kilogram", "kg"]);
  linear("mass", 0.45359237, ["pound", "lbs", "lb"]);
  linear("mass", 1e3, ["tonne"]);
  linear("mass", 14.5939029, ["slug"]);
  linear("pressure", 1, ["pascal"]);
  linear("pressure", 100, ["hectopascal", "hpa"]);
  linear("pressure", 100, ["millibar", "mbar"]);
  linear("pressure", 3386.389, ["inches of mercury", "inch of mercury", "inhg"]);
  linear("pressure", 6894.757, ["psi"]);
  linear("time", 1, ["second", "seconds", "sec"]);
  linear("time", 60, ["minute"]);
  linear("time", 3600, ["hour"]);
  linear("time", 1e-3, ["millisecond"]);
  custom("temperature", (v) => v, (v) => v, ["celsius"]);
  custom("temperature", (v) => v - 273.15, (v) => v + 273.15, ["kelvin"]);
  custom("temperature", (v) => (v - 32) / 1.8, (v) => v * 1.8 + 32, ["fahrenheit"]);
  custom("temperature", (v) => v / 1.8 - 273.15, (v) => (v + 273.15) * 1.8, ["rankine"]);
  linear("ratio", 1, ["number"]);
  linear("ratio", 0.01, ["percent"]);
  linear("ratio", 1, ["percent over 100"]);
  linear("ratio", 1, ["part"]);
  linear("volume", 1, ["liter"]);
  linear("volume", 3.785411784, ["gallon", "gallons"]);
  linear("volume", 1e3, ["cubic meter"]);
  linear("volume flow", 1, ["gallon per hour", "gallons per hour", "gph"]);
  linear("mass flow", 1, ["pound per hour", "pounds per hour"]);
  function wordForms(word) {
    const forms = [word];
    if (word.length > 2 && word.endsWith("es")) {
      forms.push(word.slice(0, -2));
    }
    if (word.length > 1 && word.endsWith("s")) {
      forms.push(word.slice(0, -1));
    }
    return forms;
  }
  function lookup(unit) {
    if (typeof unit !== "string") {
      return void 0;
    }
    const key = unit.trim().toLowerCase().replace(/\s+/g, " ");
    if (key === "") {
      return void 0;
    }
    const direct = table.get(key);
    if (direct) {
      return direct;
    }
    const words = key.split(" ");
    let candidates = [""];
    for (let i = 0; i < words.length; i++) {
      const next = [];
      for (const prefix of candidates) {
        for (const form of wordForms(words[i])) {
          next.push(i === 0 ? form : prefix + " " + form);
        }
      }
      candidates = next;
    }
    for (const c of candidates) {
      const def = table.get(c);
      if (def) {
        return def;
      }
    }
    return void 0;
  }
  function convert(value, from, to) {
    const a = lookup(from);
    const b = lookup(to);
    if (!a || !b || a.dimension !== b.dimension) {
      return NaN;
    }
    if (a === b) {
      return value;
    }
    return b.fromBase(a.toBase(value));
  }
  function dimension(unit) {
    const def = lookup(unit);
    return def ? def.dimension : void 0;
  }
  var Units = { convert, dimension };

  // ../clean-avionics/data/src/geo.ts
  var EARTH_RADIUS_NM = 3440.065;
  var _Geo = class _Geo {
    static wrapDegrees(a) {
      const r = (a % 360 + 360) % 360;
      return r >= 360 ? 0 : r;
    }
    static angleDelta(a, b) {
      const d = _Geo.wrapDegrees(b - a);
      return d > 180 ? d - 360 : d;
    }
    static lerpDegrees(from, to, t) {
      return _Geo.wrapDegrees(from + _Geo.angleDelta(from, to) * t);
    }
    static distance(from, to) {
      const p1 = from.lat * _Geo.DEG_TO_RAD;
      const p2 = to.lat * _Geo.DEG_TO_RAD;
      const dp = p2 - p1;
      const dl = (to.long - from.long) * _Geo.DEG_TO_RAD;
      const h = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
      return 2 * EARTH_RADIUS_NM * Math.asin(Math.min(1, Math.sqrt(h)));
    }
    static bearing(from, to) {
      const p1 = from.lat * _Geo.DEG_TO_RAD;
      const p2 = to.lat * _Geo.DEG_TO_RAD;
      const dl = (to.long - from.long) * _Geo.DEG_TO_RAD;
      const y = Math.sin(dl) * Math.cos(p2);
      const x = Math.cos(p1) * Math.sin(p2) - Math.sin(p1) * Math.cos(p2) * Math.cos(dl);
      return _Geo.wrapDegrees(Math.atan2(y, x) * _Geo.RAD_TO_DEG);
    }
    static destination(bearing, distance, lat, long) {
      const p1 = lat * _Geo.DEG_TO_RAD;
      const l1 = long * _Geo.DEG_TO_RAD;
      const th = bearing * _Geo.DEG_TO_RAD;
      const d = distance / EARTH_RADIUS_NM;
      const sinP2 = Math.sin(p1) * Math.cos(d) + Math.cos(p1) * Math.sin(d) * Math.cos(th);
      const p2 = Math.asin(Math.max(-1, Math.min(1, sinP2)));
      const l2 = l1 + Math.atan2(Math.sin(th) * Math.sin(d) * Math.cos(p1), Math.cos(d) - Math.sin(p1) * sinP2);
      let lon = l2 * _Geo.RAD_TO_DEG;
      if (lon > 180 || lon < -180) {
        lon = ((lon + 180) % 360 + 360) % 360 - 180;
      }
      return new GeoPos3(p2 * _Geo.RAD_TO_DEG, lon, 0);
    }
    static adfBcd32(frequency) {
      let n = Math.abs(Math.round(frequency * 10));
      let result = 0;
      let scale = 1;
      while (n > 0) {
        result += n % 10 * scale;
        n = Math.floor(n / 10);
        scale *= 16;
      }
      return result;
    }
  };
  _Geo.DEG_TO_RAD = Math.PI / 180;
  _Geo.RAD_TO_DEG = 180 / Math.PI;
  var Geo = _Geo;
  var NavMath = { Geo };

  // ../clean-avionics/data/src/create.ts
  function createData(options = {}) {
    const host = options.host ? options.host : globalHost;
    let frameIndex = 0;
    const ctx = {
      host,
      frameIndex: () => frameIndex,
      log(level, text) {
        const h = host();
        if (h) {
          h.log(level, text);
        }
      }
    };
    const vars = createVars(ctx);
    const bridge = createBridge(ctx, (name, args) => {
      if (name === "var.set") {
        const pair = vars.pairOf(args[0]);
        if (pair) {
          vars.Vars.write(pair[0], pair[1], args[1]);
        }
        return true;
      }
      if (name === "var.set.text") {
        const pair = vars.pairOf(args[0]);
        const h = host();
        if (pair && h) {
          h.setString(pair[0], String(args[1]));
        }
        return true;
      }
      return false;
    });
    const instruments = createInstrumentHost(ctx, bridge.Bridge.signal);
    function pageFrame(_time) {
      requestFrame(pageFrame);
      frameIndex++;
      const h = host();
      if (h) {
        h.snapshot();
      }
      const entries = h ? h.poll() : [];
      bridge.handlePoll(Array.isArray(entries) ? entries : [], instruments.panelEvent);
      vars.tick();
    }
    requestFrame(pageFrame);
    const Vars = vars.Vars;
    function refName(path, index) {
      return index === void 0 ? path : path + "[" + index + "]";
    }
    const Datarefs = {
      get(path, index) {
        const h = host();
        return h ? h.getVar(refName(path, index), "raw") : 0;
      },
      getText(path) {
        const h = host();
        return h ? h.getString(path) : "";
      },
      set(path, value, index) {
        const h = host();
        if (h) {
          h.setVar(refName(path, index), "raw", value);
        }
      },
      watch(path, listener, index) {
        return Vars.watch(refName(path, index), "raw", listener);
      }
    };
    function command(path, phase) {
      const h = host();
      if (h) {
        h.sendEvent("CMD:" + path, [phase]);
      }
    }
    const Commands = {
      once(path) {
        command(path, 0);
      },
      begin(path) {
        command(path, 1);
      },
      end(path) {
        command(path, 2);
      }
    };
    function loadStored(key) {
      const h = host();
      const value = h ? h.storedData("get", key, "") : "";
      return value === "" || value === void 0 || value === null ? null : value;
    }
    function saveStored(key, value) {
      const h = host();
      if (h) {
        h.storedData("set", key, String(value));
      }
      return void 0;
    }
    const Declination = {
      at(lat, lon) {
        const h = host();
        return h ? h.magVar(lat, lon) : 0;
      }
    };
    const PlaneState = createPlaneState(Vars.read);
    return {
      Vars,
      Datarefs,
      Commands,
      Units,
      Bridge: bridge.Bridge,
      Channels: bridge.Channels,
      openChannel: bridge.openChannel,
      openRelay: bridge.openRelay,
      sendFlowEvent: bridge.sendFlowEvent,
      loadStored,
      saveStored,
      Text,
      Declination,
      SessionPhase,
      sessionPhase,
      SoundId,
      Noop,
      GeoPosData,
      GeoPos,
      GeoPos3,
      Vec3Data,
      NavMath,
      AircraftModel,
      PlaneState,
      PanelInstrument: instruments.PanelInstrument,
      defineInstrument: instruments.defineInstrument,
      KeyCodes,
      InputButtons,
      MapElement: instruments.MapElement,
      MapConfig
    };
  }

  // ../clean-avionics/data/src/errors.ts
  function errorEventText(event) {
    const message = event.message !== void 0 && event.message !== null ? String(event.message) : "";
    const err = event.error;
    if (message === "" && err !== void 0 && err !== null) {
      return describeError(err);
    }
    if (err && typeof err === "object" && typeof err.stack === "string" && err.stack !== "") {
      return message + "\n" + err.stack;
    }
    return message;
  }
  function installErrorForwarding(target, host) {
    target.addEventListener("error", (event) => {
      const h = host();
      if (h) {
        h.log(2, errorEventText(event));
      }
    });
    target.addEventListener("unhandledrejection", (event) => {
      const h = host();
      if (h) {
        h.log(2, describeError(event.reason));
      }
    });
  }

  // ../clean-avionics/data/src/pageBase.ts
  var PAGE_BASE_CSS = [
    "html{color:#fff}",
    "html,body{margin:0;padding:0;overflow:hidden}",
    "*,*::before,*::after{box-sizing:border-box;min-height:0}",
    "::-webkit-scrollbar{display:none}",
    ".hidden{display:none!important}"
  ].join("");
  function installPageBase() {
    var _a;
    const doc = globalThis.document;
    if (!doc || doc.getElementById("xplane-js-page-base")) {
      return;
    }
    const style = doc.createElement("style");
    style.id = "xplane-js-page-base";
    style.textContent = PAGE_BASE_CSS;
    const parent = (_a = doc.head) != null ? _a : doc.documentElement;
    if (parent) {
      parent.insertBefore(style, parent.firstChild);
    }
  }

  // ../clean-avionics/data/src/runtime.ts
  var g = globalThis;
  if (!g.xplaneJsData) {
    g.xplaneJsData = createData();
    installPageBase();
    whenReady(() => {
      if (typeof g.addEventListener === "function") {
        installErrorForwarding(g, globalHost);
      }
    });
  }
})();
