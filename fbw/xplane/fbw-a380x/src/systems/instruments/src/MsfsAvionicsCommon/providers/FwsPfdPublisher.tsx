import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface FwsPfdBaseSimvars {
  memo_line: number;
  limitations_line: number;
  fws1_is_healthy: boolean;
  fws2_is_healthy: boolean;
  afdx_3_1_reachable: boolean;
  afdx_13_11_reachable: boolean;
  afdx_4_2_reachable: boolean;
  afdx_14_12_reachable: boolean;
}

enum FwsPfdVars {
  memoLineX = 'L:A32NX_PFD_MEMO_LINE_#index#',
  limitationsLineX = 'L:A32NX_PFD_LIMITATIONS_LINE_#index#', // 1 to 8
}

type IndexedTopics = 'memo_line' | 'limitations_line';

type FwsPfdIndexedEvents = {
  [P in keyof Pick<FwsPfdBaseSimvars, IndexedTopics> as IndexedTopic<P>]: FwsPfdBaseSimvars[P];
};

export interface FwsPfdSimvars extends FwsPfdBaseSimvars, FwsPfdIndexedEvents {}

export class FwsPfdSimvarPublisher extends VarPublisher<FwsPfdSimvars> {
  constructor(bus: Bus, pacer?: Pacer<FwsPfdSimvars>) {
    const simvars: [keyof FwsPfdSimvars, VarTopic<any>][] = [
      ['memo_line', { name: FwsPfdVars.memoLineX, type: VarUnit.Number, indexed: true }],
      ['limitations_line', { name: FwsPfdVars.limitationsLineX, type: VarUnit.Number, indexed: true }],
      ['fws1_is_healthy', { name: 'L:A32NX_FWS1_IS_HEALTHY', type: VarUnit.Bool }],
      ['fws2_is_healthy', { name: 'L:A32NX_FWS2_IS_HEALTHY', type: VarUnit.Bool }],
      ['afdx_3_1_reachable', { name: 'L:A32NX_AFDX_3_1_REACHABLE', type: VarUnit.Bool }],
      ['afdx_13_11_reachable', { name: 'L:A32NX_AFDX_13_11_REACHABLE', type: VarUnit.Bool }],
      ['afdx_4_2_reachable', { name: 'L:A32NX_AFDX_4_2_REACHABLE', type: VarUnit.Bool }],
      ['afdx_14_12_reachable', { name: 'L:A32NX_AFDX_14_12_REACHABLE', type: VarUnit.Bool }],
    ];

    super(new Map(simvars), bus, pacer);
  }
}
