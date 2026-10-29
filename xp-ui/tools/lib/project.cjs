'use strict';

const path = require('path');
const fs = require('fs');

const slash = (p) => p.split(path.sep).join('/');

const FS_INSTRUMENTS = ['Clock', 'EWD', 'FCU', 'HUD', 'MFD', 'ND', 'OIT', 'PFD', 'RMP', 'SDv2', 'popup'];
const REACT_INSTRUMENTS = ['BAT', 'EFB', 'ISISlegacy', 'OITlegacy', 'RTPI', 'SD'];
const HOSTS = { SystemsHost: 'fbw-a380x/src/systems/systems-host/SystemsHost.ts', ExtrasHost: 'fbw-a380x/src/systems/extras-host/index.ts' };

function entries(fork) {
  const inst = (n, f) => slash(path.join(fork, 'fbw-a380x/src/systems/instruments/src', n, f));
  return {
    fs: [...FS_INSTRUMENTS.map((n) => ({ name: n, file: inst(n, 'instrument.tsx') })), ...Object.entries(HOSTS).map(([n, f]) => ({ name: n, file: slash(path.join(fork, f)) }))],
    react: REACT_INSTRUMENTS.map((n) => ({ name: n, file: inst(n, 'index.tsx') })),
  };
}

const ALIASES = {
  '@atsu/*': ['fbw-a380x/src/systems/atsu/src/*'],
  '@datalink/aoc': ['fbw-common/src/systems/datalink/aoc/src/index.ts'],
  '@datalink/atc': ['fbw-common/src/systems/datalink/atc/src/index.ts'],
  '@datalink/common': ['fbw-common/src/systems/datalink/common/src/index.ts'],
  '@datalink/router': ['fbw-common/src/systems/datalink/router/src/index.ts'],
  '@failures': ['fbw-a380x/src/systems/failures/src/index.ts'],
  '@flybywiresim/bat': ['fbw-common/src/systems/instruments/src/BAT/index.ts'],
  '@flybywiresim/checklists': ['fbw-common/src/systems/shared/src/checklists/index.ts'],
  '@flybywiresim/clock': ['fbw-common/src/systems/instruments/src/Clock/index.ts'],
  '@flybywiresim/fbw-sdk': ['fbw-common/src/systems/index.ts'],
  '@flybywiresim/fbw-sdk-react': ['fbw-common/src/systems/index-react.ts'],
  '@flybywiresim/flypad': ['fbw-common/src/systems/instruments/src/EFB/index.ts'],
  '@flybywiresim/msfs-avionics-common': ['fbw-common/src/systems/instruments/src/MsfsAvionicsCommon/index.ts'],
  '@flybywiresim/navigation-display': ['fbw-common/src/systems/instruments/src/ND/index.ts'],
  '@flybywiresim/oanc': ['fbw-common/src/systems/instruments/src/OANC/index.ts'],
  '@flybywiresim/pfd': ['fbw-common/src/systems/instruments/src/PFD/index.ts'],
  '@flybywiresim/hud': ['fbw-common/src/systems/instruments/src/HUD/index.ts'],
  '@flybywiresim/rmp': ['fbw-common/src/systems/instruments/src/RMP/index.ts'],
  '@flybywiresim/popup': ['fbw-common/src/systems/instruments/src/popup/index.ts'],
  '@fmgc/*': ['fbw-a32nx/src/systems/fmgc/src/*'],
  '@instruments/common/*': ['fbw-a380x/src/systems/instruments/src/Common/*'],
  '@localization/*': ['fbw-a380x/src/localization/*'],
  '@sentry/*': ['fbw-a380x/src/systems/sentry-client/src/*'],
  '@shared/*': ['fbw-a380x/src/systems/shared/src/*'],
  '@simbridge/*': ['fbw-a32nx/src/systems/simbridge-client/src/*'],
  '@tcas/*': ['fbw-a380x/src/systems/tcas/src/*'],
  '@typings/*': ['fbw-a380x/src/typings'],
  '@providers/*': ['fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/providers/*'],
};

/**
 * Compiler options for FlyByWire's A380X sources.
 * libModules maps bare module names to library entry files.
 * typings selects the global declaration folders under fbw-common/src/typings.
 */
function compilerOptions(ts, { fork, modulesDir, libModules, jsx, typings }) {
  const paths = {};
  for (const [k, v] of Object.entries(libModules)) paths[k] = [slash(v)];
  for (const [k, v] of Object.entries(ALIASES)) paths[k] = v.map((p) => slash(path.join(fork, p)));
  const typeRoots = [slash(path.join(modulesDir, '@types'))];
  const types = typings === 'none' ? fs.readdirSync(path.join(modulesDir, '@types')).filter((t) => !t.startsWith('.')) : ['*'];
  if (typings === 'xplane') typeRoots.unshift(slash(path.join(fork, 'xplane/typings')));
  else if (typings !== 'none') typeRoots.unshift(slash(path.join(fork, 'fbw-common/src/typings')));
  const factory = jsx === 'react' ? ['React.createElement', 'React.Fragment'] : jsx;
  return {
    target: ts.ScriptTarget.ES2017,
    strict: false,
    noEmit: true,
    resolveJsonModule: true,
    skipLibCheck: true,
    typeRoots,
    types,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    module: ts.ModuleKind.Preserve,
    allowSyntheticDefaultImports: true,
    forceConsistentCasingInFileNames: true,
    jsx: ts.JsxEmit.React,
    jsxFactory: factory[0],
    jsxFragmentFactory: factory[1],
    paths,
  };
}

const FORBIDDEN = ['/@microsoft/', '/xp-kit/', '/xp-avionics/', 'msfs-avionics-mirror', '/adapt-orig/', '/xpkit'];
const forbidden = (f) => {
  const n = slash(String(f)).toLowerCase();
  return FORBIDDEN.some((x) => n.includes(x));
};

/** A compiler host that refuses to look at any forbidden path. */
function cleanHost(ts, options) {
  const host = ts.createCompilerHost(options, true);
  const fileExists = host.fileExists.bind(host);
  const readFile = host.readFile.bind(host);
  const getSourceFile = host.getSourceFile.bind(host);
  const directoryExists = host.directoryExists ? host.directoryExists.bind(host) : null;
  const realpath = host.realpath ? host.realpath.bind(host) : null;
  host.fileExists = (f) => !forbidden(f) && fileExists(f);
  host.readFile = (f) => (forbidden(f) ? undefined : readFile(f));
  host.getSourceFile = (f, ...rest) => (forbidden(f) ? undefined : getSourceFile(f, ...rest));
  if (directoryExists) host.directoryExists = (d) => !forbidden(d + '/') && directoryExists(d);
  if (realpath) host.realpath = (f) => (forbidden(f) ? f : realpath(f));
  return host;
}

module.exports = { slash, entries, compilerOptions, cleanHost, forbidden, FS_INSTRUMENTS, REACT_INSTRUMENTS, HOSTS };
