import {
  AeroMath,
  AiracCycleFormatter,
  AirportClassMask,
  ArraySubject,
  ArrayUtils,
  BasePublisher,
  BitFlags,
  BuiltInChartProvider,
  ChartImageErrorCode,
  Clock,
  ConsumerSubject,
  CssTransformBuilder,
  DateTimeFormatter,
  DisplayComponent,
  DmsFormatter2,
  EventBus,
  ExpSmoother,
  FaaChartType,
  FacilityLoader,
  FSComponent,
  FsBaseInstrument,
  HEventPublisher,
  InstrumentBackplane,
  KeyEventManager,
  LerpLookupTable,
  LidoChartType,
  LifecycleComponent,
  MapSubject,
  MappedSubject,
  MathUtils,
  NumberFormatter,
  NumberUnitSubject,
  ObjectSubject,
  ResourceHeap,
  SetSubject,
  SharedGlobal,
  SimVarPublisher,
  SimVarValueType,
  StallWarningPublisher,
  Subject,
  SubscribableMapFunctions,
  UnitType,
  Vec2Math,
  Wait,
} from '@microsoft/msfs-sdk';

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
  const bus = new EventBus();
  return { bus, p: bus.getPublisher<any>(), s: bus.getSubscriber<any>() };
};

const attrs = (el: Element) =>
  Array.from(el.attributes)
    .map((a) => `${a.name}=${a.value}`)
    .sort()
    .join('|');

add('U01', () => {
  const s = Subject.create(0);
  const log: string[] = [];
  let added = false;
  s.sub((v) => {
    log.push(`a${v}`);
    if (!added) {
      added = true;
      s.sub((w) => log.push(`c${w}`));
    }
  });
  s.sub((v) => log.push(`b${v}`));
  s.set(1);
  s.set(2);

  const s2 = Subject.create(0);
  const log2: string[] = [];
  const later: any = {};
  s2.sub((v) => {
    log2.push(`a${v}`);
    later.sub?.destroy();
  });
  later.sub = s2.sub((v) => log2.push(`b${v}`));
  s2.set(1);
  s2.set(2);

  const s3 = Subject.create(5);
  const log3: number[] = [];
  s3.sub((v) => log3.push(v), true);
  s3.set(5);
  s3.set(6);

  const s4 = Subject.create(0);
  const log4: string[] = [];
  s4.sub((v) => {
    log4.push(`x${v}`);
    if (v === 1) s4.set(2);
  });
  s4.sub((v) => log4.push(`y${v}`));
  s4.set(1);

  const s5 = Subject.create({ a: 1 });
  const log5: number[] = [];
  s5.sub((v) => log5.push(v.a));
  const same = s5.get();
  s5.set(same);
  s5.notify();
  return { log, log2, log3, log4, log5 };
});

add('U02', () => {
  const a = Subject.create(1);
  const b = Subject.create(2);
  const m = MappedSubject.create(a, b);
  const first = m.get();
  const firstCopy = [...first];
  const n: number[][] = [];
  m.sub((v) => n.push([...v]));
  a.set(5);
  const second = m.get();
  b.set(2);
  a.set(5);

  const o = Subject.create(1);
  const mo = o.map((v) => ({ v }));
  const no: number[] = [];
  mo.sub((v) => no.push(v.v));
  o.set(2);
  o.set(2);

  const mod = Subject.create(1);
  const mm = mod.map((v) => v % 2);
  const nm: number[] = [];
  mm.sub((v) => nm.push(v));
  mod.set(3);
  mod.set(4);

  const fm = MappedSubject.create(([x, y]: readonly number[]) => ({ s: x + y }), a, b);
  const nf: number[] = [];
  fm.sub((v) => nf.push(v.s));
  a.set(6);
  a.set(6);
  b.set(3);

  const pm = MappedSubject.create(([x]: readonly number[], prev?: number) => (prev ?? 100) + x, a);
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
  const arr = ArraySubject.create([1, 2]);
  const log: any[] = [];
  arr.sub(rec(log));
  arr.set([3, 4, 5]);
  arr.clear();
  arr.insert(9);
  arr.insert(8, 0);
  arr.removeAt(1);
  arr.set([]);
  arr.clear();
  const log2: any[] = [];
  const a2 = ArraySubject.create([7, 8]);
  a2.sub(rec(log2), true);
  const log3: any[] = [];
  const a3 = ArraySubject.create<number>();
  a3.sub(rec(log3), true);
  return { log, log2, log3, len: arr.length, items: [...arr.getArray()] };
});

