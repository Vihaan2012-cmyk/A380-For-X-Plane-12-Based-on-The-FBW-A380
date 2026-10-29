// usage: node diff.cjs results-old.json results-new.json
'use strict';

const [a, b] = process.argv.slice(2).map((f) => require(require('path').resolve(f)));
if (a.loadError || b.loadError) console.log(`load errors: old=${a.loadError} new=${b.loadError}`);
const ra = a.results || {};
const rb = b.results || {};
const show = (v) => {
  const s = JSON.stringify(v);
  return s === undefined ? 'missing' : s.length > 300 ? `${s.slice(0, 300)}...` : s;
};
const diffs = [];
const walk = (x, y, p) => {
  if (JSON.stringify(x) === JSON.stringify(y)) return;
  const objects = x && y && typeof x === 'object' && typeof y === 'object' && Array.isArray(x) === Array.isArray(y);
  if (objects && !Array.isArray(x)) {
    for (const k of new Set([...Object.keys(x), ...Object.keys(y)])) walk(x[k], y[k], `${p}.${k}`);
    return;
  }
  diffs.push(`${p}\n    old ${show(x)}\n    new ${show(y)}`);
};
const ids = Array.from(new Set([...Object.keys(ra), ...Object.keys(rb)]));
for (const id of ids) walk(ra[id], rb[id], id);
const differing = new Set(diffs.map((d) => d.split(/[.\n]/)[0]));
console.log(`${ids.length} items, ${differing.size} differ: ${Array.from(differing).join(' ')}`);
console.log(diffs.join('\n'));
