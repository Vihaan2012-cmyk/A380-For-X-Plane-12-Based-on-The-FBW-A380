import {
  Isa,
  CycleFormat,
  AirportKinds,
  SignalList,
  Arrays,
  TopicFeeder,
  Bits,
  ChartOrigin,
  ChartError,
  SimClock,
  BusSignal,
  Transforms,
  TimeFormat,
  View,
  AngleFormat,
  Bus,
  Smoother,
  FaaChart,
  NavDatabase,
  UI,
  HostedInstrument,
  PanelEventPublisher,
  Board,
  KeyControl,
  LookupTable,
  LidoChart,
  ScopedView,
  SignalMap,
  Derived,
  Maths,
  NumFormat,
  QuantitySignal,
  SignalRecord,
  Pool,
  SignalSet,
  SharedObject,
  VarPublisher,
  VarUnit,
  StallPublisher,
  Signal,
  Logic,
  UnitList,
  Vec2D,
  Waits,
} from 'xplane-js/ui';

const H: any = (globalThis as any).__harness;
const items: [string, () => any][] = [];
const add = (id: string, fn: () => any) => items.push([id, fn]);

const t = (fn: () => any) => {
  try {
    return fn();
  } catch (e: any) {
    return { error: String(e?.message ?? e) };
  }
};

const settle = (p: Promise<any>, ms: number) =>
  Promise.race([
    Promise.resolve(p).then(
      (v) => ({ state: 'resolved', v }),
      (e) => ({ state: 'rejected', msg: String(e?.message ?? e) }),
    ),
    new Promise((r) => H.realSetTimeout(() => r({ state: 'pending' }), ms)),
  ]);

const pubsub = () => {
  const bus = new Bus();
  return { bus, p: bus.outbox<any>(), s: bus.inbox<any>() };
};

const attrs = (el: Element) =>
  Array.from(el.attributes)
    .map((a) => `${a.name}=${a.value}`)
    .sort()
    .join('|');

add('U01', () => {
  const s = Signal.of(0);
  const log: string[] = [];
  let added = false;
  s.watch((v) => {
    log.push(`a${v}`);
    if (!added) {
      added = true;
      s.watch((w) => log.push(`c${w}`));
    }
  });
  s.watch((v) => log.push(`b${v}`));
  s.set(1);
  s.set(2);

  const s2 = Signal.of(0);
  const log2: string[] = [];
  const later: any = {};
  s2.watch((v) => {
    log2.push(`a${v}`);
    later.sub?.destroy();
  });
  later.sub = s2.watch((v) => log2.push(`b${v}`));
  s2.set(1);
  s2.set(2);

  const s3 = Signal.of(5);
  const log3: number[] = [];
  s3.watch((v) => log3.push(v), true);
  s3.set(5);
  s3.set(6);

  const s4 = Signal.of(0);
  const log4: string[] = [];
  s4.watch((v) => {
    log4.push(`x${v}`);
    if (v === 1) s4.set(2);
  });
  s4.watch((v) => log4.push(`y${v}`));
  s4.set(1);

  const s5 = Signal.of({ a: 1 });
  const log5: number[] = [];
  s5.watch((v) => log5.push(v.a));
  const same = s5.get();
  s5.set(same);
  s5.emit();
  return { log, log2, log3, log4, log5 };
});

add('U02', () => {
  const a = Signal.of(1);
  const b = Signal.of(2);
  const m = Derived.from(a, b);
  const first = m.get();
  const firstCopy = [...first];
  const n: number[][] = [];
  m.watch((v) => n.push([...v]));
  a.set(5);
  const second = m.get();
  b.set(2);
  a.set(5);

  const o = Signal.of(1);
  const mo = o.derive((v) => ({ v }));
  const no: number[] = [];
  mo.watch((v) => no.push(v.v));
  o.set(2);
  o.set(2);

  const mod = Signal.of(1);
  const mm = mod.derive((v) => v % 2);
  const nm: number[] = [];
  mm.watch((v) => nm.push(v));
  mod.set(3);
  mod.set(4);

  const fm = Derived.from(([x, y]: readonly number[]) => ({ s: x + y }), a, b);
  const nf: number[] = [];
  fm.watch((v) => nf.push(v.s));
  a.set(6);
  a.set(6);
  b.set(3);

  const pm = Derived.from(([x]: readonly number[], prev?: number) => (prev ?? 100) + x, a);
  const prevs = [pm.get()];
  a.set(1);
  prevs.push(pm.get());

  return {
    firstSameObject: first === second,
    firstAfterChange: [...first],
    firstCopy,
    second: [...second],
    isArray: Array.isArray(first),
    frozen: Object.isFrozen(first),
    n,
    no,
    nm,
    nf,
    prevs,
  };
});

add('U03', () => {
  const rec = (log: any[]) => (i: number, ty: any, item: any, arr: readonly any[]) =>
    log.push([i, ty, JSON.stringify(item === undefined ? '__undef__' : item), JSON.stringify(arr)]);
  const arr = SignalList.of([1, 2]);
  const log: any[] = [];
  arr.watch(rec(log));
  arr.set([3, 4, 5]);
  arr.clear();
  arr.insert(9);
  arr.insert(8, 0);
  arr.removeAt(1);
  arr.set([]);
  arr.clear();
  const log2: any[] = [];
  const a2 = SignalList.of([7, 8]);
  a2.watch(rec(log2), true);
  const log3: any[] = [];
  const a3 = SignalList.of<number>();
  a3.watch(rec(log3), true);
  return { log, log2, log3, len: arr.length, items: [...arr.snapshot()] };
});

