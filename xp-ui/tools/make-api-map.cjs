'use strict';
// node make-api-map.cjs <names.json> <out api-map.json> [fork]

const ts = require('typescript');
const path = require('path');
const fs = require('fs');
const { slash, entries, compilerOptions, cleanHost } = require('./lib/project.cjs');

const PRIVATE = 'E:/fbw-int/xplane-js-private';
const namesFile = process.argv[2] || path.join(PRIVATE, 'names.json');
const out = process.argv[3] || path.join(PRIVATE, 'api-map.json');
const fork = slash(path.resolve(process.argv[4] || 'E:/fbw-int/fbw-own-api'));
const LIB = slash(path.resolve(process.env.OLD_LIB || path.resolve(__dirname, '../lib/src')));
const libEntry = path.join(LIB, 'index.ts');
const names = JSON.parse(fs.readFileSync(namesFile, 'utf8'));
const TYPINGS = fork + '/fbw-common/src/typings/';
const lower = (f) => slash(f).toLowerCase();
const isFbw = (f) => lower(f).startsWith(fork.toLowerCase() + '/') && !lower(f).includes('/node_modules/') && !lower(f).startsWith(TYPINGS.toLowerCase()) && !lower(f).endsWith('.d.ts');

function declText(d) {
  let n = d;
  if (ts.isVariableDeclaration(n) && n.parent && n.parent.parent) n = n.parent.parent;
  let text = n.getText();
  if (ts.isClassDeclaration(n) || ts.isInterfaceDeclaration(n) || ts.isModuleDeclaration(n)) text = text.split('{')[0];
  else if (ts.isEnumDeclaration(n)) text = text;
  else if (ts.isFunctionLike(n) && n.body) text = text.slice(0, n.body.getStart() - n.getStart());
  else if ((ts.isPropertyDeclaration(n) || ts.isVariableStatement(n)) && n.initializer) text = text.slice(0, n.initializer.getStart() - n.getStart());
  return text.replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, '').replace(/\s+/g, ' ').trim().slice(0, 300);
}
const where = (d) => {
  const f = slash(d.getSourceFile().fileName);
  return f.startsWith(LIB) ? 'clean-avionics/lib/src' + f.slice(LIB.length) : f.startsWith(fork) ? f.slice(fork.length + 1) : f;
};

const options = compilerOptions(ts, { fork, modulesDir: path.join(fork, 'node_modules'), libModules: { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry }, jsx: ['FSComponent.buildComponent', 'FSComponent.Fragment'] });
const roots = entries(fork);
const program = ts.createProgram([libEntry, ...roots.fs.map((e) => e.file)], options, cleanHost(ts, options));
const reactOptions = compilerOptions(ts, { fork, modulesDir: path.join(fork, 'node_modules'), libModules: { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry }, jsx: 'react' });
const reactProgram = ts.createProgram(roots.react.map((e) => e.file), reactOptions, cleanHost(ts, reactOptions));
const checker = program.getTypeChecker();
const alias = (s) => (s && s.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(s) : s);
const libModule = checker.getSymbolAtLocation(program.getSourceFile(libEntry));
const libExports = new Map(checker.getExportsOfModule(libModule).map((s) => [s.name, s]));

const globalSymbols = new Map();
for (const sf of program.getSourceFiles()) {
  if (!lower(sf.fileName).startsWith(TYPINGS.toLowerCase())) continue;
  const visitDecl = (node) => {
    if ((ts.isClassDeclaration(node) || ts.isInterfaceDeclaration(node) || ts.isModuleDeclaration(node) || ts.isEnumDeclaration(node) || ts.isFunctionDeclaration(node) || ts.isTypeAliasDeclaration(node)) && node.name && ts.isIdentifier(node.name)) {
      const parentIsGlobal = ts.isSourceFile(node.parent) || (ts.isModuleBlock(node.parent) && ts.isModuleDeclaration(node.parent.parent) && node.parent.parent.name.text === 'global');
      if (parentIsGlobal && !globalSymbols.has(node.name.text)) globalSymbols.set(node.name.text, checker.getSymbolAtLocation(node.name));
    }
    if (ts.isVariableStatement(node)) {
      for (const d of node.declarationList.declarations) if (ts.isIdentifier(d.name) && !globalSymbols.has(d.name.text)) globalSymbols.set(d.name.text, checker.getSymbolAtLocation(d.name));
    }
    if (ts.isModuleDeclaration(node) && node.name.text === 'global' && node.body) ts.forEachChild(node.body, visitDecl);
  };
  ts.forEachChild(sf, visitDecl);
}

