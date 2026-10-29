import { Bus, VarPublisher, VarUnit } from 'xplane-js/ui';

export interface FMBusEvents {
  'fm.1.healthy_discrete': boolean;

  'fm.2.healthy_discrete': boolean;

  'fm.1.tuning_discrete_word': number;

  'fm.2.tuning_discrete_word': number;
}

export class FMBusPublisher extends VarPublisher<FMBusEvents> {
  constructor(bus: Bus) {
    super(
      new Map([
        ['fm.1.healthy_discrete', { name: 'L:A32NX_FM1_HEALTHY_DISCRETE', type: VarUnit.Bool }],
        ['fm.2.healthy_discrete', { name: 'L:A32NX_FM2_HEALTHY_DISCRETE', type: VarUnit.Bool }],
        ['fm.1.tuning_discrete_word', { name: 'L:A32NX_FM1_NAV_DISCRETE', type: VarUnit.Number }],
        ['fm.2.tuning_discrete_word', { name: 'L:A32NX_FM2_NAV_DISCRETE', type: VarUnit.Number }],
      ]),
      bus,
    );
  }
}
