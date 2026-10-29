import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';

const FBW = 'E:/fbw-int/fbw-aircraft';
const OUT = join(dirname(new URL(import.meta.url).pathname.replace(/^\//, '')), '..', 'tests', 'fixtures', 'perf_oracle.json');
const BUNDLE = 'E:/fbw-build/oit-pages/oracle/perfmodel.mjs';

mkdirSync(dirname(BUNDLE), { recursive: true });
execFileSync(
  join(FBW, 'node_modules/.bin/esbuild.CMD'),
  [join(FBW, 'fbw-a380x/src/systems/instruments/src/OIT/System/PerfModel.ts'), '--bundle', '--format=esm', `--outfile=${BUNDLE}`, '--log-level=warning'],
  { shell: true, stdio: 'inherit' },
);
const pm = await import(pathToFileURL(BUNDLE).href);
const BUNDLE2 = 'E:/fbw-build/oit-pages/oracle/inflightperf.mjs';
execFileSync(
  join(FBW, 'node_modules/.bin/esbuild.CMD'),
  [join(FBW, 'fbw-a380x/src/systems/instruments/src/OIT/System/InFlightPerf.ts'), '--bundle', '--format=esm', `--outfile=${BUNDLE2}`, '--log-level=warning'],
  { shell: true, stdio: 'inherit' },
);
const ifp = await import(pathToFileURL(BUNDLE2).href);

const num = (v) => (v === Infinity ? 'inf' : v === -Infinity ? '-inf' : v);
const cal = (thrust, roll, brake) => ({ thrust, roll, brake, takeoffs: 0, landings: 0 });

const takeoffs = [
  { elev: 0, qnh: 1013, oat: 15, tow: 450, cg: 33, conf: 'AUTO', flex: true, ai: 'OFF', packs: true, slope: 0, cond: 'DRY', toda: 3900, asda: 3900, angle: '0', hw: 0, cal: cal(1, 1, 1) },
  { elev: 0, qnh: 1013, oat: 15, tow: 500, cg: 36, conf: 'AUTO', flex: true, ai: 'OFF', packs: true, slope: 0, cond: 'DRY', toda: 3300, asda: 3300, angle: '90', hw: 5, cal: cal(1, 1, 1) },
  { elev: 1000, qnh: 1005, oat: 32, tow: 480, cg: 31, conf: '2', flex: true, ai: 'ENG', packs: false, slope: 0.4, cond: 'DRY', toda: 4000, asda: 4000, angle: '180', hw: -8, cal: cal(1, 1, 1) },
  { elev: 13, qnh: 1020, oat: 5, tow: 420, cg: 29.5, conf: '3', flex: false, ai: 'ENG+WING', packs: true, slope: -0.3, cond: 'WET', toda: 3600, asda: 3500, angle: '90', hw: 12, cal: cal(0.97, 1.05, 0.92) },
  { elev: 0, qnh: 1013, oat: 25, tow: 510, cg: 38, conf: '1+F', flex: false, ai: 'OFF', packs: true, slope: 0, cond: 'WET', toda: 2800, asda: 2800, angle: '0', hw: 0, cal: cal(1, 1, 1) },
  { elev: 5000, qnh: 1010, oat: 28, tow: 480, cg: 34, conf: 'AUTO', flex: true, ai: 'OFF', packs: true, slope: 0, cond: 'DRY', toda: 4200, asda: 4200, angle: '0', hw: 3, cal: cal(1.02, 0.95, 1.1) },
  { elev: 90, qnh: 995, oat: -5, tow: 380, cg: 44, conf: 'AUTO', flex: true, ai: 'ENG', packs: true, slope: 0.1, cond: 'DRY', toda: 3000, asda: 3200, angle: '90', hw: 20, cal: cal(1, 1, 1) },
  { elev: 0, qnh: 1013, oat: 15, tow: 300, cg: 26, conf: 'AUTO', flex: true, ai: 'OFF', packs: false, slope: 0, cond: 'DRY', toda: 4500, asda: 4500, angle: '0', hw: 0, cal: cal(1, 1, 1) },
];

const toResults = takeoffs.map((s) => {
  const oat = s.oat;
  const input = {
    atm: pm.atmos(s.elev, s.qnh, oat),
    oat,
    towT: s.tow,
    cg: s.cg,
    conf: s.conf,
    flex: s.flex,
    deratePct: 0,
    ratingKn: 356.81,
    packs: s.packs,
    ai: s.ai,
    slope: s.slope,
    cond: pm.TO_COND[s.cond],
    toda: s.toda,
    asda: s.asda,
    lineup: pm.LINEUP[s.angle],
    hw: s.hw,
    cal: s.cal,
  };
  const r = pm.takeoff(input);
  return {
    input: s,
    out: {
      ok: r.ok,
      limits: r.limits,
      v1: r.v1,
      vr: r.vr,
      v2: r.v2,
      conf: r.conf,
      flex: r.flex,
      todAeo: num(r.todAeo),
      todOei: num(r.todOei),
      asd: num(r.asd),
      toda: r.toda,
      asda: r.asda,
      grad2: r.grad2,
      trim: r.trim,
      limitTowT: r.limitTowT,
    },
  };
});

const landings = [
  { elev: 0, qnh: 1013, oat: 15, lw: 380, cg: 33, conf: 'FULL', mu: 0.38, rev: true, autoland: false, slope: 0, hw: 0, lda: 3500, cal: cal(1, 1, 1) },
  { elev: 600, qnh: 1000, oat: 30, lw: 410, cg: 38, conf: '3', mu: 0.28, rev: false, autoland: false, slope: -0.5, hw: 12, lda: 3000, cal: cal(1, 1, 1) },
  { elev: 1500, qnh: 1018, oat: 5, lw: 300, cg: 30, conf: 'FULL', mu: 0.17, rev: true, autoland: true, slope: 0.3, hw: -6, lda: 2600, cal: cal(1, 1, 0.9) },
  { elev: 0, qnh: 1013, oat: 15, lw: 340, cg: 41, conf: 'FULL', mu: 0.38, rev: true, autoland: false, slope: 0, hw: 25, lda: 3000, cal: cal(1, 1, 1.1) },
];
const modes = ['LO', '2', '3', 'HI', 'MAX'];
const ldgResults = landings.map((s) => {
  const r = pm.landing(
    { atm: pm.atmos(s.elev, s.qnh, s.oat), lwT: s.lw, cg: s.cg, conf: s.conf, mu: s.mu, rev: s.rev, autoland: s.autoland, slope: s.slope, hw: s.hw, lda: s.lda, cal: s.cal },
    modes,
  );
  return { input: s, modes, out: { vls: r.vls, vapp: r.vapp, rows: r.rows }, xwind: pm.crosswindLimitKt(s.mu, s.autoland) };
});

const vlsRows = [];
for (const conf of ['1+F', '2', '3', 'FULL']) {
  for (const tons of [250, 300, 350, 420, 480, 540, 600, 650]) {
    for (const cg of [25, 29, 36, 43, 48]) {
      vlsRows.push({ conf, tons, cg, vls: pm.vls(conf, tons, cg) });
    }
  }
}
const trims = [20, 29, 31, 34.5, 38, 43, 50].map((cg) => ({ cg, trim: pm.trimFor(cg) }));
const atm = [
  [0, 1013, 15],
  [1000, 1000, 30],
  [8000, 1040, -10],
  [-100, 990, 40],
].map(([e, q, t]) => ({ elev: e, qnh: q, oat: t, atm: pm.atmos(e, q, t) }));

const inflight = [];
for (const w of [300000, 380000, 450000, 510000]) {
  for (const [fl, mach, isa, eng, ff] of [[350, 0.85, 0, 4, 1], [390, 0.84, 10, 4, 1.05], [250, 0.8, -8, 3, 1], [200, 0.78, 15, 2, 0.95]]) {
    inflight.push({
      w, fl, mach, isa, eng, ff,
      cruise: ifp.cruise(w, fl, mach, isa, ff, eng),
      recMax: ifp.recMaxFl(w, mach, isa),
      opt: ifp.optimumFl(w, mach, isa),
      eoCeil: ifp.engineOutCeilingFl(w, eng, isa),
      hold: ifp.holding(w, fl, isa, eng),
      gd: ifp.greenDotKcas(w, fl),
      des: ifp.descent(fl, 600, w, 15),
      plan: ifp.plan(w, 90000, 1800, fl, mach, isa, -20, ff),
    });
  }
}

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify({ takeoff: toResults, landing: ldgResults, vls: vlsRows, trim: trims, atmos: atm, inflight }, null, 1));
console.log('wrote', OUT, toResults.length, 'takeoff', ldgResults.length, 'landing', vlsRows.length, 'vls');