add('U04', () => ({
  and: t(() => SubscribableMapFunctions.and()([])),
  or: t(() => SubscribableMapFunctions.or()([])),
  nor: t(() => SubscribableMapFunctions.nor()([])),
  not0: t(() => SubscribableMapFunctions.not()(0)),
  notTrue: t(() => SubscribableMapFunctions.not()(true)),
  and2: t(() => SubscribableMapFunctions.and()([true, false])),
  or2: t(() => SubscribableMapFunctions.or()([false, true])),
  nor2: t(() => SubscribableMapFunctions.nor()([false, false])),
}));

add('U05', () => {
  const { p, s } = pubsub();
  const log: string[] = [];
  p.pub('c1', 1, false, true);
  log.push('before');
  s.on('c1').handle((v) => log.push(`h${v}`));
  log.push('after');
  p.pub('c2', 2);
  s.on('c2').handle((v) => log.push(`d${v}`));
  log.push('after2');
  p.pub('c3', 3, false, false);
  s.on('c3').handle((v) => log.push(`e${v}`));
  log.push('after3');
  const cs = ConsumerSubject.create(s.on('c1'), 0);
  log.push(`cs${cs.get()}`);
  const cs2 = ConsumerSubject.create<number>(null, 9);
  log.push(`cs2${cs2.get()}`);
  cs2.setConsumer(s.on('c1'));
  log.push(`cs2b${cs2.get()}`);
  cs2.setConsumer(null);
  log.push(`cs2c${cs2.get()}`);
  const log2: number[] = [];
  s.on('c1').whenChanged().handle((v) => log2.push(v));
  p.pub('c1', 1);
  p.pub('c1', 2);
  const log3: string[] = [];
  const paused = s.on('c1').handle((v) => log3.push(`p${v}`), true);
  log3.push('created');
  paused.resume(true);
  log3.push('resumed');
  p.pub('c1', 3);
  return { log, log2, log3 };
});

add('U06', () => {
  const { bus, p, s } = pubsub();
  H.log.length = 0;
  s.on('t').handle((v) => H.log.push(['local', 'handle', v]));
  p.pub('t', 1, true);
  H.log.push(['mark', 'between']);
  p.pub('t', 2, false);
  const order = H.log.map((e: any[]) => `${e[0]}:${e[1]}:${typeof e[2] === 'string' ? e[2] : JSON.stringify(e[2])}`);
  void bus;
  return order;
});

const timed = (build: (c: any) => any, real: boolean, sim: boolean) => {
  const { p, s } = pubsub();
  const got: number[] = [];
  build(s.on('v')).handle((v: number) => got.push(v));
  for (let i = 0; i <= 30; i++) {
    if (real) H.clock.real += 100;
    if (sim) H.clock.sim += 100;
    p.pub('v', i);
  }
  return got;
};

add('U07', () => ({
  every1Real: t(() => timed((c) => c.every(1), true, false)),
  every05Real: t(() => timed((c) => c.every(0.5), true, false)),
  every1Sim: t(() => timed((c) => c.every(1), false, true)),
  every1Both: t(() => timed((c) => c.every(1), true, true)),
}));

add('U08', () => {
  const { p, s } = pubsub();
  const a: number[] = [];
  const b: number[] = [];
  const c: number[] = [];
  const d: number[] = [];
  s.on('v').withPrecision(1).handle((v) => a.push(v));
  s.on('v').withPrecision(0).handle((v) => b.push(v));
  s.on('v').whenChangedBy(0.5).handle((v) => c.push(v));
  s.on('v').withPrecision(1).whenChanged().handle((v) => d.push(v));
  for (const v of [1.04, 1.06, 1.12, 1.14, 1.15, 1.16, 2.5, 2.49, -0.05, 0.05]) p.pub('v', v);
  return { a, b, c, d };
});

add('U09', () => ({
  af1Real: t(() => timed((c) => c.atFrequency(1), true, false)),
  af2Real: t(() => timed((c) => c.atFrequency(2), true, false)),
  af1Sim: t(() => timed((c) => c.atFrequency(1), false, true)),
  af1ImmReal: t(() => timed((c) => c.atFrequency(1, true), true, false)),
  af1NoImmReal: t(() => timed((c) => c.atFrequency(1, false), true, false)),
  onlyAfterReal: t(() => timed((c) => c.onlyAfter(500), true, false)),
  onlyAfterSim: t(() => timed((c) => c.onlyAfter(500), false, true)),
}));

add('U10', () => {
  class P extends (BasePublisher as any) {
    go(v: number) {
      (this as any).publish('bp', v);
    }
  }
  const { bus, s } = pubsub();
  const got: number[] = [];
  s.on('bp').handle((v) => got.push(v));
  const pb: any = new P(bus);
  pb.go(1);
  pb.startPublish();
  pb.go(2);
  pb.onUpdate();
  pb.go(3);
  const late: number[] = [];
  s.on('bp').handle((v) => late.push(v));
  return { got, late };
});

