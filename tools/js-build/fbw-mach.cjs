// fbw-env.cjs for FlyByWire's `mach` instrument build, which reads its own
// command line: `node fbw-mach.cjs build --config fbw-a380x/mach.config.js --work-in-config-dir`
'use strict';

const path = require('path');

const ci = {
  AIRCRAFT_PROJECT_PREFIX: 'a380x',
  AIRCRAFT_VARIANT: 'a380-842',
  VITE_BUILD: 'false',
};

const needed = new Set(['PATH', 'SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA']);
const backslash = String.fromCharCode(92);
const definable = (value) => !value.includes(backslash) && !value.includes('"') && !/[\u0000-\u001f]/.test(value);

for (const name of Object.keys(process.env)) {
  const value = process.env[name];
  if (needed.has(name.toUpperCase())) {
    process.env[name] = value.split(backslash).join('/').split('"').join('');
  } else if (!definable(value)) {
    delete process.env[name];
  }
}
for (const [name, value] of Object.entries(ci)) {
  if (process.env[name] === undefined) {
    process.env[name] = value;
  }
}

const cli = path.resolve('node_modules/@synaptic-simulations/mach/dist/index.js');
process.argv = [process.argv[0], cli, ...process.argv.slice(2)];
require(cli);
