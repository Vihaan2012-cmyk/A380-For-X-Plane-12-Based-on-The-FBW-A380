import { ReadableBase } from 'xplane-js/ui';
import type { Gettable, Bond } from 'xplane-js/ui';
import type { Stream } from './stream';

export class BusSignal<T> extends ReadableBase<T> implements Bond {
  private _current: T;
  private readonly _same: (a: T, b: T) => boolean;
  private _link: Bond | null = null;
  private _live = true;
  private _held = false;
  private _moves = 0;

  public static from<T>(
    consumer: Stream<T> | null,
    initialVal: T,
    equalityFunc?: (a: T, b: T) => boolean,
  ): BusSignal<T> {
    return new BusSignal<T>(consumer, initialVal, equalityFunc);
  }

  private constructor(consumer: Stream<T> | null, initial: T, same?: (a: T, b: T) => boolean) {
    super();
    this._current = initial;
    this._same = same ?? ((a, b) => a === b);
    if (consumer) {
      this.connect(consumer);
    }
  }

  get isLive(): boolean {
    return this._live;
  }

  get isHeld(): boolean {
    return this._held;
  }

  public get(): T {
    return this._current;
  }

  public rebind(consumer: Stream<T> | null): this {
    this.disconnect();
    if (consumer && this._live) {
      this.connect(consumer);
    }
    return this;
  }

  public hold(): this {
    this._held = true;
    if (this._link) {
      this._link.hold();
    }
    return this;
  }

  public release(initialNotify = false): this {
    this._held = false;
    const before = this._moves;
    if (this._link) {
      this._link.release(true);
    }
    if (initialNotify && this._moves === before) {
      this.emit();
    }
    return this;
  }

  public destroy(): void {
    this._live = false;
    this.disconnect();
  }

  private connect(consumer: Stream<T>): void {
    this._link = consumer.listen((v) => {
      if (!this._same(this._current, v)) {
        this._current = v;
        this._moves++;
        this.emit();
      }
    }, this._held);
  }

  private disconnect(): void {
    if (this._link) {
      this._link.destroy();
      this._link = null;
    }
  }
}

export class BusValue<T> implements Gettable<T>, Bond {
  private _current: T;
  private _link: Bond | null = null;
  private _live = true;
  private _held = false;

  public static from<T>(consumer: Stream<T> | null, initialVal: T): BusValue<T> {
    return new BusValue<T>(consumer, initialVal);
  }

  private constructor(consumer: Stream<T> | null, initial: T) {
    this._current = initial;
    if (consumer) {
      this.connect(consumer);
    }
  }

  get isLive(): boolean {
    return this._live;
  }

  get isHeld(): boolean {
    return this._held;
  }

  public get(): T {
    return this._current;
  }

  public rebind(consumer: Stream<T> | null): this {
    this.disconnect();
    if (consumer && this._live) {
      this.connect(consumer);
    }
    return this;
  }

  public hold(): this {
    this._held = true;
    if (this._link) {
      this._link.hold();
    }
    return this;
  }

  public release(): this {
    this._held = false;
    if (this._link) {
      this._link.release(true);
    }
    return this;
  }

  public destroy(): void {
    this._live = false;
    this.disconnect();
  }

  private connect(consumer: Stream<T>): void {
    this._link = consumer.listen((v) => {
      this._current = v;
    }, this._held);
  }

  private disconnect(): void {
    if (this._link) {
      this._link.destroy();
      this._link = null;
    }
  }
}

export function makeBusState<T>(consumer: Stream<T> | null, initialVal: T): BusSignal<T> {
  return BusSignal.from(consumer, initialVal);
}
