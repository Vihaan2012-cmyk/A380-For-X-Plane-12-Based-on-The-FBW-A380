import type { Bond } from 'xplane-js/ui';
import { ListenerSet } from './listeners';

export enum MapChange {
  Added = 'Added',
  Changed = 'Changed',
  Deleted = 'Deleted',
}

type MapHandler<K, V> = (map: ReadonlyMap<K, V>, type: MapChange, key: K, value: V) => void;

export class SignalMap<K, V> {
  private readonly table: Map<K, V>;
  private readonly listeners = new ListenerSet<MapHandler<K, V>>();

  protected constructor(initial: Iterable<readonly [K, V]>) {
    this.table = new Map(initial);
  }

  static of<K, V>(initialEntries?: Iterable<readonly [K, V]>): SignalMap<K, V> {
    return new SignalMap<K, V>(initialEntries ?? []);
  }

  get size(): number {
    return this.table.size;
  }

  get(): ReadonlyMap<K, V> {
    return this.table;
  }

  lookup(key: K): V | undefined {
    return this.table.get(key);
  }

  assign(key: K, value: V): this {
    const existed = this.table.has(key);
    if (existed && this.table.get(key) === value) {
      return this;
    }
    this.table.set(key, value);
    this.emit(existed ? MapChange.Changed : MapChange.Added, key, value);
    return this;
  }

  delete(key: K): boolean {
    if (!this.table.has(key)) {
      return false;
    }
    const old = this.table.get(key) as V;
    this.table.delete(key);
    this.emit(MapChange.Deleted, key, old);
    return true;
  }

  clear(): void {
    for (const [key, value] of Array.from(this.table)) {
      this.table.delete(key);
      this.emit(MapChange.Deleted, key, value);
    }
  }

  watch(handler: MapHandler<K, V>, initialNotify = false, paused = false): Bond {
    return this.listeners.join(handler, initialNotify, paused, (target) => {
      for (const [key, value] of Array.from(this.table)) {
        target.fn(this.table, MapChange.Added, key, value);
      }
    });
  }

  private emit(type: MapChange, key: K, value: V): void {
    this.listeners.dispatch((fn) => fn(this.table, type, key, value));
  }
}