add('U04', () => ({
  and: t(() => Logic.and()([])),
  or: t(() => Logic.or()([])),
  nor: t(() => Logic.nor()([])),
  not0: t(() => Logic.not()(0)),
  notTrue: t(() => Logic.not()(true)),
  and2: t(() => Logic.and()([true, false])),
  or2: t(() => Logic.or()([false, true])),
  nor2: t(() => Logic.nor()([false, false])),
}));

add('U05', () => {
  const { p, s } = pubsub();
  const log: string[] = [];
  p.post('c1', 1, false, true);
  log.push('before');
  s.topic('c1').listen((v) => log.push(`h${v}`));
  log.push('after');
  p.post('c2', 2);
  s.topic('c2').listen((v) => log.push(`d${v}`));
  log.push('after2');
  p.post('c3', 3, false, false);
  s.topic('c3').listen((v) => log.push(`e${v}`));
  log.push('after3');
  const cs = BusSignal.from(s.topic('c1'), 0);
  log.push(`cs${cs.get()}`);
  const cs2 = BusSignal.from<number>(null, 9);
  log.push(`cs2${cs2.get()}`);
  cs2.rebind(s.topic('c1'));
  log.push(`cs2b${cs2.get()}`);
  cs2.rebind(null);
  log.push(`cs2c${cs2.get()}`);
  const log2: number[] = [];
  s.topic('c1').distinct().listen((v) => log2.push(v));
  p.post('c1', 1);
  p.post('c1', 2);
  const log3: string[] = [];
  const paused = s.topic('c1').listen((v) => log3.push(`p${v}`), true);
  log3.push('created');
  paused.release(true);
  log3.push('resumed');
  p.post('c1', 3);
  return { log, log2, log3 };
});

add('U06', () => {
  const { bus, p, s } = pubsub();
  H.log.length = 0;
  s.topic('t').listen((v) => H.log.push(['local', 'handle', v]));
  p.post('t', 1, true);
  H.log.push(['mark', 'between']);
  p.post('t', 2, false);
  const order = H.log.map((e: any[]) => `${e[0]}:${e[1]}:${typeof e[2] === 'string' ? e[2] : JSON.stringify(e[2])}`);
  void bus;
  return order;
});

const timed = (build: (c: any) => any, real: boolean, sim: boolean) => {
  const { p, s } = pubsub();
  const got: number[] = [];
  build(s.topic('v')).listen((v: number) => got.push(v));
  for (let i = 0; i <= 30; i++) {
    if (real) H.clock.real += 100;
    if (sim) H.clock.sim += 100;
    p.post('v', i);
  }
  return got;
};

add('U07', () => ({
  every1Real: t(() => timed((c) => c.sampled(1), true, false)),
  every05Real: t(() => timed((c) => c.sampled(0.5), true, false)),
  every1Sim: t(() => timed((c) => c.sampled(1), false, true)),
  every1Both: t(() => timed((c) => c.sampled(1), true, true)),
}));

add('U08', () => {
  const { p, s } = pubsub();
  const a: number[] = [];
  const b: number[] = [];
  const c: number[] = [];
  const d: number[] = [];
  s.topic('v').rounded(1).listen((v) => a.push(v));
  s.topic('v').rounded(0).listen((v) => b.push(v));
  s.topic('v').distinctBy(0.5).listen((v) => c.push(v));
  s.topic('v').rounded(1).distinct().listen((v) => d.push(v));
  for (const v of [1.04, 1.06, 1.12, 1.14, 1.15, 1.16, 2.5, 2.49, -0.05, 0.05]) p.post('v', v);
  return { a, b, c, d };
});

add('U09', () => ({
  af1Real: t(() => timed((c) => c.limit(1), true, false)),
  af2Real: t(() => timed((c) => c.limit(2), true, false)),
  af1Sim: t(() => timed((c) => c.limit(1), false, true)),
  af1ImmReal: t(() => timed((c) => c.limit(1, true), true, false)),
  af1NoImmReal: t(() => timed((c) => c.limit(1, false), true, false)),
  onlyAfterReal: t(() => timed((c) => c.after(500), true, false)),
  onlyAfterSim: t(() => timed((c) => c.after(500), false, true)),
}));

add('U10', () => {
  class P extends (TopicFeeder as any) {
    go(v: number) {
      (this as any).postTopic('bp', v);
    }
  }
  const { bus, s } = pubsub();
  const got: number[] = [];
  s.topic('bp').listen((v) => got.push(v));
  const pb: any = new P(bus);
  pb.go(1);
  pb.begin();
  pb.go(2);
  pb.tick();
  pb.go(3);
  const late: number[] = [];
  s.topic('bp').listen((v) => late.push(v));
  return { got, late };
});

const simPub = (entries: [string, any][]) => {
  const { bus, s } = pubsub();
  const sp: any = new VarPublisher(new Map<any, any>(entries), bus);
  return { bus, s, sp };
};

add('U11', () => {
  H.resetSim({ 'L:PX': 5, 'L:PY': 1 });
  const { s, sp } = simPub([
    ['px', { name: 'L:PX', type: VarUnit.Number }],
    ['py', { name: 'L:PY', type: VarUnit.Number }],
  ]);
  const got: number[] = [];
  s.topic('px').listen((v) => got.push(v));
  const readsBeforeStart = H.readNames();
  sp.begin();
  sp.tick();
  sp.tick();
  H.sim.set('L:PX', 6);
  sp.tick();
  const late: number[] = [];
  s.topic('px').listen((v) => late.push(v));
  sp.tick();
  return { got, late, readsBeforeStart, reads: H.readNames() };
});

