//  Copyright (c) 2024-2025 FlyByWire Simulations
//  SPDX-License-Identifier: GPL-3.0

import {
  UI,
  Bus,
  PanelEventPublisher,
  Board,
  HostedDevice,
  HostedInstrument,
  SimClockFeeder,
  Signal,
} from 'xplane-js/ui';
import { FmcService } from './FMC/FmcService';
import { FmcServiceInterface } from './FMC/FmcServiceInterface';
import { MfdComponent } from './MFD';
import { MfdSimvarPublisher } from './shared/MFDSimvarPublisher';
import { EnginePublisher, FailuresConsumer, RaBusPublisher } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { FGDataPublisher } from '../MsfsAvionicsCommon/providers/FGDataPublisher';
import { ResetPanelSimvarPublisher } from '../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { FmsMessagePublisher } from '../../../shared/src/publishers/FmsMessagePublisher';
import { FqmsBusPublisher } from '@shared/publishers/FqmsBusPublisher';
import { AtcDatalinkSystem } from './ATCCOM/AtcDatalinkSystem';
import { dataStatusUri } from './shared/utils';
import { FcuEfisCpBusPublisher } from '@shared/publishers/EfisCpBusPublisher';
import { PrimFctlBusPublisher } from '@shared/publishers/PrimFctlPublisher';
import { PrimFgBusPublisher } from '@shared/publishers/PrimFgPublisher';
import { PrimChoiceProvider } from '@shared/publishers/PrimChoiceProvider';
import { PanelInstrument, SessionPhase, SoundId, defineInstrument } from 'xplane-js/data';

class MfdInstrument implements HostedDevice {
  private readonly bus = new Bus();

  private readonly backplane = new Board();

  private readonly simVarPublisher = new MfdSimvarPublisher(this.bus);

  private readonly fgDataPublisher = new FGDataPublisher(this.bus);

  private readonly fmsMessagePublisher = new FmsMessagePublisher(this.bus);

  private readonly clockPublisher = new SimClockFeeder(this.bus);

  private readonly hEventPublisher = new PanelEventPublisher(this.bus);

  private readonly resetPanelPublisher = new ResetPanelSimvarPublisher(this.bus);

  private readonly radioAltimeterPublisher = new RaBusPublisher(this.bus);

  private readonly fqmsPublisher = new FqmsBusPublisher(this.bus);

  private readonly fcuEfisBusPublisher = new FcuEfisCpBusPublisher(this.bus);

  private readonly mfdCaptRef = UI.ref<MfdComponent>();

  private readonly mfdFoRef = UI.ref<MfdComponent>();

  private readonly fmcService: FmcServiceInterface;

  private readonly atcService = new AtcDatalinkSystem(this.bus);

  private readonly fmcAFailed = Signal.of(false);
  private readonly fmcBFailed = Signal.of(false);
  private readonly fmcCFailed = Signal.of(false);

  private readonly failuresConsumer = new FailuresConsumer();

  private readonly primFctlPublisher = new PrimFctlBusPublisher(this.bus);

  private readonly primFgPublisher = new PrimFgBusPublisher(this.bus);

  private readonly primChoiceProvider = new PrimChoiceProvider(this.bus);

  constructor(public readonly instrument: PanelInstrument) {
    this.backplane.attachFeeder('mfd', this.simVarPublisher);
    this.backplane.attachFeeder('hEvent', this.hEventPublisher);
    this.backplane.attachFeeder('clock', this.clockPublisher);
    this.backplane.attachFeeder('fg', this.fgDataPublisher);
    this.backplane.attachFeeder('fmsMessage', this.fmsMessagePublisher);
    this.backplane.attachFeeder('resetPanel', this.resetPanelPublisher);
    this.backplane.attachFeeder('radioAltimeter', this.radioAltimeterPublisher);
    this.backplane.attachFeeder('fqms', this.fqmsPublisher);
    this.backplane.attachFeeder('Engine', new EnginePublisher(this.bus));
    this.backplane.attachFeeder('fcu', this.fcuEfisBusPublisher);
    this.backplane.attachFeeder('primFg', this.primFgPublisher);
    this.backplane.attachFeeder('primFctl', this.primFctlPublisher);

    this.fmcService = new FmcService(
      this.bus,
      this.mfdCaptRef.peek(),
      this.fmcAFailed,
      this.fmcBFailed,
      this.fmcCFailed,
    );

    this.doInit();
  }

  public doInit(): void {
    this.backplane.setup();

    const mfd = document.getElementById('MFD_CONTENT');
    if (mfd) {
      mfd.style.display = 'flex';
      mfd.style.flexDirection = 'row';
      mfd.style.height = '1024';
    }

    UI.mount(
      <div id="MFD_LEFT_PARENT_DIV" style="width: 768px; position: relative; margin-right: 110px;" />,
      document.getElementById('MFD_CONTENT'),
    );
    UI.mount(
      <div id="MFD_RIGHT_PARENT_DIV" style="width: 768px; position: relative;" />,
      document.getElementById('MFD_CONTENT'),
    );
    UI.mount(
      <MfdComponent
        captOrFo="CAPT"
        ref={this.mfdCaptRef}
        bus={this.bus}
        fmcService={this.fmcService}
        atcService={this.atcService}
      />,
      document.getElementById('MFD_LEFT_PARENT_DIV'),
    );
    UI.mount(
      <MfdComponent
        captOrFo="FO"
        ref={this.mfdFoRef}
        bus={this.bus}
        fmcService={this.fmcService}
        atcService={this.atcService}
      />,
      document.getElementById('MFD_RIGHT_PARENT_DIV'),
    );

    // Update MFD reference for deduplication etc.
    if (this.fmcService.master) {
      this.fmcService.master.mfdReference = this.mfdCaptRef.target;
    }

    // Navigate to initial page
    this.mfdCaptRef.target.uiService.navigateTo(dataStatusUri);
    this.mfdFoRef.target.uiService.navigateTo(dataStatusUri);

    // Remove "instrument didn't load" text
    mfd?.querySelector(':scope > h1')?.remove();

    this.failuresConsumer.register(A380Failure.FmcA);
    this.failuresConsumer.register(A380Failure.FmcB);
    this.failuresConsumer.register(A380Failure.FmcC);
    this.primChoiceProvider.init();
  }

  /**
   * A callback called when the instrument gets a frame update.
   */
  public onFrame(): void {
    this.backplane.tick();
    this.failuresConsumer.update();
    this.fmcAFailed.set(this.failuresConsumer.isActive(A380Failure.FmcA));
    this.fmcBFailed.set(this.failuresConsumer.isActive(A380Failure.FmcB));
    this.fmcCFailed.set(this.failuresConsumer.isActive(A380Failure.FmcC));
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

class A380X_MFD extends HostedInstrument<MfdInstrument> {
  public createDevice(): MfdInstrument {
    return new MfdInstrument(this);
  }

  public get interactive(): boolean {
    return true;
  }

  public get templateId(): string {
    return 'A380X_MFD';
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

defineInstrument('a380x-mfd', A380X_MFD);
