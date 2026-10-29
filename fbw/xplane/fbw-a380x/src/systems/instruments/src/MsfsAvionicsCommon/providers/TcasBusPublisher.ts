import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';

export interface TcasSimVars {
  tcasTaOnly: boolean;
  tcasFault: boolean;
  tcasMode: number;
}

export class TcasBusPublisher extends VarPublisher<TcasSimVars> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['tcasTaOnly', { name: 'L:A32NX_TCAS_TA_ONLY', type: VarUnit.Bool }],
        ['tcasFault', { name: 'L:A32NX_TCAS_FAULT', type: VarUnit.Bool }],
        ['tcasMode', { name: 'L:A32NX_TCAS_MODE', type: VarUnit.Number }],
      ]),
      bus,
    );
  }
}