add('U12', () => {
  H.resetSim({ 'L:IX_1': 11, 'L:IX_2': 22, 'L:IX_0': 100, 'L:IY_1': 31 });
  const { s, sp } = simPub([
    ['ix', { name: 'L:IX_#index#', type: VarUnit.Number, indexed: true }],
    ['iy', { name: 'L:IY_#index#', type: VarUnit.Number, indexed: true, defaultIndex: null }],
    ['iz', { name: 'L:IZ_#index#', type: VarUnit.Number, indexed: true, defaultIndex: 3 }],
  ]);
  const g: Record<string, number[]> = {};
  for (const topic of ['ix', 'ix_1', 'ix_2', 'iy', 'iy_1', 'iz', 'iz_3']) {
    g[topic] = [];
    s.topic(topic).listen((v) => g[topic].push(v));
  }
  sp.begin();
  sp.tick();
  return { g, reads: H.readNames() };
});

add('U13', () => {
  H.resetSim({ 'L:B1': 1, 'L:B0': 0, 'L:B5': 5 });
  const { s, sp } = simPub([
    ['b1', { name: 'L:B1', type: VarUnit.Bool }],
    ['b0', { name: 'L:B0', type: VarUnit.Bool }],
    ['b5', { name: 'L:B5', type: VarUnit.Bool }],
  ]);
  const got: any[] = [];
  for (const topic of ['b1', 'b0', 'b5']) s.topic(topic).listen((v) => got.push([topic, v === true ? 'true' : v === false ? 'false' : v]));
  sp.begin();
  sp.tick();
  return got;
});

add('U14', () => {
  H.resetSim({});
  const enumObj: Record<string, any> = {};
  for (const [k, v] of Object.entries(VarUnit)) enumObj[k] = v;
  const entries: [string, any][] = Object.entries(VarUnit)
    .filter(([k]) => Number.isNaN(Number(k)))
    .map(([k, v]) => [`u_${k}`, { name: `L:U_${k}`, type: v }]);
  const { s, sp } = simPub(entries);
  for (const [topic] of entries) s.topic(topic).listen(() => {});
  sp.begin();
  sp.tick();
  return { enumObj, units: H.readUnits() };
});

add('U15', () => {
  const { bus, s } = pubsub();
  H.log.length = 0;
  const got: string[] = [];
  s.topic('hEvent').listen((v) => got.push(String(v)));
  const hp: any = new PanelEventPublisher(bus);
  hp.dispatchPanelEvent('A');
  hp.begin?.();
  hp.dispatchPanelEvent('B');
  hp.dispatchPanelEvent('C', true);
  return { got, transport: H.log.map((e: any[]) => `${e[0]}:${e[1]}:${typeof e[2] === 'string' ? e[2] : ''}`) };
});

add('U16', () => {
  H.resetSim({});
  const { bus, s } = pubsub();
  const topics = ['simTime', 'simTimeHiFreq', 'realTime', 'activeSimDuration'];
  const seen: Record<string, number[]> = {};
  for (const topic of topics) {
    seen[topic] = [];
    s.topic(topic).listen((v) => seen[topic].push(v));
  }
  const clock: any = new SimClock(bus);
  clock.setup();
  for (let i = 0; i < 6; i++) {
    H.clock.real += 50;
    H.clock.sim += 50;
    clock.tick();
  }
  const rel: Record<string, number[]> = {};
  for (const topic of topics) rel[topic] = seen[topic].map((v) => v - (seen[topic][0] ?? 0));
  return { counts: Object.fromEntries(topics.map((k) => [k, seen[k].length])), first: Object.fromEntries(topics.map((k) => [k, seen[k][0]])), rel, reads: H.readNames() };
});

add('U17', () => {
  H.resetSim({});
  const { bus, s } = pubsub();
  const got: any[] = [];
  s.topic('stall_warning_on').listen((v) => got.push(v));
  const sw: any = new StallPublisher(bus, 0.9);
  sw.begin();
  sw.tick();
  const names: string[] = H.readNames();
  const out: Record<string, any[]> = {};
  for (const v of [0, 0.5, 0.85, 0.89, 0.9, 0.91, 0.95, 1.2]) {
    for (const n of names) H.sim.set(n, v);
    got.length = 0;
    sw.tick();
    out[String(v)] = [...got];
  }
  return { names, out };
});

const mkPub = (log: string[], n: string) => ({ begin: () => log.push(`sp${n}`), tick: () => log.push(`pu${n}`) });
const mkInst = (log: string[], n: string) => ({ setup: () => log.push(`ii${n}`), tick: () => log.push(`iu${n}`) });

add('U18', () => {
  const log: string[] = [];
  const bp: any = new Board();
  const errs: string[] = [];
  for (const f of [
    () => bp.attachFeeder('a', mkPub(log, '1')),
    () => bp.attachFeeder('a', mkPub(log, '2')),
    () => bp.attachDevice('x', mkInst(log, '1')),
    () => bp.attachDevice('x', mkInst(log, '2')),
    () => bp.attachFeeder('b', mkPub(log, '3')),
    () => bp.attachFeeder('b', mkPub(log, '4'), true),
  ]) {
    try {
      f();
    } catch (e: any) {
      errs.push(String(e?.message ?? e));
    }
  }
  bp.setup();
  bp.tick();
  return { log, errs };
});

add('U19', () => {
  const log: string[] = [];
  const bp: any = new Board();
  bp.attachDevice('i1', mkInst(log, '1'));
  bp.attachFeeder('p1', mkPub(log, '1'));
  bp.attachDevice('i2', mkInst(log, '2'));
  bp.attachFeeder('p2', mkPub(log, '2'));
  bp.setup();
  log.push('|');
  bp.tick();
  log.push('|');
  bp.attachFeeder('p3', mkPub(log, '3'));
  bp.tick();
  return log;
});