function member(sym, dotted) {
  let cur = sym;
  for (const part of dotted.split('.')) {
    const s = alias(cur);
    let next = (s.members && s.members.get(part)) || (s.exports && s.exports.get(part));
    if (!next && s.declarations) {
      const t = checker.getDeclaredTypeOfSymbol(s);
      next = t && t.getProperty && t.getProperty(part);
      if (!next) {
        const st = checker.getTypeOfSymbolAtLocation(s, s.declarations[0]);
        next = st && st.getProperty && st.getProperty(part);
      }
    }
    if (!next) return null;
    cur = next;
  }
  return cur;
}

const problems = [];
const entriesOut = [];
function addEntry(half, oldName, spec, sym, aliasOf) {
  const [newName, members] = Array.isArray(spec) ? spec : [spec, {}];
  if (!sym) {
    problems.push(`${half}: ${oldName} not found`);
    return;
  }
  const s = alias(sym);
  const d = s.declarations && s.declarations[0];
  const e = { half, old: oldName, new: newName, kind: d ? ts.SyntaxKind[d.kind].replace(/Declaration$/, '').toLowerCase() : 'unknown', declaration: aliasOf ? `alias of ${aliasOf}` : d ? declText(d) : null, declaredIn: d ? where(d) : null, members: [] };
  for (const [m, nm] of Object.entries(members)) {
    const ms = member(sym, m);
    if (!ms) {
      problems.push(`${half}: ${oldName}.${m} not found`);
      continue;
    }
    const md = ms.declarations && ms.declarations[0];
    e.members.push({ old: m, new: nm, kind: md ? ts.SyntaxKind[md.kind].replace(/Declaration$/, '').toLowerCase() : 'unknown', declaration: md ? declText(md) : null, declaredIn: md ? where(md) : null });
  }
  entriesOut.push(e);
}

for (const [oldName, spec] of Object.entries(names.ui)) addEntry('ui', oldName, spec, libExports.get(oldName));
for (const [oldName, spec] of Object.entries(names.uiAliases)) {
  const s = libExports.get(oldName);
  addEntry('ui-alias', oldName, spec, s, s ? alias(s).name : null);
}
for (const [oldName, spec] of Object.entries(names.data)) addEntry('data', oldName, spec, globalSymbols.get(oldName));

const fbwIdentifiers = new Map();
for (const sf of [...program.getSourceFiles(), ...reactProgram.getSourceFiles()]) {
  if (!isFbw(sf.fileName)) continue;
  const visit = (n) => {
    if (ts.isIdentifier(n)) {
      const t = n.text;
      if (!fbwIdentifiers.has(t)) fbwIdentifiers.set(t, slash(sf.fileName).slice(fork.length + 1) + ':' + (sf.getLineAndCharacterOfPosition(n.getStart()).line + 1));
    }
    ts.forEachChild(n, visit);
  };
  visit(sf);
}
const newTop = new Map();
for (const e of entriesOut) {
  if (newTop.has(e.new)) problems.push(`two old names map to ${e.new}: ${newTop.get(e.new)} and ${e.old}`);
  newTop.set(e.new, e.old);
  if (fbwIdentifiers.has(e.new)) problems.push(`new name ${e.new} (for ${e.old}) already used in FlyByWire: ${fbwIdentifiers.get(e.new)}`);
  if (libExports.has(e.new) && e.new !== e.old) problems.push(`new name ${e.new} (for ${e.old}) is an existing library export`);
  if (globalSymbols.has(e.new) && e.new !== e.old) problems.push(`new name ${e.new} (for ${e.old}) is an existing page global`);
  const seen = new Map();
  for (const m of e.members) {
    if (seen.has(m.new)) problems.push(`${e.old}: two members map to ${m.new}`);
    seen.set(m.new, m.old);
  }
}

const map = {
  format: 'api-map/1',
  note: 'Internal working file. Never ship, never publish.',
  modules: Object.entries(names.modules.from).map(([o, half]) => ({ old: o, new: names.modules[half], reverse: o === 'xp-kit' ? 'when every named import is a ui-alias' : o === 'xp-ui' ? 'library self-import' : 'default' })),
  dataModule: names.modules.data,
  jsx: Object.entries(names.jsx).map(([o, n]) => ({ old: o, new: n })),
  files: Object.entries(names.files).map(([o, n]) => ({ old: o, new: n })),
  symbols: entriesOut,
  strings: [],
  wire: Object.entries(names.wire || {}).map(([o, n]) => ({ old: o, new: n })),
  untyped: Object.entries(names.untyped || {}).map(([o, n]) => ({ old: o, new: n })),
  libraryText: names.libraryText || [],
  fbwText: names.fbwText || [],
  structural: names.fbwStructural || [],
};
fs.writeFileSync(out, JSON.stringify(map, null, 1) + '\n');
console.log(`${entriesOut.length} symbols, ${entriesOut.reduce((a, e) => a + e.members.length, 0)} members`);
if (problems.length) {
  console.log(`${problems.length} problems:`);
  for (const p of problems) console.log('  ' + p);
  process.exitCode = 1;
}
