// Copyright (c) 2021-2022 2026 FlyByWire Simulations
// Copyright (c) 2021-2022 Synaptic Simulations
//
// SPDX-License-Identifier: GPL-3.0

import { Writable, Readable } from 'xplane-js/ui';
import { FlightPlanWindEntry, WindVector } from '../../data/wind';

export interface FlightPlanPerformanceData {
  /**
   * V1 speed; Unit: Knots; Null if not set.
   */
  readonly v1: Writable<number | null>;

  /**
   * Vr speed; Unit: Knots; Null if not set.
   */
  readonly vr: Writable<number | null>;

  /**
   * V2 speed; Unit: Knots; Null if not set.
   */
  readonly v2: Writable<number | null>;

  /**
   * Transition altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly databaseTransitionAltitude: Writable<number | null>;

  /**
   * Transition level from database; Unit: flight level (i.e. hundreds of feets); Null if not set.
   */
  readonly databaseTransitionLevel: Writable<number | null>;

  /**
   * Pilot entered transition altitude; Unit: Feet; Null if not set.
   */
  readonly pilotTransitionAltitude: Writable<number | null>;

  /**
   * Pilot entered transition level; Unit: flight level (i.e. hundreds of feets); Null if not set.
   */
  readonly pilotTransitionLevel: Writable<number | null>;

  /**
   * Returns pilot entered altitude if set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get transitionAltitude(): Readable<number | null>;

  /**
   * Whether returned transition altitude is from nav database;
   */
  get transitionAltitudeIsFromDatabase(): Readable<boolean>;

  /**
   * Transition level; Unit: flight level (i.e. hundreds of feets); Null if not set.
   */
  get transitionLevel(): Readable<number | null>;

  /**
   * Whether returned transition level is from nav database;
   */
  get transitionLevelIsFromDatabase(): Readable<boolean>;

  /**
   * Cost index; Unit: No unit; Null if not set.
   */
  readonly costIndex: Writable<number | null>;

  /**
   * Cruise flight level; Unit: flight level (i.e. hundreds of feets); Null if not set.
   */
  readonly cruiseFlightLevel: Writable<number | null>;

  /**
   * Cruise temperature pilot entry; Unit: degrees C Null if not set.
   */
  readonly cruiseTemperaturePilotEntry: Writable<number | null>;

  /**
   * Cruise temperature ISA temperature; Unit: degrees C Null if not set.
   * A380 specific
   */
  readonly cruiseTemperatureIsaTemp?: Writable<number | null>;

  /**
   * Cruise temperature to display; Unit: degrees C Null if not set.
   */
  get cruiseTemperature(): Readable<number | null>;

  /**
   * Whether cruise temperature is pilot entered.
   */
  get isCruiseTemperaturePilotEntered(): Readable<boolean>;

  /**
   * Default ground temperature; Unit: degrees C; Null if not set. A32NX specific
   */
  readonly defaultGroundTemperature?: Writable<number | null>;

  /**
   * Pilot ground temperature; Unit: degrees C; Null if not set. A32NX specific
   */
  readonly pilotGroundTemperature?: Writable<number | null>;

  /**
   * Ground temperature. Default if no pilot entry, pilot entered otherwise; Unit: degrees C; Null if not set. A32NX specific
   */
  readonly groundTemperature?: Readable<number | null>;

  /**
   * Whether ground temperature is pilot entered. A32NX specific

   */
  readonly groundTemperatureIsPilotEntered?: Readable<boolean>;

  /**
   * Pilot entered tropopause; Unit: Feet; Null if not set.
   */
  readonly pilotTropopause: Writable<number | null>;

  /**
   * Default tropopause; Unit: Feet; Null if not set.
   */
  readonly defaultTropopause: Writable<number | null>;

  /**
   * Tropopause. Default if no pilot entry, pilot entered otherwise; Unit: Feet; Null if not set.
   */
  get tropopause(): Readable<number | null>;

  /**
   * Whether tropopause is pilot entered.
   */
  get tropopauseIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered thrust reduction altitude; Unit: Feet; Null if not set.
   */
  readonly pilotThrustReductionAltitude: Writable<number | null>;

  /**
   * Thrust reduction altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultThrustReductionAltitude: Writable<number | null>;

  /**
   * Pilot entered thrust reduction altitude if set, from nav database otherwise; Unit: Feet; Null if not set.
   */
  get thrustReductionAltitude(): Readable<number | null>;

