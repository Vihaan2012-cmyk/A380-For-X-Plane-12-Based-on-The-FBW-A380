import { test, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createData } from '../src/create.ts';
import { FakeHost, FrameDriver, flush, installDom, removeDom } from './fakes.ts';

let host: FakeHost;
let frames: FrameDriver;
let data: ReturnType<typeof createData>;

beforeEach(() => {
  host = new FakeHost();
  frames = new FrameDriver();
  frames.install();
  data = createData({ host: () => host });
});

afterEach(() => {
  removeDom();
});

test('Datarefs.get with and without an index', () => {
  host.setNumber('sim/a', 'raw', 3);
  host.setNumber('sim/b[2]', 'raw', 7);
  assert.equal(data.Datarefs.get('sim/a'), 3);
  assert.equal(data.Datarefs.get('sim/b', 2), 7);
  assert.equal(data.Datarefs.get('sim/b', 0), 0);
  assert.deepEqual(host.named('getVar'), [
    ['getVar', 'sim/a', 'raw'],
    ['getVar', 'sim/b[2]', 'raw'],
    ['getVar', 'sim/b[0]', 'raw'],
  ]);
});

test('Datarefs.set and getText', () => {
  data.Datarefs.set('sim/a', 1.5);
  data.Datarefs.set('sim/b', 4, 3);
  host.strings.set('sim/tail', 'F-HPJA');
  assert.equal(data.Datarefs.getText('sim/tail'), 'F-HPJA');
  assert.deepEqual(host.named('setVar'), [
    ['setVar', 'sim/a', 'raw', 1.5],
    ['setVar', 'sim/b[3]', 'raw', 4],
  ]);
  assert.deepEqual(host.named('getString'), [['getString', 'sim/tail']]);
});

test('Datarefs.watch is a raw watch on the indexed name', () => {
  const seen: number[] = [];
  host.setNumber('sim/c[1]', 'raw', 9);
  const h = data.Datarefs.watch('sim/c', (v) => seen.push(v), 1);
  frames.step();
  frames.step();
  host.setNumber('sim/c[1]', 'raw', 10);
  frames.step();
  assert.deepEqual(seen, [9, 10]);
  h.dispose();
  host.setNumber('sim/c[1]', 'raw', 11);
  frames.step();
  assert.deepEqual(seen, [9, 10]);
  assert.ok(host.named('getVar').every((c) => c[1] === 'sim/c[1]' && c[2] === 'raw'));
});

test('Commands once, begin and end', () => {
  data.Commands.once('sim/a');
  data.Commands.begin('sim/b');
  data.Commands.end('sim/b');
  assert.deepEqual(host.named('sendEvent'), [
    ['sendEvent', 'CMD:sim/a', [0]],
    ['sendEvent', 'CMD:sim/b', [1]],
    ['sendEvent', 'CMD:sim/b', [2]],
  ]);
});

test('sessionPhase is flying', () => {
  assert.equal(data.sessionPhase(), 3);
  assert.equal(data.sessionPhase(), data.SessionPhase.flying);
});

test('loadStored maps empty text to null, saveStored stores String(value)', () => {
  assert.equal(data.loadStored('missing'), null);
  assert.equal(data.saveStored('k', 12), undefined);
  assert.equal(data.loadStored('k'), '12');
  assert.deepEqual(host.named('storedData'), [
    ['storedData', 'get', 'missing', ''],
    ['storedData', 'set', 'k', '12'],
    ['storedData', 'get', 'k', ''],
  ]);
});

test('Text.localize returns its key unchanged', () => {
  assert.equal(data.Text.localize('TT:MENU'), 'TT:MENU');
  assert.equal(data.Text.localize(null as unknown as string), null);
});

