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

function provider(name: string, payload: unknown): void {
  host.pollQueue.push([['provider', name, typeof payload === 'string' ? payload : JSON.stringify(payload)]]);
}

test('Bridge.call sends JSON args and parses JSON results', async () => {
  host.callImpl = (name, args) => Promise.resolve(JSON.stringify({ name, args: JSON.parse(args) }));
  const r = await data.Bridge.call('nav.find', 'EGLL', 2, { a: true });
  assert.deepEqual(r, { name: 'nav.find', args: ['EGLL', 2, { a: true }] });
  assert.deepEqual(host.named('call'), [['call', 'nav.find', '["EGLL",2,{"a":true}]']]);
});

test("Bridge.call maps 'null' to null and unparsable text to the raw text", async () => {
  host.callImpl = () => Promise.resolve('null');
  assert.equal(await data.Bridge.call('a'), null);
  host.callImpl = () => Promise.resolve('not json {');
  assert.equal(await data.Bridge.call('b'), 'not json {');
  host.callImpl = () => Promise.resolve('"text"');
  assert.equal(await data.Bridge.call('c'), 'text');
  assert.deepEqual(host.named('call')[0], ['call', 'a', '[]']);
});

test('Bridge.call rejects with the host error text', async () => {
  host.callImpl = () => Promise.reject('no such call');
  await assert.rejects(data.Bridge.call('x'), (e: unknown) => e === 'no such call');
});

test('Bridge.call never resolves without a host', async () => {
  const empty = createData({ host: () => undefined });
  let settled = false;
  empty.Bridge.call('x').then(
    () => (settled = true),
    () => (settled = true),
  );
  await flush();
  await flush();
  assert.equal(settled, false);
});

test('Bridge.signal calls the host, ignores the result and swallows rejections', async () => {
  let rejections = 0;
  const onRejection = (): void => {
    rejections++;
  };
  process.on('unhandledRejection', onRejection);
  try {
    host.callImpl = () => Promise.reject('bad');
    assert.equal(data.Bridge.signal('s', 1, 'two'), undefined);
    await flush();
    await flush();
    assert.equal(rejections, 0);
    assert.deepEqual(host.named('call'), [['call', 's', '[1,"two"]']]);
  } finally {
    process.off('unhandledRejection', onRejection);
  }
});

test('sendFlowEvent is Bridge.signal', () => {
  data.sendFlowEvent('flow.x', 1, 2);
  assert.deepEqual(host.named('call'), [['call', 'flow.x', '[1,2]']]);
});

test('provider events dispatch parsed arrays to Bridge listeners in order', () => {
  const seen: unknown[] = [];
  data.Bridge.on('ev', (...a) => seen.push(['1', ...a]));
  data.Bridge.on('ev', (...a) => seen.push(['2', ...a]));
  data.Bridge.on('other', (...a) => seen.push(['o', ...a]));
  provider('ev', [1, 'b', { c: 3 }]);
  frames.step();
  assert.deepEqual(seen, [
    ['1', 1, 'b', { c: 3 }],
    ['2', 1, 'b', { c: 3 }],
  ]);
});

test('provider payload that is not an array is wrapped; invalid JSON becomes [payload]', () => {
  const seen: unknown[][] = [];
  data.Bridge.on('ev', (...a) => seen.push(a));
  host.pollQueue.push([
    ['provider', 'ev', '{"k":1}'],
    ['provider', 'ev', '42'],
    ['provider', 'ev', 'not json'],
    ['provider', 'ev', 'null'],
  ]);
  frames.step();
  assert.deepEqual(seen, [[{ k: 1 }], [42], ['not json'], [null]]);
});

test('game and string poll entries do nothing more', () => {
  const seen: unknown[] = [];
  data.Bridge.on('g', () => seen.push('g'));
  data.Bridge.on('panel.event', () => seen.push('p'));
  host.pollQueue.push([
    ['game', 'g', 'x'],
    ['string', 'g', 'y'],
  ]);
  frames.step();
  assert.deepEqual(seen, []);
});

test('poll is called once per frame after snapshot', () => {
  data.Bridge.on('x', () => {});
  frames.step();
  frames.step();
  assert.deepEqual(host.kinds(), ['snapshot', 'poll', 'snapshot', 'poll']);
});

test("'h' entries reach Bridge listeners of 'panel.event' with the name", () => {
  const seen: unknown[][] = [];
  data.Bridge.on('panel.event', (...a) => seen.push(a));
  host.pollQueue.push([
    ['h', 'A380X_FCU_PUSH', ''],
    ['h', 'A380X_EFIS_CSTR', ''],
  ]);
  frames.step();
  assert.deepEqual(seen, [['A380X_FCU_PUSH'], ['A380X_EFIS_CSTR']]);
});

