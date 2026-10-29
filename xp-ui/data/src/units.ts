interface UnitDef {
  dimension: string;
  toBase(value: number): number;
  fromBase(value: number): number;
}

const table = new Map<string, UnitDef>();

function linear(dimension: string, factor: number, names: string[]): void {
  const def: UnitDef = {
    dimension,
    toBase: (v) => v * factor,
    fromBase: (v) => v / factor,
  };
  for (const n of names) {
    table.set(n, def);
  }
}

function custom(dimension: string, toBase: (v: number) => number, fromBase: (v: number) => number, names: string[]): void {
  const def: UnitDef = { dimension, toBase, fromBase };
  for (const n of names) {
    table.set(n, def);
  }
}

linear('length', 1, ['meter', 'meters', 'm']);
linear('length', 0.3048, ['foot', 'feet', 'ft']);
linear('length', 1852, ['nautical mile', 'nm']);
linear('length', 1000, ['kilometer', 'km']);
linear('length', 1609.344, ['statute mile', 'mile']);
linear('length', 0.0254, ['inch', 'in']);
linear('length', 0.01, ['centimeter', 'cm']);
linear('length', 0.001, ['millimeter', 'mm']);

linear('speed', 1, ['meter per second', 'm/s']);
linear('speed', 1852 / 3600, ['knot', 'knots', 'kt']);
linear('speed', 1 / 3.6, ['kilometer per hour', 'km/h']);
linear('speed', 0.3048, ['feet per second']);
linear('speed', 0.00508, ['feet per minute', 'fpm']);

linear('angle', 1, ['radian']);
linear('angle', Math.PI / 180, ['degree', 'degrees', 'deg', 'degree latitude', 'degree longitude']);

linear('mass', 1, ['kilogram', 'kg']);
linear('mass', 0.45359237, ['pound', 'lbs', 'lb']);
linear('mass', 1000, ['tonne']);
linear('mass', 14.5939029, ['slug']);

linear('pressure', 1, ['pascal']);
linear('pressure', 100, ['hectopascal', 'hpa']);
linear('pressure', 100, ['millibar', 'mbar']);
linear('pressure', 3386.389, ['inches of mercury', 'inch of mercury', 'inhg']);
linear('pressure', 6894.757, ['psi']);

linear('time', 1, ['second', 'seconds', 'sec']);
linear('time', 60, ['minute']);
linear('time', 3600, ['hour']);
linear('time', 0.001, ['millisecond']);

custom('temperature', (v) => v, (v) => v, ['celsius']);
custom('temperature', (v) => v - 273.15, (v) => v + 273.15, ['kelvin']);
custom('temperature', (v) => (v - 32) / 1.8, (v) => v * 1.8 + 32, ['fahrenheit']);
custom('temperature', (v) => v / 1.8 - 273.15, (v) => (v + 273.15) * 1.8, ['rankine']);

linear('ratio', 1, ['number']);
linear('ratio', 0.01, ['percent']);
linear('ratio', 1, ['percent over 100']);
linear('ratio', 1, ['part']);

linear('volume', 1, ['liter']);
linear('volume', 3.785411784, ['gallon', 'gallons']);
linear('volume', 1000, ['cubic meter']);

linear('volume flow', 1, ['gallon per hour', 'gallons per hour', 'gph']);
linear('mass flow', 1, ['pound per hour', 'pounds per hour']);

function wordForms(word: string): string[] {
  const forms = [word];
  if (word.length > 2 && word.endsWith('es')) {
    forms.push(word.slice(0, -2));
  }
  if (word.length > 1 && word.endsWith('s')) {
    forms.push(word.slice(0, -1));
  }
  return forms;
}

function lookup(unit: string): UnitDef | undefined {
  if (typeof unit !== 'string') {
    return undefined;
  }
  const key = unit.trim().toLowerCase().replace(/\s+/g, ' ');
  if (key === '') {
    return undefined;
  }
  const direct = table.get(key);
  if (direct) {
    return direct;
  }
  const words = key.split(' ');
  let candidates: string[] = [''];
  for (let i = 0; i < words.length; i++) {
    const next: string[] = [];
    for (const prefix of candidates) {
      for (const form of wordForms(words[i])) {
        next.push(i === 0 ? form : prefix + ' ' + form);
      }
    }
    candidates = next;
  }
  for (const c of candidates) {
    const def = table.get(c);
    if (def) {
      return def;
    }
  }
  return undefined;
}

function convert(value: number, from: string, to: string): number {
  const a = lookup(from);
  const b = lookup(to);
  if (!a || !b || a.dimension !== b.dimension) {
    return NaN;
  }
  if (a === b) {
    return value;
  }
  return b.fromBase(a.toBase(value));
}

function dimension(unit: string): string | undefined {
  const def = lookup(unit);
  return def ? def.dimension : undefined;
}

export const Units = { convert, dimension };