add('U19b', () => {
  const run = (adds: [string, string][], initFirst = false) => {
    const log: string[] = [];
    const bp: any = new Board();
    const errs: string[] = [];
    const go = (kind: string, name: string) => {
      try {
        if (kind === 'p') bp.attachFeeder(name, mkPub(log, name));
        else bp.attachDevice(name, mkInst(log, name));
      } catch (e: any) {
        errs.push(String(e?.message ?? e));
      }
    };
    if (initFirst) bp.setup();
    for (const [k, n] of adds) go(k, n);
    if (!initFirst) bp.setup();
    log.push('|');
    bp.tick();
    return { log, errs };
  };
  return {
    pubsOnly: run([
      ['p', 'p1'],
      ['p', 'p2'],
    ]),
    instsOnly: run([
      ['i', 'i1'],
      ['i', 'i2'],
    ]),
    pubThenInst: run([
      ['p', 'pa'],
      ['i', 'ia'],
      ['p', 'pb'],
      ['i', 'ib'],
    ]),
    instThenPub: run([
      ['i', 'ia'],
      ['p', 'pa'],
      ['i', 'ib'],
      ['p', 'pb'],
    ]),
    longNames: run([
      ['p', 'publisherOne'],
      ['p', 'publisherTwo'],
      ['i', 'instrumentOne'],
      ['i', 'instrumentTwo'],
    ]),
    afterInit: run(
      [
        ['p', 'late1'],
        ['i', 'late2'],
      ],
      true,
    ),
  };
});

add('U20', () => {
  const log: string[] = H.baseLog;
  log.length = 0;
  class Inst {
    constructor() {
      log.push('fs-ctor');
    }
    onFrame() {
      log.push('fs-update');
    }
    onPanelEvent() {
      log.push('fs-interaction');
    }
    onSessionChange() {
      log.push('fs-gamestate');
    }
    onFlightBegin() {
      log.push('fs-flightstart');
    }
    onSoundDone() {
      log.push('fs-soundend');
    }
    onPowered() {
      log.push('fs-poweron');
    }
    onUnpowered() {
      log.push('fs-poweroff');
    }
  }
  class X extends (HostedInstrument as any) {
    createDevice() {
      log.push('construct');
      return new Inst();
    }
    get templateId() {
      return 'X';
    }
  }
  customElements.define('probe-x', X as any);
  const x: any = new X();
  log.push('after-new');
  const named: Record<string, string> = { connectedCallback: 'onAttach', Init: 'onPrepare', onFlightStart: 'onFlightBegin', Update: 'onFrame', onInteractionEvent: 'onPanelEvent', onSoundEnd: 'onSoundDone', onPowerOn: 'onPowered', onShutDown: 'onShutdown', onGameStateChanged: 'onSessionChange' };
  for (const [label, m] of Object.entries(named)) {
    log.push(`call:${label}`);
    try {
      if (typeof x[m] === 'function') x[m](m === 'onPanelEvent' ? ['EV'] : m === 'onSessionChange' ? 1 : undefined, 3);
      else log.push(`missing:${label}`);
    } catch (e: any) {
      log.push(`err:${label}:${String(e?.message ?? e)}`);
    }
  }
  return [...log];
});

add('U31', () => {
  const log: string[] = [];
  class A extends View<any> {
    afterRender() {
      log.push('A.after');
    }
    render() {
      return <div />;
    }
  }
  class B extends View<any> {
    afterMount() {
      log.push('B.mounted');
    }
    render() {
      return <div />;
    }
  }
  class C extends View<any> {
    afterRender() {
      log.push('C.after');
    }
    afterMount() {
      log.push('C.mounted');
    }
    render() {
      return <div />;
    }
  }
  class D extends View<any> {
    render() {
      return <div />;
    }
  }
  const root = document.createElement('div');
  UI.mount(
    <div>
      <A />
      <B />
      <C />
      <D />
    </div>,
    root,
  );
  return log;
});

add('U32', () => {
  class G extends View<any> {
    render() {
      return (
        <g>
          <path d="M0 0" />
        </g>
      );
    }
  }
  class Dv extends View<any> {
    render() {
      return <div class="d" />;
    }
  }
  const walk = (el: Element, depth: number, out: string[]) => {
    out.push(`${depth}:${el.tagName}:${(el.namespaceURI || '').split('/').pop()}`);
    for (const c of Array.from(el.children)) walk(c, depth + 1, out);
    return out;
  };
  const root = document.createElement('div');
  const r1 = t(() => {
    UI.mount(
      <svg>
        <g id="g1">
          <text>t</text>
        </g>
        <G />
        <foreignObject>
          <div id="fo">
            <span />
          </div>
        </foreignObject>
      </svg>,
      root,
    );
    return walk(root, 0, []);
  });
  const svgRoot = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  const r2 = t(() => {
    UI.mount(<G />, svgRoot as any);
    UI.mount(
      <g id="g2">
        <circle />
      </g>,
      svgRoot as any,
    );
    UI.mount(<Dv />, svgRoot as any);
    UI.mount(<text>z</text>, svgRoot as any);
    return walk(svgRoot, 0, []);
  });
  const r3 = t(() => {
    const host = document.createElement('div');
    UI.mount(<G />, host);
    UI.mount(<text>q</text>, host);
    return walk(host, 0, []);
  });
  return { r1, r2, r3 };
});

add('U33', () => {
  const log: string[] = [];
  const root = document.createElement('div');
  const r = t(() => {
    UI.mount(
      <div id="b" onClick={() => log.push('click')} onMouseDown={() => log.push('md')} onclick={() => log.push('lower')} />,
      root,
    );
    const b = root.querySelector('#b')!;
    b.dispatchEvent(new (window as any).MouseEvent('click', { bubbles: true }));
    b.dispatchEvent(new (window as any).MouseEvent('mousedown', { bubbles: true }));
    return attrs(b);
  });
  return { log, attrs: r };
});

