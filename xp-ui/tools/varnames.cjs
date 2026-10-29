'use strict';
// node varnames.cjs <fork> <plugin> <out.json>
// Every MSFS simulator-variable name string in FlyByWire's A380X program.

const ts = require('typescript');
const path = require('path');
const fs = require('fs');
const { slash, entries, compilerOptions, cleanHost } = require('./lib/project.cjs');

const fork = slash(path.resolve(process.argv[2] || 'E:/fbw-int/fbw-own-api'));
const plugin = slash(path.resolve(process.argv[3] || 'E:/fbw-int/plugin'));
const out = process.argv[4] || 'E:/fbw-int/xplane-js-private/varnames.json';
const libEntry = path.resolve(__dirname, '../lib/src/index.ts');
const libModules = { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry };
const TYPINGS = (fork + '/fbw-common/src/typings/').toLowerCase();
const lower = (f) => slash(f).toLowerCase();
const isFbw = (f) => lower(f).startsWith(fork.toLowerCase() + '/') && !lower(f).includes('/node_modules/') && !lower(f).startsWith(TYPINGS) && !lower(f).endsWith('.d.ts');
const rel = (f) => slash(f).slice(fork.length + 1);

const libRs = fs.readFileSync(path.join(plugin, 'src/lib.rs'), 'utf8');
const mapBlock = libRs.slice(libRs.indexOf('fn mapping(name: &str)'));
const mapped = new Map();
for (const m of mapBlock.slice(0, mapBlock.indexOf('\n    })\n')).matchAll(/^\s*"([^"]+)" => \(?\s*"([^"]+)"/gm)) mapped.set(m[1], m[2]);
const wasm = path.join(plugin, '../fbw-aircraft/fbw-a380x/src/wasm/systems/a380_systems_wasm/src/lib.rs');
const systems = new Set();
if (fs.existsSync(wasm)) for (const m of fs.readFileSync(wasm, 'utf8').matchAll(/provides_aircraft_variable\(\s*"([^"]+)"/g)) systems.add(m[1]);

const base = (s) => s.replace(/^(A|E):/i, '').replace(/:(\d+|#index#)?$/, '');
const looksSim = (s) => /^(A:|E:)?[A-Z][A-Z0-9 _]*[A-Z0-9](:(\d+|#index#)?)?$/.test(s) && /\s/.test(s.replace(/^(A|E):/, ''));

const dict = new Set([...mapped.keys(), ...systems].map(base));
const strings = [];
const seen = new Set();

function scan(program) {
  const checker = program.getTypeChecker();
  const TARGET = /^SimVar\.(GetSimVarValue|SetSimVarValue|GetRegisteredId|GetSimVarValueFast|GetGlobalVarValue|GetGameVarValue)$/;
  for (const sf of program.getSourceFiles()) {
    if (!isFbw(sf.fileName) || seen.has(lower(sf.fileName))) continue;
    seen.add(lower(sf.fileName));
    const visit = (node) => {
      if (ts.isCallExpression(node) && TARGET.test(node.expression.getText()) && node.arguments[0]) {
        const a = node.arguments[0];
        if (ts.isStringLiteral(a) || ts.isNoSubstitutionTemplateLiteral(a)) {
          if (!/^(L|K|H|B|O|Z):/i.test(a.text)) dict.add(base(a.text));
        } else if (ts.isTemplateExpression(a) && !/^(L|K|H|B|O|Z):/i.test(a.head.text)) dict.add(base(a.head.text));
      }
      if (ts.isPropertyAssignment(node) && ts.isIdentifier(node.name) && node.name.text === 'name' && (ts.isStringLiteral(node.initializer) || ts.isNoSubstitutionTemplateLiteral(node.initializer))) {
        const t = node.initializer.text;
        if (!/^(L|K|H|B|O|Z):/i.test(t) && looksSim(t)) dict.add(base(t));
      }
      if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) || ts.isTemplateHead(node) || ts.isTemplateMiddle(node) || ts.isTemplateTail(node)) {
        const kind = ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) ? 'literal' : ts.SyntaxKind[node.kind];
        const p = node.parent;
        const ctx = ts.isCallExpression(p) ? p.expression.getText().slice(0, 60) : ts.isPropertyAssignment(p) ? 'prop:' + p.name.getText() : ts.isTemplateSpan(p) || ts.isTemplateExpression(p) ? 'template' : ts.SyntaxKind[p.kind];
        strings.push({ text: node.text, kind, ctx, site: `${rel(sf.fileName)}:${sf.getLineAndCharacterOfPosition(node.getStart()).line + 1}` });
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
}

const roots = entries(fork);
for (const [list, factory] of [[roots.fs, ['FSComponent.buildComponent', 'FSComponent.Fragment']], [roots.react, 'react']]) {
  const options = compilerOptions(ts, { fork, modulesDir: path.join(fork, 'node_modules'), libModules, jsx: factory });
  scan(ts.createProgram(list.map((e) => e.file), options, cleanHost(ts, options)));
}

const matches = {};
for (const s of strings) {
  const t = s.text;
  let hit = null;
  if (dict.has(base(t)) && (looksSim(t) || mapped.has(base(t)))) hit = 'exact';
  else if (/^(A:|E:)?[A-Z][A-Z0-9 _]*[ :]$/.test(t) && [...dict].some((d) => (d + ':').startsWith(t.replace(/^(A|E):/i, '')) || (d + ' ').startsWith(t.replace(/^(A|E):/i, '')))) hit = 'prefix';
  if (!hit) continue;
  const m = (matches[t] = matches[t] || { hit, count: 0, kinds: {}, ctx: {}, sites: [] });
  m.count++;
  m.kinds[s.kind] = (m.kinds[s.kind] || 0) + 1;
  m.ctx[s.ctx] = (m.ctx[s.ctx] || 0) + 1;
  if (m.sites.length < 6) m.sites.push(s.site);
}
const byBase = {};
for (const t of Object.keys(matches)) (byBase[base(t)] = byBase[base(t)] || []).push(t);
const spellings = Object.fromEntries(Object.entries(byBase).filter(([, v]) => v.length > 1));
fs.writeFileSync(out, JSON.stringify({ mapped: Object.fromEntries(mapped), systems: [...systems].sort(), dictionary: [...dict].sort(), matches, multiSpelling: spellings }, null, 1) + '\n');
console.log(`mapped ${mapped.size}, systems ${systems.size}, dictionary ${dict.size}, matched strings ${Object.keys(matches).length}, occurrences ${Object.values(matches).reduce((a, m) => a + m.count, 0)}, bases with several spellings ${Object.keys(spellings).length}`);
