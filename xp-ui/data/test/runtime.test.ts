import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createData, exportNames } from '../src/create.ts';
import { installErrorForwarding, errorEventText } from '../src/errors.ts';
import { FakeHost, FrameDriver, installDom, removeDom } from './fakes.ts';

type Global = Record<string, unknown>;
const g = globalThis as unknown as Global;

afterEach(() => {
  removeDom();
  delete g.xplaneJsData;
  delete g.__xphfbw;
});

const declaredValueExports = [
  'Vars',
  'Datarefs',
  'Commands',
  'Units',
  'Bridge',
  'Channels',
  'openChannel',
  'openRelay',
  'sendFlowEvent',
  'loadStored',
  'saveStored',
  'Text',
  'Declination',
  'SessionPhase',
  'sessionPhase',
  'SoundId',
  'Noop',
  'GeoPosData',
  'GeoPos',
  'GeoPos3',
  'Vec3Data',
  'NavMath',
  'AircraftModel',
  'PlaneState',
  'PanelInstrument',
  'defineInstrument',
  'KeyCodes',
  'InputButtons',
  'MapElement',
  'MapConfig',
];

test('the export object has exactly the declared value exports', () => {
  const data = createData({ host: () => undefined });
  assert.deepEqual(Object.keys(data).sort(), declaredValueExports.slice().sort());
  assert.deepEqual(exportNames.slice().sort(), declaredValueExports.slice().sort());
  for (const name of declaredValueExports) {
    assert.ok((data as unknown as Global)[name] !== undefined, name);
  }
  assert.equal(typeof data.Channels.Channel, 'function');
  assert.equal(typeof data.NavMath.Geo.distance, 'function');
  assert.equal(typeof data.Vars.VarBatch, 'function');
});

test('runtime installs globalThis.xplaneJsData once and does not touch the page before DOMContentLoaded', async () => {
  const dom = installDom();
  dom.setReady('loading');
  const frames = new FrameDriver();
  frames.install();
  const listeners: string[] = [];
  g.addEventListener = (type: string): void => {
    listeners.push(type);
  };
  try {
    await import('../src/runtime.ts');
    const first = g.xplaneJsData as Global;
    assert.ok(first);
    assert.deepEqual(Object.keys(first).sort(), declaredValueExports.slice().sort());
    assert.equal(dom.body.children.length, 0);
    assert.deepEqual(listeners, []);
    assert.equal(frames.pending.length, 1);
    const again = '../src/runtime.ts?again';
    await import(again);
    assert.equal(g.xplaneJsData, first);
    assert.equal(frames.pending.length, 1);
    dom.fireReady();
    assert.deepEqual(listeners.sort(), ['error', 'unhandledrejection']);
  } finally {
    delete g.addEventListener;
  }
});

test('index re-exports every name from globalThis.xplaneJsData', async () => {
  const data = createData({ host: () => undefined });
  g.xplaneJsData = data;
  const mod = (await import('../src/index.ts')) as unknown as Global;
  for (const name of declaredValueExports) {
    assert.equal(mod[name], (data as unknown as Global)[name], name);
  }
  assert.deepEqual(Object.keys(mod).sort(), declaredValueExports.slice().sort());
});

test('error and unhandledrejection events are forwarded to host.log(2)', () => {
  const host = new FakeHost();
  const handlers = new Map<string, (e: unknown) => void>();
  installErrorForwarding({ addEventListener: (type, fn) => handlers.set(type, fn) }, () => host);
  const err = new Error('kaboom');
  handlers.get('error')!({ message: 'Uncaught Error: kaboom', error: err });
  handlers.get('error')!({ message: 'Script error.' });
  handlers.get('unhandledrejection')!({ reason: new Error('rejected') });
  handlers.get('unhandledrejection')!({ reason: 'plain reason' });
  assert.equal(host.logs.length, 4);
  assert.ok(host.logs.every((l) => l[0] === 2));
  assert.equal(host.logs[0][1], 'Uncaught Error: kaboom\n' + err.stack);
  assert.equal(host.logs[1][1], 'Script error.');
  assert.match(host.logs[2][1], /^rejected\n/);
  assert.match(host.logs[2][1], /at /);
  assert.equal(host.logs[3][1], 'plain reason');
});

test('error forwarding without a host does nothing', () => {
  const handlers = new Map<string, (e: unknown) => void>();
  installErrorForwarding({ addEventListener: (type, fn) => handlers.set(type, fn) }, () => undefined);
  handlers.get('error')!({ message: 'x' });
  handlers.get('unhandledrejection')!({ reason: 'y' });
  assert.equal(errorEventText({ message: '', error: new Error('only error') }).startsWith('only error'), true);
});
