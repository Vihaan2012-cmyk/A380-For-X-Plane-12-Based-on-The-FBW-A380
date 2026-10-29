'use strict';
// node callsites.cjs <fork> <out.json>
// String arguments FlyByWire passes to the page globals, grouped by call.

const ts = require('typescript');
const path = require('path');
const fs = require('fs');
const { slash, entries, compilerOptions, cleanHost } = require('./lib/project.cjs');

const fork = slash(path.resolve(process.argv[2] || 'E:/fbw-int/fbw-own-api'));
const out = process.argv[3] || 'E:/fbw-int/xplane-js-private/callsites.json';
const libEntry = path.resolve(__dirname, '../lib/src/index.ts');
const libModules = { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry };
const TYPINGS = (fork + '/fbw-common/src/typings/').toLowerCase();
const lower = (f) => slash(f).toLowerCase();
const isFbw = (f) => lower(f).startsWith(fork.toLowerCase() + '/') && !lower(f).includes('/node_modules/') && !lower(f).startsWith(TYPINGS) && !lower(f).endsWith('.d.ts');
const rel = (f) => slash(f).slice(fork.length + 1);

const TARGETS = /^(SimVar\.\w+|Coherent\.(call|on|off|trigger)|RegisterViewListener|\w+\.(triggerToAllSubscribers|trigger|on|off|call)|GetStoredData|SetStoredData|DeleteStoredData|LaunchFlowEvent|Utils\.Translate|registerInstrument|Simplane\.\w+|Avionics\.Utils\.\w+|Facilities\.getMagVar)$/;

const calls = {};
const seen = new Set();

function argText(a, checker) {
  if (!a) return null;
  if (ts.isStringLiteral(a) || ts.isNoSubstitutionTemplateLiteral(a)) return { lit: a.text };
  if (ts.isTemplateExpression(a)) return { tpl: a.getText().slice(1, -1) };
  if (ts.isPropertyAccessExpression(a) || ts.isIdentifier(a)) {
    const t = checker.getTypeAtLocation(a);
    if (t.isStringLiteral()) return { lit: t.value, via: a.getText() };
    if (t.isUnion() && t.types.every((x) => x.isStringLiteral())) return { lits: t.types.map((x) => x.value), via: a.getText() };
  }
  return { expr: a.getText().slice(0, 80) };
}

function scan(program) {
  const checker = program.getTypeChecker();
  const globalDecl = (node) => {
    const s = checker.getSymbolAtLocation(node);
    return !!(s && s.declarations && s.declarations.some((d) => lower(d.getSourceFile().fileName).startsWith(TYPINGS)));
  };
  for (const sf of program.getSourceFiles()) {
    if (!isFbw(sf.fileName) || seen.has(lower(sf.fileName))) continue;
    seen.add(lower(sf.fileName));
    const visit = (node) => {
      if (ts.isCallExpression(node)) {
        const callee = node.expression;
        const name = callee.getText().replace(/\s+/g, '');
        const id = ts.isPropertyAccessExpression(callee) ? callee.name : callee;
        if (TARGETS.test(name) && globalDecl(id)) {
          const key = name.startsWith('SimVar.') || name.startsWith('Coherent.') || name.startsWith('Simplane.') || name.startsWith('Avionics.') ? name : /\.(\w+)$/.test(name) ? 'ViewListener.' + name.match(/\.(\w+)$/)[1] : name;
          const c = (calls[key] = calls[key] || { count: 0, args: {} });
          c.count++;
          const args = node.arguments.map((a) => argText(a, checker));
          const k = JSON.stringify(args.slice(0, key.startsWith('SimVar.') ? 2 : 1));
          const e = (c.args[k] = c.args[k] || { count: 0, sites: [] });
          e.count++;
          if (e.sites.length < 4) e.sites.push(`${rel(sf.fileName)}:${sf.getLineAndCharacterOfPosition(node.getStart()).line + 1}`);
        }
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
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(calls, null, 1) + '\n');
for (const [k, v] of Object.entries(calls).sort()) console.log(k, v.count, Object.keys(v.args).length);
