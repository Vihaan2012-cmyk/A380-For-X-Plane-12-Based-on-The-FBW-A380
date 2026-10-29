import { EventBus, Subject } from '@microsoft/msfs-sdk';
import * as geo from 'msfs-geo';
import { OancMarkerManager } from '@fbw/oanc/OancMarkerManager.ts';
import { BrakeToVacate } from '@fbw/a380x/systems-host/PseudoPRIM/BrakeToVacate.ts';
import { OansBrakeToVacateSelection } from '@fbw/oanc/OansBrakeToVacateSelection.ts';

globalThis.__geo = geo;
globalThis.__now = 0;
Date.now = () => globalThis.__now;
globalThis.SimVar = {
  SetSimVarValue(name, _unit, v) {
    globalThis.__vars[name] = Number(v);
  },
  GetSimVarValue() {
    return 0;
  },
};
globalThis.Path2D = class {
  moveTo() {}
  lineTo() {}
};
const ctx2d = {
  resetTransform() {},
  translate() {},
  beginPath() {},
  moveTo() {},
  lineTo() {},
  stroke() {},
  clearRect() {},
};
const canvas = { width: 768, height: 768, getContext: () => ctx2d };
const canvasRef = { instance: canvas, getOrDefault: () => canvas };
const STOP_STYLES = ['btv-stop-line-magenta', 'btv-stop-line-amber', 'btv-stop-line-red', 'btv-stop-line-green'];

class LabelList {
  constructor() {
    this.arr = [];
  }
  insert(v) {
    this.arr.push(v);
  }
  removeAt(i) {
    this.arr.splice(i, 1);
  }
  getArray() {
    return this.arr;
  }
}

let bus;
let btv;
let selection;
let arp;
let ppos;
let onGround;
let markers;
let labelManager;
let scenario;
const instrument = { deltaTime: 0 };

globalThis.__init = (json) => {
  const sc = JSON.parse(json);
  globalThis.__airports = sc.airports;
  globalThis.__vars = {};
  bus = new EventBus();
  scenario = sc;
  arp = Subject.create(sc.arp ?? null);
  ppos = Subject.create([0, 0]);
  onGround = Subject.create(true);
  labelManager = { visibleLabels: new LabelList(), labels: [] };
  btv = new BrakeToVacate(bus, instrument);
  selection = new OansBrakeToVacateSelection(
    bus,
    labelManager,
    onGround,
    ppos,
    arp,
    canvasRef,
    Subject.create(384),
    Subject.create(384),
    Subject.create(2),
    () => sc.zoomInverseScale ?? 1,
  );
  markers = new OancMarkerManager({ data: { features: sc.features ?? [] } }, labelManager, bus);
  bus.getSubscriber().on('oans_remove_btv_data').whenChanged().handle((removeBtvData) => {
    if (removeBtvData) {
      selection.clearSelection();
    }
  });
  btv.init();
};

globalThis.__step = (json) => {
  const s = JSON.parse(json);
  const pub = bus.getPublisher();
  switch (s.op) {
    case 'bus':
      pub.pub(s.topic, s.value);
      break;
    case 'tick':
      instrument.deltaTime = s.ms;
      btv.onUpdate();
      break;
    case 'select_runway':
      selection.selectRunwayFromOans(
        s.runway,
        { geometry: { coordinates: s.centerline } },
        { geometry: { coordinates: s.threshold }, properties: { brngmag: s.brngmag } },
      );
      break;
    case 'select_exit':
      selection.selectExitFromOans(s.exit, { geometry: { coordinates: s.line }, properties: { feattype: 19 } });
      break;
    case 'ppos_local':
      ppos.set(s.pos);
      break;
    case 'ac_on_ground':
      onGround.set(s.value);
      break;
    case 'rwy_ahead': {
      globalThis.__now = s.now;
      ppos.set(s.ppos_local);
      const features = {
        type: 'FeatureCollection',
        features: scenario.runway_areas.map((r) => ({
          type: 'Feature',
          properties: { idrwy: r.idrwy },
          geometry: { type: 'Polygon', coordinates: r.rings },
        })),
      };
      for (let i = 0; i < s.repeat; i++) {
        selection.updateRwyAheadAdvisory({ lat: s.lat, long: s.long }, scenario.arp, s.bearing, features);
      }
      break;
    }
    case 'marker': {
      const args = s.args ?? [];
      markers[s.fn](...args);
      break;
    }
    default:
      throw new Error('unknown step ' + s.op);
  }
};

globalThis.__snapshot = () => {
  const raw = globalThis.__vars['L:A32NX_OANS_BTV_REMAINING_DIST_TO_RWY_END'];
  if (raw !== undefined) bus.getPublisher().pub('oansRemainingDistToRwyEnd', raw);
  selection.drawBtvLayer();
  const stopLines = labelManager.labels
    .filter((l) => STOP_STYLES.includes(l.style))
    .map((l) => ({ text: l.text, style: l.style, position: l.position }));
  const symbols = labelManager.labels
    .filter((l) => l.style === 'cross-symbol' || l.style === 'flag-symbol')
    .map((l) => ({ text: l.text, style: l.style, position: l.position }));
  const out = {};
  for (const [k, v] of Object.entries(bus.cache)) {
    if (k.startsWith('ra_') || k.startsWith('ir_') || k.startsWith('lgciu') || k.startsWith('a380x_btv') || k === 'fmsLandingRunway' || k === 'oansManualStoppingDistance' || k === 'groundSpeed' || k === 'oansRemainingDistToRwyEnd' || k === 'oans_symbols_for_feature_ids') continue;
    out[k] = v === undefined ? null : v;
  }
  return JSON.stringify({ vars: globalThis.__vars, bus: out, stopLines, symbols, symbolIds: bus.cache['oans_symbols_for_feature_ids'] ?? null });
};
