import { test, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createData } from '../src/create.ts';
import { FakeHost, FrameDriver, flush, removeDom } from './fakes.ts';

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

test('read with unit string returns getString', () => {
  host.strings.set('L:TEXT', 'hello');
  assert.equal(data.Vars.read('L:TEXT', 'string'), 'hello');
  assert.equal(data.Vars.read('L:TEXT', 'STRING'), 'hello');
  assert.deepEqual(host.named('getString'), [['getString', 'L:TEXT'], ['getString', 'L:TEXT']]);
});

test('read with unit latlonalt returns a GeoPos3 from three reads', () => {
  host.setNumber('POS_LAT', 'degrees', 51.5);
  host.setNumber('POS_LON', 'degrees', -0.12);
  host.setNumber('POS_ALT', 'feet', 1200);
  const p = data.Vars.read('POS', 'LatLonAlt');
  assert.ok(p instanceof data.GeoPos3);
  assert.equal(p.lat, 51.5);
  assert.equal(p.long, -0.12);
  assert.equal(p.alt, 1200);
  assert.deepEqual(host.named('getVar'), [
    ['getVar', 'POS_LAT', 'degrees'],
    ['getVar', 'POS_LON', 'degrees'],
    ['getVar', 'POS_ALT', 'feet'],
  ]);
});

test('read with unit xyz returns a Vec3Data from three reads', () => {
  host.setNumber('V_X', 'number', 1);
  host.setNumber('V_Y', 'number', 2);
  host.setNumber('V_Z', 'number', 3);
  const v = data.Vars.read('V', 'XYZ');
  assert.ok(v instanceof data.Vec3Data);
  assert.deepEqual({ x: v.x, y: v.y, z: v.z }, { x: 1, y: 2, z: 3 });
  assert.deepEqual(host.named('getVar'), [
    ['getVar', 'V_X', 'number'],
    ['getVar', 'V_Y', 'number'],
    ['getVar', 'V_Z', 'number'],
  ]);
});

test('read with any other unit returns getVar and passes NaN through', () => {
  host.setNumber('sim/x', 'feet', 42);
  host.setNumber('sim/nan', 'Number', NaN);
  assert.equal(data.Vars.read('sim/x', 'feet'), 42);
  assert.ok(Number.isNaN(data.Vars.read('sim/nan', 'Number')));
  assert.deepEqual(host.named('getVar')[1], ['getVar', 'sim/nan', 'Number']);
});

test('write K: and H: names send an event with Number(value) || 0', async () => {
  const r1 = await data.Vars.write('K:TOGGLE', 'number', 5);
  await data.Vars.write('H:BUTTON', 'number', 'abc');
  await data.Vars.write('K:BOOL', 'bool', true);
  assert.equal(r1, undefined);
  assert.deepEqual(host.named('sendEvent'), [
    ['sendEvent', 'K:TOGGLE', [5]],
    ['sendEvent', 'H:BUTTON', [0]],
    ['sendEvent', 'K:BOOL', [1]],
  ]);
  assert.equal(host.named('setVar').length, 0);
});

test('write with unit string calls setString with String(value)', async () => {
  await data.Vars.write('L:NAME', 'string', 12);
  await data.Vars.write('L:NAME2', 'String', 'x');
  assert.deepEqual(host.named('setString'), [
    ['setString', 'L:NAME', '12'],
    ['setString', 'L:NAME2', 'x'],
  ]);
});

test('write with other units calls setVar, booleans as 1/0', async () => {
  await data.Vars.write('L:A', 'bool', true);
  await data.Vars.write('L:B', 'bool', false);
  await data.Vars.write('L:C', 'feet', '12.5');
  await data.Vars.write('L:D', 'number', 3);
  assert.deepEqual(host.named('setVar'), [
    ['setVar', 'L:A', 'bool', 1],
    ['setVar', 'L:B', 'bool', 0],
    ['setVar', 'L:C', 'feet', 12.5],
    ['setVar', 'L:D', 'number', 3],
  ]);
});

test('write resolves only after the host received the write', async () => {
  const p = data.Vars.write('L:Z', 'number', 1);
  assert.equal(host.named('setVar').length, 1);
  assert.equal(await p, undefined);
});

test('readNow and readEnv are read', () => {
  host.setNumber('E:TIME', 'seconds', 7);
  host.strings.set('S', 'txt');
  assert.equal(data.Vars.readNow('E:TIME', 'seconds'), 7);
  assert.equal(data.Vars.readEnv('E:TIME', 'seconds'), 7);
  assert.equal(data.Vars.readNow('S', 'string'), 'txt');
  assert.equal(data.Vars.readEnv('S', 'string'), 'txt');
});

test('readHost rows', () => {
  host.game.set('DB', '2026');
  host.setNumber('CAM_X', 'number', 4);
  host.setNumber('CAM', 'knots', 9);
  assert.equal(data.Vars.readHost('DB', 'string', 1, 2), '2026');
  assert.equal(host.named('getString').length, 0);
  const v = data.Vars.readHost('CAM', 'xyz');
  assert.equal(v.x, 4);
  assert.ok(v instanceof data.Vec3Data);
  assert.equal(data.Vars.readHost('CAM', 'knots', 3, 4), 9);
  const reads = host.named('getVar');
  assert.deepEqual(reads[reads.length - 1], ['getVar', 'CAM', 'knots']);
});

