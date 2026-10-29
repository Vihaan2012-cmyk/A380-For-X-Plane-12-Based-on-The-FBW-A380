import { Bus, BoardDevice, VarUnit } from 'xplane-js/ui';
import { getDisplayIndex } from '../HUD';
import { HUDSimvarPublisher, HUDSimvars } from './HUDSimvarPublisher';

export class AdirsValueProvider implements BoardDevice {
  constructor(
    private readonly bus: Bus,
    private readonly hudSimvar: HUDSimvarPublisher,
  ) {}

  /** @inheritdoc */
  public setup(): void {
    const sub = this.bus.inbox<HUDSimvars>();
    const displayIndex = getDisplayIndex();

    sub
      .topic('attHdgKnob')
      .distinct()
      .listen((k) => {
        const inertialSource = getSupplier(displayIndex, k);
        this.hudSimvar.updateSimVarSource('vsInert', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_VERTICAL_SPEED`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('pitch', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_PITCH`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('roll', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_ROLL`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('heading', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_HEADING`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('groundTrack', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_TRACK`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('fpaRaw', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_FLIGHT_PATH_ANGLE`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('daRaw', {
          name: `L:A32NX_ADIRS_IR_${inertialSource}_DRIFT_ANGLE`,
          type: VarUnit.Number,
        });
      });

    sub
      .topic('airKnob')
      .distinct()
      .listen((a) => {
        const airSource = getSupplier(displayIndex, a);
        this.hudSimvar.updateSimVarSource('speed', {
          name: `L:A32NX_ADIRS_ADR_${airSource}_COMPUTED_AIRSPEED`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('vsBaro', {
          name: `L:A32NX_ADIRS_ADR_${airSource}_BAROMETRIC_VERTICAL_SPEED`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('baroCorrectedAltitude1', {
          name: `L:A32NX_ADIRS_ADR_${airSource}_BARO_CORRECTED_ALTITUDE_1`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('baroCorrectedAltitude2', {
          name: `L:A32NX_ADIRS_ADR_${airSource}_BARO_CORRECTED_ALTITUDE_2`,
          type: VarUnit.Number,
        });
        this.hudSimvar.updateSimVarSource('mach', {
          name: `L:A32NX_ADIRS_ADR_${airSource}_MACH`,
          type: VarUnit.Number,
        });
      });
  }

  /** @inheritdoc */
  public tick(): void {
    // noop
  }
}

const isCaptainSide = (displayIndex: number | undefined) => displayIndex === 1;

const getSupplier = (displayIndex: number | undefined, knobValue: number) => {
  const adirs3ToCaptain = 0;
  const adirs3ToFO = 2;

  if (isCaptainSide(displayIndex)) {
    return knobValue === adirs3ToCaptain ? 3 : 1;
  }
  return knobValue === adirs3ToFO ? 3 : 2;
};
