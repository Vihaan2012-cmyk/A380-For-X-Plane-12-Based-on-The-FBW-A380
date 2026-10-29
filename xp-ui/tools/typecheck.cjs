'use strict';
// node typecheck.cjs <fork> [--old] [--extra <file,file relative to fork>]
// Type-checks every A380X instrument and host of a FlyByWire tree.
// Default: the converted tree against xplane-js (no simulator typings).
// --old: the original tree against the old-name library and the simulator typings.

const ts = require('typescript');
const path = require('path');
const { slash, entries, compilerOptions, cleanHost } = require('./lib/project.cjs');

const fork = slash(path.resolve(process.argv[2] || 'E:/fbw-int/fbw-own-api'));
const old = process.argv.includes('--old');
const root = path.resolve(__dirname, '..');
const ui = path.join(root, 'lib/src/index.ts');
const libModules = old ? { '@microsoft/msfs-sdk': process.env.OLD_LIB, 'xp-kit': process.env.OLD_LIB, 'xp-ui': process.env.OLD_LIB } : { 'xplane-js/ui': ui, 'xplane-js/data': path.join(root, 'spec/data-api.d.ts'), 'xplane-js/runtime': path.join(root, 'spec/runtime.d.ts') };
const fsJsx = old ? ['FSComponent.buildComponent', 'FSComponent.Fragment'] : ['UI.node', 'UI.Group'];
const e = entries(fork);
const extraAt = process.argv.indexOf('--extra');
const extra = extraAt > 0 ? process.argv[extraAt + 1].split(',').map((f) => ({ name: f, file: slash(path.resolve(fork, f)) })) : [];
let total = 0;
for (const [name, list, jsx] of [['fs', [...e.fs, ...extra], fsJsx], ['react', e.react, 'react']]) {
  const options = compilerOptions(ts, { fork, modulesDir: path.join(fork, 'node_modules'), libModules, jsx, typings: old ? 'msfs' : 'xplane' });
  const program = ts.createProgram(list.map((x) => x.file), options, cleanHost(ts, options));
  const diags = ts.getPreEmitDiagnostics(program);
  const files = program.getSourceFiles().filter((s) => !slash(s.fileName).includes('/node_modules/')).length;
  console.log(`${name}: ${list.length} entries, ${files} files, ${diags.length} errors`);
  for (const d of diags.slice(0, Number(process.env.SHOW || 40))) {
    const where = d.file ? `${slash(d.file.fileName).replace(fork + '/', '').replace(slash(root) + '/', 'clean-avionics/')}:${d.file.getLineAndCharacterOfPosition(d.start || 0).line + 1}` : '';
    console.log(`  ${where} TS${d.code} ${ts.flattenDiagnosticMessageText(d.messageText, ' ').slice(0, 220)}`);
  }
  total += diags.length;
}
process.exitCode = total ? 1 : 0;
