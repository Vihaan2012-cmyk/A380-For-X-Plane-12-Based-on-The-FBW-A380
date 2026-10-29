// node compare/xpjs-compare.mjs <old dir> <new dir> <report.json>
// Every render in both runs: identical pixels, and identical variable traffic frame by frame.
import fs from 'node:fs';
import path from 'node:path';
import { PNG } from 'pngjs';

const [oldDir, newDir, out] = process.argv.slice(2);
const rows = [];
for (const state of fs.readdirSync(oldDir).sort()) {
  const od = path.join(oldDir, state);
  if (!fs.statSync(od).isDirectory()) continue;
  for (const f of fs.readdirSync(od).filter((x) => x.endsWith('.png')).sort()) {
    const screen = f.slice(0, -4);
    const row = { state, screen };
    const np = path.join(newDir, state, f);
    if (!fs.existsSync(np)) {
      row.missing = true;
      rows.push(row);
      continue;
    }
    const a = PNG.sync.read(fs.readFileSync(path.join(od, f)));
    const b = PNG.sync.read(fs.readFileSync(np));
    if (a.width !== b.width || a.height !== b.height) row.pixels = -1;
    else {
      let diff = 0;
      let maxDelta = 0;
      for (let i = 0; i < a.data.length; i += 4) {
        const d = Math.max(Math.abs(a.data[i] - b.data[i]), Math.abs(a.data[i + 1] - b.data[i + 1]), Math.abs(a.data[i + 2] - b.data[i + 2]), Math.abs(a.data[i + 3] - b.data[i + 3]));
        if (d) {
          diff++;
          maxDelta = Math.max(maxDelta, d);
        }
      }
      row.pixels = diff;
      row.maxDelta = maxDelta;
    }
    const ot = path.join(od, `${screen}.traffic.json`);
    const nt = path.join(newDir, state, `${screen}.traffic.json`);
    if (fs.existsSync(ot) && fs.existsSync(nt)) {
      const o = JSON.parse(fs.readFileSync(ot, 'utf8')).frames;
      const n = JSON.parse(fs.readFileSync(nt, 'utf8')).frames;
      let same = 0;
      let first = -1;
      for (let i = 0; i < Math.max(o.length, n.length); i++) {
        if (o[i] && n[i] && o[i].hash === n[i].hash && o[i].n === n[i].n) same++;
        else if (first < 0) first = i;
      }
      row.frames = Math.max(o.length, n.length);
      row.trafficSameFrames = same;
      row.trafficFirstDiff = first;
      row.trafficEntries = o.reduce((s, x) => s + x.n, 0);
    }
    rows.push(row);
  }
}
const identical = rows.filter((r) => r.pixels === 0).length;
const trafficIdentical = rows.filter((r) => r.trafficFirstDiff === -1).length;
const report = { renders: rows.length, pixelIdentical: identical, trafficIdentical, differing: rows.filter((r) => r.pixels !== 0 || r.trafficFirstDiff !== -1), rows };
fs.writeFileSync(out, JSON.stringify(report, null, 1));
console.log(`${rows.length} renders: ${identical} pixel-identical, ${trafficIdentical} with identical traffic in every frame`);
for (const r of report.differing.slice(0, 60)) console.log(`  ${r.state}/${r.screen}: ${r.missing ? 'MISSING' : `${r.pixels} px (max ${r.maxDelta})`}, traffic ${r.trafficSameFrames ?? '-'}/${r.frames ?? '-'} frames, first diff ${r.trafficFirstDiff ?? '-'}`);
