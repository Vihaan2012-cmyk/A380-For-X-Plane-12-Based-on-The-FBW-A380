import type { AircraftModel } from './types.ts';

type Reader = (name: string, unit: string) => any;

export function createPlaneState(read: Reader) {
  function altitude(): number {
    return read('fbw/INDICATED_ALTITUDE', 'feet');
  }

  function heightAboveGround(_forceRead?: boolean): number {
    return read('fbw/PLANE_ALT_ABOVE_GROUND_MINUS_CG', 'feet');
  }

  function outsideAirTemp(): number | null {
    return read('sim/weather/aircraft/temperature_ambient_deg_c', 'celsius');
  }

  function groundSpeed(): number | null {
    return read('fbw/GPS_GROUND_SPEED', 'knots');
  }

  function indicatedSpeed(): number {
    return read('sim/flightmodel/position/indicated_airspeed', 'knots');
  }

  function verticalSpeed(): number {
    return read('fbw/VERTICAL_SPEED', 'feet per minute');
  }

  function onGround(): boolean {
    return read('fbw/SIM_ON_GROUND', 'bool') !== 0;
  }

  function baroSetting(units: string = 'inches of mercury'): number {
    return read('fbw/KOHLSMAN_SETTING_HG_1', units);
  }

  function baroMode(_aircraft: AircraftModel): 'QFE' | 'QNH' | 'STD' | '' {
    return read('fbw/KOHLSMAN_SETTING_STD_1', 'bool') !== 0 ? 'STD' : 'QNH';
  }

  function baroUnits(): 'inches of mercury' | 'millibar' {
    return read('L:XMLVAR_Baro_Selector_HPA_1', 'bool') !== 0 ? 'millibar' : 'inches of mercury';
  }

  function apSpeedTarget(): number | null {
    return read('fbw/AUTOPILOT_AIRSPEED_HOLD_VAR', 'knots');
  }

  function apSpeedSelected(): boolean {
    return read('fbw/AUTOPILOT_SPEED_SLOT_INDEX', 'number') === 1;
  }

  function apSpeedManaged(): boolean {
    return read('fbw/AUTOPILOT_SPEED_SLOT_INDEX', 'number') === 2;
  }

  function apMachTarget(): number | null {
    return read('fbw/AUTOPILOT_MACH_HOLD_VAR', 'number');
  }

  function apMachMode(): boolean | null {
    return read('fbw/AUTOPILOT_MANAGED_SPEED_IN_MACH', 'bool') !== 0;
  }

  function apHeadingTarget(radians: boolean = false): number | null {
    return read('fbw/AUTOPILOT_HEADING_LOCK_DIR', radians ? 'radians' : 'degrees');
  }

  function apAltitudeTarget(units: string = 'feet'): number {
    return read('fbw/AUTOPILOT_ALTITUDE_LOCK_VAR_3', units);
  }

  function apAltitudeManaged(): boolean {
    return read('fbw/AUTOPILOT_ALTITUDE_SLOT_INDEX', 'number') === 2;
  }

  function greenDotSpeed(): number | null {
    return read('L:A32NX_SPEEDS_GD', 'number');
  }

  return {
    altitude,
    heightAboveGround,
    outsideAirTemp,
    apSpeedTarget,
    apSpeedManaged,
    apSpeedSelected,
    apAltitudeManaged,
    apAltitudeTarget,
    apMachTarget,
    apMachMode,
    apHeadingTarget,
    greenDotSpeed,
    groundSpeed,
    indicatedSpeed,
    onGround,
    baroMode,
    baroUnits,
    baroSetting,
    verticalSpeed,
  };
}