add('U34', () => {
  const s = Signal.of<any>(null);
  const root = document.createElement('div');
  const r = t(() => {
    UI.mount(<div id="x" data-a={s} data-b={null} data-c={undefined} data-d={0} data-e={false} data-f={true} />, root);
    const el = root.querySelector('#x')!;
    const out = [attrs(el)];
    for (const v of ['v', undefined, false, 0, '', null, true]) {
      s.set(v);
      out.push(attrs(el));
    }
    return out;
  });
  const cls = t(() => {
    const c = Signal.of<any>(null);
    const host = document.createElement('div');
    UI.mount(<div id="y" class={c} />, host);
    const el = host.querySelector('#y')!;
    const out = [attrs(el)];
    for (const v of ['k', undefined, 'a b']) {
      c.set(v);
      out.push(attrs(el));
    }
    return out;
  });
  const clsRec = t(() => {
    const on = Signal.of(true);
    const host = document.createElement('div');
    UI.mount(<div id="z" class={{ one: on, two: false, three: true } as any} />, host);
    const el = host.querySelector('#z')!;
    const out = [el.getAttribute('class')];
    on.set(false);
    out.push(el.getAttribute('class'));
    return out;
  });
  const style = t(() => {
    const w = Signal.of<any>('10px');
    const host = document.createElement('div');
    UI.mount(<div id="s" style={{ width: w, height: '5px', display: undefined } as any} />, host);
    const el = host.querySelector('#s')!;
    const out = [el.getAttribute('style')];
    w.set(null);
    out.push(el.getAttribute('style'));
    w.set('3px');
    out.push(el.getAttribute('style'));
    return out;
  });
  return { r, cls, clsRec, style };
});

add('U35', () => {
  const host = () => document.createElement('div');
  const vnode = t(() => {
    const s = Signal.of<any>(<span>a</span>);
    const root = host();
    UI.mount(<div id="p">{s}</div>, root);
    const h = [root.innerHTML];
    s.set(<b>b</b>);
    h.push(root.innerHTML);
    return h;
  });
  const array = t(() => {
    const s = Signal.of<any>([<i>1</i>, <i>2</i>]);
    const root = host();
    UI.mount(<div>{s}</div>, root);
    const h = [root.innerHTML];
    s.set([<i>3</i>]);
    h.push(root.innerHTML);
    return h;
  });
  const text = t(() => {
    const s = Signal.of<any>('x');
    const root = host();
    UI.mount(
      <div>
        a{s}b
      </div>,
      root,
    );
    const h = [root.innerHTML];
    for (const v of [5, null, undefined, '', 'y', 0, false, true]) {
      s.set(v);
      h.push(root.innerHTML);
    }
    return h;
  });
  const statics = t(() => {
    const root = host();
    UI.mount(
      <div>
        {null}
        {undefined}
        {0}
        {false}
        {true}
        {''}
        {[1, [2, 3]]}
        {'s'}
      </div>,
      root,
    );
    return root.innerHTML;
  });
  const list = t(() => {
    const arr = SignalList.of([1, 2]);
    const root = host();
    UI.mount(<div>{arr as any}</div>, root);
    const h = [root.innerHTML];
    arr.insert(3);
    h.push(root.innerHTML);
    return h;
  });
  return { vnode, array, text, statics, list };
});

add('U53', () => {
  const root = document.createElement('div');
  const s = Signal.of<any>('X<br />Y');
  UI.mount(
    <div>
      {'A<br />B &deg;'}|{s}|end
    </div>,
    root,
  );
  const h = [root.innerHTML];
  s.set('plain');
  h.push(root.innerHTML);
  s.set('P&amp;Q<b>b</b>');
  h.push(root.innerHTML);
  s.set('1 < 2');
  h.push(root.innerHTML);
  return h;
});

add('U36', () => {
  const ref = UI.ref<any>();
  const out: any = {};
  try {
    const v = ref.target;
    out.threw = false;
    out.value = v === undefined ? '__undef__' : String(v);
  } catch (e: any) {
    out.threw = true;
    out.msg = String(e?.message ?? e);
  }
  out.def = t(() => ref.peek());
  const root = document.createElement('div');
  UI.mount(<div ref={ref} id="r" />, root);
  out.after = t(() => ref.target?.id);
  out.defAfter = t(() => ref.peek()?.id);
  return out;
});

add('U37', () => {
  const log: string[] = [];
  class Leaf extends View<{ n: string }> {
    afterRender() {
      log.push(`after:${this.props.n}`);
    }
    render() {
      return <span>{this.props.n}</span>;
    }
  }
  class Mid extends View<{ n: string }> {
    private readonly ref = UI.ref<HTMLDivElement>();
    afterRender() {
      log.push(`after:${this.props.n}:${t(() => !!this.ref.target)}:${t(() => !!this.ref.target?.isConnected)}`);
    }
    render() {
      return (
        <div ref={this.ref}>
          <Leaf n={`${this.props.n}.1`} />
          <Leaf n={`${this.props.n}.2`} />
        </div>
      );
    }
  }
  class Top extends View<any> {
    afterRender() {
      log.push('after:top');
    }
    render() {
      return (
        <div>
          <Mid n="m1" />
          <Mid n="m2" />
        </div>
      );
    }
  }
  const root = document.createElement('div');
  document.body.appendChild(root);
  UI.mount(<Top />, root);
  root.remove();
  return log;
});