  /**
   * Whether thrust reduction altitude is pilot entered;
   */
  get thrustReductionAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered acceleration altitude; Unit: Feet; Null if not set.
   */
  readonly pilotAccelerationAltitude: Writable<number | null>;

  /**
   * Acceleration altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultAccelerationAltitude: Writable<number | null>;

  /**
   * Returns pilot entered acceleration altitude if set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get accelerationAltitude(): Readable<number | null>;

  /**
   * Whether acceleration altitude is pilot entered; Null if not set.
   */
  get accelerationAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered engine-out acceleration altitude; Unit: Feet; Null if not set.
   */
  readonly pilotEngineOutAccelerationAltitude: Writable<number | null>;

  /**
   * Engine-out acceleration altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultEngineOutAccelerationAltitude: Writable<number | null>;

  /**
   * Returns pilot entered engine-out acceleration altitude if set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get engineOutAccelerationAltitude(): Readable<number | null>;

  /**
   * Whether engine-out acceleration altitude is pilot entered; Null if not set.
   */
  get engineOutAccelerationAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered missed apch thrust reduction altitude; Unit: Feet; Null if not set.
   */
  readonly pilotMissedThrustReductionAltitude: Writable<number | null>;

  /**
   * Missed apch thrust reduction altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultMissedThrustReductionAltitude: Writable<number | null>;

  /**
   * Returns pilot entered missed apch thrust reduction altitude if set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get missedThrustReductionAltitude(): Readable<number | null>;

  /**
   * Whether missed apch thrust reduction altitude is pilot entered
   */
  get missedThrustReductionAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered missed apch acceleration altitude; Unit: Feet; Null if not set.
   */
  readonly pilotMissedAccelerationAltitude: Writable<number | null>;

  /**
   * Missed apch acceleration altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultMissedAccelerationAltitude: Writable<number | null>;

  /**
   * Returns pilot entered missed apch acceleration altitude of set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get missedAccelerationAltitude(): Readable<number | null>;

  /**
   * Whether missed apch acceleration altitude is pilot entered
   */
  get missedAccelerationAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * Pilot entered missed apch engine-out acceleration altitude; Unit: Feet; Null if not set.
   */
  readonly pilotMissedEngineOutAccelerationAltitude: Writable<number | null>;

  /**
   * Missed apch engine-out acceleration altitude from nav database; Unit: Feet; Null if not set.
   */
  readonly defaultMissedEngineOutAccelerationAltitude: Writable<number | null>;

  /**
   * Returns pilot entered missed apch engine-out acceleration altitude if set, nav database value otherwise; Unit: Feet; Null if not set.
   */
  get missedEngineOutAccelerationAltitude(): Readable<number | null>;

  /**
   * Whether missed apch engine-out acceleration altitude is pilot entered
   */
  get missedEngineOutAccelerationAltitudeIsPilotEntered(): Readable<boolean>;

  /**
   * The maximum speed imposed by the climb speed limit of the main flight plan or null if not set.
   */
  readonly climbSpeedLimitSpeed: Writable<number | null>;

  /**
   * The altitude below which the climb speed limit of the main flight plan applies or null if not set.
   */
  readonly climbSpeedLimitAltitude: Writable<number | null>;

  /**
   * Whether the climb speed limit is pilot entered.
   */
  readonly isClimbSpeedLimitPilotEntered: Writable<boolean>;

  /**
   * The maximum speed imposed by the descent speed limit of the main flight plan or null if not set.
   */
  readonly descentSpeedLimitSpeed: Writable<number | null>;

  /**
   * The altitude below which the descent speed limit of the main flight plan applies or null if not set.
   */
  readonly descentSpeedLimitAltitude: Writable<number | null>;

  /**
   * Whether the descent speed limit of the main flight plan is pilot entered.
   */
  readonly isDescentSpeedLimitPilotEntered: Writable<boolean>;

  /**
   * The maximum speed imposed by the climb speed limit of the alternate flight plan or null if not set.
   */
  readonly alternateClimbSpeedLimitSpeed: Writable<number | null>;

  /**
   * The altitude below which the climb speed limit of the alternate flight plan applies or null if not set.
   */
  readonly alternateClimbSpeedLimitAltitude: Writable<number | null>;

