import { test, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createData } from '../src/create.ts';
import { FakeHost, FrameDriver, installDom, removeDom } from './fakes.ts';
import type { FakeDom } from './fakes.ts';

let host: FakeHost;
let frames: FrameDriver;
let dom: FakeDom;
let data: ReturnType<typeof createData>;

function setup(search: string = ''): void {
  removeDom();
  host = new FakeHost();
  dom = installDom(search);
  frames = new FrameDriver();
  frames.install();
  data = createData({ host: () => host });
}

beforeEach(() => setup());

afterEach(() => {
  removeDom();
});

test('defineInstrument registers the tag and appends one element when ready', () => {
  class A extends data.PanelInstrument {}
  data.defineInstrument('a-inst', A);
  assert.deepEqual(dom.defineCalls, ['a-inst']);
  assert.equal(dom.registry.get('a-inst'), A);
  assert.equal(dom.body.children.length, 1);
  assert.ok(dom.body.children[0] instanceof A);
  assert.equal((dom.body.children[0] as A).localName, 'a-inst');
});

test('defineInstrument ignores an already defined tag', () => {
  class A extends data.PanelInstrument {}
  class B extends data.PanelInstrument {}
  data.defineInstrument('same-tag', A);
  data.defineInstrument('same-tag', B);
  assert.equal(dom.registry.get('same-tag'), A);
  assert.equal(dom.body.children.length, 2);
  assert.ok(dom.body.children[1] instanceof A);
});

test('defineInstrument waits for DOMContentLoaded while loading', () => {
  dom.setReady('loading');
  class A extends data.PanelInstrument {}
  data.defineInstrument('late-inst', A);
  assert.equal(dom.body.children.length, 0);
  assert.equal(dom.readyListeners.length, 1);
  dom.fireReady();
  assert.equal(dom.body.children.length, 1);
});