add('U38', () => {
  const log: string[] = [];
  class Child extends View<any> {
    destroy() {
      log.push('child.destroy');
      super.destroy();
    }
    render() {
      return <span />;
    }
  }
  class Parent extends View<any> {
    destroy() {
      log.push('parent.destroy');
      super.destroy();
    }
    render() {
      return (
        <div>
          <Child />
        </div>
      );
    }
  }
  const root = document.createElement('div');
  const vn: any = <Parent />;
  UI.mount(vn, root);
  const r = t(() => vn.target.destroy());
  const html = root.innerHTML;
  const lc = t(() => {
    const src = Signal.of(1);
    let mapped: any;
    class L extends (ScopedView as any) {
      render() {
        mapped = src.derive((x: number) => x * 2).scopedTo((this as any).scope);
        return <div />;
      }
    }
    const node: any = <L />;
    UI.mount(node, document.createElement('div'));
    const before = mapped.isLive;
    node.target.destroy();
    return { before, after: mapped.isLive };
  });
  return { log, r, html, lc };
});

add('U40', () =>
  [[2.5], [-2.5], [1.5], [0.5], [-0.5], [0.125, 0.25], [0.375, 0.25], [1.005, 0.01], [2.345, 0.01], [7, 5], [-7.5, 5], [12.5, 1], [3.14159, 0.001]].map((a) =>
    t(() => Maths.round(a[0], a[1])),
  ),
);

add('U41', () =>
  [[10, 350], [350, 10], [0, 180], [180, 0], [0, 190], [190, 0], [-10, 10], [720, 0], [0, -180], [90, 270]].map((a) => t(() => Maths.angleDiff(a[0], a[1]))),
);

add('U42', () => [-500, 0, 5000, 11000, 15000, 20000, 25000, 32000, 47000].map((h) => t(() => Isa.isaTemp(h))));

add('U43', () => {
  const a = new Smoother(1);
  const ra = [a.latest(), a.push(10, 0.1), a.push(20, 0.1), a.latest(), a.push(20, 0)];
  const b = new Smoother(1, 5, 1);
  const rb = [b.latest(), b.push(10, 0.1), b.push(30, 2), b.push(40, 0.5), b.push(40, 1)];
  const c = new Smoother(1, null, 1);
  const rc = [c.push(10, 0.1), c.push(20, 5), c.push(30, 0.1)];
  const d = new Smoother(0, 3);
  const rd = [d.push(10, 0.1), d.push(20, 0.1)];
  return { ra, rb, rc, rd };
});

add('U44', () => {
  const t1 = new LookupTable([
    [0, 0],
    [100, 10],
  ]);
  const t2 = new LookupTable([
    [0, 0, 0],
    [10, 1, 0],
    [20, 0, 1],
    [30, 1, 1],
  ]);
  const t3 = new LookupTable([
    [5, 10],
    [1, 0],
    [3, 5],
  ]);
  const t4 = new LookupTable([[7, 2]]);
  return {
    t1: [-5, 0, 5, 10, 15].map((k) => t(() => t1.at(k))),
    t2: [
      [0.5, 0.5],
      [2, 2],
      [-1, 0.5],
      [1, 0],
    ].map((k) => t(() => t2.at(...k))),
    t3: [0, 2, 4, 7, 12].map((k) => t(() => t3.at(k))),
    t4: [0, 2, 3].map((k) => t(() => t4.at(k))),
  };
});

add('U45', () => ({ ...(AirportKinds as any) }));

add('U46', () => ({
  faa: { ...(FaaChart as any) },
  lido: { ...(LidoChart as any) },
  provider: { ...(ChartOrigin as any) },
  err: { ...(ChartError as any) },
}));

add('U47', async () => {
  H.useRealTime(true);
  try {
    const falseTimeout = await settle(Waits.until(() => false, 10, 60), 600);
    const trueNow = await settle(Waits.until(() => true, 10, 60), 600);
    let n = 0;
    const eventually = await settle(Waits.until(() => ++n > 3, 10, 2000), 600);
    const zeroTimeout = await settle(Waits.until(() => false, 10, 0), 300);
    const delay = await settle(Waits.delay(20), 300);
    const frames = await settle(Waits.frames(2), 500);
    return { falseTimeout, trueNow, eventually, zeroTimeout, delay, frames };
  } finally {
    H.useRealTime(false);
  }
});

add('U48', async () => {
  H.useRealTime(true);
  try {
    const out: any = {};
    for (const [name, init] of [
      ['zero', 0],
      ['false', false],
      ['null', null],
      ['one', 1],
      ['str', ''],
    ] as [string, any][]) {
      const s = Signal.of<any>(init);
      const p = Waits.forValue(s);
      const first = await settle(p, 50);
      s.set(init === 1 ? 2 : 1);
      const second = first.state === 'pending' ? await settle(p, 50) : 'n/a';
      out[name] = { first, second };
    }
    const s = Signal.of<any>(0);
    out.takeInitial = await settle(Waits.forValue(s, (v) => v === 0, true), 50);
    out.noTakeInitial = await settle(Waits.forValue(s, (v) => v === 0, false), 50);
    out.timeout = await settle(Waits.forValue(s, (v) => v === 9, false, 40), 300);
    return out;
  } finally {
    H.useRealTime(false);
  }
});

add('U49', () => {
  const opts: Record<string, any> = {
    fix: { precision: 1, maxDigits: 4, nanString: '----' },
    alt: { precision: 1, pad: 3, maxDigits: 4, nanString: '---' },
    wt: { precision: 0.1, nanString: '---.-' },
    pad01: { precision: 0.1, pad: 3 },
    max3: { precision: 0.01, maxDigits: 3 },
    none: {},
    p10: { precision: 10 },
  };
  const values = [0, 5, 5.25, 12.34, 99.95, 999.95, 1234.5, 12345, -3.7, -0.04, NaN, 0.05, 0.15, 2.5, 3.5, -2.5];
  const out: Record<string, any> = {};
  for (const [k, o] of Object.entries(opts)) {
    const f = t(() => NumFormat.make(o));
    out[k] = typeof f === 'function' ? values.map((v) => t(() => f(v))) : f;
  }
  return out;
});

