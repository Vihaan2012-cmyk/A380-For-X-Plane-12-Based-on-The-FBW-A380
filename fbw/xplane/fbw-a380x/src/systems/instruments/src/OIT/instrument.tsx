//  Copyright (c) 2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  UI,
  Bus,
  PanelEventPublisher,
  Board,
  HostedDevice,
  HostedInstrument,
  SimClockFeeder,
  PanelEventTopics,
} from 'xplane-js/ui';
import { AdrBusPublisher, FwcBusPublisher, FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { OIT } from './OIT';
import { InternalKbdKeyEvent, OitSimvarPublisher } from './OitSimvarPublisher';
import { OisLaptop } from './OisLaptop';
import { AircraftNetworkServerUnit } from './System/AircraftNetworkServerUnit';
import { AnsuOps } from './System/AnsuOps';
import { ResetPanelSimvarPublisher } from '../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { FqmsBusPublisher } from '@shared/publishers/FqmsBusPublisher';
import { PanelInstrument, SessionPhase, SoundId, defineInstrument, openChannel } from 'xplane-js/data';

class OitInstrument implements HostedDevice {
  private readonly bus = new Bus();

  private readonly backplane = new Board();

  private readonly clockPublisher = new SimClockFeeder(this.bus);

  private readonly oitSimvarPublisher = new OitSimvarPublisher(this.bus);

  private readonly adrPublisher = new AdrBusPublisher(this.bus);

  private readonly fwcPublisher = new FwcBusPublisher(this.bus);

  private readonly fqmsPublisher = new FqmsBusPublisher(this.bus);

  private readonly resetPanelPublisher = new ResetPanelSimvarPublisher(this.bus);

  private readonly hEventPublisher = new PanelEventPublisher(this.bus);

  private readonly failuresConsumer = new FailuresConsumer();

  private readonly laptop = new OisLaptop(
    this.bus,
    this.instrument.screenIndex === 1 ? 1 : 2,
    this.failuresConsumer,
  );

  private readonly avncsAnsu = new AnsuOps(this.bus, 1, 'nss-avncs', this.failuresConsumer);
  private readonly fltOpsAnsu = new AircraftNetworkServerUnit(this.bus, 1, 'flt-ops', this.failuresConsumer);

  constructor(public readonly instrument: PanelInstrument) {
    this.hEventPublisher = new PanelEventPublisher(this.bus);

    this.backplane.attachFeeder('hEvent', this.hEventPublisher);
    this.backplane.attachFeeder('clock', this.clockPublisher);
    this.backplane.attachFeeder('oitSimvar', this.oitSimvarPublisher);
    this.backplane.attachFeeder('adr', this.adrPublisher);
    this.backplane.attachFeeder('fwc', this.fwcPublisher);
    this.backplane.attachFeeder('fqms', this.fqmsPublisher);
    this.backplane.attachFeeder('resetPanel', this.resetPanelPublisher);
    this.backplane.attachDevice('Laptop', this.laptop);
    this.backplane.attachDevice('nssAnsu', this.avncsAnsu, true);
    this.backplane.attachDevice('fltOpsAnsu', this.fltOpsAnsu, true);

    this.doInit();
  }

  public doInit(): void {
    openChannel('nav', () => console.log('JS_LISTENER_FACILITY registered.'), true);

    this.backplane.setup();

    const oit = document.getElementById('OIT_CONTENT');

    UI.mount(
      <OIT
        bus={this.bus}
        instrument={this.instrument}
        captOrFo={this.instrument.screenIndex === 1 ? 'CAPT' : 'FO'}
        failuresConsumer={this.failuresConsumer}
        laptop={this.laptop}
        avncsAnsu={this.avncsAnsu}
      />,
      document.getElementById('OIT_CONTENT'),
    );

    this.bus
      .inbox<PanelEventTopics>()
      .topic('hEvent')
      .listen((eventName) => {
        if (
          eventName.startsWith(
            this.instrument.screenIndex === 1 ? 'A380X_LAPTOP_KEYBOARD_L' : 'A380X_LAPTOP_KEYBOARD_R',
          )
        ) {
          const key = eventName.substring(23);

          this.bus
            .outbox<InternalKbdKeyEvent>()
            .post('kbdKeyEvent', [this.instrument.screenIndex === 1 ? 'L' : 'R', key]);
        }
      });

    // Remove "instrument didn't load" text
    oit?.querySelector(':scope > h1')?.remove();
  }

  /**
   * A callback called when the instrument gets a frame update.
   */
  public onFrame(): void {
    this.backplane.tick();
    this.failuresConsumer.update();
  }

  public onPanelEvent(args: string[]): void {
    this.hEventPublisher.dispatchPanelEvent(args[0]);
  }

  public onSessionChange(_oldState: SessionPhase, _newState: SessionPhase): void {
    // noop
  }

  public onFlightBegin(): void {
    // noop
  }

  public onSoundDone(_soundEventId: SoundId): void {
    // noop
  }

  public onPowered(): void {
    // noop
  }

  public onUnpowered(): void {
    // noop
  }
}

class A380X_OIT extends HostedInstrument<OitInstrument> {
  public createDevice(): OitInstrument {
    return new OitInstrument(this);
  }

  public get interactive(): boolean {
    return true;
  }

  public get templateId(): string {
    return 'A380X_OIT';
  }

  /** @inheritdoc */
  public onPowered(): void {
    super.onPowered();

    this.device.onPowered();
  }

  /** @inheritdoc */
  public onShutdown(): void {
    super.onShutdown();

    this.device.onUnpowered();
  }
}

defineInstrument('a380x-oit', A380X_OIT);
