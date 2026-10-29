import {
  UI,
  PanelEventPublisher,
  Board,
  HostedDevice,
  HostedInstrument,
  SimClockFeeder,
} from 'xplane-js/ui';
import { ArincEventBus, FailuresConsumer, FmsDataPublisher } from '@flybywiresim/fbw-sdk';
import { SD } from './SD';
import { SDSimvarPublisher } from './SDSimvarPublisher';
import { AdirsValueProvider } from '../MsfsAvionicsCommon/AdirsValueProvider';
import { FcuEfisCpBusPublisher } from '@shared/publishers/EfisCpBusPublisher';
import { FqmsBusPublisher } from '@shared/publishers/FqmsBusPublisher';
import { PanelInstrument, SessionPhase, SoundId, defineInstrument } from 'xplane-js/data';

class SdInstrument implements HostedDevice {
  private readonly bus = new ArincEventBus();

  private readonly backplane = new Board();

  private readonly clockPublisher = new SimClockFeeder(this.bus);

  private readonly hEventPublisher = new PanelEventPublisher(this.bus);

  private readonly simVarPublisher = new SDSimvarPublisher(this.bus);

  private readonly adirsValueProvider = new AdirsValueProvider(this.bus, this.simVarPublisher, 'L');

  private readonly fmsDataPublisher = new FmsDataPublisher(this.bus);

  private readonly fcuBusPublisher = new FcuEfisCpBusPublisher(this.bus);

  private readonly fqmsPublisher = new FqmsBusPublisher(this.bus);

  private readonly failuresConsumer = new FailuresConsumer();

  constructor(public readonly instrument: PanelInstrument) {
    this.hEventPublisher = new PanelEventPublisher(this.bus);

    this.backplane.attachFeeder('hEvent', this.hEventPublisher);
    this.backplane.attachFeeder('clock', this.clockPublisher);
    this.backplane.attachFeeder('sdSimVars', this.simVarPublisher);
    this.backplane.attachFeeder('fmsData', this.fmsDataPublisher);
    this.backplane.attachFeeder('fcuBus', this.fcuBusPublisher);
    this.backplane.attachFeeder('fqms', this.fqmsPublisher);

    this.doInit();
  }

  public doInit(): void {
    this.backplane.setup();

    this.adirsValueProvider.start();

    const sdv2 = document.getElementById('SDv2_CONTENT');

    UI.mount(<SD bus={this.bus} />, document.getElementById('SDv2_CONTENT'));

    // Remove "instrument didn't load" text
    sdv2?.querySelector(':scope > h1')?.remove();
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

class A380X_SDv2 extends HostedInstrument<SdInstrument> {
  public createDevice(): SdInstrument {
    return new SdInstrument(this);
  }

  public get interactive(): boolean {
    return true;
  }

  public get templateId(): string {
    return 'A380X_SDv2';
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

defineInstrument('a380x-sdv2', A380X_SDv2);