add('U50', () => {
  const lat = AngleFormat.make('{dd}°{mm}.{s}{+[N]-[S]}', UnitList.DEGREE, 0.1);
  const lon = AngleFormat.make('{ddd}°{mm}.{s}{+[E]-[W]}', UnitList.DEGREE, 0.1);
  const values = [51.99999, 51.9999, 0.5, -0.0001, 12.3456, -33.95, 179.999, 0, -0, NaN, 59.9917, 1.0008333];
  const nan = t(() => AngleFormat.make('{dd}°{mm}', UnitList.DEGREE, 1, 'XX')(NaN));
  const tokens = ['{d}', '{dd}', '{ddd}', '{m}', '{mm}', '{s}', '{ss}', '{+[N]-[S]}', '{-[S]+[N]}', '{+}', '{-}'].map((f) => [
    f,
    t(() => AngleFormat.make(f, UnitList.DEGREE, 0.1)(-7.508)),
  ]);
  return { lat: values.map((v) => t(() => lat(v))), lon: values.map((v) => t(() => lon(-v))), nan, tokens };
});

add('U51', () => {
  const fmt = CycleFormat.make('{eff({dd}{MON}{YY})}-{exp({dd}{MON}{YY})}({YYCC})');
  const cycles: any = t(() => NavDatabase.cycles());
  const show = (c: any) => (c && typeof c === 'object' ? Object.fromEntries(Object.entries(c).filter(([, v]) => typeof v !== 'function')) : c);
  return {
    cycles: cycles && !cycles.error ? { previous: show(cycles.previous), current: show(cycles.current), next: show(cycles.next) } : cycles,
    current: t(() => fmt(cycles.current)),
    previous: t(() => fmt(cycles.previous)),
    next: t(() => fmt(cycles.next)),
    other: t(() => CycleFormat.make('{YYCC} {eff({dd}/{mm}/{YYYY})} {exp({MON})} {cc}')(cycles.current)),
  };
});

add('U52', async () => {
  const a = await settle(SharedObject.get<any>('probe') as any, 300);
  const b = await settle(SharedObject.get<any>('probe') as any, 300);
  const desc = (r: any) =>
    r.state === 'resolved'
      ? { state: r.state, owner: r.v?.isOwner, hasInstance: !!r.v?.value, detached: t(() => r.v?.detached?.get()) }
      : r;
  return { a: desc(a), b: desc(b), sameInstance: a.state === 'resolved' && b.state === 'resolved' ? a.v.value === b.v.value : 'n/a' };
});

add('K01', async () => {
  const { bus } = pubsub();
  H.log.length = 0;
  const r: any = await settle(KeyControl.connect(bus), 300);
  const out: any = { state: r.state, afterGet: H.transport() };
  if (r.state === 'resolved') {
    H.log.length = 0;
    t(() => r.v.intercept('PROBE_KEY', false));
    out.intercept = H.transport();
    H.log.length = 0;
    out.trigger = await settle(r.v.fire('PROBE_KEY_2', true, 7), 300);
    out.triggerCalls = H.transport();
  }
  return out;
});

add('K02', async () => {
  const { bus, s } = pubsub();
  const got: any[] = [];
  s.topic('key_intercept').listen((v) => got.push(v));
  const r: any = await settle(KeyControl.connect(bus), 300);
  if (r.state !== 'resolved') return { state: r.state };
  t(() => r.v.intercept('PROBE_AXIS', false));
  H.emit('keyIntercepted', 'PROBE_AXIS', 11, 22, 33);
  H.emit('keyIntercepted', 'PROBE_AXIS', 5);
  await H.frame();
  return got;
});

add('R01', () => {
  const log: string[] = [];
  let n = 0;
  const heap = new Pool<any>(
    () => {
      log.push(`make${++n}`);
      return { id: n };
    },
    (r) => log.push(`destroy${r.id}`),
    (r) => log.push(`alloc${r.id}`),
    (r) => log.push(`free${r.id}`),
    2,
    6,
    3,
  );
  log.push('|');
  const got = [heap.take(), heap.take(), heap.take(), heap.take(), heap.take()];
  log.push('|');
  for (const g of got) heap.give(g);
  log.push('|');
  const more = [heap.take(), heap.take(), heap.take(), heap.take(), heap.take(), heap.take()];
  const over = t(() => heap.take());
  log.push('|');
  for (const g of more) heap.give(g);
  return { log, over: over && over.error ? over : 'ok' };
});

