// @ts-strict-ignore
/* eslint-disable no-case-declarations */
// Copyright (c) 2024 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0

import {
  ClockTopics,
  Bus,
  SessionState,
  BoardDevice,
  KeyControl,
  VarUnit,
  Waits,
} from 'xplane-js/ui';
import { FmgcFlightPhase } from '@shared/flightphase';
import { SessionPhase, Vars } from 'xplane-js/data';

enum TimeOfDayState {
  Day = 1,
  DuskDawn = 2,
  Night = 3,
}

enum FwcFlightPhase {
  ElecPwr = 1,
  FirstEngineStarted = 2,
  SecondEngineTakeOffPower = 3,
  AtOrAboveEightyKnots = 4,
  AtOrAboveV1 = 5,
  LiftOff = 6,
  AtOrAbove400Feet = 7,
  AtOrAbove1500FeetTo800Feet = 8,
  AtOrBelow800Feet = 9,
  TouchDown = 10,
  AtOrBelowEightyKnots = 11,
  EnginesShutdown = 12,
}
export class LightSync implements BoardDevice {
  private keyInterceptManager: KeyControl;

  private readonly bus: Bus;

  constructor(bus: Bus) {
    this.bus = bus;
  }

  public setup(): void {
    Promise.all([
      KeyControl.connect(this.bus),
      Waits.forValue(SessionState.get(), (state) => state === SessionPhase.flying, true),
    ]).then(([keyEventManager]) => {
      this.keyInterceptManager = keyEventManager;
      this.initLighting();
      this.bus
        .inbox<ClockTopics>()
        .topic('realTime')
        .after(1500)
        .listen((_now) => {
          this.updateLighting();
        });
    });
  }

  public initLighting(): void {
    console.log('[LightSync] initializing lighting to defaults');

    const msfsCabinCircuitVar = Vars.read('A:fbw/CIRCUIT_SWITCH_ON_151', 'bool');
    if (!msfsCabinCircuitVar) {
      Vars.write('K:ELECTRICAL_CIRCUIT_TOGGLE', 'number', 151);
    }

    const autoBrightness = this.getAutoBrightness();

    // OVHD Reading Lights
    this.setPotentiometer(96, 0); // Capt
    this.setPotentiometer(97, 0); // F/O

    // Glareshield
    this.setPotentiometer(84, autoBrightness < 50 ? 1.5 * autoBrightness : 0); // Int Lt
    this.setPotentiometer(87, autoBrightness); // Lcd Brt
    this.setPotentiometer(10, 0); // table Cpt
    this.setPotentiometer(11, 0); // table F/O

    // Instruments Cpt
    this.setPotentiometer(88, autoBrightness); // PFD
    this.setPotentiometer(89, autoBrightness); // ND
    this.setPotentiometer(94, autoBrightness / 2); // wxRadar
    this.setPotentiometer(98, autoBrightness); // MFD
    this.setPotentiometer(78, autoBrightness); // OIT
    this.setPotentiometer(71, autoBrightness); // HUD

    // Instruments F/O
    this.setPotentiometer(90, autoBrightness); // PFD
    this.setPotentiometer(91, autoBrightness); // ND
    this.setPotentiometer(95, autoBrightness / 2); // wxRadar
    this.setPotentiometer(99, autoBrightness); // MFD
    this.setPotentiometer(79, autoBrightness); // OIT
    this.setPotentiometer(72, autoBrightness); // HUD

    // Pedestal

    this.setRmpBrightness(1, autoBrightness * 100); // rmpCptLightLevel
    this.setRmpBrightness(2, autoBrightness * 100); // rmpFoLightLevel
    this.setRmpBrightness(3, autoBrightness * 100); // rmpOvhdLightLevel
    this.setPotentiometer(92, autoBrightness); // ecamUpperLightLevel
    this.setPotentiometer(93, autoBrightness); // ecamLowerLightLevel
    this.setPotentiometer(76, autoBrightness); // pedFloodLightLevel
    // this.setPotentiometer(83, autoBrightness); // mainPnlFloodLightLevel
    Vars.write('L:A380X_PED_LIGHTING_MIP_FLOOD_LT_KNOB', VarUnit.Number, autoBrightness / 100);
    this.setPotentiometer(85, autoBrightness); // integralLightLevel
    // this.setPotentiometer(7, autoBrightness); // ambianceLightLevel
    Vars.write('L:A380X_PED_LIGHTING_AMBIENT_LT_KNOB', VarUnit.Number, autoBrightness / 100);
  }

