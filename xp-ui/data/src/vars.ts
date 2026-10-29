import type { Context } from './host.ts';
import { describeError } from './host.ts';
import { GeoPos3, Vec3Data } from './types.ts';

interface WatchEntry {
  name: string;
  unit: string;
  listener: (value: any) => void;
  startAfter: number;
  seen: boolean;
  last: unknown;
}

interface BatchRequest {
  startAfter: number;
  callback: (rows: any) => void;
}

function sameValue(a: unknown, b: unknown): boolean {
  return a === b || (a !== a && b !== b);
}

export function createVars(ctx: Context) {
  const pairs: Array<[string, string]> = [];
  const pairIds = new Map<string, number>();
  const watches: WatchEntry[] = [];
  const batches: BatchRequest[] = [];

  function lower(unit: unknown): string {
    return typeof unit === 'string' ? unit.toLowerCase() : String(unit).toLowerCase();
  }

  function readXyz(name: string): Vec3Data {
    const h = ctx.host();
    const v = new Vec3Data();
    if (h) {
      v.x = h.getVar(name + '_X', 'number');
      v.y = h.getVar(name + '_Y', 'number');
      v.z = h.getVar(name + '_Z', 'number');
    }
    return v;
  }

  function read(name: string, unit: string, _source?: string): any {
    const h = ctx.host();
    const u = lower(unit);
    if (u === 'string') {
      return h ? h.getString(name) : '';
    }
    if (u === 'latlonalt') {
      if (!h) {
        return new GeoPos3(0, 0, 0);
      }
      return new GeoPos3(h.getVar(name + '_LAT', 'degrees'), h.getVar(name + '_LON', 'degrees'), h.getVar(name + '_ALT', 'feet'));
    }
    if (u === 'xyz') {
      return readXyz(name);
    }
    return h ? h.getVar(name, unit) : 0;
  }

  function write(name: string, unit: string, value: any, _source?: string): Promise<any> {
    const h = ctx.host();
    if (h) {
      if (typeof name === 'string' && (name.startsWith('K:') || name.startsWith('H:'))) {
        h.sendEvent(name, [Number(value) || 0]);
      } else if (lower(unit) === 'string') {
        h.setString(name, String(value));
      } else {
        h.setVar(name, unit, typeof value === 'boolean' ? (value ? 1 : 0) : Number(value));
      }
    }
    return Promise.resolve(undefined);
  }

  function readNow(name: string, unit: string, source?: string): any {
    return read(name, unit, source);
  }

  function readEnv(name: string, unit: string): any {
    return read(name, unit);
  }

  function readHost(name: string, unit: string, _param1?: number, _param2?: number): any {
    const h = ctx.host();
    const u = lower(unit);
    if (u === 'string') {
      return h ? h.gameString(name) : '';
    }
    if (u === 'xyz') {
      return readXyz(name);
    }
    return h ? h.getVar(name, unit) : 0;
  }

  function register(name: any, unit: any, _source?: any): number {
    const n = String(name);
    const u = String(unit);
    const key = n + '\u0000' + u;
    const existing = pairIds.get(key);
    if (existing !== undefined) {
      return existing;
    }
    const id = pairs.length;
    pairs.push([n, u]);
    pairIds.set(key, id);
    return id;
  }

  function readById(id: number): any {
    const pair = pairs[id];
    return pair ? read(pair[0], pair[1]) : 0;
  }

  function readTextById(id: number): any {
    const pair = pairs[id];
    if (!pair) {
      return '';
    }
    const h = ctx.host();
    return h ? h.getString(pair[0]) : '';
  }

  class VarBatch {
    readonly countName: string;
    readonly idName: string;
    readonly columns: Array<{ name: string; unit: string; type?: string }> = [];
    constructor(countName: string, idName: string) {
      this.countName = countName;
      this.idName = idName;
    }
    add(name: string, unit: string, type?: string): void {
      this.columns.push({ name, unit, type });
    }
  }

  function readBatch(_batch: VarBatch, callback: (rows: any) => void, _source?: string): void {
    batches.push({ startAfter: ctx.frameIndex(), callback });
  }

  function watch(name: string, unit: string, listener: (value: any) => void): { dispose(): void } {
    const entry: WatchEntry = { name, unit, listener, startAfter: ctx.frameIndex(), seen: false, last: undefined };
    watches.push(entry);
    return {
      dispose(): void {
        const i = watches.indexOf(entry);
        if (i >= 0) {
          watches.splice(i, 1);
        }
      },
    };
  }

  function tick(): void {
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
      const due: BatchRequest[] = [];
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
    VarBatch,
  };

  function pairOf(id: unknown): [string, string] | undefined {
    return typeof id === 'number' ? pairs[id] : undefined;
  }

  return { Vars, tick, pairOf };
}

export type VarsApi = ReturnType<typeof createVars>['Vars'];
