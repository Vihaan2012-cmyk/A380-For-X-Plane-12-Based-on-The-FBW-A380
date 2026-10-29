import type { Host, LogLevel, PollEntry } from '../src/host.ts';

export type Call = [string, ...unknown[]];

export class FakeHost implements Host {
  view = 1;
  screen = 'PFD';
  aircraftDir = '/aircraft';
  calls: Call[] = [];
  vars = new Map<string, number>();
  strings = new Map<string, string>();
  game = new Map<string, string>();
  stored = new Map<string, string>();
  files = new Map<string, string>();
  pollQueue: PollEntry[][] = [];
  callImpl: (name: string, argsJson: string) => Promise<string> = () => Promise.resolve('null');
  magVarImpl: (lat: number, lon: number) => number = (lat, lon) => lat + lon / 1000;
  logs: Array<[LogLevel, string]> = [];

  setNumber(name: string, unit: string, value: number): void {
    this.vars.set(name + '|' + unit, value);
  }

  snapshot(): void {
    this.calls.push(['snapshot']);
  }

  getVar(name: string, unit: string): number {
    this.calls.push(['getVar', name, unit]);
    const v = this.vars.get(name + '|' + unit);
    return v === undefined ? 0 : v;
  }

  setVar(name: string, unit: string, value: number): void {
    this.calls.push(['setVar', name, unit, value]);
  }

  getString(name: string): string {
    this.calls.push(['getString', name]);
    const v = this.strings.get(name);
    return v === undefined ? '' : v;
  }

  setString(name: string, value: string): void {
    this.calls.push(['setString', name, value]);
  }

  sendEvent(name: string, values: number[]): void {
    this.calls.push(['sendEvent', name, values]);
  }

  call(name: string, argsJson: string): Promise<string> {
    this.calls.push(['call', name, argsJson]);
    return this.callImpl(name, argsJson);
  }

  poll(): PollEntry[] {
    this.calls.push(['poll']);
    const next = this.pollQueue.shift();
    return next === undefined ? [] : next;
  }

  gameString(name: string): string {
    this.calls.push(['gameString', name]);
    const v = this.game.get(name);
    return v === undefined ? '' : v;
  }

  storedData(op: 'get' | 'set' | 'delete' | 'search', key: string, value: string): string {
    this.calls.push(['storedData', op, key, value]);
    if (op === 'get') {
      const v = this.stored.get(key);
      return v === undefined ? '' : v;
    }
    if (op === 'set') {
      this.stored.set(key, value);
    }
    return '';
  }

  readFile(path: string): string | null {
    this.calls.push(['readFile', path]);
    const v = this.files.get(path);
    return v === undefined ? null : v;
  }

  log(level: LogLevel, text: string): void {
    this.calls.push(['log', level, text]);
    this.logs.push([level, text]);
  }

  magVar(lat: number, lon: number): number {
    this.calls.push(['magVar', lat, lon]);
    return this.magVarImpl(lat, lon);
  }

  loaded(ok: boolean, text: string): void {
    this.calls.push(['loaded', ok, text]);
  }

  named(kind: string): Call[] {
    return this.calls.filter((c) => c[0] === kind);
  }

  kinds(): string[] {
    return this.calls.map((c) => c[0]);
  }
}

type FrameCallback = (time: number) => void;

export class FrameDriver {
  pending: FrameCallback[] = [];
  time = 1000;

  install(): void {
    const clock = { now: (): number => this.time };
    Object.defineProperty(globalThis, 'performance', { value: clock, configurable: true, writable: true });
    (globalThis as { requestAnimationFrame?: (cb: FrameCallback) => number }).requestAnimationFrame = (cb: FrameCallback) => {
      this.pending.push(cb);
      return this.pending.length;
    };
  }

  step(dt: number = 16): void {
    const callbacks = this.pending;
    this.pending = [];
    this.time += dt;
    for (const cb of callbacks) {
      cb(this.time);
    }
  }
}

export function flush(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

export interface FakeDom {
  registry: Map<string, new () => unknown>;
  defineCalls: string[];
  body: FakeElementLike;
  templates: Map<string, { content: { cloneNode(deep: boolean): unknown } }>;
  readyListeners: Array<() => void>;
  setReady(state: string): void;
  fireReady(): void;
}

export interface FakeElementLike {
  localName: string;
  children: unknown[];
  appendChild<T>(node: T): T;
}

export function installDom(search: string = ''): FakeDom {
  const g = globalThis as Record<string, unknown>;

  class FakeElement implements FakeElementLike {
    localName = '';
    children: unknown[] = [];
    appendChild<T>(node: T): T {
      this.children.push(node);
      const el = node as { connectedCallback?: () => void };
      if (el && typeof el.connectedCallback === 'function') {
        el.connectedCallback();
      }
      return node;
    }
  }

  const registry = new Map<string, new () => unknown>();
  const defineCalls: string[] = [];
  const templates = new Map<string, { content: { cloneNode(deep: boolean): unknown } }>();
  const readyListeners: Array<() => void> = [];
  const body = new FakeElement();
  let readyState = 'complete';

  const document = {
    get readyState(): string {
      return readyState;
    },
    body,
    getElementById(id: string): unknown {
      const t = templates.get(id);
      return t === undefined ? null : t;
    },
    createElement(tag: string): unknown {
      const C = registry.get(tag);
      const el = C ? (new C() as FakeElement) : new FakeElement();
      el.localName = tag;
      return el;
    },
    addEventListener(type: string, fn: () => void): void {
      if (type === 'DOMContentLoaded') {
        readyListeners.push(fn);
      }
    },
  };

  g.HTMLElement = FakeElement;
  g.document = document;
  g.customElements = {
    define(tag: string, ctor: new () => unknown): void {
      defineCalls.push(tag);
      if (registry.has(tag)) {
        throw new Error('already defined: ' + tag);
      }
      registry.set(tag, ctor);
    },
    get(tag: string): unknown {
      return registry.get(tag);
    },
  };
  g.DOMParser = class {
    parseFromString(text: string, type: string): unknown {
      return { parsed: text, type };
    }
  };
  g.location = { search };

  return {
    registry,
    defineCalls,
    body,
    templates,
    readyListeners,
    setReady(state: string): void {
      readyState = state;
    },
    fireReady(): void {
      readyState = 'interactive';
      for (const fn of readyListeners.splice(0)) {
        fn();
      }
    },
  };
}

const realPerformance = Object.getOwnPropertyDescriptor(globalThis, 'performance');

export function removeDom(): void {
  const g = globalThis as Record<string, unknown>;
  if (realPerformance) {
    Object.defineProperty(globalThis, 'performance', realPerformance);
  }
  for (const key of ['HTMLElement', 'document', 'customElements', 'DOMParser', 'location', 'requestAnimationFrame']) {
    delete g[key];
  }
}
