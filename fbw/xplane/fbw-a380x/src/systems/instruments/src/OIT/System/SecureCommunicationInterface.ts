// Copyright (c) 2025 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import { ClockTopics, BusSignal, Bus, BoardDevice, Derived, Bond } from 'xplane-js/ui';
import { AdrBusEvents, Arinc429LocalVarConsumerSubject, FmsData, FwcBusEvents } from '@flybywiresim/fbw-sdk';
import { ResetPanelSimvars } from '../../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { FqmsBusEvents } from '@shared/publishers/FqmsBusPublisher';
import { OitSimvars } from '../OitSimvarPublisher';

export class SecureCommunicationInterface implements BoardDevice {
  private readonly subscriptions: Bond[] = [];

  private readonly fromAircraftSubscriptions: Bond[] = [];
  private readonly toAircraftSubscriptions: Bond[] = [];

  private readonly sub = this.bus.inbox<
    ResetPanelSimvars & OitSimvars & FmsData & AdrBusEvents & FwcBusEvents & ClockTopics & FqmsBusEvents
  >();

  private readonly nssDataToAvncsOff = BusSignal.from(this.sub.topic('nssDataToAvncsOff'), false);

  // Data from avionics
  public readonly zuluTime = BusSignal.from(this.sub.topic('simTime'), Date.now());

  public readonly fltNumber = BusSignal.from(this.sub.topic('fmsFlightNumber'), null);
  public readonly fltOrigin = BusSignal.from(this.sub.topic('fmsOrigin'), null);
  public readonly fltDestination = BusSignal.from(this.sub.topic('fmsDestination'), null);

  public readonly parkBrakeSet = BusSignal.from(this.sub.topic('parkBrakeSet'), false);

  public readonly airspeed = Arinc429LocalVarConsumerSubject.create(this.sub.topic('adr_computed_airspeed_1'));

  public readonly fwcDiscreteWord126 = Arinc429LocalVarConsumerSubject.create(
    this.sub.topic('a32nx_fwc_discrete_word_126_1'),
  );

  public readonly onGround = this.fwcDiscreteWord126.derive((dw) => dw.bitValueOr(28, true));

  public readonly doorsOpenPercentageOver100 = BusSignal.from(this.sub.topic('cabinDoorOpen'), 0);

  public readonly fqmsFuelQuantity = Arinc429LocalVarConsumerSubject.create(this.sub.topic('fqms_total_fuel_on_board'));

  public readonly hydGreenPressurized = BusSignal.from(this.sub.topic('hydGreenPressurized'), false);
  public readonly hydYellowPressurized = BusSignal.from(this.sub.topic('hydYellowPressurized'), false);
  public readonly hydraulicsPressurized = Derived.from(
    ([green, yellow]) => green || yellow,
    this.hydGreenPressurized,
    this.hydYellowPressurized,
  );

  constructor(private readonly bus: Bus) {}

  /** @inheritdoc */
  setup(): void {
    this.subscriptions.push(this.nssDataToAvncsOff, this.zuluTime);

    this.fromAircraftSubscriptions.push(
      this.fltNumber,
      this.fltOrigin,
      this.fltDestination,
      this.parkBrakeSet,
      this.airspeed,
      this.fwcDiscreteWord126,
      this.onGround,
      this.doorsOpenPercentageOver100,
      this.fqmsFuelQuantity,
      this.hydGreenPressurized,
      this.hydYellowPressurized,
      this.hydraulicsPressurized,
    );

    this.nssDataToAvncsOff.watch((v) => this.toAircraftSubscriptions.forEach((s) => (v ? s.hold() : s.release())));
  }

  /** @inheritdoc */
  tick(): void {}

  destroy() {
    for (const s of this.subscriptions) {
      s.destroy();
    }

    for (const s of this.fromAircraftSubscriptions) {
      s.destroy();
    }

    for (const s of this.toAircraftSubscriptions) {
      s.destroy();
    }
  }
}