  /**
   * Whether the climb speed limit of the alternate flight plan is pilot entered.
   */
  readonly isAlternateClimbSpeedLimitPilotEntered: Writable<boolean>;

  /**
   * The maximum speed imposed by the descent speed limit of the alternate flight plan or null if not set.
   */
  readonly alternateDescentSpeedLimitSpeed: Writable<number | null>;

  /**
   * The altitude below which the descent speed limit of the alternate flight plan applies or null if not set.
   */
  readonly alternateDescentSpeedLimitAltitude: Writable<number | null>;

  /**
   * Whether the descent speed limit of the alternate flight plan is pilot entered.
   */
  readonly isAlternateDescentSpeedLimitPilotEntered: Writable<boolean>;

  /**
   * The zero fuel weight entered by the pilot in tonnes, or null if not set.
   */
  readonly zeroFuelWeight: Writable<number | null>;

  /**
   * The zero fuel weight center of gravity entered by the pilot as a percentage, or null if not set
   */
  readonly zeroFuelWeightCenterOfGravity: Writable<number | null>;

  /**
   * The block fuel entered by the pilot in tonnes, or null if not set.
   */
  readonly blockFuel: Writable<number | null>;

  /**
   * The taxi fuel entered by the pilot in tonnes, or null if not set.
   */
  readonly pilotTaxiFuel: Writable<number | null>;

  /**
   * The taxi fuel from the AMI database in tonnes
   */
  readonly defaultTaxiFuel: Writable<number>;

  /**
   * Returns the pilot entered taxi fuel if set, the AMI taxi fuel value otherwise; Unit: tonnes; Null if not set.
   */
  get taxiFuel(): Readable<number | null>;

  /**
   * Whether taxi fuel is pilot entered.
   */
  get taxiFuelIsPilotEntered(): Readable<boolean>;

  /**
   * The route reserve fuel entered by the pilot in tonnes, or null if not set.
   */
  readonly pilotRouteReserveFuel: Writable<number | null>;

  /**
   * Whether the route reserve fuel weight is pilot entered.
   */
  get isRouteReserveFuelPilotEntered(): Readable<boolean>;

  /**
   * The route reserve fuel percentage entered by the pilot as a percentage, or null if not set.
   */
  readonly pilotRouteReserveFuelPercentage: Writable<number | null>;

  /**
   * The route reserve fuel percentage from the AMI database
   */
  readonly defaultRouteReserveFuelPercentage: Writable<number>;

  /**
   * Returns the pilot entered route reserve fuel percentage if set, the AMI route reserve fuel percentage value otherwise
   */
  get routeReserveFuelPercentage(): Readable<number>;

  /**
   * Whether the route reserve fuel percentage is pilot entered.
   */
  get isRouteReserveFuelPercentagePilotEntered(): Readable<boolean>;

  /**
   * The alternate fuel entered by the pilot in tonnes, or null if not set.
   */
  readonly pilotAlternateFuel: Writable<number | null>;

  /**
   * The alternate fuel calculated by predictions in tonnes, or null if not set.
   */
  readonly calculatedAlternateFuel: Writable<number | null>;

  /**
   * Returns the pilot alternate fuel if set, the calculated value otherwise
   */
  get alternateFuel(): Readable<number | null>;

  /**
   * Whether alternate fuel is pilot entered.
   */
  get isAlternateFuelPilotEntered(): Readable<boolean>;

  /**
   * The final holding fuel entered by the pilot in tonnes, or null if not set.
   */
  readonly pilotFinalHoldingFuel: Writable<number | null>;

  /**
   * The final holding fuel calculated by predictions in tonnes, or null if not set.
   */
  readonly calculatedFinalHoldingFuel: Writable<number | null>;

  /**
   * Returns the pilot entered final holding fuel if set, the calculated value otherwise
   */
  get finalHoldingFuel(): Readable<number | null>;

  /**
   * Whether final holding fuel weight is pilot entered.
   */
  get isFinalHoldingFuelPilotEntered(): Readable<boolean>;

  /**
   * The final holding time entered by the pilot in minutes, or null if not set.
   */
  readonly pilotFinalHoldingTime: Writable<number | null>;

  /**
   * The final holding time from the AMI database in minutes
   */
  readonly defaultFinalHoldingTime: Writable<number>;