const simPub = (entries: [string, any][]) => {
  const { bus, s } = pubsub();
  const sp: any = new SimVarPublisher(new Map<any, any>(entries), bus);
  return { bus, s, sp };
};

add('U11', () => {
  H.resetSim({ 'L:PX': 5, 'L:PY': 1 });
  const { s, sp } = simPub([
    ['px', { name: 'L:PX', type: SimVarValueType.Number }],
    ['py', { name: 'L:PY', type: SimVarValueType.Number }],
  ]);
  const got: number[] = [];
  s.on('px').handle((v) => got.push(v));
  const readsBeforeStart = H.readNames();
  sp.startPublish();
  sp.onUpdate();
  sp.onUpdate();
  H.sim.set('L:PX', 6);
  sp.onUpdate();
  const late: number[] = [];
  s.on('px').handle((v) => late.push(v));
  sp.onUpdate();
  return { got, late, readsBeforeStart, reads: H.readNames() };
});

add('U12', () => {
  H.resetSim({ 'L:IX_1': 11, 'L:IX_2': 22, 'L:IX_0': 100, 'L:IY_1': 31 });
  const { s, sp } = simPub([
    ['ix', { name: 'L:IX_#index#', type: SimVarValueType.Number, indexed: true }],
    ['iy', { name: 'L:IY_#index#', type: SimVarValueType.Number, indexed: true, defaultIndex: null }],
    ['iz', { name: 'L:IZ_#index#', type: SimVarValueType.Number, indexed: true, defaultIndex: 3 }],
  ]);
  const g: Record<string, number[]> = {};
  for (const topic of ['ix', 'ix_1', 'ix_2', 'iy', 'iy_1', 'iz', 'iz_3']) {
    g[topic] = [];
    s.on(topic).handle((v) => g[topic].push(v));
  }
  sp.startPublish();
  sp.onUpdate();
  return { g, reads: H.readNames() };
});

add('U13', () => {
  H.resetSim({ 'L:B1': 1, 'L:B0': 0, 'L:B5': 5 });
  const { s, sp } = simPub([
    ['b1', { name: 'L:B1', type: SimVarValueType.Bool }],
    ['b0', { name: 'L:B0', type: SimVarValueType.Bool }],
    ['b5', { name: 'L:B5', type: SimVarValueType.Bool }],
  ]);
  const got: any[] = [];
  for (const topic of ['b1', 'b0', 'b5']) s.on(topic).handle((v) => got.push([topic, v === true ? 'true' : v === false ? 'false' : v]));
  sp.startPublish();
  sp.onUpdate();
  return got;
});

add('U14', () => {
  H.resetSim({});
  const enumObj: Record<string, any> = {};
  for (const [k, v] of Object.entries(SimVarValueType)) enumObj[k] = v;
  const entries: [string, any][] = Object.entries(SimVarValueType)
    .filter(([k]) => Number.isNaN(Number(k)))
    .map(([k, v]) => [`u_${k}`, { name: `L:U_${k}`, type: v }]);
  const { s, sp } = simPub(entries);
  for (const [topic] of entries) s.on(topic).handle(() => {});
  sp.startPublish();
  sp.onUpdate();
  return { enumObj, units: H.readUnits() };
});

add('U15', () => {
  const { bus, s } = pubsub();
  H.log.length = 0;
  const got: string[] = [];
  s.on('hEvent').handle((v) => got.push(String(v)));
  const hp: any = new HEventPublisher(bus);
  hp.dispatchHEvent('A');
  hp.startPublish?.();
  hp.dispatchHEvent('B');
  hp.dispatchHEvent('C', true);
  return { got, transport: H.log.map((e: any[]) => `${e[0]}:${e[1]}:${typeof e[2] === 'string' ? e[2] : ''}`) };
});

add('U16', () => {
  H.resetSim({});
  const { bus, s } = pubsub();
  const topics = ['simTime', 'simTimeHiFreq', 'realTime', 'activeSimDuration'];
  const seen: Record<string, number[]> = {};
  for (const topic of topics) {
    seen[topic] = [];
    s.on(topic).handle((v) => seen[topic].push(v));
  }
  const clock: any = new Clock(bus);
  clock.init();
  for (let i = 0; i < 6; i++) {
    H.clock.real += 50;
    H.clock.sim += 50;
    clock.onUpdate();
  }
  const rel: Record<string, number[]> = {};
  for (const topic of topics) rel[topic] = seen[topic].map((v) => v - (seen[topic][0] ?? 0));
  return { counts: Object.fromEntries(topics.map((k) => [k, seen[k].length])), first: Object.fromEntries(topics.map((k) => [k, seen[k][0]])), rel, reads: H.readNames() };
});

