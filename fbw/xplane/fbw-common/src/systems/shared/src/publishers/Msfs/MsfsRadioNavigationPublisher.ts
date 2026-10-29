// @ts-strict-ignore
import {
  Bus,
  IndexedTopic,
  Pacer,
  VarPublisher,
  VarTopic,
  VarUnit,
} from 'xplane-js/ui';

interface MsfsRadioNavigationBaseEvents {
  /** Radar altitude in feet. */
  msfs_radio_height: number;
}

type IndexedTopics = null;

type MsfsRadioNavigationIndexedEvents = {
  [P in keyof Pick<
    MsfsRadioNavigationBaseEvents,
    IndexedTopics
  > as IndexedTopic<P>]: MsfsRadioNavigationBaseEvents[P];
};

/**
 * Events for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_RadioNavigation_Variables.htm.
 * Event names are the same as the simvar names, with msfs_ prefix, and index as suffix for indexed simvars.
 */
export interface MsfsRadioNavigationEvents extends MsfsRadioNavigationBaseEvents, MsfsRadioNavigationIndexedEvents {}

/**
 * Publisher for simvars listed on https://docs.flightsimulator.com/html/Programming_Tools/SimVars/Aircraft_SimVars/Aircraft_RadioNavigation_Variables.htm.
 */
export class MsfsRadioNavigationPublisher extends VarPublisher<MsfsRadioNavigationEvents> {
  /**
   * Create a publisher.
   * @param bus The EventBus to publish to
   * @param pacer An optional pacer to use to control the rate of publishing
   */
  public constructor(bus: Bus, pacer?: Pacer<MsfsRadioNavigationEvents>) {
    const simvars = new Map<keyof MsfsRadioNavigationEvents, VarTopic<any>>([
      ['msfs_radio_height', { name: `sim/cockpit2/gauges/indicators/radio_altimeter_height_ft_pilot`, type: VarUnit.Feet }],
    ]);

    super(simvars, bus, pacer);
  }
}