add('X01', () => ({
  flat: Arrays.flat([1, [2, 3], [[4]] as any]),
  any: [Bits.any(5, 4), Bits.any(5, 2), Bits.all(7, 5), Bits.all(5, 7), Bits.any(0, 0), Bits.all(0, 0)],
  vec: t(() => {
    const v = Vec2D.make();
    Vec2D.fromPolar(2, Math.PI / 2, v);
    return [Array.from(Vec2D.make(3, 4)), Vec2D.magnitude(Vec2D.make(3, 4)), Vec2D.angle(Vec2D.make(-1, 0)), Array.from(v).map((x) => Math.round(x * 1e9) / 1e9)];
  }),
  css: t(() => {
    const c = Transforms.translate3d('px');
    const r = [c.css()];
    c.set(1.234, 5, 0, 0.1, 1, 1);
    r.push(c.css());
    const d = Transforms.translate3d();
    d.set(1, 2, 3);
    r.push(d.css());
    return r;
  }),
  units: t(() => [
    UnitList.NMILE.to(1, UnitList.METER),
    UnitList.FOOT.to(1000, UnitList.METER),
    UnitList.KILOGRAM.to(1000, UnitList.POUND),
    UnitList.TONNE.to(1, UnitList.KILOGRAM),
    UnitList.KILOMETER.from(1, UnitList.NMILE),
    UnitList.METER.quantity(5).inUnit(UnitList.FOOT),
  ]),
  nus: t(() => {
    const s = QuantitySignal.of(UnitList.METER.quantity(100));
    const ft = s.inUnit(UnitList.FOOT);
    const r = [ft.get()];
    s.set(200);
    r.push(ft.get());
    s.set(1, UnitList.KILOMETER);
    r.push(ft.get(), s.get().amount);
    return r;
  }),
}));

add('X02', () => {
  const ms = SignalMap.of<string, number>([['a', 1]]);
  const mlog: any[] = [];
  ms.watch((_m, ty, k, v) => mlog.push([ty, k, v]), true);
  ms.assign('b', 2);
  ms.assign('a', 1);
  ms.assign('a', 3);
  ms.delete('b');
  ms.delete('zz');
  ms.clear();
  const ss = SignalSet.of([1, 2]);
  const sm = ss.derive((s) => [...s].sort().join(','));
  const slog: string[] = [sm.get()];
  sm.watch((v) => slog.push(v));
  ss.add(3);
  ss.add(3);
  ss.delete(1);
  ss.clear();
  const os = SignalRecord.of({ a: 1, b: 2 });
  const olog: any[] = [];
  os.watch((v) => olog.push({ ...v }));
  os.set({ a: 1 });
  os.set({ a: 5 });
  os.set({ b: 7, a: 5 });
  return { mlog, slog, olog, size: ms.size };
});

add('X03', () => {
  const ts = Date.UTC(2026, 9, 21, 7, 5, 9);
  const fmts = ['{HH}:{mm}', '{HH}{mm}', '{dd}/{mm}/{YYYY}', '{HH}:{mm}:{ss}', '{MON} {dd}', '{h}:{mm} {am}'];
  return Object.fromEntries(
    fmts.map((f) => [f, [ts, NaN, 0].map((v) => t(() => TimeFormat.make(f, { nanString: '--:--' })(v)))]),
  );
});

add('U06b', async () => {
  H.useRealTime(true);
  try {
    const { p, s } = pubsub();
    const got: number[] = [];
    s.topic('t2').listen((v) => got.push(v));
    await new Promise((r) => H.realSetTimeout(r, 60));
    H.log.length = 0;
    p.post('t2', 7, true);
    p.post('t2', 8, false);
    await new Promise((r) => H.realSetTimeout(r, 30));
    return { got, transport: H.transport() };
  } finally {
    H.useRealTime(false);
  }
});

add('U16b', async () => {
  H.useRealTime(true);
  try {
    const { bus, s } = pubsub();
    const n: Record<string, number> = { simTime: 0, simTimeHiFreq: 0, realTime: 0, activeSimDuration: 0 };
    for (const k of Object.keys(n)) s.topic(k).listen(() => n[k]++);
    const clock: any = new SimClock(bus);
    clock.setup();
    await new Promise((r) => H.realSetTimeout(r, 250));
    const idle = { ...n };
    for (let i = 0; i < 3; i++) clock.tick();
    const afterUpdates = { ...n };
    await new Promise((r) => H.realSetTimeout(r, 100));
    return { idleHiFreqTicks: idle.simTimeHiFreq > 5 ? 'many' : idle.simTimeHiFreq, idleOther: idle.simTime, afterUpdates: { ...afterUpdates, simTimeHiFreq: afterUpdates.simTimeHiFreq > 5 ? 'many' : afterUpdates.simTimeHiFreq }, end: n.simTimeHiFreq > 5 ? 'many' : n.simTimeHiFreq };
  } finally {
    H.useRealTime(false);
  }
});

const ser = (v: any, depth = 0): any => {
  if (depth > 8) return '__deep__';
  if (v === undefined) return '__undef__';
  if (typeof v === 'number') {
    if (Number.isNaN(v)) return 'NaN';
    if (Object.is(v, -0)) return '-0';
    if (!Number.isFinite(v)) return String(v);
    return Math.round(v * 1e9) / 1e9;
  }
  if (typeof v === 'function') return '__fn__';
  if (v === null || typeof v !== 'object') return v;
  if (typeof (globalThis as any).Element !== 'undefined' && v instanceof (globalThis as any).Element) return `__el__${v.outerHTML}`;
  if (v instanceof Map) return { __map__: Array.from(v.entries()).map((e) => ser(e, depth + 1)) };
  if (v instanceof Set) return { __set__: Array.from(v).map((e) => ser(e, depth + 1)) };
  if (Array.isArray(v) || ArrayBuffer.isView(v)) return Array.from(v as any).map((e) => ser(e, depth + 1));
  const o: any = {};
  for (const k of Object.keys(v)) o[k] = ser(v[k], depth + 1);
  return o;
};

(globalThis as any).__runProbe = async () => {
  const results: Record<string, any> = {};
  for (const [id, fn] of items) {
    H.reset();
    try {
      let v = fn();
      if (v && typeof v.then === 'function') v = await settle(v, 5000).then((r: any) => (r.state === 'resolved' ? r.v : { asyncState: r.state, msg: r.msg }));
      results[id] = ser(v);
    } catch (e: any) {
      results[id] = { error: String(e?.message ?? e) };
    }
  }
  return results;
};