test('register gives stable ids from 0 and readById/readTextById read them', () => {
  const a = data.Vars.register('L:A', 'feet');
  const b = data.Vars.register('L:B', 'number');
  const a2 = data.Vars.register('L:A', 'feet');
  const c = data.Vars.register('L:A', 'meters');
  assert.deepEqual([a, b, a2, c], [0, 1, 0, 2]);
  host.setNumber('L:A', 'feet', 100);
  host.strings.set('L:B', 'bee');
  assert.equal(data.Vars.readById(a), 100);
  assert.equal(data.Vars.readTextById(b), 'bee');
  assert.equal(data.Vars.readById(99), 0);
  assert.equal(data.Vars.readTextById(99), '');
});

test('register of a string unit reads text through readById', () => {
  const id = data.Vars.register('L:S', 'string');
  host.strings.set('L:S', 'abc');
  assert.equal(data.Vars.readById(id), 'abc');
});

test('VarBatch stores names and columns; readBatch answers [] on the next frame', async () => {
  const batch = new data.Vars.VarBatch('COUNT', 'ID');
  batch.add('L:X', 'number');
  batch.add('L:Y', 'feet', 'f64');
  assert.equal(batch.countName, 'COUNT');
  assert.equal(batch.idName, 'ID');
  assert.deepEqual(batch.columns, [
    { name: 'L:X', unit: 'number', type: undefined },
    { name: 'L:Y', unit: 'feet', type: 'f64' },
  ]);
  const rows: unknown[] = [];
  data.Vars.readBatch(batch, (r) => rows.push(r));
  await flush();
  assert.equal(rows.length, 0);
  frames.step();
  assert.deepEqual(rows, [[]]);
  frames.step();
  assert.equal(rows.length, 1);
});

test('watch starts on the next frame, fires first time and on change, NaN equals NaN', () => {
  const seen: unknown[] = [];
  host.setNumber('L:W', 'number', 1);
  const handle = data.Vars.watch('L:W', 'number', (v) => seen.push(v));
  assert.equal(host.named('getVar').length, 0);
  frames.step();
  assert.deepEqual(seen, [1]);
  frames.step();
  assert.deepEqual(seen, [1]);
  host.setNumber('L:W', 'number', 2);
  frames.step();
  assert.deepEqual(seen, [1, 2]);
  host.setNumber('L:W', 'number', NaN);
  frames.step();
  frames.step();
  assert.equal(seen.length, 3);
  assert.ok(Number.isNaN(seen[2]));
  handle.dispose();
  host.setNumber('L:W', 'number', 5);
  frames.step();
  assert.equal(seen.length, 3);
});

test('watch reads once per frame after the snapshot', () => {
  data.Vars.watch('L:W', 'feet', () => {});
  frames.step();
  frames.step();
  const k = host.kinds().filter((x) => x === 'snapshot' || x === 'getVar' || x === 'poll');
  assert.deepEqual(k, ['snapshot', 'poll', 'getVar', 'snapshot', 'poll', 'getVar']);
});

test('watch registered inside a frame begins on the following frame', () => {
  const seen: unknown[] = [];
  host.setNumber('L:A', 'number', 3);
  data.Vars.watch('L:T', 'number', () => {
    if (seen.length === 0) {
      data.Vars.watch('L:A', 'number', (v) => seen.push(['A', v]));
    }
    seen.push('T');
  });
  frames.step();
  assert.deepEqual(seen, ['T']);
  frames.step();
  assert.deepEqual(seen, ['T', ['A', 3]]);
});

test('watch listener errors are logged and do not stop other watches', () => {
  const seen: unknown[] = [];
  data.Vars.watch('L:A', 'number', () => {
    throw new Error('boom');
  });
  data.Vars.watch('L:B', 'number', (v) => seen.push(v));
  frames.step();
  assert.deepEqual(seen, [0]);
  assert.equal(host.logs.length, 1);
  assert.equal(host.logs[0][0], 2);
  assert.match(host.logs[0][1], /boom/);
});

test('without a host reads give 0 or empty text and writes do nothing', async () => {
  const empty = createData({ host: () => undefined });
  assert.equal(empty.Vars.read('L:A', 'number'), 0);
  assert.equal(empty.Vars.read('L:A', 'string'), '');
  const p = empty.Vars.read('P', 'latlonalt');
  assert.deepEqual([p.lat, p.long, p.alt], [0, 0, 0]);
  const v = empty.Vars.read('V', 'xyz');
  assert.deepEqual([v.x, v.y, v.z], [0, 0, 0]);
  assert.equal(empty.Vars.readHost('X', 'string'), '');
  assert.equal(empty.Vars.readHost('X', 'number'), 0);
  assert.equal(await empty.Vars.write('L:A', 'number', 1), undefined);
  const id = empty.Vars.register('L:A', 'number');
  assert.equal(empty.Vars.readById(id), 0);
  assert.equal(empty.Vars.readTextById(id), '');
  const seen: unknown[] = [];
  empty.Vars.watch('L:A', 'number', (x) => seen.push(x));
  frames.step();
  assert.deepEqual(seen, [0]);
});

test('the default host is globalThis.__xphfbw', () => {
  const g = globalThis as { __xphfbw?: FakeHost };
  g.__xphfbw = host;
  try {
    const d = createData();
    host.setNumber('L:G', 'number', 8);
    assert.equal(d.Vars.read('L:G', 'number'), 8);
    delete g.__xphfbw;
    assert.equal(d.Vars.read('L:G', 'number'), 0);
  } finally {
    delete g.__xphfbw;
  }
});