add('U17', () => {
  H.resetSim({});
  const { bus, s } = pubsub();
  const got: any[] = [];
  s.on('stall_warning_on').handle((v) => got.push(v));
  const sw: any = new StallWarningPublisher(bus, 0.9);
  sw.startPublish();
  sw.onUpdate();
  const names: string[] = H.readNames();
  const out: Record<string, any[]> = {};
  for (const v of [0, 0.5, 0.85, 0.89, 0.9, 0.91, 0.95, 1.2]) {
    for (const n of names) H.sim.set(n, v);
    got.length = 0;
    sw.onUpdate();
    out[String(v)] = [...got];
  }
  return { names, out };
});

const mkPub = (log: string[], n: string) => ({ startPublish: () => log.push(`sp${n}`), onUpdate: () => log.push(`pu${n}`) });
const mkInst = (log: string[], n: string) => ({ init: () => log.push(`ii${n}`), onUpdate: () => log.push(`iu${n}`) });

add('U18', () => {
  const log: string[] = [];
  const bp: any = new InstrumentBackplane();
  const errs: string[] = [];
  for (const f of [
    () => bp.addPublisher('a', mkPub(log, '1')),
    () => bp.addPublisher('a', mkPub(log, '2')),
    () => bp.addInstrument('x', mkInst(log, '1')),
    () => bp.addInstrument('x', mkInst(log, '2')),
    () => bp.addPublisher('b', mkPub(log, '3')),
    () => bp.addPublisher('b', mkPub(log, '4'), true),
  ]) {
    try {
      f();
    } catch (e: any) {
      errs.push(String(e?.message ?? e));
    }
  }
  bp.init();
  bp.onUpdate();
  return { log, errs };
});

add('U19', () => {
  const log: string[] = [];
  const bp: any = new InstrumentBackplane();
  bp.addInstrument('i1', mkInst(log, '1'));
  bp.addPublisher('p1', mkPub(log, '1'));
  bp.addInstrument('i2', mkInst(log, '2'));
  bp.addPublisher('p2', mkPub(log, '2'));
  bp.init();
  log.push('|');
  bp.onUpdate();
  log.push('|');
  bp.addPublisher('p3', mkPub(log, '3'));
  bp.onUpdate();
  return log;
});

add('U19b', () => {
  const run = (adds: [string, string][], initFirst = false) => {
    const log: string[] = [];
    const bp: any = new InstrumentBackplane();
    const errs: string[] = [];
    const go = (kind: string, name: string) => {
      try {
        if (kind === 'p') bp.addPublisher(name, mkPub(log, name));
        else bp.addInstrument(name, mkInst(log, name));
      } catch (e: any) {
        errs.push(String(e?.message ?? e));
      }
    };
    if (initFirst) bp.init();
    for (const [k, n] of adds) go(k, n);
    if (!initFirst) bp.init();
    log.push('|');
    bp.onUpdate();
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
    Update() {
      log.push('fs-update');
    }
    onInteractionEvent() {
      log.push('fs-interaction');
    }
    onGameStateChanged() {
      log.push('fs-gamestate');
    }
    onFlightStart() {
      log.push('fs-flightstart');
    }
    onSoundEnd() {
      log.push('fs-soundend');
    }
    onPowerOn() {
      log.push('fs-poweron');
    }
    onPowerOff() {
      log.push('fs-poweroff');
    }
  }
  class X extends (FsBaseInstrument as any) {
    constructInstrument() {
      log.push('construct');
      return new Inst();
    }
    get templateID() {
      return 'X';
    }
  }
  const x: any = new X();
  log.push('after-new');
  for (const m of ['connectedCallback', 'Init', 'onFlightStart', 'Update', 'onInteractionEvent', 'onSoundEnd', 'onPowerOn', 'onShutDown', 'onGameStateChanged']) {
    log.push(`call:${m}`);
    try {
      if (typeof x[m] === 'function') x[m](m === 'onInteractionEvent' ? ['EV'] : m === 'onGameStateChanged' ? 1 : undefined, 3);
      else log.push(`missing:${m}`);
    } catch (e: any) {
      log.push(`err:${m}:${String(e?.message ?? e)}`);
    }
  }
  return [...log];
});

