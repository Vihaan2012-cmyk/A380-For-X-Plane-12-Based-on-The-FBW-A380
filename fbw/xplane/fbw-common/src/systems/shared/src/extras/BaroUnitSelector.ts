import { NXDataStore } from '@flybywiresim/fbw-sdk';
import { SessionState, Waits } from 'xplane-js/ui';
import { NearestSearchType } from '../../../navdata/client/backends/Msfs/FsTypes';
import { Bridge, Channels, SessionPhase, Vars, openChannel } from 'xplane-js/data';

export class BaroUnitSelector {
  private static readonly IN_HG_REGIONS = ['K', 'C', 'M', 'P', 'RJ', 'RO', 'TI', 'TJ'];

  private facilityListener?: Channels.Channel;

  constructor(private readonly setBaroSelector: (isHpa: boolean) => void) {}

  public performSelection(): void {
    const configInitBaroUnit = NXDataStore.getLegacy('CONFIG_INIT_BARO_UNIT', 'AUTO');
    if (configInitBaroUnit !== 'AUTO') {
      this.setBaroSelector(configInitBaroUnit == 'HPA');
    } else if (this.facilityListener) {
      this.autoSetBaroUnit();
    } else {
      openChannel('nav', () => this.autoSetBaroUnit(), true);
    }
  }

  /** @private */
  private async autoSetBaroUnit() {
    await Waits.forValue(SessionState.get(), (v) => v === SessionPhase.flying, true);

    const sessionId = await Bridge.call('nav.nearest.open', NearestSearchType.Airport);
    const handler = Bridge.on('nav.nearest.done', (result: { sessionId: number; added: string[] }) => {
      if (result.sessionId === sessionId) {
        handler.clear();

        const useInHg =
          result.added.length > 0 &&
          (BaroUnitSelector.IN_HG_REGIONS.includes(result.added[0].charAt(7)) ||
            BaroUnitSelector.IN_HG_REGIONS.includes(result.added[0].substring(7, 9)));

        this.setBaroSelector(!useInHg);
      }
    });

    const lat = Vars.read('fbw/PLANE_LATITUDE', 'degree latitude');
    const lon = Vars.read('fbw/PLANE_LONGITUDE', 'degree longitude');

    Bridge.call('nav.nearest.search', sessionId, lat, lon, 50000, 1);
  }
}
