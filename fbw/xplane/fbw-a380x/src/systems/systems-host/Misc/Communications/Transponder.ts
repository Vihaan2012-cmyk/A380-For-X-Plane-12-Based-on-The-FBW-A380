import {
  BusSignal,
  Bus,
  SessionState,
  BoardDevice,
  VarUnit,
  Signal,
  Readable,
  Waits,
} from 'xplane-js/ui';
import { MfdSurvEvents } from '../../../instruments/src/MsfsAvionicsCommon/providers/MfdSurvPublisher';
import { FailuresConsumer } from '@flybywiresim/fbw-sdk';
import { A380Failure } from '@failures';
import { SessionPhase, Vars } from 'xplane-js/data';

// FIXME implement the rest of the transponder and it's ARINC bus interface

enum MsfsTransponderState {
  Off = 0,
  Standby = 1,
  Test = 2,
  /** Mode A only */
  On = 3,
  /** Mode C */
  Alt = 4,
  /** Mode S */
  Ground = 5,
}

/** The transponder contained within the AESS and interfacing with MSFS. */
export class Transponder implements BoardDevice {
  private readonly sub = this.bus.inbox<MfdSurvEvents>();

  private readonly isFailed = Signal.of(false);
  private readonly failureKey = this.index === 2 ? A380Failure.Transponder2 : A380Failure.Transponder1;

  private readonly msfsCircuitVar = `fbw/CIRCUIT_SWITCH_ON_${this.msfsCircuit}`;

  private readonly isAuto = BusSignal.from(this.sub.topic('mfd_xpdr_set_auto'), false);
  private readonly isAltReportingOn = BusSignal.from(this.sub.topic('mfd_xpdr_set_alt_reporting'), true);

  private readonly msfsTransponderState = Signal.of(MsfsTransponderState.Off);
  private readonly msfsTransponderStateVar = `fbw/TRANSPONDER_STATE_${this.index}`;

  /**
   * Ctor.
   * @param index The transponder index (1-based).
   * @param msfsCircuit The MSFS circuit index for this transceiver (used to switch the radio off when it should not transmit/receive).
   * @param isPowered Whether the transceiver is powered.
   * @param failuresConsumer The failures consumer.
   */
  constructor(
    private readonly bus: Bus,
    private readonly index: 1 | 2,
    private readonly msfsCircuit: number,
    private readonly isPowered: Readable<boolean>,
    private readonly failuresConsumer: FailuresConsumer,
  ) {}

  /** @inheritdoc */
  setup(): void {
    this.failuresConsumer.register(this.failureKey);

    this.msfsTransponderState.watch(
      (v) => Vars.write(this.msfsTransponderStateVar, VarUnit.Enum, v),
      true,
    );

    Waits.forValue(SessionState.get(), (v) => v === SessionPhase.flying).then(() => {
      // set initial squawk code as the cfg file param doesn't seem to work
      Vars.write('K:XPNDR_SET', 'number', 0x2000);
      if (!Vars.read('L:A32NX_COLD_AND_DARK_SPAWN', 'Bool')) {
        this.bus.outbox<MfdSurvEvents>().post('mfd_xpdr_set_auto', true, true);
      }
    });
  }

  /** @inheritdoc */
  tick(): void {
    this.isFailed.set(this.failuresConsumer.isActive(this.failureKey));
    const isMsfsTransponderOn = Vars.read(this.msfsCircuitVar, 'boolean') > 0;
    const shouldBeOn = this.isPowered.get() && !this.isFailed.get();

    if (isMsfsTransponderOn !== shouldBeOn) {
      Vars.write('K:ELECTRICAL_CIRCUIT_TOGGLE', 'number', this.msfsCircuit);
    }

    // FIXME better logic. Each AESS has a hardwired LGERS air/ground signal on the 380
    const isOnGround = Vars.read('L:A32NX_LGCIU_1_LEFT_GEAR_COMPRESSED', VarUnit.Bool);

    switch (true) {
      case !shouldBeOn:
        this.msfsTransponderState.set(MsfsTransponderState.Off);
        break;
      case !this.isAuto.get():
        this.msfsTransponderState.set(MsfsTransponderState.Standby);
        break;
      case !isOnGround && this.isAltReportingOn.get():
        this.msfsTransponderState.set(MsfsTransponderState.Alt);
        break;
      case !isOnGround:
        this.msfsTransponderState.set(MsfsTransponderState.On);
        break;
      default:
        this.msfsTransponderState.set(MsfsTransponderState.Ground);
        break;
    }
  }
}
