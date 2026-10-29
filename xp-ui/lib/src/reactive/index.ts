export type ValueHandler<T> = (value: T) => void;

export interface Bond {
  readonly isLive: boolean;
  readonly isHeld: boolean;
  hold(): this;
  release(initialNotify?: boolean): this;
  destroy(): void;
}

export interface Lifespan {
  adopt(subscription: { destroy(): void }): void;
}

export interface Gettable<T> {
  get(): T;
}

export interface Settable<T, I = T> extends Gettable<T> {
  set(value: I): void;
}

export interface Readable<T> extends Gettable<T> {
  watch(handler: (value: T) => void, initialNotify?: boolean, paused?: boolean): Bond;
  derive<M>(fn: (input: T, previousVal?: M) => M, equalityFunc?: (a: M, b: M) => boolean): DerivedReadable<M>;
  forward(to: Writable<any, T>, paused?: boolean): Bond;
  forward<M>(to: Writable<any, M>, map: (fromVal: T, toVal: M) => M, paused?: boolean): Bond;
}

export interface Writable<T, I = T> extends Readable<T>, Settable<T, I> {}

export interface DerivedReadable<T> extends Readable<T>, Bond {
  scopedTo(lifecycle: Lifespan): this;
}

const sameRef = (a: unknown, b: unknown): boolean => a === b;

class Hook<T> implements Bond {
  private live = true;
  private held: boolean;

  constructor(
    private readonly target: ValueHandler<T>,
    private readonly peek: () => T,
    private readonly unlink: (hook: Hook<T>) => void,
    held: boolean,
  ) {
    this.held = held;
  }

  get isLive(): boolean {
    return this.live;
  }

  get isHeld(): boolean {
    return this.held;
  }

  hold(): this {
    this.held = true;
    return this;
  }

  release(initialNotify = false): this {
    if (!this.live) {
      return this;
    }
    this.held = false;
    if (initialNotify) {
      this.target(this.peek());
    }
    return this;
  }

  destroy(): void {
    if (this.live) {
      this.live = false;
      this.unlink(this);
    }
  }

  deliver(value: T): void {
    if (this.live && !this.held) {
      try {
        this.target(value);
      } catch (err) {
        console.error(err);
      }
    }
  }
}

export abstract class ReadableBase<T> implements Readable<T> {
  private hooks: Hook<T>[] = [];

  abstract get(): T;

  watch(handler: (value: T) => void, initialNotify = false, paused = false): Bond {
    const hook = new Hook<T>(
      handler,
      () => this.get(),
      (h) => {
        const at = this.hooks.indexOf(h);
        if (at >= 0) {
          this.hooks.splice(at, 1);
        }
      },
      paused,
    );
    this.hooks.push(hook);
    if (initialNotify && !paused) {
      handler(this.get());
    }
    return hook;
  }

  derive<M>(fn: (input: T, previousVal?: M) => M, equalityFunc?: (a: M, b: M) => boolean): DerivedReadable<M> {
    return Derived.fromInputs<[T], M>([this], (vals, prev) => fn(vals[0], prev), equalityFunc);
  }

  forward(to: Writable<any, T>, paused?: boolean): Bond;
  forward<M>(to: Writable<any, M>, map: (fromVal: T, toVal: M) => M, paused?: boolean): Bond;
  forward(to: Writable<any, any>, arg1?: boolean | ((a: T, b: any) => any), arg2?: boolean): Bond {
    const convert = typeof arg1 === 'function' ? arg1 : undefined;
    const startHeld = (typeof arg1 === 'function' ? arg2 : arg1) === true;
    const forward = convert ? (v: T) => to.set(convert(v, to.get())) : (v: T) => to.set(v);
    return this.watch(forward, true, startHeld);
  }

  protected emit(): void {
    if (this.hooks.length === 0) {
      return;
    }
    const round = this.hooks.length === 1 ? this.hooks : this.hooks.slice();
    for (let i = 0; i < round.length; i++) {
      round[i].deliver(this.get());
    }
  }
}

export class Signal<T> extends ReadableBase<T> implements Writable<T> {
  private readonly isSame: (a: T, b: T) => boolean;

  protected constructor(protected value: T, equalityFunc?: (a: T, b: T) => boolean) {
    super();
    this.isSame = equalityFunc ?? sameRef;
  }

  static of<T>(initialVal: T, equalityFunc?: (a: T, b: T) => boolean): Signal<T> {
    return new Signal<T>(initialVal, equalityFunc);
  }

  get(): T {
    return this.value;
  }

  set(value: T): void {
    if (!this.isSame(this.value, value)) {
      this.value = value;
      this.emit();
    }
  }

  emit(): void {
    super.emit();
  }
}

type SourceValues<I extends any[]> = { [K in keyof I]: Readable<I[K]> };

export class Derived<I extends any[], T> extends ReadableBase<T> implements DerivedReadable<T> {
  private current!: T;
  private feeds: Bond[] = [];
  private alive = true;
  private halted = false;

  private constructor(
    private readonly sources: readonly Readable<any>[],
    private readonly compute: (inputs: Readonly<I>, previousVal?: T) => T,
    private readonly isSame: (a: T, b: T) => boolean,
  ) {
    super();
    this.current = this.evaluate(undefined);
    this.feeds = sources.map((s) => s.watch(() => this.refresh()));
  }

