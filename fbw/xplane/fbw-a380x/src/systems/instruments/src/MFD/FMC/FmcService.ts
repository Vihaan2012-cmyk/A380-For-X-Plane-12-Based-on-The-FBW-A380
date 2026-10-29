import { FmsDisplayInterface } from '@fmgc/flightplanning/interface/FmsDisplayInterface';
import {
  BusSignal,
  Bus,
  Derived,
  VarUnit,
  Signal,
  Readable,
  Bond,
} from 'xplane-js/ui';
import { FlightManagementComputer } from './FlightManagementComputer';
import { FmcInterface, FmcOperatingModes } from './FmcInterface';
import { FmcIndex, FmcServiceInterface } from './FmcServiceInterface';
import { MfdDisplayInterface } from '../MFD';
import { MfdSimvars } from '../shared/MFDSimvarPublisher';
import { ResetPanelSimvars } from '../../MsfsAvionicsCommon/providers/ResetPanelPublisher';
import { DummyFlightManagementComputer } from './DummyFlightManagementComputer';
import { Vars } from 'xplane-js/data';

/*
 * Handles navigation (and potentially other aspects) for MFD pages
 */
export class FmcService implements FmcServiceInterface {
  protected subs = [] as Bond[];

  private readonly sub = this.bus.inbox<MfdSimvars & ResetPanelSimvars>();

  private readonly dcEssBusPowered = BusSignal.from(this.sub.topic('dcBusEss'), false);
  private readonly fmcAReset = BusSignal.from(this.sub.topic('a380x_reset_panel_fmc_a'), false);
  private readonly fmcAInop = Derived.from(
    ([powered, reset, fail]) => !powered || reset || fail,
    this.dcEssBusPowered,
    this.fmcAReset,
    this.fmcAFailed,
  );

  private readonly dc1BusPowered = BusSignal.from(this.sub.topic('dcBus1'), false);
  private readonly fmcBReset = BusSignal.from(this.sub.topic('a380x_reset_panel_fmc_b'), false);
  private readonly fmcBInop = Derived.from(
    ([powered, reset, fail]) => !powered || reset || fail,
    this.dc1BusPowered,
    this.fmcBReset,
    this.fmcBFailed,
  );

  private readonly dc2BusPowered = BusSignal.from(this.sub.topic('dcBus2'), false);
  private readonly fmcCReset = BusSignal.from(this.sub.topic('a380x_reset_panel_fmc_c'), false);
  private readonly fmcCInop = Derived.from(
    ([powered, reset, fail]) => !powered || reset || fail,
    this.dc2BusPowered,
    this.fmcCReset,
    this.fmcCFailed,
  );
  private readonly fmsDataKnob = BusSignal.from(this.sub.topic('fmsDataKnob'), 1);

  private readonly fmsCaptSideFailed = Derived.from(
    ([knob, fmcAFailed, fmcBFailed, fmcCFailed]) => (knob === 0 ? fmcBFailed && fmcCFailed : fmcAFailed && fmcCFailed),
    this.fmsDataKnob,
    this.fmcAInop,
    this.fmcBInop,
    this.fmcCInop,
  );
  private readonly fmsFoSideFailed = Derived.from(
    ([knob, fmcAFailed, fmcBFailed, fmcCFailed]) => (knob === 2 ? fmcAFailed && fmcCFailed : fmcBFailed && fmcCFailed),
    this.fmsDataKnob,
    this.fmcAInop,
    this.fmcBInop,
    this.fmcCInop,
  );

  constructor(
    private readonly bus: Bus,
    private readonly mfdReference: (FmsDisplayInterface & MfdDisplayInterface) | null,
    private readonly fmcAFailed: Readable<boolean>,
    private readonly fmcBFailed: Readable<boolean>,
    private readonly fmcCFailed: Readable<boolean>,
  ) {
    this.fmsCaptSideFailed.watch((f) => Vars.write('L:A32NX_FMS_L_FAILED', VarUnit.Bool, f));
    this.fmsFoSideFailed.watch((f) => Vars.write('L:A32NX_FMS_R_FAILED', VarUnit.Bool, f));
  }

  public readonly master: FmcInterface = new FlightManagementComputer(
    FmcIndex.FmcA,
    FmcOperatingModes.Master,
    this.bus,
    this.fmcAInop,
    this.mfdReference,
  );

  public readonly slave: FmcInterface | null = null;

  public readonly standby: FmcInterface | null = null;

  protected fmc: FmcInterface[] = [this.master];
  protected dummyFmc: DummyFlightManagementComputer[] = [
    new DummyFlightManagementComputer(FmcIndex.FmcB, this.fmcBInop),
    new DummyFlightManagementComputer(FmcIndex.FmcC, this.fmcCInop),
  ];

  // FIXME as soon as we have multiple real FMCs, we need to properly implement master/slave/standby logic and not just return the first FMC for master and null for the others
  // Example for get master(): return this.fmc.find((it) => it.operatingMode === FmcOperatingModes.Master) ?? null;

  has(forFmcIndex: FmcIndex) {
    return this.fmc[forFmcIndex] !== undefined;
  }

  get(forFmcIndex: FmcIndex) {
    return this.fmc[forFmcIndex];
  }

  setMfdReference(forFmcIndex: FmcIndex, mfd: FmsDisplayInterface & MfdDisplayInterface) {
    if (this.fmc[forFmcIndex] === undefined) {
      return;
    }

    this.fmc[forFmcIndex].mfdReference = mfd;
  }

  public readonly masterFmcChanged = Signal.of(false);
}