test('on().clear removes exactly that registration; off removes every registration of fn', () => {
  const seen: string[] = [];
  const f = (): void => {
    seen.push('f');
  };
  const g = (): void => {
    seen.push('g');
  };
  const h1 = data.Bridge.on('e', f);
  data.Bridge.on('e', f);
  data.Bridge.on('e', g);
  h1.clear();
  provider('e', []);
  frames.step();
  assert.deepEqual(seen, ['f', 'g']);
  data.Bridge.on('e', f);
  data.Bridge.off('e', f);
  provider('e', []);
  frames.step();
  assert.deepEqual(seen, ['f', 'g', 'g']);
  h1.clear();
  data.Bridge.off('missing', f);
});

test('listener errors are logged and dispatch continues', () => {
  const seen: string[] = [];
  data.Bridge.on('e', () => {
    throw new Error('listener failed');
  });
  data.Bridge.on('e', () => seen.push('ok'));
  provider('e', []);
  frames.step();
  assert.deepEqual(seen, ['ok']);
  assert.equal(host.logs.length, 1);
  assert.match(host.logs[0][1], /listener failed/);
});

test('openChannel calls onReady once asynchronously', async () => {
  let ready = 0;
  data.openChannel('c', () => ready++);
  assert.equal(ready, 0);
  await flush();
  assert.equal(ready, 1);
  await flush();
  assert.equal(ready, 1);
});

test('channel.event dispatches to open channels of that name', () => {
  const seen: unknown[] = [];
  const a = data.openChannel('fms');
  const b = data.openChannel('fms');
  const other = data.openChannel('other');
  a.on('update', (...args) => seen.push(['a', ...args]));
  b.on('update', (...args) => seen.push(['b', ...args]));
  other.on('update', (...args) => seen.push(['x', ...args]));
  provider('channel.event', ['fms', 'update', 1, 2]);
  frames.step();
  assert.deepEqual(seen, [
    ['a', 1, 2],
    ['b', 1, 2],
  ]);
});

test('channel.broadcasted unwraps the real event name and arguments', () => {
  const seen: unknown[] = [];
  const c = data.openChannel('sync');
  c.on('topic', (...args) => seen.push(args));
  c.on('channel.broadcasted', () => seen.push('wrong'));
  provider('channel.event', ['sync', 'channel.broadcasted', 'topic', { v: 1 }, 'z']);
  frames.step();
  assert.deepEqual(seen, [[{ v: 1 }, 'z']]);
});

test('channel.event also reaches Bridge listeners of channel.event first', () => {
  const seen: unknown[] = [];
  data.Bridge.on('channel.event', (...args) => seen.push(['bridge', ...args]));
  const c = data.openChannel('n');
  c.on('e', (...args) => seen.push(['chan', ...args]));
  provider('channel.event', ['n', 'e', 5]);
  frames.step();
  assert.deepEqual(seen, [
    ['bridge', 'n', 'e', 5],
    ['chan', 5],
  ]);
});

test('channel off with and without fn, and close', () => {
  const seen: string[] = [];
  const c = data.openChannel('n');
  const f = (): void => {
    seen.push('f');
  };
  const g = (): void => {
    seen.push('g');
  };
  c.on('e', f);
  c.on('e', f);
  c.on('e', g);
  c.off('e', f);
  provider('channel.event', ['n', 'e']);
  frames.step();
  assert.deepEqual(seen, ['g']);
  c.on('e', f);
  c.off('e');
  provider('channel.event', ['n', 'e']);
  frames.step();
  assert.deepEqual(seen, ['g']);
  c.on('e', g);
  c.close();
  provider('channel.event', ['n', 'e']);
  frames.step();
  assert.deepEqual(seen, ['g']);
});

test('channel call, signal and broadcast go through the host', async () => {
  host.callImpl = () => Promise.resolve('[1,2]');
  const c = data.openChannel('fms');
  assert.deepEqual(await c.call('fms.get', 'a'), [1, 2]);
  c.signal('fms.poke', 3);
  c.broadcast('topic', { x: 1 }, 2);
  assert.deepEqual(host.named('call'), [
    ['call', 'fms.get', '["a"]'],
    ['call', 'fms.poke', '[3]'],
    ['call', 'channel.broadcast', '["fms","topic",{"x":1},2]'],
  ]);
});

test('openRelay calls onReady once asynchronously', async () => {
  let ready = 0;
  data.openRelay(() => ready++);
  assert.equal(ready, 0);
  await flush();
  assert.equal(ready, 1);
});

test('relay send signals relay.send with text or JSON', () => {
  const r = data.openRelay();
  r.send('k1', 'already text');
  r.send('k2', { a: [1] });
  r.send('k3', 5);
  assert.deepEqual(host.named('call'), [
    ['call', 'relay.send', '["k1","already text"]'],
    ['call', 'relay.send', '["k2","{\\"a\\":[1]}"]'],
    ['call', 'relay.send', '["k3","5"]'],
  ]);
});

