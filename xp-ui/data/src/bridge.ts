import type { Context, PollEntry } from './host.ts';
import { describeError } from './host.ts';
import type { EventHandle, Listener, Relay } from './types.ts';

interface Registration {
  fn: Listener;
}

function parseResult(text: unknown): any {
  if (typeof text !== 'string') {
    return text;
  }
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function swallow(): void {}

export type LocalCall = (name: string, args: unknown[]) => boolean;

export function createBridge(ctx: Context, local: LocalCall = () => false) {
  const listeners = new Map<string, Registration[]>();
  const channels = new Set<Channel>();
  const relayReceivers: Array<{ key: string; fn: (data: any) => void }> = [];

  function invoke(fn: Function, args: unknown[]): void {
    try {
      fn(...args);
    } catch (e) {
      ctx.log(2, describeError(e));
    }
  }

  function call(name: string, ...args: any[]): Promise<any> {
    try {
      if (local(name, args)) {
        return Promise.resolve(undefined);
      }
    } catch (e) {
      return Promise.reject(e);
    }
    const h = ctx.host();
    if (!h) {
      return new Promise<any>(() => {});
    }
    let pending: Promise<string>;
    try {
      pending = Promise.resolve(h.call(name, JSON.stringify(args)));
    } catch (e) {
      return Promise.reject(e);
    }
    return pending.then(parseResult);
  }

  function signal(name: string, ...args: any[]): void {
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
    } catch {
      return;
    }
  }

  function on(name: string, listener: Listener): EventHandle {
    const reg: Registration = { fn: listener };
    let list = listeners.get(name);
    if (!list) {
      list = [];
      listeners.set(name, list);
    }
    list.push(reg);
    return {
      clear(): void {
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
      },
    };
  }

  function off(name: string, listener: Listener): void {
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

  function emit(name: string, args: unknown[]): void {
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
    readonly name: string;
    private readonly handlers = new Map<string, Listener[]>();
    private closed = false;

    constructor(name: string = '') {
      this.name = name;
    }

    on(name: string, listener: Listener): void {
      let list = this.handlers.get(name);
      if (!list) {
        list = [];
        this.handlers.set(name, list);
      }
      list.push(listener);
    }

    off(name: string, listener?: Listener): void {
      if (listener === undefined) {
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

    call(name: string, ...args: any[]): Promise<any> {
      return call(name, ...args);
    }

    signal(name: string, ...args: any[]): void {
      signal(name, ...args);
    }

    broadcast(event: any, ...args: any[]): void {
      signal('channel.broadcast', this.name, event, ...args);
    }

    close: () => void = (): void => {
      this.closed = true;
      channels.delete(this);
    };

    dispatch(event: string, args: unknown[]): void {
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

  function openChannel(name: string, onReady?: () => void, _single?: boolean): Channel {
    const channel = new Channel(name);
    channels.add(channel);
    if (onReady) {
      Promise.resolve().then(() => onReady());
    }
    return channel;
  }

  function openRelay(onReady?: () => void): Relay {
    const relay: Relay = {
      send(key: string, data: any): void {
        signal('relay.send', key, typeof data === 'string' ? data : JSON.stringify(data));
      },
      receive(key: string, listener: (data: any) => void): void {
        relayReceivers.push({ key, fn: listener });
      },
    };
    if (onReady) {
      Promise.resolve().then(() => onReady());
    }
    return relay;
  }

  function sendFlowEvent(name: string, ...args: any[]): void {
    signal(name, ...args);
  }

  function channelEvent(args: unknown[]): void {
    if (args.length < 2) {
      return;
    }
    const channelName = args[0];
    const event = args[1];
    const rest = args.slice(2);
    let realName: unknown = event;
    let realArgs: unknown[] = rest;
    if (event === 'channel.broadcasted') {
      realName = rest[0];
      realArgs = rest.slice(1);
    }
    for (const channel of Array.from(channels)) {
      if (channel.name === channelName) {
        channel.dispatch(String(realName), realArgs);
      }
    }
    if (channelName === 'relay') {
      const key = args[1];
      const data = args[2];
      let value: unknown = data;
      if (typeof data === 'string') {
        try {
          value = JSON.parse(data);
        } catch {
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

  function providerEvent(name: string, payload: string): void {
    let args: unknown;
    try {
      args = JSON.parse(payload);
    } catch {
      args = [payload];
    }
    const list: unknown[] = Array.isArray(args) ? args : [args];
    emit(name, list);
    if (name === 'channel.event') {
      channelEvent(list);
    }
  }

  function handlePoll(entries: PollEntry[], panelEvent: (name: string) => void): void {
    for (const entry of entries) {
      const kind = entry[0];
      const name = entry[1];
      if (kind === 'h') {
        panelEvent(name);
        emit('panel.event', [name]);
      } else if (kind === 'provider') {
        providerEvent(name, entry[2]);
      }
    }
  }

  const Bridge = { call, on, off, signal };
  const Channels = { Channel };

  return { Bridge, Channels, openChannel, openRelay, sendFlowEvent, handlePoll };
}

export type BridgeApi = ReturnType<typeof createBridge>;
