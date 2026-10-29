import { ReadableBase, Derived } from 'xplane-js/ui';
import type { DerivedReadable, Quantity, Readable, Measure } from 'xplane-js/ui';

type UnitCarrier<F extends string> = { readonly unit?: Measure<F> };

export class QuantitySignal<F extends string, U extends Measure<F> = Measure<F>> extends ReadableBase<Quantity<F, U>> {
  private current: Quantity<F, U>;
  private home: Measure<F> | undefined;

  protected constructor(initial: Quantity<F, U>) {
    super();
    this.current = initial;
    this.home = (initial as UnitCarrier<F>).unit;
  }

  static of<F extends string, U extends Measure<F>>(initialVal: Quantity<F, U>): QuantitySignal<F, U> {
    return new QuantitySignal<F, U>(initialVal);
  }

  get(): Quantity<F, U> {
    return this.current;
  }

  set(value: Quantity<F> | number, unit?: Measure<F>): void {
    let next: Quantity<F, U>;
    if (typeof value === 'number') {
      if (this.home === undefined) {
        if (unit === undefined) {
          return;
        }
        this.home = unit;
      }
      const raw = unit === undefined ? value : this.home.from(value, unit);
      next = this.home.quantity(raw) as Quantity<F, U>;
    } else {
      const carried = (value as UnitCarrier<F>).unit;
      if (this.home === undefined && carried !== undefined) {
        this.home = carried;
      }
      if (this.home === undefined) {
        next = value as Quantity<F, U>;
      } else {
        next = this.home.quantity(value.inUnit(this.home)) as Quantity<F, U>;
      }
    }
    const before = this.current.amount;
    const after = next.amount;
    if (before === after || (Number.isNaN(before) && Number.isNaN(after))) {
      return;
    }
    this.current = next;
    this.emit();
  }

  inUnit(unit: Measure<F> | Readable<Measure<F>>): DerivedReadable<number> {
    if (typeof (unit as Readable<Measure<F>>).watch === 'function') {
      return Derived.from(([n, u]) => n.inUnit(u), this, unit as Readable<Measure<F>>) as unknown as DerivedReadable<number>;
    }
    return this.derive((n) => n.inUnit(unit as Measure<F>));
  }
}
