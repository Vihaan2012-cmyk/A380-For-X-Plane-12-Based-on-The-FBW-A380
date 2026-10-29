// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ClockTopics,
  BusSignal,
  Bus,
  BoardDevice,
  VarUnit,
  Signal,
  Bond,
} from 'xplane-js/ui';
import { FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { ResetPanelSimvars } from '../../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { OitSimvars } from '../OitSimvarPublisher';
import { SecureCommunicationInterface } from './SecureCommunicationInterface';
import { Vars } from 'xplane-js/data';

type AnsuIndex = 1 | 2;

type AnsuType = 'nss-avncs' | 'flt-ops';

export class AircraftNetworkServerUnit implements BoardDevice {
  //TODO ANSU should be under systems host and split between flt-opts and nss-avncs?
  protected readonly subscriptions: Bond[] = [];

  protected readonly sub = this.bus.inbox<ResetPanelSimvars & OitSimvars & ClockTopics>();

  protected readonly failureKey =
    this.type === 'flt-ops' && this.index === 1
      ? A380Failure.FltOpsAnsu
      : this.index === 1
        ? A380Failure.NssAnsu1
        : A380Failure.NssAnsu2;

  protected readonly powered = Signal.of(false);

  protected readonly _isHealthy = Signal.of(false);
  protected readonly isHealthySimVar = `L:A32NX_${this.type === 'nss-avncs' ? 'NSS' : 'FLTOPS'}_ANSU_${this.index.toFixed(0)}_IS_HEALTHY`;

  protected readonly nssMasterOff = BusSignal.from(this.sub.topic('nssMasterOff'), false);

  protected readonly resetPbStatus = BusSignal.from(
    this.sub.topic(this.type === 'flt-ops' ? 'a380x_reset_panel_nss_flt_ops' : 'a380x_reset_panel_nss_avncs'),
    false,
  );

  public readonly sci = new SecureCommunicationInterface(this.bus);

  constructor(
    protected readonly bus: Bus,
    protected readonly index: AnsuIndex, // use only one ANSU index per type for now
    protected readonly type: AnsuType,
    protected readonly failuresConsumer: FailuresConsumer,
  ) {}

  /** @inheritdoc */
  setup(): void {
    this.failuresConsumer.register(this.failureKey);

    this.subscriptions.push(
      this._isHealthy.watch((v) => Vars.write(this.isHealthySimVar, VarUnit.Bool, v), true),
    );
  }

  /** @inheritdoc */
  tick(): void {
    const failed = this.failuresConsumer.isActive(this.failureKey);

    if (this.type === 'nss-avncs') {
      if (this.index === 1) {
        this.powered.set(
          Vars.read('L:A32NX_ELEC_AC_2_BUS_IS_POWERED', VarUnit.Bool) ||
            Vars.read('L:A32NX_ELEC_DC_HOT_2_BUS_IS_POWERED', VarUnit.Bool),
        );
      } else {
        this.powered.set(
          Vars.read('L:A32NX_ELEC_AC_1_BUS_IS_POWERED', VarUnit.Bool) ||
            Vars.read('L:A32NX_ELEC_AC_EMER_BUS_IS_POWERED', VarUnit.Bool) ||
            Vars.read('L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED', VarUnit.Bool),
        );
      }
    } else if (this.type === 'flt-ops') {
      this.powered.set(
        Vars.read('L:A32NX_ELEC_AC_ESS_BUS_IS_POWERED', VarUnit.Bool) ||
          Vars.read('L:A32NX_ELEC_DC_HOT_1_BUS_IS_POWERED', VarUnit.Bool),
      );
    }

    this._isHealthy.set(!failed && this.powered.get() && !this.resetPbStatus.get() && !this.nssMasterOff.get());

    if (!this._isHealthy.get()) {
      this.reset();
      return;
    }
  }

  reset(): void {
    // Called when reset panel p/b is pulled out
  }

  destroy() {
    for (const s of this.subscriptions) {
      s.destroy();
    }
  }
}