test('Text.newGuid is a lower case RFC 4122 v4 UUID', () => {
  const seen = new Set<string>();
  for (let i = 0; i < 200; i++) {
    const g = data.Text.newGuid();
    assert.match(g, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
    seen.add(g);
  }
  assert.equal(seen.size, 200);
});

test('Declination.at uses magVar, 0 without a host', () => {
  assert.equal(data.Declination.at(10, 2000), 12);
  assert.deepEqual(host.named('magVar'), [['magVar', 10, 2000]]);
  assert.equal(createData({ host: () => undefined }).Declination.at(10, 20), 0);
});

test('Noop functions', () => {
  assert.equal(typeof data.Noop.Void, 'function');
  assert.equal(data.Noop.Void(), undefined);
  assert.equal(data.Noop.Boolean(true), undefined);
  assert.equal(data.Noop.Boolean.length, 1);
});

test('SoundId', () => {
  const a = new data.SoundId();
  assert.equal(a.str, '');
  assert.equal(a.value, 0);
  const b = new data.SoundId('chime');
  assert.equal(b.str, 'chime');
  assert.equal(b.value, 0);
});

test('GeoPosData, GeoPos and GeoPos3 constructors', () => {
  const d = new data.GeoPosData();
  assert.deepEqual([d.lat, d.long], [0, 0]);
  const d2 = new data.GeoPosData(1, 2);
  assert.deepEqual([d2.lat, d2.long], [1, 2]);
  const p = new data.GeoPos(3, 4);
  assert.deepEqual([p.lat, p.long], [3, 4]);
  assert.ok(p instanceof data.GeoPosData);
  const p2 = new data.GeoPos({ lat: 5, long: 6 });
  assert.deepEqual([p2.lat, p2.long], [5, 6]);
  const p3 = new data.GeoPos();
  assert.deepEqual([p3.lat, p3.long], [0, 0]);
  const g = new data.GeoPos3(1, 2);
  assert.deepEqual([g.lat, g.long, g.alt], [1, 2, 0]);
  const g2 = new data.GeoPos3(1, 2, 300);
  assert.equal(g2.alt, 300);
  const g3 = new data.GeoPos3({ lat: 7, long: 8, alt: 9 });
  assert.deepEqual([g3.lat, g3.long, g3.alt], [7, 8, 9]);
  const g4 = new data.GeoPos3({ lat: 7, long: 8 }, undefined, 55);
  assert.equal(g4.alt, 0);
  const ll = g3.toLatLong();
  assert.ok(ll instanceof data.GeoPos);
  assert.deepEqual([ll.lat, ll.long], [7, 8]);
  assert.equal('alt' in ll, false);
});

test('Vec3Data fields are 0', () => {
  const v = new data.Vec3Data();
  assert.deepEqual({ ...v }, { x: 0, y: 0, z: 0 });
});

test('enum values', () => {
  assert.equal(data.SessionPhase.mainmenu, 0);
  assert.equal(data.SessionPhase.loading, 1);
  assert.equal(data.SessionPhase.briefing, 2);
  assert.equal(data.SessionPhase.flying, 3);
  assert.equal(data.SessionPhase[3], 'flying');
  assert.deepEqual(
    [data.AircraftModel.CJ4, data.AircraftModel.A320, data.AircraftModel.B747_8, data.AircraftModel.AS01B, data.AircraftModel.AS02A],
    [0, 1, 2, 3, 4],
  );
  assert.equal(data.AircraftModel[2], 'B747_8');
  const k = data.KeyCodes;
  assert.deepEqual(
    [k.KEY_CANCEL, k.KEY_HELP, k.KEY_BACK_SPACE, k.KEY_TAB, k.KEY_CLEAR, k.KEY_RETURN, k.KEY_ENTER, k.KEY_SHIFT, k.KEY_CONTROL, k.KEY_ALT],
    [3, 6, 8, 9, 12, 13, 13, 16, 17, 18],
  );
  assert.deepEqual(
    [k.KEY_PAUSE, k.KEY_CAPS_LOCK, k.KEY_ESCAPE, k.KEY_SPACE, k.KEY_PAGE_UP, k.KEY_PAGE_DOWN, k.KEY_END, k.KEY_HOME],
    [19, 20, 27, 32, 33, 34, 35, 36],
  );
  assert.deepEqual(
    [k.KEY_LEFT, k.KEY_UP, k.KEY_RIGHT, k.KEY_DOWN, k.KEY_DELETE, k.KEY_0, k.KEY_9, k.KEY_A, k.KEY_P, k.KEY_Z],
    [37, 38, 39, 40, 46, 48, 57, 65, 80, 90],
  );
  assert.equal(k[13], 'KEY_ENTER');
  const names = Object.keys(k).filter((n) => !/^\d+$/.test(n));
  assert.equal(names.length, 28);
});

test('InputButtons.MENU_A', () => {
  assert.equal(data.InputButtons.MENU_A, 'MENU_VALID');
});

test('MapConfig.load calls back asynchronously; applyTo does nothing', async () => {
  const cfg = new data.MapConfig();
  let called = 0;
  cfg.load('/x.json', () => called++);
  assert.equal(called, 0);
  await flush();
  assert.equal(called, 1);
  assert.equal(cfg.applyTo({}), undefined);
});

test('MapElement is an element whose methods do nothing', () => {
  removeDom();
  installDom();
  const d = createData({ host: () => host });
  const el = new d.MapElement();
  assert.ok(el instanceof (globalThis as unknown as { HTMLElement: new () => object }).HTMLElement);
  assert.equal(el.configId, 0);
  assert.deepEqual(el.configs, []);
  assert.equal(el.setMapId(1), undefined);
  assert.equal(el.setConfig(2), undefined);
  assert.equal(el.setParams({ lla: new d.GeoPos3(1, 2, 3), radius: 10 }), undefined);
  assert.equal(el.setVisible(true), undefined);
  assert.equal(host.calls.length, 0);
});

test('PlaneState number getters read the specified names and units', () => {
  const ps = data.PlaneState;
  const rows: Array<[() => unknown, string, string]> = [
    [() => ps.altitude(), 'fbw/INDICATED_ALTITUDE', 'feet'],
    [() => ps.heightAboveGround(), 'fbw/PLANE_ALT_ABOVE_GROUND_MINUS_CG', 'feet'],
    [() => ps.outsideAirTemp(), 'sim/weather/aircraft/temperature_ambient_deg_c', 'celsius'],
    [() => ps.groundSpeed(), 'fbw/GPS_GROUND_SPEED', 'knots'],
    [() => ps.indicatedSpeed(), 'sim/flightmodel/position/indicated_airspeed', 'knots'],
    [() => ps.verticalSpeed(), 'fbw/VERTICAL_SPEED', 'feet per minute'],
    [() => ps.baroSetting(), 'fbw/KOHLSMAN_SETTING_HG_1', 'inches of mercury'],
    [() => ps.baroSetting('millibar'), 'fbw/KOHLSMAN_SETTING_HG_1', 'millibar'],
    [() => ps.apSpeedTarget(), 'fbw/AUTOPILOT_AIRSPEED_HOLD_VAR', 'knots'],
    [() => ps.apMachTarget(), 'fbw/AUTOPILOT_MACH_HOLD_VAR', 'number'],
    [() => ps.apHeadingTarget(), 'fbw/AUTOPILOT_HEADING_LOCK_DIR', 'degrees'],
    [() => ps.apHeadingTarget(false), 'fbw/AUTOPILOT_HEADING_LOCK_DIR', 'degrees'],
    [() => ps.apHeadingTarget(true), 'fbw/AUTOPILOT_HEADING_LOCK_DIR', 'radians'],
    [() => ps.apAltitudeTarget(), 'fbw/AUTOPILOT_ALTITUDE_LOCK_VAR_3', 'feet'],
    [() => ps.apAltitudeTarget('meters'), 'fbw/AUTOPILOT_ALTITUDE_LOCK_VAR_3', 'meters'],
    [() => ps.greenDotSpeed(), 'L:A32NX_SPEEDS_GD', 'number'],
  ];
  let value = 10.5;
  for (const [get, name, unit] of rows) {
    host.calls = [];
    host.setNumber(name, unit, value);
    assert.equal(get(), value, name + ' ' + unit);
    assert.deepEqual(host.calls, [['getVar', name, unit]]);
    value += 1;
  }
});

test('PlaneState boolean and text getters', () => {
  const ps = data.PlaneState;
  const check = (get: () => unknown, name: string, unit: string, cases: Array<[number, unknown]>): void => {
    for (const [raw, expected] of cases) {
      host.calls = [];
      host.setNumber(name, unit, raw);
      assert.equal(get(), expected, name + ' = ' + raw);
      assert.deepEqual(host.calls, [['getVar', name, unit]]);
    }
  };
  check(() => ps.onGround(), 'fbw/SIM_ON_GROUND', 'bool', [[0, false], [1, true], [-1, true]]);
  check(() => ps.baroMode(data.AircraftModel.A320), 'fbw/KOHLSMAN_SETTING_STD_1', 'bool', [[0, 'QNH'], [1, 'STD']]);
  check(() => ps.baroUnits(), 'L:XMLVAR_Baro_Selector_HPA_1', 'bool', [[0, 'inches of mercury'], [1, 'millibar']]);
  check(() => ps.apSpeedSelected(), 'fbw/AUTOPILOT_SPEED_SLOT_INDEX', 'number', [[0, false], [1, true], [2, false]]);
  check(() => ps.apSpeedManaged(), 'fbw/AUTOPILOT_SPEED_SLOT_INDEX', 'number', [[0, false], [1, false], [2, true]]);
  check(() => ps.apMachMode(), 'fbw/AUTOPILOT_MANAGED_SPEED_IN_MACH', 'bool', [[0, false], [1, true]]);
  check(() => ps.apAltitudeManaged(), 'fbw/AUTOPILOT_ALTITUDE_SLOT_INDEX', 'number', [[1, false], [2, true], [3, false]]);
});
