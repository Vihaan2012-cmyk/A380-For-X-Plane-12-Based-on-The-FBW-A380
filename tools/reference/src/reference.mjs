#!/usr/bin/env node
// Renders FBW A380X instruments in Chromium for a named simulator state.
//
//   npm run reference -- --state approach            all default screens
//   npm run reference -- --state all --screen PFD_L,EWD
//   npm run reference -- --state approach --record   also record DOM/CSS/SVG usage
//
// Output: E:/fbw-build/reference-png/<state>/<SCREEN>.png, plus
// <SCREEN>.access.json (every variable/storage/Coherent access and errors)
// and state.resolved.json (the flat state both renderers load).
import fs from 'node:fs';
import path from 'node:path';
import { DOM_USAGE_DIR, REFERENCE_PNG_DIR, TOOL_ROOT } from './config.mjs';
import { loadScreens } from './panel.mjs';
import { startServer } from './server.mjs';
import { listStates, loadState } from './state.mjs';

const DEFAULT_SCREENS = ['PFD_L', 'PFD_R', 'ND_L', 'ND_R', 'EWD', 'SD', 'FCU', 'CLOCK'];

function parseArgs(argv) {
  const args = { state: 'all', screen: null, record: false, headed: false, settle: null, out: REFERENCE_PNG_DIR, verbose: false, dom: false, domEvery: 0, systemsHost: false, evaluate: null, jobs: 1, gpu: true, runtime: 'msfs', traffic: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = () => argv[++i];
    if (a === '--state') args.state = next();
    else if (a === '--screen') args.screen = next();
    else if (a === '--record') args.record = true;
    else if (a === '--dom') args.dom = true;
    else if (a === '--dom-every') { args.dom = true; args.domEvery = Number(next()); }
    else if (a === '--headed') args.headed = true;
    else if (a === '--settle') args.settle = Number(next());
    else if (a === '--out') args.out = next();
    else if (a === '--verbose' || a === '-v') args.verbose = true;
    else if (a === '--systems-host') args.systemsHost = true;
    else if (a === '--eval') args.evaluate = next();
    else if (a === '--jobs') args.jobs = Math.max(1, Number(next()));
    else if (a === '--cpu') args.gpu = false;
    else if (a === '--runtime') args.runtime = next();
    else if (a === '--traffic') args.traffic = true;
    else if (a === '--help' || a === '-h') {
      console.log('usage: npm run reference -- [--state <name|all|none>] [--screen <A,B|all>] [--record] [--dom] [--dom-every frames] [--settle ms] [--out dir] [--systems-host] [--jobs n] [--cpu] [--headed] [-v]');
      process.exit(0);
    } else throw new Error(`unknown argument ${a}`);
  }
  return args;
}

