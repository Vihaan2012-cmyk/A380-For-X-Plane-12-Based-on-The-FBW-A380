// @ts-strict-ignore
import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface MsfsMiscBaseEvents {
  /** Gets the speed relative to the earths surface in knots */
  msfs_ground_velocity: number;
  /** Ground truth latitude. Don't use this for navigation, use ADIRS instead. */
  msfs_latitude: number;
  /** Ground truth longitude. Don't use this for navigation, use ADIRS instead. */
  msfs_longitude: number;
}

type IndexedTopics = null;

type MsfsMiscIndexedEvents = {
  [P in keyof Pick<MsfsMiscBaseEvents, IndexedTopics> as IndexedTopic<P>]: MsfsMiscBaseEvents[P];
};

/**
 * Events for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_Misc_Variables.htm.
 * Event names are the same as the simvar names, with msfs_ prefix, and index as suffix for indexed simvars.
 */
export interface MsfsMiscEvents extends MsfsMiscBaseEvents, MsfsMiscIndexedEvents {}

/**
 * Publisher for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_Misc_Variables.htm.
 */
export class MsfsMiscPublisher extends VarPublisher<MsfsMiscEvents> {
  /**
   * Create a publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<MsfsMiscEvents>) {
    const simvars = new Map<keyof MsfsMiscEvents, VarTopic<any>>([
      ['msfs_ground_velocity', { name: `fbw/GROUND_VELOCITY`, type: VarUnit.Knots }],
      ['msfs_latitude', { name: `fbw/PLANE_LATITUDE`, type: VarUnit.Degree }],
      ['msfs_longitude', { name: `fbw/PLANE_LONGITUDE`, type: VarUnit.Degree }],
    ]);

    super(simvars, bus, pacer);
  }
}
