export class Maths {
  static round(value: number, precision = 1): number {
    if (!(precision > 0)) {
      return value;
    }
    const steps = Math.round(value / precision);
    const out = steps * precision;
    const places = decimals(precision);
    return places > 0 ? Number(out.toFixed(Math.min(places, 100))) : out;
  }

  static angleDiff(start: number, end: number): number {
    const wrapped = (((end - start) % 360) + 360) % 360;
    return wrapped === 0 ? 0 : wrapped;
  }
}

export function decimals(step: number): number {
  const text = String(step);
  const expAt = text.indexOf('e-');
  if (expAt >= 0) {
    const mantissa = text.slice(0, expAt);
    const dot = mantissa.indexOf('.');
    return Number(text.slice(expAt + 2)) + (dot >= 0 ? mantissa.length - dot - 1 : 0);
  }
  const dot = text.indexOf('.');
  return dot < 0 ? 0 : text.length - dot - 1;
}

export class Bits {
  static any(flags: number, mask: number): boolean {
    return (flags & mask) !== 0;
  }

  static all(flags: number, mask: number): boolean {
    return (flags & mask) === mask;
  }
}

export class Arrays {
  static flat<T>(array: readonly (T | readonly T[])[]): T[] {
    const out: T[] = [];
    for (const item of array) {
      if (Array.isArray(item)) {
        out.push(...(item as readonly T[]));
      } else {
        out.push(item as T);
      }
    }
    return out;
  }
}

export class Isa {
  static isaTemp(altitude: number): number {
    const km = altitude / 1000;
    if (km <= 11) {
      return 15 - 6.5 * km;
    }
    if (km <= 20) {
      return -56.5;
    }
    if (km <= 32) {
      return -56.5 + (km - 20);
    }
    if (km <= 47) {
      return -44.5 + 2.8 * (km - 32);
    }
    return -2.5;
  }
}

export class Smoother {
  private state: number | null;

  constructor(
    private readonly timeConstant: number,
    initial: number | null = null,
    private readonly maxGap: number = Infinity,
  ) {
    this.state = initial;
  }

  push(value: number, dt: number): number {
    const previous = this.state;
    if (previous === null || dt > this.maxGap || !(this.timeConstant > 0)) {
      this.state = value;
    } else {
      const weight = 1 - Math.exp(-Math.max(dt, 0) / this.timeConstant);
      this.state = previous + (value - previous) * weight;
    }
    return this.state;
  }

  latest(): number | null {
    return this.state;
  }
}

export class LookupTable {
  private readonly axes: number[][] = [];
  private readonly gridValues = new Map<string, number>();

  constructor(breakpoints: readonly (readonly number[])[]) {
    const dims = breakpoints.length > 0 ? breakpoints[0].length - 1 : 0;
    for (let d = 0; d < dims; d++) {
      const seen = new Set<number>();
      for (const row of breakpoints) {
        seen.add(row[d + 1]);
      }
      this.axes.push([...seen].sort((a, b) => a - b));
    }
    for (const row of breakpoints) {
      this.gridValues.set(row.slice(1).join('|'), row[0]);
    }
  }

  at(...key: number[]): number {
    return this.evaluate(key, []);
  }

  private evaluate(key: readonly number[], fixed: number[]): number {
    const d = fixed.length;
    if (d === this.axes.length) {
      return this.gridValues.get(fixed.join('|')) ?? NaN;
    }
    const axis = this.axes[d];
    const q = key[d];
    const last = axis.length - 1;
    if (q <= axis[0]) {
      return this.evaluate(key, [...fixed, axis[0]]);
    }
    if (q >= axis[last]) {
      return this.evaluate(key, [...fixed, axis[last]]);
    }
    let hi = 1;
    while (axis[hi] < q) {
      hi++;
    }
    const lowAt = axis[hi - 1];
    const highAt = axis[hi];
    const lowVal = this.evaluate(key, [...fixed, lowAt]);
    const highVal = this.evaluate(key, [...fixed, highAt]);
    if (Number.isNaN(lowVal)) {
      return highVal;
    }
    if (Number.isNaN(highVal)) {
      return lowVal;
    }
    return lowVal + ((q - lowAt) / (highAt - lowAt)) * (highVal - lowVal);
  }
}

export interface Translate3d {
  set(x: number, y: number, z: number, xPrecision?: number, yPrecision?: number, zPrecision?: number): void;
  css(): string;
}

class Translate3dBuilder implements Translate3d {
  private readonly parts: [number, number, number] = [0, 0, 0];

  constructor(private readonly suffix: string) {}

  set(x: number, y: number, z: number, xPrecision?: number, yPrecision?: number, zPrecision?: number): void {
    const snap = (v: number, step?: number) => (step === undefined || !(step > 0) ? v : Math.round(v / step) * step);
    this.parts[0] = snap(x, xPrecision);
    this.parts[1] = snap(y, yPrecision);
    this.parts[2] = snap(z, zPrecision);
  }

  css(): string {
    return `translate3d(${this.parts.map((p) => `${p}${this.suffix}`).join(', ')})`;
  }
}

export class Transforms {
  static translate3d(unit = 'px'): Translate3d {
    return new Translate3dBuilder(unit);
  }
}

export interface ReadonlyVec extends Readonly<Omit<Float64Array, 'set' | 'copyWithin' | 'fill' | 'reverse' | 'sort'>> {
  readonly [n: number]: number;
}

export class Vec2D {
  static make(): Float64Array;
  static make(x: number, y: number): Float64Array;
  static make(x = 0, y = 0): Float64Array {
    return Float64Array.of(x, y);
  }

  static set(x: number, y: number, vec: Float64Array): Float64Array {
    vec.set([x, y]);
    return vec;
  }

  static fromPolar(r: number, theta: number, vec: Float64Array): Float64Array {
    return Vec2D.set(r * Math.cos(theta), r * Math.sin(theta), vec);
  }

  static angle(vec: ReadonlyVec): number {
    return Math.atan2(vec[1], vec[0]);
  }

  static magnitude(vec: ReadonlyVec): number {
    return Math.hypot(vec[0], vec[1]);
  }
}

export interface GeoPointLong {
  lat: number;
  long: number;
}

export interface GeoPoint {
  lat: number;
  lon: number;
}