add('U31', () => {
  const log: string[] = [];
  class A extends DisplayComponent<any> {
    onAfterRender() {
      log.push('A.after');
    }
    render() {
      return <div />;
    }
  }
  class B extends DisplayComponent<any> {
    mounted() {
      log.push('B.mounted');
    }
    render() {
      return <div />;
    }
  }
  class C extends DisplayComponent<any> {
    onAfterRender() {
      log.push('C.after');
    }
    mounted() {
      log.push('C.mounted');
    }
    render() {
      return <div />;
    }
  }
  class D extends DisplayComponent<any> {
    render() {
      return <div />;
    }
  }
  const root = document.createElement('div');
  FSComponent.render(
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
  class G extends DisplayComponent<any> {
    render() {
      return (
        <g>
          <path d="M0 0" />
        </g>
      );
    }
  }
  class Dv extends DisplayComponent<any> {
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
    FSComponent.render(
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
    FSComponent.render(<G />, svgRoot as any);
    FSComponent.render(
      <g id="g2">
        <circle />
      </g>,
      svgRoot as any,
    );
    FSComponent.render(<Dv />, svgRoot as any);
    FSComponent.render(<text>z</text>, svgRoot as any);
    return walk(svgRoot, 0, []);
  });
  const r3 = t(() => {
    const host = document.createElement('div');
    FSComponent.render(<G />, host);
    FSComponent.render(<text>q</text>, host);
    return walk(host, 0, []);
  });
  return { r1, r2, r3 };
});

add('U33', () => {
  const log: string[] = [];
  const root = document.createElement('div');
  const r = t(() => {
    FSComponent.render(
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
  const s = Subject.create<any>(null);
  const root = document.createElement('div');
  const r = t(() => {
    FSComponent.render(<div id="x" data-a={s} data-b={null} data-c={undefined} data-d={0} data-e={false} data-f={true} />, root);
    const el = root.querySelector('#x')!;
    const out = [attrs(el)];
    for (const v of ['v', undefined, false, 0, '', null, true]) {
      s.set(v);
      out.push(attrs(el));
    }
    return out;
  });
  const cls = t(() => {
    const c = Subject.create<any>(null);
    const host = document.createElement('div');
    FSComponent.render(<div id="y" class={c} />, host);
    const el = host.querySelector('#y')!;
    const out = [attrs(el)];
    for (const v of ['k', undefined, 'a b']) {
      c.set(v);
      out.push(attrs(el));
    }
    return out;
  });
  const clsRec = t(() => {
    const on = Subject.create(true);
    const host = document.createElement('div');
    FSComponent.render(<div id="z" class={{ one: on, two: false, three: true } as any} />, host);
    const el = host.querySelector('#z')!;
    const out = [el.getAttribute('class')];
    on.set(false);
    out.push(el.getAttribute('class'));
    return out;
  });
  const style = t(() => {
    const w = Subject.create<any>('10px');
    const host = document.createElement('div');
    FSComponent.render(<div id="s" style={{ width: w, height: '5px', display: undefined } as any} />, host);
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
    const s = Subject.create<any>(<span>a</span>);
    const root = host();
    FSComponent.render(<div id="p">{s}</div>, root);
    const h = [root.innerHTML];
    s.set(<b>b</b>);
    h.push(root.innerHTML);
    return h;
  });
  const array = t(() => {
    const s = Subject.create<any>([<i>1</i>, <i>2</i>]);
    const root = host();
    FSComponent.render(<div>{s}</div>, root);
    const h = [root.innerHTML];
    s.set([<i>3</i>]);
    h.push(root.innerHTML);
    return h;
  });
  const text = t(() => {
    const s = Subject.create<any>('x');
    const root = host();
    FSComponent.render(
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
    FSComponent.render(
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
    const arr = ArraySubject.create([1, 2]);
    const root = host();
    FSComponent.render(<div>{arr as any}</div>, root);
    const h = [root.innerHTML];
    arr.insert(3);
    h.push(root.innerHTML);
    return h;
  });
  return { vnode, array, text, statics, list };
});

add('U53', () => {
  const root = document.createElement('div');
  const s = Subject.create<any>('X<br />Y');
  FSComponent.render(
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
  const ref = FSComponent.createRef<any>();
  const out: any = {};
  try {
    const v = ref.instance;
    out.threw = false;
    out.value = v === undefined ? '__undef__' : String(v);
  } catch (e: any) {
    out.threw = true;
    out.msg = String(e?.message ?? e);
  }
  out.def = t(() => ref.getOrDefault());
  const root = document.createElement('div');
  FSComponent.render(<div ref={ref} id="r" />, root);
  out.after = t(() => ref.instance?.id);
  out.defAfter = t(() => ref.getOrDefault()?.id);
  return out;
});

add('U37', () => {
  const log: string[] = [];
  class Leaf extends DisplayComponent<{ n: string }> {
    onAfterRender() {
      log.push(`after:${this.props.n}`);
    }
    render() {
      return <span>{this.props.n}</span>;
    }
  }
  class Mid extends DisplayComponent<{ n: string }> {
    private readonly ref = FSComponent.createRef<HTMLDivElement>();
    onAfterRender() {
      log.push(`after:${this.props.n}:${t(() => !!this.ref.instance)}:${t(() => !!this.ref.instance?.isConnected)}`);
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
  class Top extends DisplayComponent<any> {
    onAfterRender() {
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
  FSComponent.render(<Top />, root);
  root.remove();
  return log;
});

add('U38', () => {
  const log: string[] = [];
  class Child extends DisplayComponent<any> {
    destroy() {
      log.push('child.destroy');
      super.destroy();
    }
    render() {
      return <span />;
    }
  }
  class Parent extends DisplayComponent<any> {
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
  FSComponent.render(vn, root);
  const r = t(() => vn.instance.destroy());
  const html = root.innerHTML;
  const lc = t(() => {
    const src = Subject.create(1);
    let mapped: any;
    class L extends (LifecycleComponent as any) {
      render() {
        mapped = src.map((x: number) => x * 2).withLifecycle((this as any).defaultLifecycle);
        return <div />;
      }
    }
    const node: any = <L />;
    FSComponent.render(node, document.createElement('div'));
    const before = mapped.isAlive;
    node.instance.destroy();
    return { before, after: mapped.isAlive };
  });
  return { log, r, html, lc };
});

add('U40', () =>
  [[2.5], [-2.5], [1.5], [0.5], [-0.5], [0.125, 0.25], [0.375, 0.25], [1.005, 0.01], [2.345, 0.01], [7, 5], [-7.5, 5], [12.5, 1], [3.14159, 0.001]].map((a) =>
    t(() => MathUtils.round(a[0], a[1])),
  ),
);

add('U41', () =>
  [[10, 350], [350, 10], [0, 180], [180, 0], [0, 190], [190, 0], [-10, 10], [720, 0], [0, -180], [90, 270]].map((a) => t(() => MathUtils.diffAngleDeg(a[0], a[1]))),
);

add('U42', () => [-500, 0, 5000, 11000, 15000, 20000, 25000, 32000, 47000].map((h) => t(() => AeroMath.isaTemperature(h))));

add('U43', () => {
  const a = new ExpSmoother(1);
  const ra = [a.last(), a.next(10, 0.1), a.next(20, 0.1), a.last(), a.next(20, 0)];
  const b = new ExpSmoother(1, 5, 1);
  const rb = [b.last(), b.next(10, 0.1), b.next(30, 2), b.next(40, 0.5), b.next(40, 1)];
  const c = new ExpSmoother(1, null, 1);
  const rc = [c.next(10, 0.1), c.next(20, 5), c.next(30, 0.1)];
  const d = new ExpSmoother(0, 3);
  const rd = [d.next(10, 0.1), d.next(20, 0.1)];
  return { ra, rb, rc, rd };
});

add('U44', () => {
  const t1 = new LerpLookupTable([
    [0, 0],
    [100, 10],
  ]);
  const t2 = new LerpLookupTable([
    [0, 0, 0],
    [10, 1, 0],
    [20, 0, 1],
    [30, 1, 1],
  ]);
  const t3 = new LerpLookupTable([
    [5, 10],
    [1, 0],
    [3, 5],
  ]);
  const t4 = new LerpLookupTable([[7, 2]]);
  return {
    t1: [-5, 0, 5, 10, 15].map((k) => t(() => t1.get(k))),
    t2: [
      [0.5, 0.5],
      [2, 2],
      [-1, 0.5],
      [1, 0],
    ].map((k) => t(() => t2.get(...k))),
    t3: [0, 2, 4, 7, 12].map((k) => t(() => t3.get(k))),
    t4: [0, 2, 3].map((k) => t(() => t4.get(k))),
  };
});

add('U45', () => ({ ...(AirportClassMask as any) }));

add('U46', () => ({
  faa: { ...(FaaChartType as any) },
  lido: { ...(LidoChartType as any) },
  provider: { ...(BuiltInChartProvider as any) },
  err: { ...(ChartImageErrorCode as any) },
}));

add('U47', async () => {
  H.useRealTime(true);
  try {
    const falseTimeout = await settle(Wait.awaitCondition(() => false, 10, 60), 600);
    const trueNow = await settle(Wait.awaitCondition(() => true, 10, 60), 600);
    let n = 0;
    const eventually = await settle(Wait.awaitCondition(() => ++n > 3, 10, 2000), 600);
    const zeroTimeout = await settle(Wait.awaitCondition(() => false, 10, 0), 300);
    const delay = await settle(Wait.awaitDelay(20), 300);
    const frames = await settle(Wait.awaitFrames(2), 500);
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
      const s = Subject.create<any>(init);
      const p = Wait.awaitSubscribable(s);
      const first = await settle(p, 50);
      s.set(init === 1 ? 2 : 1);
      const second = first.state === 'pending' ? await settle(p, 50) : 'n/a';
      out[name] = { first, second };
    }
    const s = Subject.create<any>(0);
    out.takeInitial = await settle(Wait.awaitSubscribable(s, (v) => v === 0, true), 50);
    out.noTakeInitial = await settle(Wait.awaitSubscribable(s, (v) => v === 0, false), 50);
    out.timeout = await settle(Wait.awaitSubscribable(s, (v) => v === 9, false, 40), 300);
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
    const f = t(() => NumberFormatter.create(o));
    out[k] = typeof f === 'function' ? values.map((v) => t(() => f(v))) : f;
  }
  return out;
});

add('U50', () => {
  const lat = DmsFormatter2.create('{dd}°{mm}.{s}{+[N]-[S]}', UnitType.DEGREE, 0.1);
  const lon = DmsFormatter2.create('{ddd}°{mm}.{s}{+[E]-[W]}', UnitType.DEGREE, 0.1);
  const values = [51.99999, 51.9999, 0.5, -0.0001, 12.3456, -33.95, 179.999, 0, -0, NaN, 59.9917, 1.0008333];
  const nan = t(() => DmsFormatter2.create('{dd}°{mm}', UnitType.DEGREE, 1, 'XX')(NaN));
  const tokens = ['{d}', '{dd}', '{ddd}', '{m}', '{mm}', '{s}', '{ss}', '{+[N]-[S]}', '{-[S]+[N]}', '{+}', '{-}'].map((f) => [
    f,
    t(() => DmsFormatter2.create(f, UnitType.DEGREE, 0.1)(-7.508)),
  ]);
  return { lat: values.map((v) => t(() => lat(v))), lon: values.map((v) => t(() => lon(-v))), nan, tokens };
});

add('U51', () => {
  const fmt = AiracCycleFormatter.create('{eff({dd}{MON}{YY})}-{exp({dd}{MON}{YY})}({YYCC})');
  const cycles: any = t(() => FacilityLoader.getDatabaseCycles());
  const show = (c: any) => (c && typeof c === 'object' ? Object.fromEntries(Object.entries(c).filter(([, v]) => typeof v !== 'function')) : c);
  return {
    cycles: cycles && !cycles.error ? { previous: show(cycles.previous), current: show(cycles.current), next: show(cycles.next) } : cycles,
    current: t(() => fmt(cycles.current)),
    previous: t(() => fmt(cycles.previous)),
    next: t(() => fmt(cycles.next)),
    other: t(() => AiracCycleFormatter.create('{YYCC} {eff({dd}/{mm}/{YYYY})} {exp({MON})} {cc}')(cycles.current)),
  };
});

add('U52', async () => {
  const a = await settle(SharedGlobal.get<any>('probe') as any, 300);
  const b = await settle(SharedGlobal.get<any>('probe') as any, 300);
  const desc = (r: any) =>
    r.state === 'resolved'
      ? { state: r.state, owner: r.v?.isViewOwner, hasInstance: !!r.v?.instance, detached: t(() => r.v?.isDetached?.get()) }
      : r;
  return { a: desc(a), b: desc(b), sameInstance: a.state === 'resolved' && b.state === 'resolved' ? a.v.instance === b.v.instance : 'n/a' };
});

add('K01', async () => {
  const { bus } = pubsub();
  H.log.length = 0;
  const r: any = await settle(KeyEventManager.getManager(bus), 300);
  const out: any = { state: r.state, afterGet: H.transport() };
  if (r.state === 'resolved') {
    H.log.length = 0;
    t(() => r.v.interceptKey('PROBE_KEY', false));
    out.intercept = H.transport();
    H.log.length = 0;
    out.trigger = await settle(r.v.triggerKey('PROBE_KEY_2', true, 7), 300);
    out.triggerCalls = H.transport();
  }
  return out;
});

add('K02', async () => {
  const { bus, s } = pubsub();
  const got: any[] = [];
  s.on('key_intercept').handle((v) => got.push(v));
  const r: any = await settle(KeyEventManager.getManager(bus), 300);
  if (r.state !== 'resolved') return { state: r.state };
  t(() => r.v.interceptKey('PROBE_AXIS', false));
  H.emit('keyIntercepted', 'PROBE_AXIS', 11, 22, 33);
  H.emit('keyIntercepted', 'PROBE_AXIS', 5);
  return got;
});

add('R01', () => {
  const log: string[] = [];
  let n = 0;
  const heap = new ResourceHeap<any>(
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
  const got = [heap.allocate(), heap.allocate(), heap.allocate(), heap.allocate(), heap.allocate()];
  log.push('|');
  for (const g of got) heap.free(g);
  log.push('|');
  const more = [heap.allocate(), heap.allocate(), heap.allocate(), heap.allocate(), heap.allocate(), heap.allocate()];
  const over = t(() => heap.allocate());
  log.push('|');
  for (const g of more) heap.free(g);
  return { log, over: over && over.error ? over : 'ok' };
});

add('X01', () => ({
  flat: ArrayUtils.flat([1, [2, 3], [[4]] as any]),
  any: [BitFlags.isAny(5, 4), BitFlags.isAny(5, 2), BitFlags.isAll(7, 5), BitFlags.isAll(5, 7), BitFlags.isAny(0, 0), BitFlags.isAll(0, 0)],
  vec: t(() => {
    const v = Vec2Math.create();
    Vec2Math.setFromPolar(2, Math.PI / 2, v);
    return [Array.from(Vec2Math.create(3, 4)), Vec2Math.abs(Vec2Math.create(3, 4)), Vec2Math.theta(Vec2Math.create(-1, 0)), Array.from(v).map((x) => Math.round(x * 1e9) / 1e9)];
  }),
  css: t(() => {
    const c = CssTransformBuilder.translate3d('px');
    const r = [c.resolve()];
    c.set(1.234, 5, 0, 0.1, 1, 1);
    r.push(c.resolve());
    const d = CssTransformBuilder.translate3d();
    d.set(1, 2, 3);
    r.push(d.resolve());
    return r;
  }),
  units: t(() => [
    UnitType.NMILE.convertTo(1, UnitType.METER),
    UnitType.FOOT.convertTo(1000, UnitType.METER),
    UnitType.KILOGRAM.convertTo(1000, UnitType.POUND),
    UnitType.TONNE.convertTo(1, UnitType.KILOGRAM),
    UnitType.KILOMETER.convertFrom(1, UnitType.NMILE),
    UnitType.METER.createNumber(5).asUnit(UnitType.FOOT),
  ]),
  nus: t(() => {
    const s = NumberUnitSubject.create(UnitType.METER.createNumber(100));
    const ft = s.asUnit(UnitType.FOOT);
    const r = [ft.get()];
    s.set(200);
    r.push(ft.get());
    s.set(1, UnitType.KILOMETER);
    r.push(ft.get(), s.get().number);
    return r;
  }),
}));

add('X02', () => {
  const ms = MapSubject.create<string, number>([['a', 1]]);
  const mlog: any[] = [];
  ms.sub((_m, ty, k, v) => mlog.push([ty, k, v]), true);
  ms.setValue('b', 2);
  ms.setValue('a', 1);
  ms.setValue('a', 3);
  ms.delete('b');
  ms.delete('zz');
  ms.clear();
  const ss = SetSubject.create([1, 2]);
  const sm = ss.map((s) => [...s].sort().join(','));
  const slog: string[] = [sm.get()];
  sm.sub((v) => slog.push(v));
  ss.add(3);
  ss.add(3);
  ss.delete(1);
  ss.clear();
  const os = ObjectSubject.create({ a: 1, b: 2 });
  const olog: any[] = [];
  os.sub((v) => olog.push({ ...v }));
  os.set({ a: 1 });
  os.set({ a: 5 });
  os.set({ b: 7, a: 5 });
  return { mlog, slog, olog, size: ms.size };
});

add('X03', () => {
  const ts = Date.UTC(2026, 9, 21, 7, 5, 9);
  const fmts = ['{HH}:{mm}', '{HH}{mm}', '{dd}/{mm}/{YYYY}', '{HH}:{mm}:{ss}', '{MON} {dd}', '{h}:{mm} {am}'];
  return Object.fromEntries(
    fmts.map((f) => [f, [ts, NaN, 0].map((v) => t(() => DateTimeFormatter.create(f, { nanString: '--:--' })(v)))]),
  );
});

add('U06b', async () => {
  H.useRealTime(true);
  try {
    const { p, s } = pubsub();
    const got: number[] = [];
    s.on('t2').handle((v) => got.push(v));
    await new Promise((r) => H.realSetTimeout(r, 60));
    H.log.length = 0;
    p.pub('t2', 7, true);
    p.pub('t2', 8, false);
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
    for (const k of Object.keys(n)) s.on(k).handle(() => n[k]++);
    const clock: any = new Clock(bus);
    clock.init();
    await new Promise((r) => H.realSetTimeout(r, 250));
    const idle = { ...n };
    for (let i = 0; i < 3; i++) clock.onUpdate();
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
