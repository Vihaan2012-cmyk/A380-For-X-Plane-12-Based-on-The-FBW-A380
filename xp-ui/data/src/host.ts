export type PollKind = 'h' | 'provider' | 'game' | 'string';
export type PollEntry = [kind: PollKind, name: string, payload: string];
export type LogLevel = 0 | 1 | 2;

export interface Host {
  readonly view: number;
  readonly screen: string;
  readonly aircraftDir: string;
  snapshot(): void;
  getVar(name: string, unit: string): number;
  setVar(name: string, unit: string, value: number): void;
  getString(name: string): string;
  setString(name: string, value: string): void;
  sendEvent(name: string, values: number[]): void;
  call(name: string, argsJson: string): Promise<string>;
  poll(): PollEntry[];
  gameString(name: string): string;
  storedData(op: 'get' | 'set' | 'delete' | 'search', key: string, value: string): string;
  readFile(path: string): string | null;
  log(level: LogLevel, text: string): void;
  magVar(lat: number, lon: number): number;
  loaded(ok: boolean, text: string): void;
}

export type HostSource = () => Host | undefined;

export function globalHost(): Host | undefined {
  const h = (globalThis as { __xphfbw?: Host | null }).__xphfbw;
  return h == null ? undefined : h;
}

export function describeError(e: unknown): string {
  if (e instanceof Error) {
    return e.stack ? e.message + '\n' + e.stack : e.message;
  }
  if (e && typeof e === 'object' && 'message' in e) {
    const o = e as { message: unknown; stack?: unknown };
    return typeof o.stack === 'string' ? String(o.message) + '\n' + o.stack : String(o.message);
  }
  return String(e);
}

export function errorMessage(e: unknown): string {
  if (e instanceof Error) {
    return e.message;
  }
  if (e && typeof e === 'object' && 'message' in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}

export interface Context {
  host: HostSource;
  frameIndex(): number;
  log(level: LogLevel, text: string): void;
}

export type FrameCallback = (time: number) => void;

export function requestFrame(cb: FrameCallback): boolean {
  const g = globalThis as { requestAnimationFrame?: (cb: FrameCallback) => number };
  if (typeof g.requestAnimationFrame !== 'function') {
    return false;
  }
  g.requestAnimationFrame(cb);
  return true;
}

export function frameTime(time: unknown): number {
  if (typeof time === 'number' && Number.isFinite(time)) {
    return time;
  }
  return clockNow();
}

export function clockNow(): number {
  const g = globalThis as { performance?: { now(): number } };
  return g.performance && typeof g.performance.now === 'function' ? g.performance.now() : Date.now();
}