  static from<I extends any[], T>(mapFunc: (inputs: Readonly<I>, previousVal?: T) => T, ...inputs: { [K in keyof I]: Readable<I[K]> }): Derived<I, T>;
  static from<I extends any[]>(...inputs: { [K in keyof I]: Readable<I[K]> }): Derived<I, Readonly<I>>;
  static from(...args: any[]): Derived<any, any> {
    if (typeof args[0] === 'function') {
      return new Derived<any, any>(args.slice(1), args[0], sameRef);
    }
    const intoTuple = (vals: any[], prev?: any[]) => {
      if (!prev) {
        return vals.slice();
      }
      for (let i = 0; i < vals.length; i++) {
        prev[i] = vals[i];
      }
      return prev;
    };
    return new Derived<any, any>(args, intoTuple as any, () => false);
  }

  static fromInputs<I extends any[], T>(
    sources: SourceValues<I>,
    fn: (inputs: Readonly<I>, previousVal?: T) => T,
    equalityFunc?: (a: T, b: T) => boolean,
  ): Derived<I, T> {
    return new Derived<I, T>(sources as readonly Readable<any>[], fn, equalityFunc ?? sameRef);
  }

  get isLive(): boolean {
    return this.alive;
  }

  get isHeld(): boolean {
    return this.halted;
  }

  get(): T {
    return this.current;
  }

  hold(): this {
    if (this.alive && !this.halted) {
      this.halted = true;
      this.feeds.forEach((f) => f.hold());
    }
    return this;
  }

  release(initialNotify = false): this {
    if (this.alive && this.halted) {
      this.halted = false;
      this.feeds.forEach((f) => f.release());
      const changed = this.refresh();
      if (initialNotify && !changed) {
        this.emit();
      }
    }
    return this;
  }

  destroy(): void {
    if (this.alive) {
      this.alive = false;
      this.feeds.forEach((f) => f.destroy());
      this.feeds = [];
    }
  }

  scopedTo(lifecycle: Lifespan): this {
    lifecycle.adopt(this);
    return this;
  }

  private evaluate(previous: T | undefined): T {
    const vals = this.sources.map((s) => s.get()) as unknown as I;
    return this.compute(vals, previous);
  }

  private refresh(): boolean {
    const next = this.evaluate(this.current);
    if (this.isSame(this.current, next)) {
      return false;
    }
    this.current = next;
    this.emit();
    return true;
  }
}

export class Logic {
  static not<T>(): (input: T) => boolean {
    return (input) => !input;
  }

  static and(): (inputs: readonly boolean[]) => boolean {
    return (inputs) => inputs.length > 0 && inputs.every((v) => !!v);
  }

  static or(): (inputs: readonly boolean[]) => boolean {
    return (inputs) => inputs.some((v) => !!v);
  }

  static nor(): (inputs: readonly boolean[]) => boolean {
    return (inputs) => !inputs.some((v) => !!v);
  }
}

class FixedValue<T> extends ReadableBase<T> {
  constructor(private readonly fixed: T) {
    super();
  }

  get(): T {
    return this.fixed;
  }
}

export class Readables {
  static isReadable<T = any>(value: any): value is Readable<T> {
    if (value instanceof ReadableBase) {
      return true;
    }
    return (
      typeof value === 'object' &&
      value !== null &&
      typeof value.get === 'function' &&
      typeof value.watch === 'function' &&
      typeof value.derive === 'function' &&
      typeof value.forward === 'function'
    );
  }

  static isWritable<T = any, I = T>(value: any): value is Writable<T, I> {
    return Readables.isReadable(value) && typeof (value as any).set === 'function';
  }

  static toReadable<T, IsSubscribable extends boolean>(value: T | Readable<T>, exclude: IsSubscribable): Readable<T> {
    void exclude;
    return Readables.isReadable<T>(value) ? value : new FixedValue<T>(value as T);
  }
}

export class Emitter<T = any, E = any> {
  private listeners: { fn: (sender: T, data: E) => void; held: boolean; live: boolean }[] = [];

  listen(handler: (sender: T, data: E) => void, paused = false): Bond {
    const entry = { fn: handler, held: paused, live: true };
    this.listeners.push(entry);
    const handle: Bond = {
      get isLive() {
        return entry.live;
      },
      get isHeld() {
        return entry.held;
      },
      hold() {
        entry.held = true;
        return handle;
      },
      release() {
        entry.held = false;
        return handle;
      },
      destroy: () => {
        entry.live = false;
        const at = this.listeners.indexOf(entry);
        if (at >= 0) {
          this.listeners.splice(at, 1);
        }
      },
    };
    return handle;
  }

  emit(sender: T, data: E): void {
    for (const entry of this.listeners.slice()) {
      if (entry.live && !entry.held) {
        entry.fn(sender, data);
      }
    }
  }
}

export class Holder<T> {
  private stored!: T;

  private constructor() {}

  static of<T>(initialVal: T): Holder<T> {
    const box = new Holder<T>();
    box.stored = initialVal;
    return box;
  }

  get(): T {
    return this.stored;
  }

  set(value: T): void {
    this.stored = value;
  }
}

export type ClassToggles = { readonly [className: string]: boolean | Readable<boolean> };

export function makeState<T>(initialVal: T): Signal<T> {
  return Signal.of(initialVal);
}

export function makeComputed<I extends any[], T>(
  mapFunc: (inputs: Readonly<I>, previousVal?: T) => T,
  ...inputs: { [K in keyof I]: Readable<I[K]> }
): Derived<I, T> {
  return Derived.from<I, T>(mapFunc, ...inputs);
}
