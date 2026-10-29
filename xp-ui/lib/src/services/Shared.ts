import { Signal } from 'xplane-js/ui';

export interface SharedHandle<T> {
  readonly isOwner: boolean;
  readonly value: T;
  readonly detached: Signal<boolean>;
}

export class Ids {
  public static random(): string {
    const bytes = new Uint8Array(16);
    const rng = (globalThis as { crypto?: { getRandomValues?: (a: Uint8Array) => Uint8Array } }).crypto;
    if (rng && rng.getRandomValues) {
      rng.getRandomValues(bytes);
    } else {
      for (let i = 0; i < 16; i++) {
        bytes[i] = Math.floor(Math.random() * 256);
      }
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (b) => (b + 0x100).toString(16).slice(1));
    return [hex.slice(0, 4), hex.slice(4, 6), hex.slice(6, 8), hex.slice(8, 10), hex.slice(10)]
      .map((part) => part.join(''))
      .join('-');
  }
}

const REGISTRY_KEY = '__xpui_shared_objects__';

interface RegistryEntry {
  object: object;
  detached: Signal<boolean>;
}

function registry(): Map<string, RegistryEntry> {
  const host = globalThis as unknown as Record<string, Map<string, RegistryEntry> | undefined>;
  let map = host[REGISTRY_KEY];
  if (!map) {
    map = new Map();
    host[REGISTRY_KEY] = map;
  }
  return map;
}

export class SharedObject {
  public static get<T extends object>(name: string): Promise<SharedHandle<T>> {
    const table = registry();
    let entry = table.get(name);
    const owner = entry === undefined;
    if (!entry) {
      entry = { object: {}, detached: Signal.of(false) };
      table.set(name, entry);
    }
    const ref: SharedHandle<T> = {
      isOwner: owner,
      value: entry.object as T,
      detached: entry.detached,
    };
    return Promise.resolve(ref);
  }
}

export class Pool<T> {
  private readonly spare: T[] = [];
  private alive = 0;

  public constructor(
    private readonly make: () => T,
    private readonly dispose: (resource: T) => void,
    private readonly handedOut?: (resource: T) => void,
    private readonly handedBack?: (resource: T) => void,
    initialSize = 0,
    private readonly limit = Infinity,
    private readonly shrinkAbove = Infinity,
  ) {
    const count = Math.min(initialSize, limit);
    for (let i = 0; i < count; i++) {
      this.spare.push(this.make());
      this.alive++;
    }
  }

  public take(): T {
    let resource = this.spare.pop();
    if (resource === undefined) {
      if (this.alive >= this.limit) {
        throw new Error('Resource heap has reached its allocation limit.');
      }
      resource = this.make();
      this.alive++;
    }
    this.handedOut?.(resource);
    return resource;
  }

  public give(resource: T): void {
    this.handedBack?.(resource);
    if (this.alive > this.shrinkAbove) {
      this.alive--;
      this.dispose(resource);
    } else {
      this.spare.push(resource);
    }
  }
}
