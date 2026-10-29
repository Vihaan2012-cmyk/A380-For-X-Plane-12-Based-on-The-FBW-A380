import type { NavCycle, Readable } from 'xplane-js/ui';
import { Signal } from 'xplane-js/ui';

export interface IcaoCode {
  readonly type: string;
  readonly region: string;
  readonly airport: string;
  readonly ident: string;
}

export class IcaoCodes {
  public static make(type: string, region: string, airport: string, ident: string): IcaoCode {
    return { type, region, airport, ident };
  }
}

const CYCLE_MS = 28 * 86_400_000;
const CYCLE_ANCHOR = Date.UTC(2020, 0, 2);

function cycleStarting(index: number): NavCycle {
  const effective = CYCLE_ANCHOR + index * CYCLE_MS;
  const year = new Date(effective).getUTCFullYear();
  const jan1 = Date.UTC(year, 0, 1);
  const firstOfYear = CYCLE_ANCHOR + Math.ceil((jan1 - CYCLE_ANCHOR) / CYCLE_MS) * CYCLE_MS;
  const cycle = Math.round((effective - firstOfYear) / CYCLE_MS) + 1;
  return {
    cycle,
    cycleString: String(cycle).padStart(2, '0'),
    ident: `${String(year % 100).padStart(2, '0')}${String(cycle).padStart(2, '0')}`,
    effectiveTimestamp: effective,
    expirationTimestamp: effective + CYCLE_MS,
  } as NavCycle;
}

export class NavDatabase {
  public static cycles(): { previous: NavCycle; current: NavCycle; next: NavCycle } {
    const index = Math.floor((Date.now() - CYCLE_ANCHOR) / CYCLE_MS);
    return { previous: cycleStarting(index - 1), current: cycleStarting(index), next: cycleStarting(index + 1) };
  }
}

export enum AirportKinds {
  None = 0,
  HardSurface = 2,
  SoftSurface = 4,
  AllWater = 8,
  HeliportOnly = 16,
  Private = 32,
}

export interface RunwayId {
  number: string;
  designator: string;
}

export type ComSlot = 1 | 2 | 3;

export interface MetarReport {
  icao: string;
  metarString: string;
}

export enum ChartOrigin {
  Faa = 'Faa',
  Lido = 'Lido',
}

export interface ChartInfo {
  guid: string;
  name: string;
  type: string;
  runways: RunwayId[];
}

export interface ChartList {
  charts: { charts: ChartInfo[] }[];
}

export interface ChartPageSet {
  pages: { urls: { name: string; url: string }[] }[];
}

export interface ChartPicture {
  imageUrl: string;
}

export enum ChartError {
  None,
  Timeout,
  UnspecifiedRetry,
  UnspecifiedNoRetry,
  Unknown,
}

export enum FaaChart {
  Apd = 'Apd',
  Dau = 'Dau',
  Dp = 'Dp',
  Hot = 'Hot',
  Iap = 'Iap',
  Lah = 'Lah',
  Min = 'Min',
  Odp = 'Odp',
  Star = 'Star',
  Unknown = 'Unknown',
}

export enum LidoChart {
  Afc = 'Afc',
  Agc = 'Agc',
  Aoi = 'Aoi',
  Apc = 'Apc',
  Iac = 'Iac',
  Lvc = 'Lvc',
  Mrc = 'Mrc',
  ObstDep = 'ObstDep',
  Sid = 'Sid',
  SidInitialClimb = 'SidInitialClimb',
  SidPt = 'SidPt',
  Star = 'Star',
  Unknown = 'Unknown',
  Vac = 'Vac',
}

const boundViews = new WeakSet<ChartPanel>();

export class ChartPanel {
  private readonly current = Signal.of<ChartPicture>({ imageUrl: '' });

  public get image(): Readable<ChartPicture> {
    return this.current;
  }

  public show(url: string): void {
    if (boundViews.has(this)) {
      this.current.set({ imageUrl: url });
    }
  }
}

export class Charts {
  public static attachViewer(view: ChartPanel): void {
    boundViews.add(view);
  }

  public static listFor(_icao: IcaoCode, _provider: string): Promise<ChartList> {
    return Promise.resolve({ charts: [] });
  }

  public static pagesOf(_guid: string): Promise<ChartPageSet> {
    return Promise.resolve({ pages: [] });
  }
}