  /**
   * The final holding time calculated by predictions in minutes, or null if not set.
   */
  readonly calculatedFinalHoldingTime: Writable<number | null>;

  /**
   * Returns the pilot entered final holding time in minutes if set, the AMI final holding time value otherwise
   */
  get finalHoldingTime(): Readable<number>;

  /**
   * Whether final holding time is pilot entered.
   */
  get isFinalHoldingTimePilotEntered(): Readable<boolean>;

  /**
   * The minimum fuel on board at the destination entered by the pilot in tonnes, or null if not set.
   */
  readonly pilotMinimumDestinationFuelOnBoard: Writable<number | null>;

  /**
   *  The minimum fuel on board at the destination. Pilot entered if set, final fuel + alternate fuel otherwise. Null if none of these are defined; Unit: tonnes.
   */
  get minimumDestinationFuelOnBoard(): Readable<number | null>;

  /**
   * Whether minimum fuel on board at the destination is pilot entered.
   */
  get isMinimumDestinationFuelOnBoardPilotEntered(): Readable<boolean>;

  /**
   * The trip wind value entered by the pilot in kts, or null if not set.
   * Positive values indicate a tailwind, negative values indicate a headwind.
   */
  readonly pilotTripWind: Writable<number | null>;

  /**
   * The takeoff shift entered by the pilot in metres, or null if not set.
   */
  readonly takeoffShift: Writable<number | null>;

  /**
   * The takeoff flaps setting entered by the pilot, or null if not set.
   */
  readonly takeoffFlaps: Writable<0 | 1 | 2 | 3 | null>;

  /**
   * The THS setting entered by the pilot, or null if not set.
   * +ve for nose up, -ve for nose down
   */
  readonly trimmableHorizontalStabilizer: Writable<number | null>;

  /**
   * The flex takeoff temperature entered by the pilot in degrees, or null if not set.
   */
  readonly flexTakeoffTemperature: Writable<number | null>;

  /**
   * The preselected climb speed entered by the pilot in knots, or null if not set.
   */
  readonly preselectedClimbSpeed: Writable<number | null>;

  /**
   * The preselected cruise speed entered by the pilot either in kts or as a Mach number, or null if not set.
   */
  readonly preselectedCruiseSpeed: Writable<number | null>;

  /**
   * The managed descent speed entered by the pilot in knots, or null if not set.
   */
  readonly pilotManagedDescentSpeed: Writable<number | null>;

  /**
   * The managed descent Mach number entered by the pilot in knots, or null if not set.
   */
  readonly pilotManagedDescentMach: Writable<number | null>;

  /**
   * The QNH at the destination airport entered by the pilot in hPa or inHg, or null if not set.
   */
  readonly approachQnh: Writable<number | null>;

  /**
   * The temperature at the destination airport entered by the pilot in degrees, or null if not set.
   */
  readonly approachTemperature: Writable<number | null>;

  /**
   * The wind direction at the destination airport entered by the pilot in degrees magnetic, or null if not set.
   */
  readonly approachWindDirection: Writable<number | null>;

  /**
   * The wind magnitude at the destination airport entered by the pilot in knots, or null if not set.
   */
  readonly approachWindMagnitude: Writable<number | null>;

  /**
   * Whether the wind magnitude and direction were entered by the pilot, false if no entry has been made or if the wind
   * has been automatically transferred from a descent wind entry at ground level
   */
  readonly isApproachWindPilotEntered: Writable<boolean>;

  /**
   * The approach speed Vapp manually overridden by the pilot in knots, or null if not set.
   */
  readonly pilotVapp: Writable<number | null>;

  /**
   * The barometric minimum entered by the pilot, or null if not set.
   */
  readonly approachBaroMinimum: Writable<number | null>;

  /**
   * The radio minimum entered by the pilot, or null if not set.
   */
  readonly approachRadioMinimum: Writable<'NO DH' | number | null>;

  /**
   * Whether the flaps three setting is selected by the pilot for the approach
   */
  readonly approachFlapsThreeSelected: Writable<boolean>;

  /**
   * The wind entries for the climb segment entered by the pilot
   */
  readonly climbWindEntries: Writable<FlightPlanWindEntry[]>;

  /**
   * The wind entries for the descent segment entered by the pilot
   */
  readonly descentWindEntries: Writable<FlightPlanWindEntry[]>;