test('relay receive parses JSON text and passes other data unchanged', () => {
  const seen: unknown[] = [];
  const r = data.openRelay();
  r.receive('k', (d) => seen.push(['k', d]));
  r.receive('other', (d) => seen.push(['o', d]));
  const r2 = data.openRelay();
  r2.receive('k', (d) => seen.push(['k2', d]));
  host.pollQueue.push([
    ['provider', 'channel.event', JSON.stringify(['relay', 'k', '{"a":1}'])],
    ['provider', 'channel.event', JSON.stringify(['relay', 'k', 'not json'])],
    ['provider', 'channel.event', JSON.stringify(['relay', 'k', { b: 2 }])],
    ['provider', 'channel.event', JSON.stringify(['relay', 'k', 7])],
    ['provider', 'channel.event', JSON.stringify(['relay', 'k', '"quoted"'])],
  ]);
  frames.step();
  assert.deepEqual(seen, [
    ['k', { a: 1 }],
    ['k2', { a: 1 }],
    ['k', 'not json'],
    ['k2', 'not json'],
    ['k', { b: 2 }],
    ['k2', { b: 2 }],
    ['k', 7],
    ['k2', 7],
    ['k', 'quoted'],
    ['k2', 'quoted'],
  ]);
});

test('panel events go to instruments before Bridge listeners', () => {
  removeDom();
  installDom();
  frames = new FrameDriver();
  frames.install();
  const d = createData({ host: () => host });
  const order: unknown[] = [];
  class Inst extends d.PanelInstrument {
    onPanelEvent(args: string[]): void {
      order.push(['inst', args]);
    }
  }
  d.defineInstrument('panel-inst', Inst);
  d.Bridge.on('panel.event', (n) => order.push(['bridge', n]));
  host.pollQueue.push([['h', 'BTN', '']]);
  frames.step();
  assert.deepEqual(order, [
    ['inst', ['BTN']],
    ['bridge', 'BTN'],
  ]);
});

test("Bridge.call 'var.set' writes the registered pair in the page and resolves undefined", async () => {
  const num = data.Vars.register('L:A', 'feet');
  const flag = data.Vars.register('L:B', 'bool');
  const key = data.Vars.register('K:EVENT', 'number');
  const text = data.Vars.register('L:T', 'string');
  assert.equal(await data.Bridge.call('var.set', num, 12.5), undefined);
  assert.equal(await data.Bridge.call('var.set', flag, true), undefined);
  assert.equal(await data.Bridge.call('var.set', key, 3), undefined);
  assert.equal(await data.Bridge.call('var.set', text, 42), undefined);
  assert.equal(await data.Bridge.call('var.set', 99, 1), undefined);
  assert.deepEqual(host.calls, [
    ['setVar', 'L:A', 'feet', 12.5],
    ['setVar', 'L:B', 'bool', 1],
    ['sendEvent', 'K:EVENT', [3]],
    ['setString', 'L:T', '42'],
  ]);
});

test("Bridge.call 'var.set.text' sets the text of the registered name and resolves undefined", async () => {
  const id = data.Vars.register('L:NAME', 'number');
  assert.equal(await data.Bridge.call('var.set.text', id, 77), undefined);
  assert.equal(await data.Bridge.call('var.set.text', id, 'abc'), undefined);
  assert.equal(await data.Bridge.call('var.set.text', 5, 'x'), undefined);
  assert.deepEqual(host.calls, [
    ['setString', 'L:NAME', '77'],
    ['setString', 'L:NAME', 'abc'],
  ]);
});

test("Bridge.signal and channel calls with 'var.set' names are handled in the page", async () => {
  const a = data.Vars.register('L:A', 'number');
  data.Bridge.signal('var.set', a, 4);
  data.Bridge.signal('var.set.text', a, 'four');
  data.Bridge.signal('var.set', 50, 4);
  const c = data.openChannel('n');
  assert.equal(await c.call('var.set', a, 5), undefined);
  c.signal('var.set.text', a, 'five');
  await flush();
  assert.deepEqual(host.calls, [
    ['setVar', 'L:A', 'number', 4],
    ['setString', 'L:A', 'four'],
    ['setVar', 'L:A', 'number', 5],
    ['setString', 'L:A', 'five'],
  ]);
  assert.equal(host.named('call').length, 0);
});

test("'var.set' calls resolve without a host while other calls never resolve", async () => {
  const empty = createData({ host: () => undefined });
  const id = empty.Vars.register('L:A', 'number');
  assert.equal(await empty.Bridge.call('var.set', id, 1), undefined);
  assert.equal(await empty.Bridge.call('var.set.text', id, 'x'), undefined);
  assert.equal(empty.Bridge.signal('var.set', id, 1), undefined);
});
