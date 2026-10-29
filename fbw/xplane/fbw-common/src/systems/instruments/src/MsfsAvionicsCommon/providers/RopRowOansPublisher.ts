import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';

export interface RopRowOansSimVars {
  rowRopWord1Raw: number;
  oansWord1Raw: number;
}

export class RopRowOansPublisher extends VarPublisher<RopRowOansSimVars> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['rowRopWord1Raw', { name: 'L:A32NX_ROW_ROP_WORD_1', type: VarUnit.Enum }],
        ['oansWord1Raw', { name: 'L:A32NX_OANS_WORD_1', type: VarUnit.Enum }],
      ]),
      bus,
    );
  }
}