test('successful attach: onAttach, loaded(true), instrument.ready signal, frame loop', () => {
  const order: string[] = [];
  let pendingInAttach = -1;
  class A extends data.PanelInstrument {
    onAttach(): void {
      order.push('attach');
      pendingInAttach = frames.pending.length;
      super.onAttach();
    }
    protected onPrepare(): void {
      order.push('init');
    }
  }
  assert.equal(frames.pending.length, 1);
  data.defineInstrument('ok-inst', A);
  assert.deepEqual(order, ['attach', 'init']);
  assert.equal(pendingInAttach, 2);
  const loaded = host.named('loaded');
  assert.deepEqual(loaded, [['loaded', true, '']]);
  const calls = host.named('call');
  assert.equal(calls.length, 1);
  assert.equal(calls[0][1], 'instrument.ready');
  const args = JSON.parse(calls[0][2] as string);
  assert.equal(args.length, 2);
  assert.match(args[0], /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  assert.equal(args[1], 'ok-inst');
  const loadedAt = host.kinds().indexOf('loaded');
  const callAt = host.kinds().indexOf('call');
  assert.ok(loadedAt < callAt);
  assert.equal(frames.pending.length, 2);
});

test('failing attach logs message and stack and reports loaded(false)', () => {
  const order: string[] = [];
  class Bad extends data.PanelInstrument {
    onAttach(): void {
      throw new Error('cannot start');
    }
    protected onFlightBegin(): void {
      order.push('begin');
    }
    protected onFrame(): void {
      order.push('frame');
    }
  }
  data.defineInstrument('bad-inst', Bad);
  const logs = host.named('log');
  assert.equal(logs.length, 1);
  assert.equal(logs[0][1], 2);
  assert.match(logs[0][2] as string, /cannot start/);
  assert.match(logs[0][2] as string, /at /);
  assert.deepEqual(host.named('loaded'), [['loaded', false, 'cannot start']]);
  assert.equal(host.named('call').length, 0);
  assert.equal(frames.pending.length, 2);
  frames.step();
  frames.step();
  assert.deepEqual(order, []);
  assert.equal(frames.pending.length, 1);
  assert.equal(host.named('snapshot').length, 2);
});

test('onAttach appends a deep clone of the template once, then readPanelConfig and onPrepare', () => {
  const clones: boolean[] = [];
  dom.templates.set('tpl', {
    content: {
      cloneNode(deep: boolean): unknown {
        clones.push(deep);
        return { clone: clones.length };
      },
    },
  });
  host.files.set('/VFS/panel/panel.xml', '<PanelInfo><X/></PanelInfo>');
  const order: string[] = [];
  class T extends data.PanelInstrument {
    get templateId(): string {
      return 'tpl';
    }
    protected readPanelConfig(): void {
      order.push('config');
      super.readPanelConfig();
    }
    protected onPrepare(): void {
      order.push('init');
    }
  }
  data.defineInstrument('tpl-inst', T);
  const el = dom.body.children[0] as T & { children: unknown[] };
  assert.deepEqual(clones, [true]);
  assert.deepEqual(el.children, [{ clone: 1 }]);
  assert.deepEqual(order, ['config', 'init']);
  assert.deepEqual(el.panelConfig as unknown, { parsed: '<PanelInfo><X/></PanelInfo>', type: 'text/xml' });
  el.onAttach();
  assert.deepEqual(clones, [true]);
  assert.deepEqual(order, ['config', 'init', 'config', 'init']);
  assert.deepEqual(host.named('readFile')[0], ['readFile', '/VFS/panel/panel.xml']);
});

test('readPanelConfig falls back to <PanelInfo/>', () => {
  class P extends data.PanelInstrument {}
  data.defineInstrument('p-inst', P);
  const el = dom.body.children[0] as P;
  assert.deepEqual(el.panelConfig as unknown, { parsed: '<PanelInfo/>', type: 'text/xml' });
});

test('base getters and no-op methods', () => {
  class P extends data.PanelInstrument {}
  data.defineInstrument('base-inst', P);
  const el = dom.body.children[0] as P;
  assert.equal(el.templateId, '');
  assert.equal(el.interactive, true);
  assert.equal(el.screenIndex, 1);
  assert.equal(el.frameDelta, 0);
  assert.equal(el.sessionState(), 3);
  assert.equal(el.onPanelEvent(['x']), undefined);
  assert.equal(el.onSoundDone(new data.SoundId('s')), undefined);
  assert.equal(el.onPowered(), undefined);
  assert.equal(el.onShutdown(), undefined);
});

test('screenIndex comes from the Index parameter, case-insensitive', () => {
  for (const [search, expected] of [
    ['?Index=3', 3],
    ['?index=2&x=1', 2],
    ['?x=1&INDEX=4', 4],
    ['?other=5', 1],
    ['', 1],
    ['?Index=abc', 1],
    ['?Index=7.9', 7],
  ] as Array<[string, number]>) {
    setup(search);
    class P extends data.PanelInstrument {}
    data.defineInstrument('idx-inst', P);
    assert.equal((dom.body.children[0] as P).screenIndex, expected, search);
  }
});

test('frame loop order: snapshot, poll, watches, instruments', () => {
  const order: string[] = [];
  const origSnapshot = host.snapshot.bind(host);
  host.snapshot = () => {
    order.push('snapshot');
    origSnapshot();
  };
  const origPoll = host.poll.bind(host);
  host.poll = () => {
    order.push('poll');
    return origPoll();
  };
  host.pollQueue.push([['provider', 'p', '[]']]);
  data.Bridge.on('p', () => order.push('provider'));
  class P extends data.PanelInstrument {
    protected onFrame(): void {
      order.push('frame');
    }
  }
  data.defineInstrument('loop-inst', P);
  data.Vars.watch('L:W', 'number', () => order.push('watch'));
  frames.step();
  assert.deepEqual(order, ['snapshot', 'poll', 'provider', 'watch', 'frame']);
  assert.equal(frames.pending.length, 2);
});

test('first frame calls onSessionChange(briefing, flying) then onFlightBegin, then onFrame each frame', () => {
  const order: unknown[] = [];
  class P extends data.PanelInstrument {
    protected onSessionChange(from: number, to: number): void {
      order.push(['session', from, to]);
    }
    protected onFlightBegin(): void {
      order.push('begin');
    }
    protected onFrame(): void {
      order.push('frame');
    }
  }
  data.defineInstrument('first-inst', P);
  frames.step();
  frames.step();
  assert.deepEqual(order, [['session', 2, 3], 'begin', 'frame', 'frame']);
});

test('frameDelta is the time since the request on the first callback, then between callbacks', () => {
  const deltas: number[] = [];
  class P extends data.PanelInstrument {
    protected onFrame(): void {
      deltas.push(this.frameDelta);
    }
  }
  data.defineInstrument('delta-inst', P);
  frames.time += 4;
  frames.step(16);
  frames.step(20);
  frames.step(5);
  assert.deepEqual(deltas, [20, 20, 5]);
});

test('instruments run in attach order and exceptions are logged without stopping the loop', () => {
  const order: string[] = [];
  class Bad extends data.PanelInstrument {
    protected onFrame(): void {
      order.push('bad');
      throw new Error('frame failed');
    }
  }
  class Good extends data.PanelInstrument {
    protected onFrame(): void {
      order.push('good');
    }
  }
  data.defineInstrument('bad-frame', Bad);
  data.defineInstrument('good-frame', Good);
  frames.step();
  frames.step();
  assert.deepEqual(order, ['bad', 'good', 'bad', 'good']);
  const logs = host.named('log');
  assert.equal(logs.length, 2);
  assert.ok(logs.every((l) => l[1] === 2 && /frame failed/.test(l[2] as string)));
  assert.equal(frames.pending.length, 3);
});

test('panel events reach every live instrument', () => {
  const seen: unknown[] = [];
  class A extends data.PanelInstrument {
    onPanelEvent(args: string[]): void {
      seen.push(['a', args]);
    }
  }
  class B extends data.PanelInstrument {
    onPanelEvent(args: string[]): void {
      seen.push(['b', args]);
    }
  }
  data.defineInstrument('pa-inst', A);
  data.defineInstrument('pb-inst', B);
  host.pollQueue.push([['h', 'PUSH', '']]);
  frames.step();
  assert.deepEqual(seen, [
    ['a', ['PUSH']],
    ['b', ['PUSH']],
  ]);
});

test('connectedCallback runs once per element', () => {
  let attaches = 0;
  class A extends data.PanelInstrument {
    onAttach(): void {
      attaches++;
      super.onAttach();
    }
  }
  data.defineInstrument('once-inst', A);
  const el = dom.body.children[0] as A & { connectedCallback(): void };
  el.connectedCallback();
  assert.equal(attaches, 1);
  assert.equal(host.named('loaded').length, 1);
});

test('without a host instruments still attach and run', () => {
  removeDom();
  dom = installDom();
  frames = new FrameDriver();
  frames.install();
  const d = createData({ host: () => undefined });
  let framesSeen = 0;
  class A extends d.PanelInstrument {
    protected onFrame(): void {
      framesSeen++;
    }
  }
  d.defineInstrument('nohost-inst', A);
  frames.step();
  frames.step();
  assert.equal(framesSeen, 2);
  const el = dom.body.children[0] as A;
  assert.deepEqual(el.panelConfig as unknown, { parsed: '<PanelInfo/>', type: 'text/xml' });
});

test('the page chain starts when the data object is created and always runs', () => {
  assert.equal(frames.pending.length, 1);
  frames.step();
  frames.step();
  assert.deepEqual(host.kinds(), ['snapshot', 'poll', 'snapshot', 'poll']);
  assert.equal(frames.pending.length, 1);
});

test('every chain requests its next frame before doing anything else', () => {
  const pendingAt: Array<[string, number]> = [];
  const origSnapshot = host.snapshot.bind(host);
  host.snapshot = () => {
    pendingAt.push(['snapshot', frames.pending.length]);
    origSnapshot();
  };
  class P extends data.PanelInstrument {
    protected onFrame(): void {
      pendingAt.push(['frame', frames.pending.length]);
    }
  }
  data.defineInstrument('first-req', P);
  frames.step();
  assert.deepEqual(pendingAt, [
    ['snapshot', 1],
    ['frame', 2],
  ]);
});

test('a chain requested during onAttach runs after the instrument chain', () => {
  const order: string[] = [];
  const raf = (globalThis as unknown as { requestAnimationFrame: (cb: (t: number) => void) => number }).requestAnimationFrame;
  class P extends data.PanelInstrument {
    onAttach(): void {
      super.onAttach();
      const own = (): void => {
        raf(own);
        order.push('own');
      };
      raf(own);
    }
    protected onFrame(): void {
      order.push('frame');
    }
  }
  data.Vars.watch('L:X', 'number', () => order.push('watch'));
  data.defineInstrument('own-chain', P);
  frames.step();
  frames.step();
  assert.deepEqual(order, ['watch', 'frame', 'own', 'frame', 'own']);
});

test('frameDelta is measured per chain', () => {
  const deltas: unknown[] = [];
  class A extends data.PanelInstrument {
    protected onFrame(): void {
      deltas.push(['a', this.frameDelta]);
    }
  }
  class B extends data.PanelInstrument {
    protected onFrame(): void {
      deltas.push(['b', this.frameDelta]);
    }
  }
  data.defineInstrument('delta-a', A);
  frames.step(10);
  frames.step(10);
  data.defineInstrument('delta-b', B);
  frames.step(7);
  frames.step(3);
  assert.deepEqual(deltas, [
    ['a', 10],
    ['a', 10],
    ['a', 7],
    ['b', 7],
    ['a', 3],
    ['b', 3],
  ]);
});
