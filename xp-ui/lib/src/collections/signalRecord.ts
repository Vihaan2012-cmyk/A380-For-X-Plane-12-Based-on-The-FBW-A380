import { ReadableBase } from 'xplane-js/ui';

export class SignalRecord<T extends object> extends ReadableBase<Readonly<T>> {
  private current: Readonly<T>;

  protected constructor(initial: T) {
    super();
    this.current = { ...initial };
  }

  static of<T extends object>(initialVal: T): SignalRecord<T> {
    return new SignalRecord<T>(initialVal);
  }

  get(): Readonly<T> {
    return this.current;
  }

  set(value: Partial<Readonly<T>>): void {
    const keys = Object.keys(value) as (keyof T)[];
    const differs = keys.some((k) => !Object.is(this.current[k], (value as T)[k]));
    if (differs) {
      this.current = { ...this.current, ...value };
      this.emit();
    }
  }
}
