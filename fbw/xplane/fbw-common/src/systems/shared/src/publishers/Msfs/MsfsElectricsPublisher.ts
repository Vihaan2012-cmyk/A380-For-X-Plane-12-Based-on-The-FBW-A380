import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface MsfsElectricsBaseEvents {
  /** Gets the availability of external power */
  msfs_external_power_available: boolean;
}

type IndexedTopics = 'msfs_external_power_available';

type MsfsElectricsIndexedEvents = {
  [P in keyof Pick<MsfsElectricsBaseEvents, IndexedTopics> as IndexedTopic<P>]: MsfsElectricsBaseEvents[P];
};

/**
 * Events for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_Electrics_Variables.htm.
 * Event names are the same as the simvar names, with msfs_ prefix, and index as suffix for indexed simvars.
 */
export interface MsfsElectricsEvents extends MsfsElectricsBaseEvents, MsfsElectricsIndexedEvents {}

/**
 * Publisher for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_Electrics_Variables.htm.
 */
export class MsfsElectricsPublisher extends VarPublisher<MsfsElectricsEvents> {
  /**
   * Create a publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<MsfsElectricsEvents>) {
    const simvars = new Map<keyof MsfsElectricsEvents, VarTopic<any>>([
      [
        'msfs_external_power_available',
        { name: `fbw/EXTERNAL_POWER_AVAILABLE_#index#`, type: VarUnit.Bool, indexed: true },
      ],
    ]);

    super(simvars, bus, pacer);
  }
}
