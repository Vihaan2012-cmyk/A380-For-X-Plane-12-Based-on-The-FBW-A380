// Copyright (c) 2026 FlyByWire Simulations
// SPDX-License-Identifier: GPL-3.0
//
// node xplane/make-lock.cjs <reference node_modules>
// Writes xplane/package-lock.json with exactly the package versions and layout of a reference
// install, for the packages the X-Plane build depends on, and no @microsoft package.

'use strict';

const fs = require('fs');
const path = require('path');

const here = __dirname;
const ref = path.resolve(process.argv[2] || 'E:/fbw-int/fbw-aircraft-ts/node_modules');
const root = path.dirname(ref);
const pkg = JSON.parse(fs.readFileSync(path.join(here, 'package.json'), 'utf8'));
const read = (dir) => {
  try {
    return JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
  } catch {
    return null;
  }
};

function resolveFrom(dir, name) {
  for (let d = dir; d.startsWith(root); d = path.dirname(d)) {
    const candidate = path.join(d, 'node_modules', name);
    if (fs.existsSync(path.join(candidate, 'package.json'))) return candidate;
    if (d === root) break;
  }
  return null;
}

const packages = {};
const queue = Object.keys(pkg.dependencies).map((n) => [root, n, false]);
const seen = new Set();
while (queue.length) {
  const [from, name, optional] = queue.shift();
  if (name.startsWith('@microsoft/')) continue;
  const dir = resolveFrom(from, name);
  if (!dir) {
    if (!optional) console.error(`missing: ${name} (from ${path.relative(root, from) || '.'})`);
    continue;
  }
  const key = path.relative(root, dir).split(path.sep).join('/');
  if (seen.has(key)) continue;
  seen.add(key);
  const p = read(dir);
  const entry = { version: p.version, resolved: `https://registry.npmjs.org/${p.name}/-/${p.name.split('/').pop()}-${p.version}.tgz` };
  for (const field of ['dependencies', 'optionalDependencies', 'peerDependencies']) {
    if (p[field] && Object.keys(p[field]).length) entry[field] = p[field];
  }
  if (p.engines) entry.engines = p.engines;
  if (p.os) entry.os = p.os;
  if (p.cpu) entry.cpu = p.cpu;
  if (p.bin) entry.bin = typeof p.bin === 'string' ? { [p.name.split('/').pop()]: p.bin } : p.bin;
  if (p.license) entry.license = p.license;
  packages[key] = entry;
  for (const n of Object.keys(p.dependencies || {})) queue.push([dir, n, false]);
  for (const n of Object.keys(p.optionalDependencies || {})) queue.push([dir, n, true]);
  for (const n of Object.keys(p.peerDependencies || {})) {
    const meta = p.peerDependenciesMeta && p.peerDependenciesMeta[n];
    queue.push([dir, n, !!(meta && meta.optional)]);
  }
}

const sorted = Object.fromEntries(Object.entries(packages).sort(([a], [b]) => a.localeCompare(b)));
const lock = {
  name: pkg.name,
  lockfileVersion: 3,
  requires: true,
  packages: { '': { name: pkg.name, license: pkg.license, dependencies: pkg.dependencies }, ...sorted },
};
fs.writeFileSync(path.join(here, 'package-lock.json'), JSON.stringify(lock, null, 2) + '\n');
console.log(`${Object.keys(sorted).length} packages from ${ref}`);
