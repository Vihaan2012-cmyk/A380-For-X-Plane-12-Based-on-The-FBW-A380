import type * as Api from 'xplane-js/ui';
import * as Impl from './index';

type Covers<I, A> = [Exclude<keyof A, keyof I>] extends [never] ? true : never;
type Inst<T> = T extends abstract new (...a: any[]) => infer R ? R : never;

export const checks: [
  Covers<typeof Impl.UI, typeof Api.UI>,
  Covers<typeof Impl.View, typeof Api.View>,
  Covers<Inst<typeof Impl.View>, Inst<typeof Api.View>>,
  Covers<Inst<typeof Impl.ScopedView>, Inst<typeof Api.ScopedView>>,
  Covers<Inst<typeof Impl.Ref>, Inst<typeof Api.Ref>>,
  Covers<Inst<typeof Impl.Board>, Inst<typeof Api.Board>>,
  Covers<Inst<typeof Impl.HostedInstrument>, Inst<typeof Api.HostedInstrument>>,
  Covers<typeof Impl.Parts, typeof Api.Parts>,
  Covers<typeof Impl.Part, typeof Api.Part>,
] = [true, true, true, true, true, true, true, true, true];

export const nodeOk: Api.ViewNode = null as unknown as Impl.ViewNode;
export const propsOk: Api.ViewProps = null as unknown as Impl.ViewProps;
export const childOk: Api.ViewChildren = null as unknown as Impl.ViewChildren;
export const instOk: Api.BoardDevice = null as unknown as Impl.BoardDevice;
export const fsOk: Api.HostedDevice = null as unknown as Impl.HostedDevice;
export const bpOk: Api.BoardDevice = null as unknown as Impl.BoardDevice;
