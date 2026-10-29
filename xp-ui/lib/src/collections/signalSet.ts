import { Signal } from 'xplane-js/ui';
import type { DerivedReadable, Bond } from 'xplane-js/ui';

export interface ReadableSet<T> {
  get(): ReadonlySet<T>;
}

export class SignalSet<T> implements ReadableSet<T> {
  private readonly members: Set<T>;
  private readonly view: Signal<ReadonlySet<T>>;

  protected constructor(initial: Iterable<T>) {
    this.members = new Set(initial);
    this.view = Signal.of<ReadonlySet<T>>(this.members);
  }

  static of<T>(initialKeys?: Iterable<T>): SignalSet<T> {
    return new SignalSet<T>(initialKeys ?? []);
  }

  get(): ReadonlySet<T> {
    return this.members;
  }

  has(key: T): boolean {
    return this.members.has(key);
  }

  add(key: T): this {
    if (!this.members.has(key)) {
      this.members.add(key);
      this.view.emit();
    }
    return this;
  }

  delete(key: T): boolean {
    const removed = this.members.delete(key);
    if (removed) {
      this.view.emit();
    }
    return removed;
  }

  clear(): void {
    for (const key of Array.from(this.members)) {
      this.members.delete(key);
      this.view.emit();
    }
  }

  watch(handler: (set: ReadonlySet<T>) => void, initialNotify?: boolean, paused?: boolean): Bond {
    return this.view.watch(handler, initialNotify, paused);
  }

  derive<M>(fn: (input: ReadonlySet<T>, previousVal?: M) => M, equalityFunc?: (a: M, b: M) => boolean): DerivedReadable<M> {
    return this.view.derive(fn, equalityFunc);
  }
}
