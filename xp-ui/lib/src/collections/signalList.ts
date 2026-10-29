import type { Bond } from 'xplane-js/ui';
import { ListenerSet } from './listeners';

export enum ListChange {
  Added = 'Added',
  Removed = 'Removed',
  Cleared = 'Cleared',
}

export type ListListener<T> = (
  index: number,
  type: ListChange,
  item: T | readonly T[] | undefined,
  array: readonly T[],
) => void;

export interface ReadableList<T> {
  readonly length: number;
  get(index: number): T;
  snapshot(): readonly T[];
  watch(handler: ListListener<T>, initialNotify?: boolean, paused?: boolean): Bond;
}

export class SignalList<T> implements ReadableList<T> {
  private contents: readonly T[];
  private readonly listeners = new ListenerSet<ListListener<T>>();

  protected constructor(initial: readonly T[]) {
    this.contents = initial.slice();
  }

  static of<T>(arr?: T[]): SignalList<T> {
    return new SignalList<T>(arr ?? []);
  }

  get length(): number {
    return this.contents.length;
  }

  get(index: number): T {
    return this.contents[index];
  }

  snapshot(): readonly T[] {
    return this.contents;
  }

  all(): readonly T[] {
    return this.contents;
  }

  set(arr: readonly T[]): void {
    this.contents = [];
    this.announce(0, ListChange.Cleared, undefined, this.contents);
    this.contents = arr.slice();
    this.announce(0, ListChange.Added, this.contents, this.contents);
  }

  insert(item: T, index?: number): void {
    const size = this.contents.length;
    const at = index === undefined || index < 0 || index > size ? size : index;
    this.contents = [...this.contents.slice(0, at), item, ...this.contents.slice(at)];
    this.announce(at, ListChange.Added, item, this.contents);
  }

  removeAt(index: number): void {
    if (!(index >= 0 && index < this.contents.length)) {
      return;
    }
    const gone = this.contents[index];
    this.contents = this.contents.filter((_, i) => i !== index);
    this.announce(index, ListChange.Removed, gone, this.contents);
  }

  clear(): void {
    this.contents = [];
    this.announce(0, ListChange.Cleared, undefined, this.contents);
  }

  watch(handler: ListListener<T>, initialNotify = false, paused = false): Bond {
    return this.listeners.join(handler, initialNotify, paused, (target) => {
      target.fn(0, ListChange.Added, this.contents, this.contents);
    });
  }

  private announce(index: number, type: ListChange, item: T | readonly T[] | undefined, after: readonly T[]): void {
    this.listeners.dispatch((fn) => fn(index, type, item, after));
  }
}

export function makeStateList<T>(arr?: T[]): SignalList<T> {
  return SignalList.of(arr);
}