  public tick(): void {}

  public updateLighting(): void {
    // Follow EFB Setting for Cabin Auto Brightness
    if (Vars.read('L:A32NX_CABIN_USING_AUTOBRIGHTNESS', 'bool')) {
      const timeOfDay = Vars.read('E:fbw/TIME_OF_DAY', 'Enum');
      const fmgcFlightPhase = Vars.read('L:A32NX_FMGC_FLIGHT_PHASE', 'Enum');

      switch (timeOfDay) {
        case TimeOfDayState.Day:
          this.setPotentiometer(60, 0);
          Vars.write('L:A32NX_CABIN_AUTOBRIGHTNESS', 'number', 0);
          break;
        case TimeOfDayState.DuskDawn:
        case TimeOfDayState.Night:
          switch (fmgcFlightPhase) {
            case FmgcFlightPhase.Preflight:
              this.setPotentiometer(
                60,
                Vars.read('L:A32NX_FWC_FLIGHT_PHASE', 'Enum') >= FwcFlightPhase.FirstEngineStarted ||
                  Vars.read('A:fbw/ON_ANY_RUNWAY', 'bool')
                  ? 1
                  : 85,
                true,
              );
              break;
            case FmgcFlightPhase.Takeoff:
            case FmgcFlightPhase.Approach:
            case FmgcFlightPhase.GoAround:
              this.setPotentiometer(60, 1, true);
              break;
            case FmgcFlightPhase.Climb:
            case FmgcFlightPhase.Cruise:
              this.setPotentiometer(60, 0, true);
              break;
            case FmgcFlightPhase.Descent:
              this.setPotentiometer(
                60,
                Math.round(Vars.read('A:sim/flightmodel/position/elevation', 'feet') / 100) * 100 >=
                  Vars.read('L:A32NX_AIRLINER_CRUISE_ALTITUDE', 'number') - 5000
                  ? 5
                  : 0,
                true,
              );
              break;
            case FmgcFlightPhase.Done:
              this.setPotentiometer(60, 85, true);
              break;
          }
          break;
      }
    } else {
      const manualBrightness = Vars.read('L:A32NX_CABIN_MANUAL_BRIGHTNESS', 'number');
      this.setPotentiometer(60, manualBrightness, false);
    }
  }

  private setPotentiometer(potentiometer: number, brightness: number, autoBrightness: boolean = false) {
    if (this.keyInterceptManager) {
      this.keyInterceptManager.fire('LIGHT_POTENTIOMETER_SET', false, potentiometer, brightness);
    }
    if (autoBrightness) {
      Vars.write('L:A32NX_CABIN_AUTOBRIGHTNESS', 'number', brightness);
    }
  }

  /**
   * Sets the RMP brightness knobs.
   * @param rmp The RMP to set.
   * @param brightness The brighness value in percent [0, 100].
   */
  private setRmpBrightness(rmp: 1 | 2 | 3, brightness: number): void {
    // FIXME should use the B events, but not supported in FS2020.
    Vars.write(`L:A380X_RMP_#RMP_ID#_BRIGHTNESS_KNOB`, VarUnit.Number, brightness);
  }

  /**
   * Gets the automatic brightness level from the sim.
   * @returns Brightness in the range [0, 1].
   */
  private getAutoBrightness(): number {
    /** automatic brightness based on ambient light, [0, 1] scale */
    return Math.max(15, Math.min(85, Vars.read('fbw/GLASSCOCKPIT_AUTOMATIC_BRIGHTNESS', 'percent')));
  }
}
