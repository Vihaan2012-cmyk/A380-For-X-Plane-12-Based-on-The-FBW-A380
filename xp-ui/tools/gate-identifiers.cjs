'use strict';
// node gate-identifiers.cjs <html_ui or bundle dir> <report.json> [--map api-map.json] [--fbw converted-fork] [--extra file.js ...]
// Fails when a shipped bundle contains an old API name, an old wire name or an MSFS variable name,
// or when the library's own code or file paths, or an --extra script, carry an old API name in a string or a path.

const ts = require('typescript');
const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
const opt = (n, d) => (args.includes('--' + n) ? args[args.indexOf('--' + n) + 1] : d);
const root = args[0];
const out = args[1];
const map = JSON.parse(fs.readFileSync(opt('map', 'E:/fbw-int/xplane-js-private/api-map.json'), 'utf8'));
const fbw = opt('fbw', 'E:/fbw-int/fbw-own-api');
const extras = [];
for (let i = 0; i < args.length; i++) if (args[i] === '--extra') extras.push(args[i + 1]);

const walk = (d) => fs.readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(path.join(d, e.name)) : [path.join(d, e.name)]));
const GENERIC = new Set(['get', 'set', 'map', 'sub', 'pub', 'on', 'create', 'handle', 'notify', 'pause', 'resume', 'destroy', 'register', 'items', 'every', 'init', 'next', 'last', 'value', 'free', 'clear', 'resolve', 'theta', 'abs', 'instance', 'schedule', 'number', 'reader', 'render', 'remove', 'Fragment', 'mounted', 'publish']);

const ids = new Map();
const strs = new Map();
const add = (m, k, kind) => {
  if (!m.has(k)) m.set(k, kind);
};
for (const s of map.symbols) {
  add(ids, s.old, `${s.half} symbol`);
  for (const m of s.members) {
    const o = m.old.split('.').pop();
    if (o !== m.new && !GENERIC.has(o)) add(ids, o, `${s.half} member of ${s.old}`);
  }
}
for (const u of map.untyped || []) add(ids, u.old, 'page global');
for (const s of map.symbols) {
  ids.delete(s.new);
  for (const m of s.members) ids.delete(m.new);
}
for (const w of map.wire) add(strs, w.old, 'wire name');
for (const s of map.strings) {
  if (s.piece === 'full' && s.old !== s.new) add(strs, s.old, 'MSFS variable name');
  if (s.piece === 'head' && s.old.length > 3) add(strs, s.old, 'MSFS variable name prefix');
}

const oldApi = new Set(map.symbols.map((s) => s.old).filter((o) => o.length >= 4));
for (const s of map.symbols) oldApi.delete(s.new);
const oldApiLower = new Set([...oldApi].map((o) => o.toLowerCase()));

const fbwOwn = new Set();
const declares = (n) =>
  ts.isMethodDeclaration(n) || ts.isMethodSignature(n) || ts.isPropertyDeclaration(n) || ts.isPropertySignature(n) || ts.isGetAccessor(n) || ts.isSetAccessor(n) || ts.isFunctionDeclaration(n) || ts.isClassDeclaration(n) || ts.isInterfaceDeclaration(n) || ts.isTypeAliasDeclaration(n) || ts.isEnumDeclaration(n) || ts.isEnumMember(n) || ts.isVariableDeclaration(n) || ts.isParameter(n) || ts.isPropertyAssignment(n) || ts.isShorthandPropertyAssignment(n);
