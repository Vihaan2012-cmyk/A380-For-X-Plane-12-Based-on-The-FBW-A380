import { NearbyFacilityMonitor } from './fbw_sdk.js';

export const NavigationDatabaseBackend = { Msfs: 0 };

const backend = {
  async getAirports(idents) {
    return idents.filter((i) => globalThis.__airports[i]).map((i) => ({ ident: i, location: globalThis.__airports[i].location }));
  },
  async getRunways(icao) {
    return ((globalThis.__airports[icao] || {}).runways || []).map((r) => ({
      ident: r.ident,
      thresholdLocation: r.thresholdLocation,
      bearing: r.bearing,
      length: r.length,
    }));
  },
};

export class NavigationDatabase {
  constructor() {
    this.backendDatabase = backend;
  }
  createNearbyFacilityMonitor() {
    return new NearbyFacilityMonitor();
  }
}

export const NavigationDatabaseService = { activeDatabase: undefined };