  /**
   * The average wind vector for the alternate flight plan, or null if not set.
   */
  readonly alternateWind: Writable<WindVector>;

  /** Estimated takeoff time timestamp, in unix epoch milliseconds */
  readonly estimatedTakeoffTime: Writable<number | null>;

  readonly estimatedTakeoffTimeExpired: Writable<boolean | null>;

  // ----------------------------------------------
  // A380 specific
  // ----------------------------------------------

  /**
   * The number of passengers on this flight, or null if not set.
   */
  readonly paxNumber?: Writable<number | null>;

  /**
   * The selected power settting for takeoff (TOGA, FLEX, DERATED), or null if not set.
   * A380 specific
   */
  readonly takeoffPowerSetting?: Writable<TakeoffPowerSetting>;

  /**
   * The selected derated power settting for takeoff, or null if not set.
   * A380 specific
   */
  readonly takeoffDeratedSetting?: Writable<TakeoffDerated>;

  /**
   * The GW CG for takeoff, or null if not set.
   * A380 specific
   */
  readonly takeoffThsFor?: Writable<number | null>;

  /**
   * The packs setting for takeoff, or null if not set.
   * A380 specific
   */
  readonly takeoffPacks?: Writable<TakeoffPacks | null>;

  /**
   * The anti ice setting for takeoff, or null if not set.
   * A380 specific
   */
  readonly takeoffAntiIce?: Writable<TakeoffAntiIce | null>;

  /**
   * The NADP activation status, or null if not set.
   * A380 specific
   */
  readonly noiseEnabled?: Writable<boolean>;

  /**
   * The NADP N1 setting, or null if not set.
   * A380 specific
   */
  readonly noiseN1?: Writable<number | null>;

  /**
   * The NADP speed setting in knots, or null if not set.
   * A380 specific
   */
  readonly noiseSpeed?: Writable<number | null>;

  /**
   * The NADP end altitude in feet, or null if not set.
   * A380 specific
   */
  readonly noiseEndAltitude?: Writable<number | null>;

  /**
   * The cost index mode (ECON or LRC), or null if not set.
   * A380 specific
   */
  readonly costIndexMode?: Writable<CostIndexMode | null>;

  /**
   * The derated climb setting, or null if not set.
   * A380 specific
   */
  readonly climbDerated?: Writable<ClimbDerated | null>;

  /**
   * The descent cabin rate setting in ft/min, or null if not set.
   * A380 specific
   */
  readonly descentCabinRate?: Writable<number | null>;

  /**
   * The takeoff weight latched at the moment the Takeoff phase begins, in kg. Null until then.
   * A380 specific
   */
  readonly takeoffWeight?: Writable<number | null>;

  clone(): this;

  destroy(): void;

  hasSubscription(key: string): boolean;

  pipeTo(other: FlightPlanPerformanceData, isBeforeEngineStart: boolean): void;
}

export type FlightPlanPerformanceDataProperties = {
  [K in keyof FlightPlanPerformanceData as FlightPlanPerformanceData[K] extends Writable<any> | undefined
    ? K
    : never]: FlightPlanPerformanceData[K] extends Writable<infer T> ? Writable<T> : never;
};

export interface SerializedFlightPlanPerformanceData {
  cruiseFlightLevel: number | null;
  cruiseTemperaturePilotEntry: number | null;
  cruiseTemperatureIsaTemp?: number | null; // A380 specific
  defaultGroundTemperature?: number | null; // A32NX specific
  pilotGroundTemperature?: number | null; // A32NX specific
  costIndex: number | null;
  defaultTropopause: number;
  pilotTropopause: number | null;

  v1: number | null;

  vr: number | null;

  v2: number | null;

  pilotThrustReductionAltitude: number | null;
  defaultThrustReductionAltitude: number | null;

  pilotAccelerationAltitude: number | null;
  defaultAccelerationAltitude: number | null;

  pilotEngineOutAccelerationAltitude: number | null;
  defaultEngineOutAccelerationAltitude: number | null;

  pilotMissedThrustReductionAltitude: number | null;
  defaultMissedThrustReductionAltitude: number | null;

  pilotMissedAccelerationAltitude: number | null;
  defaultMissedAccelerationAltitude: number | null;

  pilotMissedEngineOutAccelerationAltitude: number | null;
  defaultMissedEngineOutAccelerationAltitude: number | null;

