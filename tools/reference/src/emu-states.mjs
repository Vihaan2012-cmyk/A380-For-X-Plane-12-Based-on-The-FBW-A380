// Turns the emulator's variable snapshots (emulator/src/bin/golden_states.rs)
// into harness states: states/emu-*.json.
//
//   node src/emu-states.mjs <snapshot dir>
//
// Aircraft variables become L: keys; simulator variables become A: keys in
// the unit FlyByWire's systems register them in (a380_systems_wasm's
// provides_aircraft_variable, the table the X-Plane bridge converts from).
import fs from 'node:fs';
import path from 'node:path';
import { FBW_ROOT, STATES_DIR } from './config.mjs';

const FLT_BY_SCENARIO = [
  [/cold-dark|batteries|ground-power|apu-fire-ground|-apu$/, 'apron.FLT'],
  [/engines-idle|taxi|takeoff|rollout/, 'taxi.flt'],
  [/approach/, 'approach.FLT'],
  [/./, 'Climb.flt'],
];

function storedUnits() {
  const src = fs.readFileSync(path.join(FBW_ROOT, 'fbw-a380x/src/wasm/systems/a380_systems_wasm/src/lib.rs'), 'utf8');
  const units = new Map();
  for (const part of src.split('provides_aircraft_variable(').slice(1)) {
    const strings = part.split('"');
    if (strings.length >= 4 && !units.has(strings[1])) units.set(strings[1], strings[3]);
  }
  return units;
}

const baseName = (name) => {
  const cut = name.lastIndexOf(':');
  return cut > 0 && /^\d+$/.test(name.slice(cut + 1)) ? name.slice(0, cut) : name;
};

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node src/emu-states.mjs <snapshot dir>');
  process.exit(2);
}
const units = storedUnits();
for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.json')).sort()) {
  const snap = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
  const vars = {};
  for (const [named, name, value] of snap.vars) {
    if (named) {
      vars[`L:${name}`] = { value, src: 'emulator' };
    } else {
      const unit = units.get(baseName(name));
      vars[`A:${name}`] = unit ? { value, unit, src: 'emulator' } : { value, src: 'emulator' };
    }
  }
  const flt = FLT_BY_SCENARIO.find(([re]) => re.test(snap.name))[1];
  const derived = (key, from, unit) => {
    if (!vars[key] && vars[from]) vars[key] = { value: vars[from].value, unit, src: `emulator ${from}` };
  };
  derived('A:GEAR IS ON GROUND', 'A:SIM ON GROUND', 'Bool');
  derived('A:PLANE ALTITUDE', 'A:PRESSURE ALTITUDE', 'feet');
  derived('A:INDICATED ALTITUDE', 'A:PRESSURE ALTITUDE', 'feet');
  derived('A:PLANE ALT ABOVE GROUND MINUS CG', 'A:PLANE ALT ABOVE GROUND', 'feet');
  if (!/cold-dark|batteries/.test(snap.name)) {
    vars['L:A32NX_COLD_AND_DARK_SPAWN'] = { value: 0, src: 'not a cold-and-dark spawn: the warning system skips its 60 s self-test' };
  }
  const state = {
    name: snap.name,
    description: `${snap.description}${snap.notes.length ? ` (${snap.notes.join('; ')})` : ''}. Every variable from the offline emulator.`,
    extends: [],
    flt,
    time: { utc: '2026-06-21T12:00:00Z', settleMs: 20000 },
    vars,
  };
  fs.writeFileSync(path.join(STATES_DIR, `${snap.name}.json`), `${JSON.stringify(state, null, 1)}\n`);
  console.log(`${snap.name}: ${Object.keys(vars).length} vars, flt ${flt}`);
}
