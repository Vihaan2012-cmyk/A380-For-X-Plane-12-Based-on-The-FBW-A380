export enum Dimension {
  Distance = 'distance',
  Angle = 'angle',
  Weight = 'weight',
}

export interface Measure<F extends string> {
  to(value: number, toUnit: Measure<F>): number;
  from(value: number, fromUnit: Measure<F>): number;
  quantity(value: number): Quantity<F, this>;
}

export interface Quantity<F extends string, U extends Measure<F> = Measure<F>> {
  readonly amount: number;
  inUnit(unit: Measure<F>): number;
}

export class LinearMeasure<F extends string> implements Measure<F> {
  constructor(
    readonly family: F,
    readonly name: string,
    readonly baseFactor: number,
  ) {}

  to(value: number, toUnit: Measure<F>): number {
    return toUnit.from(value, this);
  }

  from(value: number, fromUnit: Measure<F>): number {
    const source = fromUnit as LinearMeasure<F>;
    return (value * source.baseFactor) / this.baseFactor;
  }

  quantity(value: number): Quantity<F, this> {
    return new UnitQuantity<F, this>(value, this);
  }
}

class UnitQuantity<F extends string, U extends Measure<F>> implements Quantity<F, U> {
  constructor(
    readonly amount: number,
    readonly unit: U,
  ) {}

  inUnit(unit: Measure<F>): number {
    return unit === this.unit ? this.amount : this.unit.to(this.amount, unit);
  }
}

export class UnitList {
  static readonly METER = new LinearMeasure(Dimension.Distance, 'meter', 1);
  static readonly FOOT = new LinearMeasure(Dimension.Distance, 'foot', 0.3048);
  static readonly KILOMETER = new LinearMeasure(Dimension.Distance, 'kilometer', 1000);
  static readonly NMILE = new LinearMeasure(Dimension.Distance, 'nautical mile', 1852);
  static readonly DEGREE = new LinearMeasure(Dimension.Angle, 'degree', 1);
  static readonly KILOGRAM = new LinearMeasure(Dimension.Weight, 'kilogram', 1);
  static readonly POUND = new LinearMeasure(Dimension.Weight, 'pound', 0.45359237);
  static readonly TONNE = new LinearMeasure(Dimension.Weight, 'tonne', 1000);
}
