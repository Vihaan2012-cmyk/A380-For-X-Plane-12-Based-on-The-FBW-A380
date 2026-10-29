import { vitest, beforeEach } from 'vitest';
import { bearingTo, placeBearingDistance } from 'msfs-geo';
import { GeoPointLong } from 'xplane-js/ui';

let values;

beforeEach(() => {
  values = {};
});

const read = (name) => (Object.prototype.hasOwnProperty.call(values, name) ? values[name] : 0);

// The host XPHFBW gives each page (window.__xphfbw), under JS Library for X-Plane's data runtime.
global.__xphfbw = {
  view: 0,
  screen: '',
  aircraftDir: '',
  snapshot() {},
  getVar: vitest.fn((name, _) => read(name)),
  setVar: vitest.fn((name, _, value) => {
    values[name] = value;
  }),
  getString: (name) => String(read(name) || ''),
  setString: (name, value) => {
    values[name] = value;
  },
  sendEvent() {},
  call: async <T>() => new Promise<T>(() => {}),
  poll: () => [],
  gameString: () => '',
  storedData: () => '',
  readFile: () => null,
  log() {},
  magVar: () => 0, // FIXME ship magnetic model
  loaded() {},
};

await import('xplane-js/runtime');

const geo = (global as any).xplaneJsData.NavMath.Geo;
geo.bearing = (from: GeoPointLong, to: GeoPointLong): number => bearingTo(from, to);
geo.destination = (bearing: number, distance: number, lat: number, long: number) => placeBearingDistance({ lat, long }, bearing, distance);
