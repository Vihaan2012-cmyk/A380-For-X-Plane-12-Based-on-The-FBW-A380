import type * as Api from 'xplane-js/ui';
import * as Impl from './index';

type Pick1<K extends keyof typeof Impl & keyof typeof Api> = (typeof Impl)[K] extends (typeof Api)[K] ? true : never;

export const classChecks: [
  Pick1<'NumFormat'>,
  Pick1<'TimeFormat'>,
  Pick1<'AngleFormat'>,
  Pick1<'CycleFormat'>,
  Pick1<'Arrays'>,
  Pick1<'Bits'>,
  Pick1<'Maths'>,
  Pick1<'Vec2D'>,
  Pick1<'Isa'>,
  Pick1<'Smoother'>,
  Pick1<'LookupTable'>,
  Pick1<'Transforms'>,
  Pick1<'UnitList'>,
] = [true, true, true, true, true, true, true, true, true, true, true, true, true];

export const sample: Api.NumFormatOptions = {} as Impl.NumFormatOptions;
export const cyc: Api.NavCycle = {} as Impl.NavCycle;
export const cyc2: Impl.NavCycle = {} as Api.NavCycle;
export const ll1: Api.GeoPointLong = {} as Impl.GeoPointLong;
export const ll2: Api.GeoPoint = {} as Impl.GeoPoint;
export const fam: Api.Dimension = Impl.Dimension.Distance as unknown as Api.Dimension;
export const u: Api.Measure<Api.Dimension.Distance> = Impl.UnitList.METER;
export const n: Api.Quantity<Api.Dimension.Distance> = Impl.UnitList.METER.quantity(1);
export const ro: Api.ReadonlyVec = Impl.Vec2D.make() as Impl.ReadonlyVec;
export const css: Api.Translate3d = Impl.Transforms.translate3d();
