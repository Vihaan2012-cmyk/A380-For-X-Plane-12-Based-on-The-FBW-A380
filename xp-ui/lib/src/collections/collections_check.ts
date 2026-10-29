import * as Api from 'xplane-js/ui';
import { SignalList, SignalSet, SignalMap, SignalRecord, QuantitySignal, ListChange, MapChange, makeStateList } from './index';

export const a: Api.SignalList<number> = SignalList.of<number>();
export const b: Api.SignalSet<number> = SignalSet.of<number>();
export const c: Api.SignalMap<string, number> = SignalMap.of<string, number>();
export const d: Api.SignalRecord<{ x: number }> = SignalRecord.of({ x: 1 });
export const e: Api.QuantitySignal<Api.Dimension.Distance> = QuantitySignal.of(Api.UnitList.METER.quantity(1));
export const f: Api.ListChange = ListChange.Added;
export const g: Api.MapChange = MapChange.Added;
export const h: Api.SignalList<number> = makeStateList([1]);
