import { Bus } from './messageBus';
import { Vars } from 'xplane-js/data';

export class Pacer<E> {
  protected readonly _pacerFor?: E;
}

export interface BoardFeeder {
  begin(): void;
  tick?(): void;
}

export abstract class TopicFeeder<E> {
  private readonly _hub: Bus;
  private _open = false;

  constructor(bus: Bus, _pacer?: Pacer<E>) {
    this._hub = bus;
  }

  public begin(): void {
    this._open = true;
  }

  public tick(): void {}

  protected isStarted(): boolean {
    return this._open;
  }

  protected postTopic<K extends keyof E>(topic: K, data: E[K], sync = false, isCached = true): void {
    if (this._open) {
      this._hub.post(topic as string, data, sync, isCached);
    }
  }
}

export enum VarUnit {
  Number = 'number',
  Bool = 'bool',
  Enum = 'enum',
  Degree = 'degrees',
  String = 'string',
  Knots = 'knots',
  Feet = 'feet',
  NM = 'nautical mile',
  MHz = 'MHz',
  KHz = 'KHz',
  Seconds = 'seconds',
  PercentOver100 = 'percent over 100',
  LLA = 'latlonalt',
  PSI = 'psi',
  Radians = 'radians',
  GPH = 'gph',
  GAL = 'gallons',
  FPM = 'feet per minute',
}

export interface VarSpec {
  name: string;
  type: VarUnit;
}

export interface VarTopic<T> extends VarSpec {
  indexed?: boolean;
  defaultIndex?: number | null;
  transform?: (value: any) => T;
}

interface Located {
  name: string;
  entry: VarTopic<any>;
}

function withIndex(name: string, index: number): string {
  return name.includes('#index#') ? name.split('#index#').join(String(index)) : `${name}:${index}`;
}

export class VarPublisher<E> extends TopicFeeder<E> {
  protected readonly resolvedVars = new Map<string, VarSpec>();
  protected readonly watchedTopics = new Set<string>();
  private readonly _catalog = new Map<string, VarTopic<any>>();
  private readonly _origins = new Map<string, VarTopic<any> | undefined>();
  private readonly _demandBus: Bus;

  constructor(
    simVarMap: Iterable<readonly [keyof E, VarTopic<any>]>,
    bus: Bus,
    pacer?: Pacer<E>,
  ) {
    super(bus, pacer);
    for (const [topic, entry] of simVarMap) {
      this._catalog.set(topic as string, entry);
    }
    this._demandBus = bus;
    bus.watchDemand((topic) => {
      if (this.locate(topic)) {
        this.onTopicWatched(topic as keyof E & string);
      }
    });
  }

  protected onTopicWatched(topic: keyof E & string): void {
    const found = this.locate(topic);
    let def = this.resolvedVars.get(topic);
    if (!def) {
      if (!found) {
        return;
      }
      def = { name: found.name, type: found.entry.type };
      this.resolvedVars.set(topic, def);
    }
    this._origins.set(topic, found ? found.entry : this._catalog.get(topic));
    this.watchedTopics.add(topic);
  }

  public override tick(): void {
    if (!this.isStarted()) {
      return;
    }
    const sv = Vars;
    if (!sv) {
      return;
    }
    for (const topic of this.watchedTopics) {
      if (!this._demandBus.hasListeners(topic)) {
        this.watchedTopics.delete(topic);
        continue;
      }
      const def = this.resolvedVars.get(topic);
      if (!def) {
        continue;
      }
      let value = sv.read(def.name, def.type);
      const entry = this._origins.get(topic);
      if (entry && entry.transform) {
        value = entry.transform(value);
      } else if (def.type === VarUnit.Bool) {
        value = !!value;
      }
      this.postTopic(topic as keyof E, value);
    }
  }

  private locate(topic: string): Located | null {
    const direct = this._catalog.get(topic);
    if (direct) {
      if (!direct.indexed) {
        return { name: direct.name, entry: direct };
      }
      if (direct.defaultIndex === null) {
        return null;
      }
      return { name: withIndex(direct.name, direct.defaultIndex ?? 1), entry: direct };
    }
    const cut = /^(.+)_(\d+)$/.exec(topic);
    if (cut) {
      const base = this._catalog.get(cut[1]);
      if (base && base.indexed) {
        return { name: withIndex(base.name, Number(cut[2])), entry: base };
      }
    }
    return null;
  }
}

export interface PanelEventTopics {
  hEvent: string;
}

export class PanelEventPublisher extends TopicFeeder<PanelEventTopics> {
  constructor(bus: Bus) {
    super(bus);
  }

  public dispatchPanelEvent(hEvent: string, sync = false): void {
    this.postTopic('hEvent', hEvent, sync, false);
  }
}

export interface StallTopics {
  stall_warning_on: boolean;
}

export class StallPublisher extends VarPublisher<StallTopics> {
  private readonly _aoaLimit: number;

  constructor(bus: Bus, stallAoA: number, pacer?: Pacer<StallTopics>) {
    super(
      [['stall_warning_on', { name: 'fbw/STALL_WARNING', type: VarUnit.Bool, transform: (v: unknown) => !!v }]],
      bus,
      pacer,
    );
    this._aoaLimit = stallAoA;
  }
}

export interface WeightTopics {
  total_weight: number;
}

export class WeightPublisher extends VarPublisher<WeightTopics> {
  constructor(bus: Bus, pacer?: Pacer<WeightTopics>) {
    super([['total_weight', { name: 'sim/flightmodel/weight/m_total', type: 'pounds' as VarUnit }]], bus, pacer);
  }
}
