import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Units } from '../src/units.ts';

function near(actual: number, expected: number, eps: number = 1e-9): void {
  assert.ok(Math.abs(actual - expected) <= eps * Math.max(1, Math.abs(expected)), actual + ' != ' + expected);
}

const rows: Array<[string, number, string[]]> = [
  ['length', 1, ['meter', 'meters', 'm']],
  ['length', 0.3048, ['foot', 'feet', 'ft']],
  ['length', 1852, ['nautical mile', 'nautical miles', 'nm']],
  ['length', 1000, ['kilometer', 'kilometers', 'km']],
  ['length', 1609.344, ['statute mile', 'statute miles', 'mile', 'miles']],
  ['length', 0.0254, ['inch', 'inches', 'in']],
  ['length', 0.01, ['centimeter', 'centimeters', 'cm']],
  ['length', 0.001, ['millimeter', 'millimeters', 'mm']],
  ['speed', 1, ['meter per second', 'meters per second', 'm/s']],
  ['speed', 1852 / 3600, ['knot', 'knots', 'kt', 'kts']],
  ['speed', 1 / 3.6, ['kilometer per hour', 'kilometers per hour', 'km/h']],
  ['speed', 0.3048, ['feet per second']],
  ['speed', 0.00508, ['feet per minute', 'fpm']],
  ['angle', 1, ['radian', 'radians']],
  ['angle', Math.PI / 180, ['degree', 'degrees', 'deg', 'degree latitude', 'degrees latitude', 'degree longitude', 'degrees longitude']],
  ['mass', 1, ['kilogram', 'kilograms', 'kg']],
  ['mass', 0.45359237, ['pound', 'pounds', 'lbs', 'lb']],
  ['mass', 1000, ['tonne', 'tonnes']],
  ['mass', 14.5939029, ['slug', 'slugs']],
  ['pressure', 1, ['pascal', 'pascals']],
  ['pressure', 100, ['hectopascal', 'hectopascals', 'hpa']],
  ['pressure', 100, ['millibar', 'millibars', 'mbar']],
  ['pressure', 3386.389, ['inches of mercury', 'inch of mercury', 'inhg']],
  ['pressure', 6894.757, ['psi']],
  ['time', 1, ['second', 'seconds', 'sec']],
  ['time', 60, ['minute', 'minutes']],
  ['time', 3600, ['hour', 'hours']],
  ['time', 0.001, ['millisecond', 'milliseconds']],
  ['ratio', 1, ['number']],
  ['ratio', 0.01, ['percent']],
  ['ratio', 1, ['percent over 100']],
  ['ratio', 1, ['part', 'parts']],
  ['volume', 1, ['liter', 'liters']],
  ['volume', 3.785411784, ['gallon', 'gallons']],
  ['volume', 1000, ['cubic meter', 'cubic meters']],
];

const base: Record<string, string> = {
  length: 'meter',
  speed: 'meter per second',
  angle: 'radian',
  mass: 'kilogram',
  pressure: 'pascal',
  time: 'second',
  ratio: 'number',
  volume: 'liter',
};

test('every linear unit converts to its base unit by its factor', () => {
  for (const [dim, factor, names] of rows) {
    for (const name of names) {
      assert.equal(Units.dimension(name), dim, name);
      near(Units.convert(2.5, name, base[dim]), 2.5 * factor);
      near(Units.convert(2.5 * factor, base[dim], name), 2.5);
    }
  }
});

test('names are trimmed and case-insensitive', () => {
  near(Units.convert(1, '  FEET ', 'Meters'), 0.3048);
  near(Units.convert(1, 'Nautical  Mile', 'KM'), 1.852);
  near(Units.convert(1, 'InHg', 'hPa'), 33.86389);
  assert.equal(Units.dimension(' Knots '), 'speed');
});

test('conversions between non-base units', () => {
  near(Units.convert(1, 'nm', 'ft'), 1852 / 0.3048);
  near(Units.convert(100, 'knots', 'km/h'), 185.2);
  near(Units.convert(1000, 'fpm', 'feet per second'), 1000 / 60);
  near(Units.convert(180, 'degrees', 'radians'), Math.PI);
  near(Units.convert(1, 'tonne', 'lbs'), 1000 / 0.45359237);
  near(Units.convert(1013.25, 'millibar', 'inhg'), (1013.25 * 100) / 3386.389);
  near(Units.convert(2, 'hours', 'minutes'), 120);
  near(Units.convert(50, 'percent', 'percent over 100'), 0.5);
  near(Units.convert(1, 'cubic meter', 'gallons'), 1000 / 3.785411784);
});

test('temperature offsets', () => {
  near(Units.convert(0, 'celsius', 'kelvin'), 273.15);
  near(Units.convert(100, 'celsius', 'fahrenheit'), 212);
  near(Units.convert(-40, 'celsius', 'fahrenheit'), -40);
  near(Units.convert(0, 'celsius', 'rankine'), 273.15 * 1.8);
  near(Units.convert(32, 'fahrenheit', 'celsius'), 0);
  near(Units.convert(0, 'kelvin', 'celsius'), -273.15);
  near(Units.convert(491.67, 'rankine', 'celsius'), 0);
  near(Units.convert(300, 'kelvin', 'fahrenheit'), (300 - 273.15) * 1.8 + 32);
  near(Units.convert(500, 'rankine', 'kelvin'), 500 / 1.8);
  assert.equal(Units.dimension('Fahrenheit'), 'temperature');
});

test('flow units each convert only to themselves', () => {
  assert.equal(Units.convert(5, 'gallons per hour', 'gph'), 5);
  assert.equal(Units.convert(5, 'pounds per hour', 'pound per hour'), 5);
  assert.ok(Number.isNaN(Units.convert(5, 'gph', 'pounds per hour')));
  assert.ok(Number.isNaN(Units.convert(5, 'gph', 'gallons')));
  assert.notEqual(Units.dimension('gph'), Units.dimension('pounds per hour'));
});

test('unknown units and dimension mismatches give NaN', () => {
  assert.ok(Number.isNaN(Units.convert(1, 'furlong', 'meter')));
  assert.ok(Number.isNaN(Units.convert(1, 'meter', 'furlong')));
  assert.ok(Number.isNaN(Units.convert(1, 'feet', 'knots')));
  assert.ok(Number.isNaN(Units.convert(1, 'celsius', 'number')));
  assert.ok(Number.isNaN(Units.convert(1, 'mach', 'knots')));
  assert.ok(Number.isNaN(Units.convert(1, '', 'meter')));
  assert.equal(Units.dimension('mach'), undefined);
  assert.equal(Units.dimension('furlong'), undefined);
});

test('same unit returns the value', () => {
  assert.equal(Units.convert(12.345, 'feet', 'ft'), 12.345);
  assert.equal(Units.convert(-7, 'celsius', 'celsius'), -7);
  assert.ok(Number.isNaN(Units.convert(NaN, 'feet', 'meters')));
});
