import type { ValueHandler, Bond } from 'xplane-js/ui';

export interface Stream<T> {
  listen(handler: ValueHandler<T>, paused?: boolean): Bond;
  distinct(): Stream<T>;
  distinctBy(amount: number): Stream<T>;
  rounded(precision: number): Stream<T>;
  limit(frequency: number, immediateFirstPublish?: boolean): Stream<T>;
  after(deltaTime: number): Stream<T>;
  sampled(frequency: number): Stream<T>;
}

export interface Inbox<E> {
  topic<K extends keyof E>(topic: K): Stream<E[K]>;
}

export interface Outbox<E> {
  post<K extends keyof E>(topic: K, data: E[K], sync?: boolean, isCached?: boolean): void;
}

export type IndexedTopic<T extends string> = `${T}_${number}`;

export type TopicsByIndex<T extends Record<string, any>, Index extends number> = {
  [P in keyof T as `${P & string}_${Index}`]: T[P];
};

type Opener<T> = (handler: ValueHandler<T>, paused: boolean) => Bond;

interface Stage<T> {
  push: ValueHandler<T>;
  dispose?: () => void;
}

class Linked implements Bond {
  constructor(
    private readonly _core: Bond,
    private readonly _cleanup: () => void,
  ) {}

  get isLive(): boolean {
    return this._core.isLive;
  }

  get isHeld(): boolean {
    return this._core.isHeld;
  }

  hold(): this {
    this._core.hold();
    return this;
  }

  release(initialNotify?: boolean): this {
    this._core.release(initialNotify);
    return this;
  }

  destroy(): void {
    this._cleanup();
    this._core.destroy();
  }
}

function stacked<T>(source: Opener<T>, make: (out: ValueHandler<T>) => Stage<T>): Stream<T> {
  return new StreamSource<T>((handler, paused) => {
    const stage = make(handler);
    const core = source(stage.push, paused);
    return stage.dispose ? new Linked(core, stage.dispose) : core;
  });
}

function rateLimited<T>(hz: number, out: ValueHandler<T>, immediate = true): Stage<T> {
  const gap = 1000 / hz;
  let lastOut = immediate ? -Infinity : Date.now();
  let waiting: { value: T } | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const release = (): void => {
    timer = null;
    if (waiting) {
      const { value } = waiting;
      waiting = null;
      lastOut = Date.now();
      out(value);
    }
  };

  return {
    push: (value) => {
      const t = Date.now();
      const since = t - lastOut;
      if (since >= gap || since < 0) {
        if (timer !== null) {
          clearTimeout(timer);
          timer = null;
        }
        waiting = null;
        lastOut = t;
        out(value);
        return;
      }
      waiting = { value };
      if (timer === null) {
        timer = setTimeout(release, gap - since);
      }
    },
    dispose: () => {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      waiting = null;
    },
  };
}

export class StreamSource<T> implements Stream<T> {
  private readonly _open: Opener<T>;

  constructor(subscribe: (handler: ValueHandler<T>, paused: boolean) => Bond) {
    this._open = subscribe;
  }

  public listen(handler: ValueHandler<T>, paused = false): Bond {
    return this._open(handler, paused);
  }

  public distinct(): Stream<T> {
    return stacked(this._open, (out) => {
      let seen = false;
      let prev: T | undefined;
      return {
        push: (value) => {
          if (!seen || value !== prev) {
            seen = true;
            prev = value;
            out(value);
          }
        },
      };
    });
  }

  public distinctBy(amount: number): Stream<T> {
    return stacked(this._open, (out) => {
      let seen = false;
      let prev = 0;
      return {
        push: (value) => {
          const n = value as unknown as number;
          if (!seen || Math.abs(n - prev) >= amount) {
            seen = true;
            prev = n;
            out(value);
          }
        },
      };
    });
  }

  public rounded(precision: number): Stream<T> {
    const scale = Math.pow(10, precision);
    return stacked(this._open, (out) => {
      let seen = false;
      let prev: unknown;
      return {
        push: (value) => {
          if (typeof value !== 'number') {
            out(value);
            return;
          }
          const snapped = (Math.round(value * scale) / scale) as unknown as T;
          if (!seen || snapped !== prev) {
            seen = true;
            prev = snapped;
            out(snapped);
          }
        },
      };
    });
  }

  public limit(frequency: number, immediateFirstPublish = true): Stream<T> {
    return stacked(this._open, (out) => rateLimited(frequency, out, immediateFirstPublish));
  }

  public after(deltaTime: number): Stream<T> {
    return stacked(this._open, (out) => {
      let mark = Date.now();
      return {
        push: (value) => {
          const now = Date.now();
          if (now - mark >= deltaTime) {
            mark = now;
            out(value);
          }
        },
      };
    });
  }

  public sampled(frequency: number): Stream<T> {
    return this.limit(frequency);
  }
}
