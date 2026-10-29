// node compare/pfd-variants.mjs <new dir> <old dir>...
// For each render the old runs made: how many distinct images they produced, and whether the new render equals one of them.
import fs from 'node:fs';
import path from 'node:path';
import { PNG } from 'pngjs';

const [newDir, ...oldDirs] = process.argv.slice(2);
const pixels = (f) => (fs.existsSync(f) ? PNG.sync.read(fs.readFileSync(f)).data.toString('base64') : null);
let unmatched = 0;
let varying = 0;
for (const state of fs.readdirSync(oldDirs[0]).sort()) {
  if (!fs.statSync(path.join(oldDirs[0], state)).isDirectory()) continue;
  for (const f of fs.readdirSync(path.join(oldDirs[0], state)).filter((x) => x.endsWith('.png')).sort()) {
    const olds = oldDirs.map((d) => pixels(path.join(d, state, f))).filter(Boolean);
    const distinct = new Set(olds);
    const now = pixels(path.join(newDir, state, f));
    const match = now !== null && distinct.has(now);
    if (distinct.size > 1) varying++;
    if (!match) unmatched++;
    if (distinct.size > 1 || !match) console.log(`${state}/${f.slice(0, -4)}: old runs ${olds.length}, distinct ${distinct.size}, new ${now === null ? 'missing' : match ? 'equals an old image' : 'equals none'}`);
  }
}
console.log(`${varying} renders vary between old runs, ${unmatched} new renders equal no old image`);
