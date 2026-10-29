// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  SimClockFeeder,
  HostedInstrument,
  UI,
  HostedDevice,
  PanelEventPublisher,
  Board,
  Signal,
} from 'xplane-js/ui';

import { ArincEventBus, VhfComIndices } from '@flybywiresim/fbw-sdk';
import { RmpState, RmpStateControllerEvents, VhfComManager } from '@flybywiresim/rmp';

import { RmpMessages } from './Components/RmpMessages';
import { RmpPageStack } from './Components/RmpPageStack';
import { Flasher } from './Systems/Flasher';
import { A380xRmpStateController } from './Systems/RmpStateController';
import { KeypadController } from './Systems/KeypadController';
import { RmpMessageManager } from './Systems/RmpMessageManager';
import { AudioControlManager } from './Systems/AudioControlManager';
import { AudioControlLocalVarPublisher } from './Data/AudioControlPublisher';
import { VhfRadioPublisher } from './Data/VhfRadioPublisher';
import { TransponderPublisher } from './Data/TransponderPublisher';
import { TransmitMessageMonitor } from './Systems/TransmitMessageMonitor';

import './style.scss';
import { PanelInstrument, SessionPhase, SoundId, defineInstrument } from 'xplane-js/data';

class RmpInstrument implements HostedDevice {
  private readonly bus = new ArincEventBus();

  private readonly backplane = new Board();

  private readonly clockPublisher = new SimClockFeeder(this.bus);

  private readonly hEventPublisher = new PanelEventPublisher(this.bus);

  private readonly rmpIndex = this.instrument.screenIndex as 1 | 2 | 3;

  private readonly stateController = new A380xRmpStateController(this.bus, this.rmpIndex);

  private readonly vhfControllers = [
    new VhfComManager(this.bus, VhfComIndices.Vhf1, this.rmpIndex),
    new VhfComManager(this.bus, VhfComIndices.Vhf2, this.rmpIndex),
    new VhfComManager(this.bus, VhfComIndices.Vhf3, this.rmpIndex),
  ];

  private readonly flasher = new Flasher(this.bus);

  private readonly messageManager = new RmpMessageManager(this.bus, this.rmpIndex);

  private readonly keypadController = new KeypadController(this.bus, this.rmpIndex);

  private readonly audioControlManager = new AudioControlManager(this.bus, this.rmpIndex);

  private readonly audioControlPublisher = new AudioControlLocalVarPublisher(this.bus, this.rmpIndex);

  private readonly vhfRadioPublisher = new VhfRadioPublisher(this.bus);

  private readonly transponderPublisher = new TransponderPublisher(this.bus);

  private readonly transmitMessageMonitor = new TransmitMessageMonitor(this.bus);

  private readonly screenOff = Signal.of(true);

  constructor(public readonly instrument: PanelInstrument) {
    if (
      this.instrument.screenIndex !== 1 &&
      this.instrument.screenIndex !== 2 &&
      this.instrument.screenIndex !== 3
    ) {
      throw new Error('Invalid RMP index (panel.cfg)!');
    }

    this.backplane.attachFeeder('ClockPublisher', this.clockPublisher);
    this.backplane.attachFeeder('HEventPublisher', this.hEventPublisher);

    this.backplane.attachDevice('RmpStateController', this.stateController);
    this.backplane.attachDevice('AudioControlManager', this.audioControlManager);
    this.backplane.attachFeeder('AudioControlPublisher', this.audioControlPublisher);
    this.backplane.attachFeeder('VhfRadioPublisher', this.vhfRadioPublisher);
    this.backplane.attachFeeder('TransponderPublisher', this.transponderPublisher);
    this.backplane.attachDevice('TransmitMessageMonitor', this.transmitMessageMonitor);

    for (const vhf of this.vhfControllers) {
      this.backplane.attachDevice(`VhfController${vhf.index}`, vhf, true);
    }

    this.doInit();
  }

  private doInit(): void {
    this.backplane.setup();

    UI.mount(
      <div
        class={{
          'rmp-content': true,
          hidden: this.screenOff,
        }}
      >
        <RmpPageStack bus={this.bus} />
        <RmpMessages bus={this.bus} />
        {/* <RefImages /> */}
      </div>,
      document.getElementById('RMP_CONTENT'),
    );

    // Remove "instrument didn't load" text
    document.getElementById('RMP_CONTENT')!.querySelector(':scope > h1')!.remove();

    this.bus
      .inbox<RmpStateControllerEvents>()
      .topic('rmp_state')
      .listen((v) => this.screenOff.set(v !== RmpState.On));
  }

  /**
   * A callback called when the instrument gets a frame update.
   */
  public onFrame(): void {
    this.backplane.tick();
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

class A380X_RMP extends HostedInstrument<RmpInstrument> {
  public createDevice(): RmpInstrument {
    return new RmpInstrument(this);
  }

  public get interactive(): boolean {
    // FIXME set to false when finished with ref images
    return true;
  }

  public get templateId(): string {
    return 'A380X_RMP';
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

defineInstrument('a380x-rmp', A380X_RMP);