for (const f of ['fbw-a380x/src', 'fbw-common/src', 'fbw-a32nx/src'].flatMap((d) => walk(path.join(fbw, d)))) {
  if (!/\.(ts|tsx)$/.test(f) || f.includes(`${path.sep}typings${path.sep}`)) continue;
  const sf = ts.createSourceFile(f, fs.readFileSync(f, 'utf8'), ts.ScriptTarget.Latest, false, f.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const visit = (n) => {
    if (declares(n) && n.name && ts.isIdentifier(n.name) && ids.has(n.name.text)) fbwOwn.add(n.name.text);
    ts.forEachChild(n, visit);
  };
  visit(sf);
}

const files = [...(fs.existsSync(root) ? walk(root) : []), ...extras].filter((f) => f.endsWith('.js'));
const findings = [];
const allowed = {};
const thirdParty = {};
const MARK = /^ {0,4}\/\/ ((?:\.\.\/)*[\w@.\-/]+\.(?:m?[jt]sx?|cjs))$/gm;
for (const f of files) {
  const text = fs.readFileSync(f, 'utf8');
  const sf = ts.createSourceFile(f, text, ts.ScriptTarget.Latest, false, ts.ScriptKind.JS);
  const lineOf = (pos) => sf.getLineAndCharacterOfPosition(pos).line + 1;
  const marks = [...text.matchAll(MARK)].map((m) => [m.index, m[1]]);
  const ours = (pos) => extras.includes(f) || moduleAt(pos).includes('clean-avionics/');
  for (const [at, p] of marks) {
    if (!p.includes('clean-avionics/')) continue;
    for (const seg of p.split('/')) {
      if (oldApiLower.has(seg.replace(/\.[^.]*$/, '').toLowerCase())) findings.push({ file: path.relative(root, f), line: lineOf(at), name: seg, kind: `old API name as a library file or folder name (${p})` });
    }
  }
  const moduleAt = (pos) => {
    let lo = 0;
    let hi = marks.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (marks[mid][0] <= pos) {
        found = mid;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return found >= 0 ? marks[found][1] : '';
  };
  const bound = new Set();
  const collect = (n) => {
    if ((ts.isFunctionDeclaration(n) || ts.isClassDeclaration(n) || ts.isClassExpression(n) || ts.isVariableDeclaration(n) || ts.isParameter(n) || ts.isBindingElement(n)) && n.name && ts.isIdentifier(n.name)) bound.add(n.name.text);
    ts.forEachChild(n, collect);
  };
  collect(sf);
  const visit = (node) => {
    const k = node.kind;
    if (k === ts.SyntaxKind.Identifier) {
      const t = node.text;
      const pos = node.getStart(sf);
      const mod = moduleAt(pos);
      if (ids.has(t)) {
        if (bound.has(t)) allowed[t] = (allowed[t] || 0) + 1;
        else if (mod.includes('node_modules/')) {
          const pkg = mod.replace(/^.*node_modules\//, '').split('/').slice(0, mod.includes('node_modules/@') ? 2 : 1).join('/');
          thirdParty[pkg] = (thirdParty[pkg] || 0) + 1;
        } else if (fbwOwn.has(t)) allowed[t] = (allowed[t] || 0) + 1;
        else findings.push({ file: path.relative(root, f), line: lineOf(pos), name: t, kind: ids.get(t) });
      }
    } else if (k === ts.SyntaxKind.StringLiteral || k === ts.SyntaxKind.NoSubstitutionTemplateLiteral || k === ts.SyntaxKind.TemplateHead || k === ts.SyntaxKind.TemplateMiddle || k === ts.SyntaxKind.TemplateTail) {
      const t = node.text;
      const pos = node.getStart(sf);
      if (strs.has(t) && !moduleAt(pos).includes('node_modules/')) findings.push({ file: path.relative(root, f), line: lineOf(pos), name: t, kind: strs.get(t) });
      if (ours(pos)) {
        for (const w of t.match(/[A-Za-z_$][\w$]*/g) || []) {
          if (oldApi.has(w)) findings.push({ file: path.relative(root, f), line: lineOf(pos), name: w, kind: 'old API name in a library string' });
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}
const byName = {};
const kindOf = {};
for (const x of findings) {
  byName[x.name] = (byName[x.name] || 0) + 1;
  kindOf[x.name] = x.kind;
}
const report = { gate: 'identifiers', files: files.length, oldNames: ids.size, oldStrings: strs.size, findings: findings.length, byName, allowedBecauseDeclaredLocally: allowed, thirdPartyPackagesNotChecked: thirdParty, sample: findings.slice(0, 200), pass: findings.length === 0 };
fs.writeFileSync(out, JSON.stringify(report, null, 1) + '\n');
console.log(`${report.pass ? 'PASS' : 'FAIL'} identifiers: ${files.length} files, ${findings.length} findings, ${Object.keys(allowed).length} names the pages or FlyByWire declare themselves`);
for (const [n, c] of Object.entries(byName).sort((a, b) => b[1] - a[1]).slice(0, 30)) console.log(`  ${c}\t${n}\t${kindOf[n]}`);
process.exitCode = report.pass ? 0 : 1;
