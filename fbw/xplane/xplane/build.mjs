// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
//
// node xplane/build.mjs --out <dir> [--library <clean-avionics>] [--only Name,Name]

import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const fork = path.resolve(here, '..');
const require = createRequire(path.join(here, 'package.json'));
const args = process.argv.slice(2);
const opt = (n, d) => (args.includes('--' + n) ? args[args.indexOf('--' + n) + 1] : d);
const out = path.resolve(opt('out', 'E:/fbw-build/xpjs/out'));
const library = path.resolve(opt('library', 'E:/fbw-int/clean-avionics'));
const only = opt('only', '') ? new Set(opt('only').split(',')) : null;

const lock = fs.readFileSync(path.join(here, 'package-lock.json'), 'utf8');
if (lock.includes('@microsoft/') || fs.existsSync(path.join(here, 'node_modules', '@microsoft'))) {
  console.error('refusing to build: a Microsoft package is in the lockfile or node_modules');
  process.exit(1);
}

const esbuild = require('esbuild');
const imagePlugin = require('esbuild-plugin-inline-image');
const postCssPlugin = require('esbuild-style-plugin-v2');
const tailwind = require('tailwindcss');
const postCssColorFunctionalNotation = require('postcss-color-functional-notation');
const postCssInset = require('postcss-inset');

const a380 = path.join(fork, 'fbw-a380x');
const inst = (n, f) => path.join(a380, 'src/systems/instruments/src', n, f);
const FS = ['Clock', 'EWD', 'FCU', 'HUD', 'MFD', 'ND', 'OIT', 'PFD', 'RMP', 'SDv2', 'popup'];
const REACT = ['BAT', 'EFB', 'ISISlegacy', 'OITlegacy', 'RTPI', 'SD'];

const jobs = [
  ...FS.map((n) => ({ kind: 'fs', name: n, entry: inst(n, 'instrument.tsx'), file: n.toLowerCase(), template: `A380X_${n}`, markup: `<div id="${n}_CONTENT"><h1>If you're seeing this, instrument didn't load.</h1></div>` })),
  ...REACT.map((n) => ({ kind: 'react', name: n, entry: inst(n, 'index.tsx'), file: n.toLowerCase(), template: n, markup: `<div id="MSFS_REACT_MOUNT"><h1>If you're seeing this, instrument didn't load.</h1></div>`, interactive: JSON.parse(fs.readFileSync(inst(n, 'config.json'), 'utf8')).isInteractive !== false })),
  { kind: 'host', name: 'SystemsHost', entry: path.join(a380, 'src/systems/systems-host/SystemsHost.ts'), file: 'SystemsHost', page: 'systems-host', template: 'A380X_SYSTEMSHOST', markup: '' },
  { kind: 'host', name: 'ExtrasHost', entry: path.join(a380, 'src/systems/extras-host/index.ts'), file: 'index', page: 'extras-host', template: 'A380X_EXTRASHOST', markup: '' },
].filter((j) => !only || only.has(j.name));

const xplaneJs = {
  name: 'xplane-js',
  setup(build) {
    build.onResolve({ filter: /^xplane-js\/(ui|data)$/ }, (a) => ({ path: path.join(library, a.path.endsWith('/ui') ? 'lib/src/index.ts' : 'data/src/index.ts') }));
    build.onResolve({ filter: /^tslib$/ }, () => ({ path: path.join(library, 'lib/src/compat/ts-helpers.ts') }));
    build.onResolve({ filter: /^@flybywiresim\/api-client$/ }, () => ({ path: path.join(here, 'vendor/api-client-0.16.2/src/index.ts') }));
    build.onResolve({ filter: /^@sentry\/integrations$/ }, () => ({ path: require.resolve('@sentry/integrations/esm/captureconsole.js'), sideEffects: false }));
  },
};

const define = {
  'process.env': '{}',
  'process.env.AIRCRAFT_PROJECT_PREFIX': '"a380x"',
  'process.env.AIRCRAFT_VARIANT': '"a380-842"',
  'process.env.VITE_BUILD': 'false',
  'process.env.MODULE': 'false',
  DEBUG: 'false',
};

const styles = () =>
  postCssPlugin({
    extract: true,
    postcss: {
      plugins: [tailwind(path.join(fork, 'fbw-common/src/systems/instruments/src/EFB/tailwind.config.js')), postCssColorFunctionalNotation(), postCssInset()],
    },
  });

const tmp = path.join(out, '..', 'xplane-build-tmp');
fs.mkdirSync(tmp, { recursive: true });

function reactEntry(j) {
  const f = path.join(tmp, `react-${j.name}.ts`);
  const index = j.entry.split(path.sep).join('/');
  fs.writeFileSync(
    f,
    `import { PanelInstrument, defineInstrument } from 'xplane-js/data';
class ReactInstrument extends PanelInstrument {
  get templateId(): string {
    return '${j.template}';
  }
  get interactive(): boolean {
    return ${j.interactive};
  }
  onAttach(): void {
    super.onAttach();
    require('${index}');
  }
  onFrame(): void {
    super.onFrame();
    this.dispatchEvent(new CustomEvent('update', { detail: this.frameDelta }));
  }
  onPanelEvent(args: string[]): void {
    const name = String(args);
    this.dispatchEvent(new CustomEvent(name));
    this.dispatchEvent(new CustomEvent('*', { detail: name }));
  }
}
defineInstrument('a380x-${j.file}', ReactInstrument);
`,
  );
  return f;
}

function page(j) {
  const css = fs.existsSync(path.join(out, 'html_ui/Pages/VCockpit/Instruments/A380X', j.name, `${j.file}.css`)) ? `<link rel="stylesheet" href="${j.file}.css">\n` : '';
  return `<!doctype html>\n<html>\n<head>\n<meta charset="utf-8">\n${css}</head>\n<body>\n<template id="${j.template}">${j.markup}</template>\n<script src="${j.file}.js"></script>\n</body>\n</html>\n`;
}

let failed = 0;
for (const j of jobs) {
  const dir = path.join(out, 'html_ui/Pages/VCockpit/Instruments/A380X', j.name);
  fs.mkdirSync(dir, { recursive: true });
  const t0 = Date.now();
  try {
    await esbuild.build({
      absWorkingDir: a380,
      entryPoints: [j.kind === 'react' ? reactEntry(j) : j.entry],
      outfile: path.join(dir, `${j.file}.js`),
      external: ['/Images/*', '/Fonts/*'],
      bundle: true,
      target: 'es2017',
      format: 'iife',
      logLevel: 'error',
      minify: false,
      sourcemap: false,
      treeShaking: j.kind === 'host' ? false : undefined,
      define,
      plugins: [xplaneJs, imagePlugin({ limit: -1 }), styles()],
    });
    fs.writeFileSync(path.join(dir, `${j.page ?? j.file}.html`), page(j));
    console.log(`${j.name}: ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  } catch (e) {
    failed++;
    console.error(`${j.name}: FAILED ${e.message}`);
  }
}

const runtimeOut = path.join(out, 'runtime/xplane-js-data.js');
try {
  await esbuild.build({ entryPoints: [path.join(library, 'data/src/runtime.ts')], outfile: runtimeOut, bundle: true, target: 'es2017', format: 'iife', logLevel: 'error', sourcemap: false });
  console.log('runtime: ok');
} catch (e) {
  failed++;
  console.error(`runtime: FAILED ${e.message}`);
}
fs.rmSync(tmp, { recursive: true, force: true });
process.exit(failed ? 1 : 0);