  databaseTransitionAltitude: number | null;
  pilotTransitionAltitude: number | null;

  databaseTransitionLevel: number | null;
  pilotTransitionLevel: number | null;

  climbSpeedLimitSpeed: number | null;
  climbSpeedLimitAltitude: number | null;
  isClimbSpeedLimitPilotEntered: boolean;

  descentSpeedLimitSpeed: number | null;
  descentSpeedLimitAltitude: number | null;
  isDescentSpeedLimitPilotEntered: boolean;

  alternateClimbSpeedLimitSpeed: number | null;
  alternateClimbSpeedLimitAltitude: number | null;
  isAlternateClimbSpeedLimitPilotEntered: boolean;

  alternateDescentSpeedLimitSpeed: number | null;
  alternateDescentSpeedLimitAltitude: number | null;
  isAlternateDescentSpeedLimitPilotEntered: boolean;

  zeroFuelWeight: number | null;
  zeroFuelWeightCenterOfGravity: number | null;
  blockFuel: number | null;
  pilotTaxiFuel: number | null;
  defaultTaxiFuel: number;
  pilotRouteReserveFuel: number | null;
  pilotRouteReserveFuelPercentage: number | null;
  defaultRouteReserveFuelPercentage: number;
  pilotAlternateFuel: number | null;
  pilotFinalHoldingFuel: number | null;
  pilotFinalHoldingTime: number | null;
  defaultFinalHoldingTime: number;
  pilotMinimumDestinationFuelOnBoard: number | null;
  pilotTripWind: number | null;

  takeoffShift: number | null;
  takeoffFlaps: 0 | 1 | 2 | 3 | null;
  trimmableHorizontalStabilizer: number | null;
  flexTemperature: number | null;
  preselectedClimbSpeed: number | null;
  preselectedCruiseSpeed: number | null;
  pilotManagedDescentSpeed: number | null;
  pilotManagedDescentMach: number | null;
  approachQnh: number | null;
  approachTemperature: number | null;
  approachWindDirection: number | null;
  approachWindMagnitude: number | null;
  isApproachWindPilotEntered: boolean;
  pilotVapp: number | null;
  approachBaroMinimum: number | null;
  approachRadioMinimum: 'NO DH' | number | null;
  approachFlapsThreeSelected: boolean;

  climbWindEntries: FlightPlanWindEntry[];
  descentWindEntries: FlightPlanWindEntry[];
  alternateWind: WindVector;

  estimatedTakeoffTime: number | null;
  estimatedTakeoffTimeExpired: boolean | null;
  // A380 specific
  paxNumber?: number | null;
  takeoffPowerSetting?: TakeoffPowerSetting | null;
  takeoffDeratedSetting?: TakeoffDerated | null;
  takeoffThsFor?: number | null;
  takeoffPacks?: TakeoffPacks | null;
  takeoffAntiIce?: TakeoffAntiIce | null;
  noiseEnabled?: boolean;
  noiseN1?: number | null;
  noiseSpeed?: number | null;
  noiseEndAltitude?: number | null;
  costIndexMode?: CostIndexMode | null;
  climbDerated?: ClimbDerated | null;
  descentCabinRate?: number | null;
  takeoffWeight?: number | null;
}

// FIXME move to AMI database
export class DefaultPerformanceData {
  static readonly ClimbSpeedLimitSpeed = 250;

  static readonly ClimbSpeedLimitAltitude = 10_000;

  static readonly DescentSpeedLimitSpeed = 250;

  static readonly DescentSpeedLimitAltitude = 10_000;
}

// A380 specific types
export enum TakeoffPowerSetting {
  TOGA = 0,
  FLEX = 1,
  DERATED = 2,
}

export enum TakeoffDerated {
  D01 = 0,
  D02 = 1,
  D03 = 2,
  D04 = 3,
  D05 = 4,
}

export enum TakeoffPacks {
  OFF_APU = 0,
  ON = 1,
}

export enum TakeoffAntiIce {
  OFF = 0,
  ENG_ONLY = 1,
  ENG_WINGS = 2,
}

export enum CostIndexMode {
  LRC = 0,
  ECON = 1,
}

export enum ClimbDerated {
  NONE = 0,
  D01 = 1,
  D02 = 2,
  D03 = 3,
  D04 = 4,
  D05 = 5,
}
