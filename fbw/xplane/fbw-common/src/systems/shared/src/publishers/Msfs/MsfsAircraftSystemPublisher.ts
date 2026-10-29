import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface MsfsAircraftSystemBaseEvents {
  /** Adjust the potentiometer of the indexed lighting. Index is defined in the appropriate lightdef hashmap setting. 0-1 */
  msfs_light_potentiometer: number;
}

type IndexedTopics = 'msfs_light_potentiometer';

type MsfsAircraftSystemIndexedEvents = {
  [P in keyof Pick<
    MsfsAircraftSystemBaseEvents,
    IndexedTopics
  > as IndexedTopic<P>]: MsfsAircraftSystemBaseEvents[P];
};

/**
 * Events for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_System_Variables.htm.
 * Event names are the same as the simvar names, with msfs_ prefix, and index as suffix for indexed simvars.
 */
export interface MsfsAircraftSystemEvents extends MsfsAircraftSystemBaseEvents, MsfsAircraftSystemIndexedEvents {}

/**
 * Publisher for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_System_Variables.htm.
 */
export class MsfsAircraftSystemPublisher extends VarPublisher<MsfsAircraftSystemEvents> {
  /**
   * Create a publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<MsfsAircraftSystemEvents>) {
    const simvars = new Map<keyof MsfsAircraftSystemEvents, VarTopic<any>>([
      [
        'msfs_light_potentiometer',
        { name: `fbw/LIGHT_POTENTIOMETER_#index#`, type: VarUnit.PercentOver100, indexed: true },
      ],
    ]);

    super(simvars, bus, pacer);
  }
}
