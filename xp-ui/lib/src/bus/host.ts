import { Signal } from 'xplane-js/ui';
import type { Readable } from 'xplane-js/ui';
import { Bus } from './messageBus';
import { TopicFeeder } from './publishers';
import { Bridge, SessionPhase, Vars, openChannel, sessionPhase } from 'xplane-js/data';

export interface ClockTopics {
  simTime: number;
  simTimeHiFreq: number;
  realTime: number;
  activeSimDuration: number;
}

const SECONDS_BEFORE_UNIX_EPOCH = 62135596800;

class TimeSampler {
  private _lastSim = NaN;
  private _awake = 0;

  public take(): ClockTopics {
    const real = Date.now();
    const sim = simNow();
    const sv = Vars;
    if (!Number.isNaN(this._lastSim)) {
      const step = sim - this._lastSim;
      if (step > 0 && step < 3600000) {
        this._awake += step;
      }
    }
    this._lastSim = sim;
    let active = this._awake;
    if (sv) {
      const simSeconds = sv.read('E:fbw/SIMULATION_TIME', 'seconds');
      if (typeof simSeconds === 'number' && Number.isFinite(simSeconds)) {
        active = simSeconds * 1000;
      }
    }
    return { simTime: sim, simTimeHiFreq: sim, realTime: real, activeSimDuration: active };
  }
}

function simNow(): number {
  const sv = Vars;
  if (sv) {
    const absolute = sv.read('E:fbw/ABSOLUTE_TIME', 'seconds');
    if (typeof absolute === 'number' && absolute > SECONDS_BEFORE_UNIX_EPOCH) {
      return (absolute - SECONDS_BEFORE_UNIX_EPOCH) * 1000;
    }
  }
  return Date.now();
}

function everyFrame(tick: () => void): void {
  const g = globalThis as any;
  const next = (): void => {
    if (typeof g.requestAnimationFrame === 'function') {
      g.requestAnimationFrame(step);
    } else {
      setTimeout(step, 16);
    }
  };
  const step = (): void => {
    tick();
    next();
  };
  next();
}

export class SimClock {
  private readonly _hub: Bus;
  private readonly _sampler = new TimeSampler();
  private _running = false;

  constructor(bus: Bus) {
    this._hub = bus;
  }

  public setup(): void {
    if (!this._running) {
      this._running = true;
      everyFrame(() => this._hub.post('simTimeHiFreq', simNow()));
    }
  }

  public tick(): void {
    if (!this._running) {
      return;
    }
    const s = this._sampler.take();
    this._hub.post('realTime', s.realTime);
    this._hub.post('simTime', s.simTime);
    this._hub.post('activeSimDuration', s.activeSimDuration);
  }
}

export class SimClockFeeder extends TopicFeeder<ClockTopics> {
  private readonly _sampler = new TimeSampler();

  private _ticking = false;

  constructor(bus: Bus) {
    super(bus);
  }

  public override begin(): void {
    super.begin();
    if (!this._ticking) {
      this._ticking = true;
      everyFrame(() => this.postTopic('simTimeHiFreq', simNow()));
    }
  }

  public override tick(): void {
    if (!this.isStarted()) {
      return;
    }
    const s = this._sampler.take();
    this.postTopic('realTime', s.realTime);
    this.postTopic('simTime', s.simTime);
    this.postTopic('activeSimDuration', s.activeSimDuration);
  }
}

export class SessionState {
  private static _shared: Signal<SessionPhase | undefined> | undefined;

  public static get(): Readable<SessionPhase | undefined> {
    if (!SessionState._shared) {
      const state = Signal.of<SessionPhase | undefined>(undefined);
      SessionState._shared = state;
      const refresh = (): void => {
        const g = globalThis as any;
        if (typeof sessionPhase === 'function') {
          state.set(sessionPhase());
        } else if (SessionPhase) {
          state.set(SessionPhase.flying);
        }
      };
      const loop = (): void => {
        refresh();
        setTimeout(loop, 100);
      };
      setTimeout(loop, 0);
    }
    return SessionState._shared;
  }
}

export interface KeyPress {
  key: string;
  value0?: number;
  value1?: number;
  value2?: number;
}

export interface KeyTopics {
  key_intercept: KeyPress;
}

const managers = new WeakMap<Bus, Promise<KeyControl>>();

export class KeyControl {
  private constructor(private readonly _pipe: any) {}

  public static connect(bus: Bus): Promise<KeyControl> {
    let found = managers.get(bus);
    if (!found) {
      found = new Promise<KeyControl>((resolve) => {
        const attempt = (): void => {
          const g = globalThis as any;
          const co = Bridge;
          if (co) {
            const mgr = new KeyControl(co);
            const forward = (key: string, v1?: number, v0?: number, v2?: number): void => {
              bus.post('key_intercept', { key, value0: v0, value1: v1, value2: v2 }, false, false);
            };
            if (typeof openChannel === 'function') {
              const listener = openChannel('keys', () => undefined);
              listener.on('key.intercepted', forward);
            } else {
              co.on('key.intercepted', forward);
            }
            resolve(mgr);
          } else {
            setTimeout(attempt, 100);
          }
        };
        attempt();
      });
      managers.set(bus, found);
    }
    return found;
  }

  public intercept(key: string, passThrough: boolean): void {
    this._pipe.call('key.intercept', key, passThrough ? 0 : 1);
  }

  public fire(key: string, bypass: boolean, value0?: number, value1?: number, value2?: number): Promise<void> {
    return Promise.resolve(this._pipe.call('key.fire', key, bypass, value0 ?? 0, value1 ?? 0, value2 ?? 0)).then(
      () => undefined,
    );
  }
}
