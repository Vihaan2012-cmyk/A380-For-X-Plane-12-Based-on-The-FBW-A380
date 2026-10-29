import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createData } from '../src/create.ts';

const data = createData({ host: () => undefined });
const Geo = data.NavMath.Geo;
const R = 3440.065;
const rad = (d: number): number => (d * Math.PI) / 180;
const deg = (r: number): number => (r * 180) / Math.PI;

function near(actual: number, expected: number, eps: number = 1e-9): void {
  assert.ok(Math.abs(actual - expected) <= eps, actual + ' != ' + expected);
}

function refWrap(a: number): number {
  let r = a;
  while (r < 0) r += 360;
  while (r >= 360) r -= 360;
  return r;
}

function refHaversine(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const a = Math.sin(rad(lat2 - lat1) / 2) ** 2 + Math.cos(rad(lat1)) * Math.cos(rad(lat2)) * Math.sin(rad(lon2 - lon1) / 2) ** 2;
  return 2 * R * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
}

function refBearing(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const y = Math.sin(rad(lon2 - lon1)) * Math.cos(rad(lat2));
  const x = Math.cos(rad(lat1)) * Math.sin(rad(lat2)) - Math.sin(rad(lat1)) * Math.cos(rad(lat2)) * Math.cos(rad(lon2 - lon1));
  return refWrap(deg(Math.atan2(y, x)));
}

const points: Array<[number, number]> = [
  [51.47, -0.4543],
  [40.6413, -73.7781],
  [25.2532, 55.3657],
  [-33.9399, 151.1753],
  [1.3644, 103.9915],
  [0, 179.9],
  [0, -179.9],
  [-89, 10],
];

test('DEG_TO_RAD and RAD_TO_DEG', () => {
  assert.equal(Geo.DEG_TO_RAD, Math.PI / 180);
  assert.equal(Geo.RAD_TO_DEG, 180 / Math.PI);
});

test('wrapDegrees reduces into [0, 360)', () => {
  const cases: number[] = [0, 359.5, 360, 361, -1, -360, -721, 720.25, 1e6 + 0.5, -0.25];
  for (const a of cases) {
    const r = Geo.wrapDegrees(a);
    assert.ok(r >= 0 && r < 360, String(a));
    near(r, refWrap(a), 1e-9);
  }
});

test('angleDelta is b - a reduced into (-180, 180]', () => {
  const cases: Array<[number, number]> = [
    [10, 20],
    [20, 10],
    [350, 10],
    [10, 350],
    [0, 180],
    [180, 0],
    [90, 270],
    [-720, 45],
  ];
  for (const [a, b] of cases) {
    let ref = refWrap(b - a);
    if (ref > 180) ref -= 360;
    const d = Geo.angleDelta(a, b);
    assert.ok(d > -180 && d <= 180);
    near(d, ref);
  }
  assert.equal(Geo.angleDelta(0, 180), 180);
  assert.equal(Geo.angleDelta(180, 0), 180);
});

test('lerpDegrees follows the shortest arc', () => {
  near(Geo.lerpDegrees(350, 10, 0.5), 0);
  near(Geo.lerpDegrees(10, 350, 0.25), 5);
  near(Geo.lerpDegrees(0, 90, 0.5), 45);
  for (const [f, t, k] of [[300, 40, 0.3], [40, 300, 0.7], [123, 321, 0.1]] as Array<[number, number, number]>) {
    let delta = refWrap(t - f);
    if (delta > 180) delta -= 360;
    near(Geo.lerpDegrees(f, t, k), refWrap(f + delta * k));
  }
});

test('distance is the haversine great-circle distance in nautical miles', () => {
  for (const p of points) {
    for (const q of points) {
      const d = Geo.distance(new data.GeoPosData(p[0], p[1]), { lat: q[0], long: q[1] });
      near(d, refHaversine(p[0], p[1], q[0], q[1]), 1e-6);
    }
  }
  near(Geo.distance({ lat: 0, long: 0 }, { lat: 0, long: 1 }), (R * Math.PI) / 180, 1e-9);
});

test('bearing is the initial great-circle course in [0, 360)', () => {
  for (const p of points) {
    for (const q of points) {
      if (p === q) continue;
      const b = Geo.bearing({ lat: p[0], long: p[1] }, { lat: q[0], long: q[1] });
      assert.ok(b >= 0 && b < 360);
      near(b, refBearing(p[0], p[1], q[0], q[1]), 1e-9);
    }
  }
  near(Geo.bearing({ lat: 0, long: 0 }, { lat: 1, long: 0 }), 0);
  near(Geo.bearing({ lat: 0, long: 0 }, { lat: 0, long: 1 }), 90);
  near(Geo.bearing({ lat: 0, long: 0 }, { lat: -1, long: 0 }), 180);
  near(Geo.bearing({ lat: 0, long: 0 }, { lat: 0, long: -1 }), 270);
});

test('destination follows the great circle and returns a GeoPos3', () => {
  const cases: Array<[number, number, number, number]> = [
    [45, 100, 51.47, -0.4543],
    [270, 3000, 40.6413, -73.7781],
    [90, 120, 0, 179.5],
    [270, 120, 0, -179.5],
    [180, 600, 10, 20],
    [0, 60, 0, 0],
  ];
  for (const [brg, dist, lat, lon] of cases) {
    const p = Geo.destination(brg, dist, lat, lon);
    assert.ok(p instanceof data.GeoPos3);
    assert.equal(p.alt, 0);
    assert.ok(p.long >= -180 && p.long <= 180);
    const d = dist / R;
    const lat2 = Math.asin(Math.sin(rad(lat)) * Math.cos(d) + Math.cos(rad(lat)) * Math.sin(d) * Math.cos(rad(brg)));
    let lon2 = deg(rad(lon) + Math.atan2(Math.sin(rad(brg)) * Math.sin(d) * Math.cos(rad(lat)), Math.cos(d) - Math.sin(rad(lat)) * Math.sin(lat2)));
    while (lon2 > 180) lon2 -= 360;
    while (lon2 < -180) lon2 += 360;
    near(p.lat, deg(lat2), 1e-9);
    near(p.long, lon2, 1e-9);
    near(refHaversine(lat, lon, p.lat, p.long), dist, 1e-6);
  }
  const north = Geo.destination(0, 60, 0, 0);
  near(north.lat, deg(60 / R), 1e-12);
  near(north.long, 0, 1e-12);
});

test('adfBcd32 packs round(f * 10) into nibbles', () => {
  const ref = (f: number): number => {
    const s = String(Math.round(f * 10));
    let r = 0;
    for (const ch of s) r = r * 16 + Number(ch);
    return r;
  };
  assert.equal(Geo.adfBcd32(345.0), 0x3450);
  assert.equal(Geo.adfBcd32(1750.5), 0x17505);
  assert.equal(Geo.adfBcd32(190), 0x1900);
  assert.equal(Geo.adfBcd32(0), 0);
  for (const f of [200, 415.5, 999.9, 1234.56, 530.04]) {
    assert.equal(Geo.adfBcd32(f), ref(f));
  }
});
