import type { ValueHandler, Bond } from 'xplane-js/ui';
import { StreamSource, Stream, Inbox, Outbox } from './stream';
import { openRelay } from 'xplane-js/data';

interface WireMessage {
  origin: string;
  topic: string;
  data: any;
  cached: boolean;
}

interface Slot {
  listeners: Listener[];
  has: boolean;
  value: any;
}

class Listener implements Bond {
  private _alive = true;
  private _paused: boolean;
  private _missed: boolean;

  constructor(
    private readonly _deliver: (value: any) => void,
    paused: boolean,
    private readonly _unhook: (self: Listener) => void,
    private readonly _current: (() => readonly [boolean, any]) | null,
  ) {
    this._paused = paused;
    this._missed = paused;
  }

  get isLive(): boolean {
    return this._alive;
  }

  get isHeld(): boolean {
    return this._paused;
  }

  fire(value: any): void {
    if (!this._alive) {
      return;
    }
    if (this._paused) {
      this._missed = true;
      return;
    }
    this._deliver(value);
  }

  replay(): void {
    if (!this._alive || !this._current) {
      return;
    }
    const [has, value] = this._current();
    if (has) {
      this._deliver(value);
    }
  }

  hold(): this {
    this._paused = true;
    return this;
  }

  release(initialNotify = false): this {
    if (!this._alive) {
      return this;
    }
    this._paused = false;
    if (initialNotify || this._missed) {
      this._missed = false;
      this.replay();
    }
    return this;
  }

  destroy(): void {
    if (this._alive) {
      this._alive = false;
      this._unhook(this);
    }
  }
}

const sameContextPeers = new Set<(msg: WireMessage) => void>();
const WIRE_KEY = 'xp-ui.bus';
const BACKLOG_LIMIT = 2000;

function toPeers(receive: (msg: WireMessage) => void, msg: WireMessage): void {
  sameContextPeers.forEach((peer) => {
    if (peer !== receive) {
      peer(msg);
    }
  });
}

function hostWire(receive: (msg: WireMessage) => void): ((msg: WireMessage) => void) | null {
  const g = globalThis as any;
  if (typeof openRelay !== 'function') {
    return null;
  }
  let ready = false;
  let pending: WireMessage[] = [];
  let scheduled = false;
  let listener: any = null;
  const flush = (): void => {
    scheduled = false;
    if (!ready || pending.length === 0) {
      return;
    }
    const events = pending;
    pending = [];
    try {
      listener.send(WIRE_KEY, { events });
    } catch {
      return;
    }
  };
  try {
    listener = openRelay(() => {
      ready = true;
      flush();
    });
    const land = (packet: any): void => {
      const parsed = typeof packet === 'string' ? JSON.parse(packet) : packet;
      const list: WireMessage[] = parsed && Array.isArray(parsed.events) ? parsed.events : [parsed];
      for (const msg of list) {
        receive(msg);
      }
    };
    if (typeof listener.receive === 'function') {
      listener.receive(WIRE_KEY, land);
    } else {
      listener.on(WIRE_KEY, land);
    }
  } catch {
    return null;
  }
  sameContextPeers.add(receive);
  return (msg) => {
    toPeers(receive, msg);
    pending.push(msg);
    if (pending.length > BACKLOG_LIMIT) {
      pending.shift();
    }
    if (ready && !scheduled) {
      scheduled = true;
      Promise.resolve().then(flush);
    }
  };
}

function joinWire(receive: (msg: WireMessage) => void): (msg: WireMessage) => void {
  const viaHost = hostWire(receive);
  if (viaHost) {
    return viaHost;
  }
  const g = globalThis as any;
  if (typeof g.BroadcastChannel === 'function') {
    try {
      const channel = new g.BroadcastChannel(WIRE_KEY);
      channel.onmessage = (ev: { data: WireMessage }) => receive(ev.data);
      if (typeof channel.unref === 'function') {
        channel.unref();
      }
      return (msg) => {
        try {
          channel.postMessage(msg);
        } catch {
          return;
        }
      };
    } catch {
      sameContextPeers.add(receive);
    }
  } else {
    sameContextPeers.add(receive);
  }
  return (msg) => toPeers(receive, msg);
}

let busCounter = 0;

export class Bus {
  private readonly _slots = new Map<string, Slot>();
  private readonly _wildcards: Listener[] = [];
  private readonly _demandWatchers: Array<(topic: string) => void> = [];
  private readonly _busTag = `bus${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}${busCounter++}`;
  private readonly _send: (msg: WireMessage) => void;

  constructor() {
    this._send = joinWire((msg) => this.land(msg));
  }

  public inbox<E>(): Inbox<E> {
    return {
      topic: <K extends keyof E>(topic: K): Stream<E[K]> =>
        new StreamSource<E[K]>((handler, paused) => this.listen(topic as string, handler, paused)),
    };
  }

  public outbox<E>(): Outbox<E> {
    return {
      post: (topic, data, sync, isCached) => this.post(topic as string, data, sync, isCached),
    };
  }

  public receiver<E>(): Inbox<E> {
    return this.inbox<E>();
  }

  public listen(topic: string, handler: ValueHandler<any>, paused = false): Bond {
    const slot = this.slotOf(topic);
    const listener: Listener = new Listener(
      handler,
      paused,
      (self) => {
        const at = slot.listeners.indexOf(self);
        if (at >= 0) {
          slot.listeners.splice(at, 1);
        }
      },
      () => [slot.has, slot.value],
    );
    const wasEmpty = slot.listeners.length === 0;
    slot.listeners.push(listener);
    if (wasEmpty) {
      this._demandWatchers.slice().forEach((watch) => watch(topic));
    }
    if (!paused) {
      listener.replay();
    }
    return listener;
  }

  public listenAll(handler: (topic: string, data: any) => void): Bond {
    const listener: Listener = new Listener(
      (packed: { topic: string; data: any }) => handler(packed.topic, packed.data),
      false,
      (self) => {
        const at = this._wildcards.indexOf(self);
        if (at >= 0) {
          this._wildcards.splice(at, 1);
        }
      },
      null,
    );
    this._wildcards.push(listener);
    return listener;
  }

  public post(topic: string, data: any, sync = false, isCached = true): void {
    this.dispatch(topic, data, isCached);
    if (sync) {
      this._send({ origin: this._busTag, topic, data, cached: isCached });
    }
  }

  public hasListeners(topic: string): boolean {
    const slot = this._slots.get(topic);
    return !!slot && slot.listeners.length > 0;
  }

  public watchDemand(watcher: (topic: string) => void): void {
    this._demandWatchers.push(watcher);
    this._slots.forEach((slot, topic) => {
      if (slot.listeners.length > 0) {
        watcher(topic);
      }
    });
  }

  private slotOf(topic: string): Slot {
    let slot = this._slots.get(topic);
    if (!slot) {
      slot = { listeners: [], has: false, value: undefined };
      this._slots.set(topic, slot);
    }
    return slot;
  }

  private land(msg: WireMessage): void {
    if (msg.origin !== this._busTag) {
      this.dispatch(msg.topic, msg.data, msg.cached);
    }
  }

  private dispatch(topic: string, data: any, cached: boolean): void {
    const slot = this.slotOf(topic);
    slot.has = cached;
    slot.value = cached ? data : undefined;
    slot.listeners.slice().forEach((l) => l.fire(data));
    if (this._wildcards.length > 0) {
      const packed = { topic, data };
      this._wildcards.slice().forEach((l) => l.fire(packed));
    }
  }
}