async function renderScreen({ browser, server, screen, state, args, hostScripts }) {
  const context = await browser.newContext({
    viewport: { width: screen.width, height: screen.height },
    deviceScaleFactor: 1,
    colorScheme: 'dark',
    reducedMotion: 'no-preference',
  });
  if (process.env.REFERENCE_BLOCK_HOSTS) {
    const blocked = new RegExp(process.env.REFERENCE_BLOCK_HOSTS);
    await context.route((url) => blocked.test(url.hostname), (route) => route.abort());
  }
  const page = await context.newPage();
  const consoleLines = [];
  page.on('console', (m) => consoleLines.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', (e) => consoleLines.push(`[pageerror] ${e.stack || e.message}`));

  const settleMs = args.settle ?? state.time.settleMs;
  const frameMs = state.time.frameMs;
  const start = state.time.epochMs - settleMs;

  // The virtual clock (host/freeze.js) starts settleMs before the state's UTC
  // time and does not move while the panel loads; then exactly settleMs of
  // frames run, ending at the state's time.
  const cfg = { screen, state: { vars: state.vars, storage: state.storage, time: state.time }, record: args.record, clockStart: start, names: hostScripts.names, traffic: args.traffic, trafficMap: hostScripts.trafficMap, trafficKeep: process.env.XPJS_TRAFFIC_KEEP ? process.env.XPJS_TRAFFIC_KEEP.split(',').map(Number) : undefined, dbgName: process.env.XPJS_DBG_NAME, setVars: process.env.XPJS_SET_VARS, pageScripts: process.env.XPJS_PAGE_SCRIPTS };
  await page.addInitScript({ content: `window.__REFERENCE__ = ${JSON.stringify(cfg)};` });
  await page.addInitScript({ content: hostScripts.freeze });
  if (args.record) await page.addInitScript({ content: hostScripts.recorder });

  if (args.traffic) await page.addInitScript({ content: hostScripts.traffic });
  const index = screen.gauges.map((g) => (g.url.match(/[?&]Index=(\d+)/i) || [])[1]).find(Boolean) || '1';
  await page.goto(args.runtime === 'xpjs' ? `${server.origin}/reference/host/xpjs.html?Index=${index}` : `${server.origin}/reference/host/vcockpit.html`);
  const loadDeadline = Date.now() + Number(process.env.REFERENCE_LOAD_TIMEOUT_MS || 120000);
  while (!(await page.evaluate(() => window.__refPanelLoaded === true))) {
    if (Date.now() > loadDeadline) {
      throw new Error(`panel did not load in ${process.env.REFERENCE_LOAD_TIMEOUT_MS || 120000} ms\n${consoleLines.slice(-25).join('\n')}`);
    }
    await page.evaluate(() => window.__refIdle(10));
  }
  // Fonts must be ready before the first frame measures text.
  await page.evaluate(() => document.fonts.ready.then(() => true));

  const frames = Math.round(settleMs / frameMs);
  const domSnaps = [];
  const snapDom = async (label) => domSnaps.push(`<!-- ${label} -->\n` + (await page.evaluate(() => new XMLSerializer().serializeToString(document.documentElement))));
  for (let f = 0; f < frames; f++) {
    await page.evaluate((ms) => window.__refStep(ms).then(() => window.__refNoteAnimations()), frameMs);
    if (args.domEvery && (f + 1) % args.domEvery === 0 && f + 1 < frames) await snapDom(`frame ${f + 1}`);
  }
  await page.evaluate(() => document.fonts.ready.then(() => true));
  const frozen = await page.evaluate(() => window.__refFreezeAnimations());
  if (args.evaluate) {
    const result = await page.evaluate((expr) => {
      try {
        return JSON.stringify((0, eval)(expr));
      } catch (e) {
        return `eval error: ${e}`;
      }
    }, args.evaluate);
    console.log(`    eval: ${result}`);
  }

  const dir = path.join(args.out, state.name);
  fs.mkdirSync(dir, { recursive: true });
  if (args.dom) {
    await snapDom(`frame ${frames}`);
    fs.writeFileSync(path.join(dir, `${screen.name}.dom.html`), domSnaps.join('\n'));
  }
  const png = path.join(dir, `${screen.name}.png`);
  await page.screenshot({ path: png, animations: 'allow', caret: 'hide', scale: 'css', timeout: 60000 });
  const mfdDom = await page.evaluate(() => window.__refMfdDom || null);
  if (mfdDom) fs.writeFileSync(path.join(dir, `${screen.name}.mfd-dom.html`), mfdDom);

  const access = await page.evaluate(() => window.__refAccessLog());
  access.animationsFrozen = frozen;
  access.animations = await page.evaluate(() => window.__refAnimationUsage());
  access.screen = { name: screen.name, width: screen.width, height: screen.height, gauges: screen.gauges, skippedGauges: screen.skippedGauges };
  access.clock = { start, end: state.time.epochMs, frames, frameMs, ...(await page.evaluate(() => window.__refClockStats())) };
  const stateKeys = new Set(Object.keys(state.vars).map((k) => k.toUpperCase()));
  access.missingInputs = access.reads
    .filter((r) => !r.found && !stateKeys.has(r.key.toUpperCase()))
    .map((r) => ({ key: r.key, units: r.units }))
    .sort((a, b) => a.key.localeCompare(b.key));
  fs.writeFileSync(path.join(dir, `${screen.name}.access.json`), JSON.stringify(access, null, 1));
  fs.writeFileSync(path.join(dir, `${screen.name}.console.log`), consoleLines.join('\n'));
  if (args.traffic) fs.writeFileSync(path.join(dir, `${screen.name}.traffic.json`), JSON.stringify(await page.evaluate(() => window.__refTraffic())));

  if (args.record) {
    const usage = await page.evaluate(() => window.__refCollectDomUsage());
    fs.mkdirSync(DOM_USAGE_DIR, { recursive: true });
    fs.writeFileSync(path.join(DOM_USAGE_DIR, `${state.name}.${screen.name}.json`), JSON.stringify(usage, null, 1));
  }

  await context.close();
  return { png, reads: access.reads.length, missing: access.missingInputs.length, errors: access.errors.length, console: consoleLines };
}

function readNames(file) {
  const names = { exact: {}, prefixes: [], wire: {} };
  if (!fs.existsSync(file)) return names;
  for (const line of fs.readFileSync(file, 'utf8').split(/\r?\n/)) {
    const tab = line.indexOf('\t');
    if (tab < 0) continue;
    const [head, internal] = [line.slice(0, tab), line.slice(tab + 1)];
    if (head[0] === '=') names.exact[head.slice(1)] = internal;
    else if (head[0] === '>') names.prefixes.push([head.slice(1), internal]);
    else if (head[0] === '@') names.wire[head.slice(1)] = internal;
  }
  names.prefixes.sort((a, b) => b[0].length - a[0].length);
  return names;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const { chromium } = await import('playwright');
  const { file: panelFile, screens } = loadScreens();
  const states = args.state === 'all' ? listStates() : args.state.split(',');
  const hostScripts = {
    freeze: fs.readFileSync(path.join(TOOL_ROOT, 'host/freeze.js'), 'utf8'),
    recorder: fs.readFileSync(path.join(TOOL_ROOT, 'host/recorder.js'), 'utf8'),
    traffic: fs.readFileSync(path.join(TOOL_ROOT, 'host/traffic.js'), 'utf8'),
    names: readNames(process.env.XPJS_NAMES || path.join(TOOL_ROOT, '../../src/xp_names.txt')),
    trafficMap: process.env.XPJS_TRAFFIC_MAP ? JSON.parse(fs.readFileSync(process.env.XPJS_TRAFFIC_MAP, 'utf8')) : undefined,
  };

  const server = await startServer({ log: (m) => args.verbose && console.log(`  [server] ${m}`) });
  console.log(`panel.cfg: ${panelFile}\nbundles:   ${server.bundles}\nserver:    ${server.origin}`);
  const browser = await chromium.launch({
    headless: !args.headed,
    args: [
      '--font-render-hinting=none',
      '--disable-lcd-text',
      '--force-color-profile=srgb',
      '--hide-scrollbars',
      ...(args.gpu ? ['--use-angle=d3d11', '--enable-gpu-rasterization', '--ignore-gpu-blocklist'] : ['--disable-gpu']),
    ],
  });

  let failures = 0;
  try {
    const tasks = [];
    for (const stateName of states) {
      const state = loadState(stateName);
      const wanted = (args.screen && args.screen !== 'all' ? args.screen.split(',') : state.screens ?? DEFAULT_SCREENS).map((s) => s.trim());
      const outDir = path.join(args.out, state.name);
      fs.mkdirSync(outDir, { recursive: true });
      fs.writeFileSync(path.join(outDir, 'state.resolved.json'), JSON.stringify({ format: 'fbw-reference-state-resolved/1', name: state.name, time: state.time, vars: state.vars, storage: state.storage, meta: state.meta, sources: state.sources }, null, 1));
      console.log(`state ${state.name}: ${Object.keys(state.vars).length} vars`);
      for (const name of wanted) tasks.push({ state, name });
    }
    const hostGauge = screens.find((s) => s.name === 'SYSTEMS_HOST')?.gauges[0];
    const renderOne = async ({ state, name }) => {
      const found = screens.find((s) => s.name === name);
      const screen = found && args.systemsHost && hostGauge ? { ...found, gauges: [...found.gauges, { ...hostGauge, x: 0, y: 0, w: 1, h: 1 }] } : found;
      if (!screen) {
        console.log(`  ${state.name}/${name}: not in panel.cfg`);
        failures++;
        return;
      }
      const t0 = Date.now();
      try {
        const r = await renderScreen({ browser, server, screen, state, args, hostScripts });
        console.log(`  ${state.name}/${name} ${screen.width}x${screen.height}: ${r.png} (${r.reads} vars read, ${r.missing} not in state, ${r.errors} errors, ${((Date.now() - t0) / 1000).toFixed(1)}s)`);
        if (args.verbose) r.console.filter((l) => /error|warn/i.test(l)).slice(0, 20).forEach((l) => console.log(`    ${l}`));
      } catch (e) {
        failures++;
        console.log(`  ${state.name}/${name}: FAILED ${e.stack || e}`);
      }
    };
    let next = 0;
    await Promise.all(
      Array.from({ length: Math.min(args.jobs, tasks.length) }, async () => {
        while (next < tasks.length) await renderOne(tasks[next++]);
      }),
    );
  } finally {
    await browser.close();
    await server.close();
  }
  if (server.misses.size) console.log(`\nfiles not found (${server.misses.size}): ${[...server.misses].slice(0, 20).join(', ')}`);
  process.exit(failures ? 1 : 0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
