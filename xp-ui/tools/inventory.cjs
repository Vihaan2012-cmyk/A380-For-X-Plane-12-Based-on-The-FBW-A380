'use strict';
// node inventory.cjs <fork> <out.json>

const ts = require('typescript');
const path = require('path');
const fs = require('fs');
const { execSync } = require('child_process');
const { slash, entries, compilerOptions, cleanHost } = require('./lib/project.cjs');

const LIB = slash(path.resolve(__dirname, '../lib/src')).toLowerCase() + '/';
const fork = slash(path.resolve(process.argv[2] || 'E:/fbw-int/fbw-own-api'));
const out = process.argv[3] || 'E:/fbw-int/xplane-js-private/api-inventory.json';
const TYPINGS = (fork + '/fbw-common/src/typings/').toLowerCase();
const modulesDir = path.join(fork, 'node_modules');
const libEntry = path.resolve(__dirname, '../lib/src/index.ts');
const libModules = { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry };

const lower = (f) => slash(f).toLowerCase();
const isLibFile = (f) => lower(f).startsWith(LIB) && !lower(f).endsWith('_check.ts');
const isTypingsFile = (f) => lower(f).startsWith(TYPINGS);
const isFbwFile = (f) => {
  const n = lower(f);
  return n.startsWith(fork.toLowerCase() + '/') && !n.includes('/node_modules/') && !n.startsWith(TYPINGS) && !n.endsWith('.d.ts');
};
const rel = (f) => slash(f).slice(fork.length + 1);

const OWNER_KINDS = [ts.SyntaxKind.ClassDeclaration, ts.SyntaxKind.InterfaceDeclaration, ts.SyntaxKind.EnumDeclaration, ts.SyntaxKind.ModuleDeclaration, ts.SyntaxKind.TypeAliasDeclaration, ts.SyntaxKind.VariableDeclaration, ts.SyntaxKind.FunctionDeclaration];

function declKind(d) {
  switch (d.kind) {
    case ts.SyntaxKind.ClassDeclaration: return 'class';
    case ts.SyntaxKind.InterfaceDeclaration: return 'interface';
    case ts.SyntaxKind.EnumDeclaration: return 'enum';
    case ts.SyntaxKind.ModuleDeclaration: return 'namespace';
    case ts.SyntaxKind.TypeAliasDeclaration: return 'type';
    case ts.SyntaxKind.FunctionDeclaration: return 'function';
    case ts.SyntaxKind.VariableDeclaration: return 'variable';
    case ts.SyntaxKind.MethodDeclaration: case ts.SyntaxKind.MethodSignature: return 'method';
    case ts.SyntaxKind.PropertyDeclaration: case ts.SyntaxKind.PropertySignature: return 'property';
    case ts.SyntaxKind.GetAccessor: case ts.SyntaxKind.SetAccessor: return 'accessor';
    case ts.SyntaxKind.EnumMember: return 'enum-member';
    case ts.SyntaxKind.Constructor: return 'constructor';
    case ts.SyntaxKind.Parameter: return 'parameter';
    case ts.SyntaxKind.TypeParameter: return 'type-parameter';
    default: return ts.SyntaxKind[d.kind];
  }
}

function ownerChain(d) {
  const names = [];
  for (let p = d.parent; p && !ts.isSourceFile(p); p = p.parent) {
    if (OWNER_KINDS.includes(p.kind) && p.name && ts.isIdentifier(p.name) && !(ts.isModuleDeclaration(p) && p.name.text === 'global')) names.unshift(p.name.text);
  }
  return names;
}

function qualify(sym) {
  const d = sym.declarations && sym.declarations[0];
  if (!d) return { name: sym.name, kind: 'unknown' };
  const chain = ownerChain(d);
  if (d.kind === ts.SyntaxKind.Parameter || d.kind === ts.SyntaxKind.TypeParameter) return null;
  const own = d.name && (ts.isIdentifier(d.name) || ts.isStringLiteral(d.name) || ts.isPrivateIdentifier(d.name)) ? d.name.text : sym.name;
  if (d.kind === ts.SyntaxKind.Constructor) return { name: chain.concat(['constructor']).join('.'), kind: 'constructor', decl: d };
  return { name: chain.concat([own]).join('.'), kind: declKind(d), decl: d };
}

function declText(d) {
  if (!d) return '';
  let n = d;
  if (ts.isVariableDeclaration(n) && n.parent && n.parent.parent) n = n.parent.parent;
  let text = n.getText();
  if (ts.isClassDeclaration(n) || ts.isInterfaceDeclaration(n) || ts.isModuleDeclaration(n)) text = text.split('{')[0].trim();
  else if (ts.isEnumDeclaration(n)) text = text.replace(/\s+/g, ' ');
  else if (ts.isFunctionLike(n) && n.body) text = text.slice(0, n.body.getStart() - n.getStart()).trim();
  else if (ts.isPropertyDeclaration(n) && n.initializer) text = text.slice(0, n.initializer.getStart() - n.getStart()).replace(/\s*=\s*$/, '').trim();
  return text.replace(/\s+/g, ' ').slice(0, 400);
}

function bucket(map, q, d) {
  const parts = q.name.split('.');
  const top = parts[0];
  const t = (map[top] = map[top] || { kind: null, declaration: null, file: null, refs: 0, files: new Set(), kinds: {}, members: {} });
  if (parts.length === 1) {
    t.kind = t.kind || q.kind;
    if (!t.declaration && q.decl) {
      t.declaration = declText(q.decl);
      t.file = slash(q.decl.getSourceFile().fileName);
    }
    return t;
  }
  if (!t.file && q.decl) {
    let root = q.decl;
    while (root.parent && !ts.isSourceFile(root.parent)) root = root.parent;
    t.file = slash(q.decl.getSourceFile().fileName);
  }
  const key = parts.slice(1).join('.');
  const m = (t.members[key] = t.members[key] || { kind: q.kind, declaration: q.decl ? declText(q.decl) : null, refs: 0, files: new Set(), kinds: {}, via: {}, sites: [] });
  return m;
}

function record(entry, node, kind, via) {
  const sf = node.getSourceFile();
  const { line } = sf.getLineAndCharacterOfPosition(node.getStart());
  entry.refs++;
  entry.kinds[kind] = (entry.kinds[kind] || 0) + 1;
  entry.files.add(rel(sf.fileName));
  if (via && entry.via) entry.via[via] = (entry.via[via] || 0) + 1;
  if (entry.sites && entry.sites.length < 12) entry.sites.push(`${rel(sf.fileName)}:${line + 1}`);
}

function useKind(node) {
  const p = node.parent;
  if (ts.isPropertyAccessExpression(p) && p.name === node) return ts.isCallExpression(p.parent) && p.parent.expression === p ? 'call' : ts.isBinaryExpression(p.parent) && p.parent.left === p && p.parent.operatorToken.kind === ts.SyntaxKind.EqualsToken ? 'assign' : 'access';
  if (ts.isNewExpression(p) && p.expression === node) return 'new';
  if (ts.isCallExpression(p) && p.expression === node) return 'call';
  if (ts.isExpressionWithTypeArguments(p) && ts.isHeritageClause(p.parent)) return p.parent.token === ts.SyntaxKind.ExtendsKeyword ? 'extends' : 'implements';
  if (ts.isTypeReferenceNode(p) || ts.isQualifiedName(p) || ts.isTypeQueryNode(p)) return 'type';
  if (ts.isJsxOpeningElement(p) || ts.isJsxSelfClosingElement(p) || ts.isJsxClosingElement(p)) return 'jsx-tag';
  if (ts.isJsxAttribute(p)) return 'jsx-attr';
  if (ts.isQualifiedName(p) || ts.isPropertyAccessExpression(p)) return 'qualifier';
  if (ts.isImportSpecifier(p) || ts.isExportSpecifier(p)) return 'import';
  return 'ref';
}

const library = {};
const globals = {};
const imports = {};
const untyped = {};
const programs = [];
const fbwFiles = new Set();
const GLOBAL_NAMES = /^(SimVar|Coherent|Simplane|Avionics|BaseInstrument|RegisterViewListener|RegisterGenericDataListener|GetStoredData|SetStoredData|DeleteStoredData|LaunchFlowEvent|Utils|registerInstrument|Include|EmptyCallback|SimVarValueType|Facilities|GameState|LatLong|LatLongAlt|NavSystem|TemplateElement|fastToFixed|ViewListener)$/;

function scan(program, jsx) {
  const checker = program.getTypeChecker();
  const alias = (s) => (s && s.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(s) : s);
  const where = (sym) => {
    if (!sym || !sym.declarations || !sym.declarations.length) return null;
    if (sym.declarations.some((d) => isLibFile(d.getSourceFile().fileName))) return 'lib';
    if (sym.declarations.some((d) => isTypingsFile(d.getSourceFile().fileName))) return 'global';
    return null;
  };
  const target = (w) => (w === 'lib' ? library : globals);
  const receiver = (expr) => {
    try {
      const t = checker.getTypeAtLocation(expr);
      const s = t.aliasSymbol || t.getSymbol();
      const n = s ? s.name : checker.typeToString(t);
      return n === '__type' || n === '__object' ? checker.typeToString(t).slice(0, 60) : n.replace(/<.*$/, '');
    } catch {
      return undefined;
    }
  };

  for (const sf of program.getSourceFiles()) {
    if (!isFbwFile(sf.fileName)) continue;
    const key = lower(sf.fileName);
    if (fbwFiles.has(key)) continue;
    fbwFiles.add(key);

    for (const st of sf.statements) {
      if (ts.isImportDeclaration(st) && ts.isStringLiteral(st.moduleSpecifier) && ['@microsoft/msfs-sdk', 'xp-kit', 'xp-ui'].includes(st.moduleSpecifier.text)) {
        const mod = st.moduleSpecifier.text;
        const nb = st.importClause && st.importClause.namedBindings;
        if (nb && ts.isNamedImports(nb)) {
          for (const el of nb.elements) {
            const imported = (el.propertyName || el.name).text;
            const m = (imports[mod] = imports[mod] || {});
            const e = (m[imported] = m[imported] || { files: new Set(), localNames: {}, typeOnly: 0 });
            e.files.add(rel(sf.fileName));
            e.localNames[el.name.text] = (e.localNames[el.name.text] || 0) + 1;
            if (st.importClause.isTypeOnly || el.isTypeOnly) e.typeOnly++;
          }
        }
      }
    }

    const visit = (node) => {
      if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) {
        let sym;
        try {
          sym = alias(checker.getSymbolAtLocation(node));
        } catch {
          sym = undefined;
        }
        const w = where(sym);
        if (w) {
          const q = qualify(sym);
          if (q) {
            const k = useKind(node);
            const p = node.parent;
            const via = ts.isPropertyAccessExpression(p) && p.name === node ? receiver(p.expression) : undefined;
            record(bucket(target(w), q), node, k, via);
          }
        }
        const p = node.parent;
        if (p && (ts.isPropertyAssignment(p) || ts.isShorthandPropertyAssignment(p) || ts.isMethodDeclaration(p)) && p.name === node && ts.isObjectLiteralExpression(p.parent)) {
          const ct = checker.getContextualType(p.parent);
          const types = ct ? (ct.isUnion() ? ct.types : [ct]) : [];
          for (const t of types) {
            const prop = t.getProperty && t.getProperty(node.text);
            const pw = where(prop);
            if (pw) {
              const q = qualify(prop);
              if (q) record(bucket(target(pw), q), node, 'object-literal');
              break;
            }
          }
        }
      }
      if ((ts.isClassDeclaration(node) || ts.isClassExpression(node)) && node.heritageClauses) {
        const bases = [];
        for (const hc of node.heritageClauses) for (const ht of hc.types) bases.push(checker.getTypeAtLocation(ht));
        for (const m of node.members) {
          if (!m.name || !(ts.isIdentifier(m.name) || ts.isStringLiteral(m.name))) continue;
          for (const b of bases) {
            const prop = b.getProperty && b.getProperty(m.name.text);
            const pw = where(prop);
            if (pw) {
              const q = qualify(prop);
              if (q) record(bucket(target(pw), q), m.name, 'override');
              break;
            }
          }
        }
      }
      if (ts.isPropertyAccessExpression(node) && GLOBAL_NAMES.test(node.name.text)) {
        const obj = node.expression;
        const txt = obj.getText().replace(/\s+/g, '');
        if (/^(window|globalThis|\(window asany\)|\(globalThis asany\)|\(windowasany\)|\(globalThisasany\)|self)$/.test(txt)) {
          const u = (untyped[node.name.text] = untyped[node.name.text] || { refs: 0, files: new Set(), kinds: {}, sites: [] });
          record(u, node.name, 'global-property');
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
}

const roots = entries(fork);
for (const [jsx, list, factory] of [['fs', roots.fs, ['FSComponent.buildComponent', 'FSComponent.Fragment']], ['react', roots.react, 'react']]) {
  const options = compilerOptions(ts, { fork, modulesDir, libModules, jsx: factory });
  const host = cleanHost(ts, options);
  const t0 = Date.now();
  const program = ts.createProgram(list.map((e) => e.file), options, host);
  const diags = ts.getPreEmitDiagnostics(program).filter((d) => d.file && isFbwFile(d.file.fileName));
  const files = program.getSourceFiles().filter((s) => isFbwFile(s.fileName)).length;
  scan(program, jsx);
  const byCode = {};
  for (const d of diags) byCode[d.code] = (byCode[d.code] || 0) + 1;
  programs.push({
    jsx,
    roots: list.map((e) => ({ name: e.name, file: rel(e.file) })),
    fbwFiles: files,
    errors: diags.length,
    errorsByCode: byCode,
    firstErrors: diags.slice(0, 40).map((d) => {
      const { line } = d.file.getLineAndCharacterOfPosition(d.start || 0);
      return `${rel(d.file.fileName)}:${line + 1} TS${d.code} ${ts.flattenDiagnosticMessageText(d.messageText, ' ').slice(0, 200)}`;
    }),
    seconds: Math.round((Date.now() - t0) / 1000),
  });
  console.error(`${jsx}: ${files} FlyByWire files, ${diags.length} errors, ${Math.round((Date.now() - t0) / 1000)} s`);
}

const plain = (o) => JSON.parse(JSON.stringify(o, (k, v) => (v instanceof Set ? [...v].sort() : v)));
const summarize = (map) => {
  const o = {};
  for (const name of Object.keys(map).sort()) {
    const e = map[name];
    o[name] = {
      kind: e.kind,
      declaration: e.declaration,
      file: e.file ? e.file.replace(/^.*?\/(clean-avionics|fbw-own-api)\//, '$1/') : null,
      refs: e.refs,
      fileCount: e.files.size,
      kinds: e.kinds,
      members: Object.fromEntries(
        Object.keys(e.members)
          .sort()
          .map((m) => {
            const x = e.members[m];
            return [m, { kind: x.kind, declaration: x.declaration, refs: x.refs, fileCount: x.files.size, kinds: x.kinds, via: x.via, sites: x.sites }];
          }),
      ),
    };
  }
  return o;
};
const git = (dir) => {
  try {
    return execSync('git rev-parse --short HEAD', { cwd: dir }).toString().trim();
  } catch {
    return null;
  }
};

const importSummary = {};
for (const [mod, names] of Object.entries(imports)) {
  importSummary[mod] = {};
  for (const n of Object.keys(names).sort()) importSummary[mod][n] = { files: names[n].files.size, localNames: names[n].localNames, typeOnly: names[n].typeOnly };
}
const importedNames = new Set(Object.values(imports).flatMap((m) => Object.keys(m)));

const result = {
  format: 'api-inventory/1',
  fork: { path: fork, commit: git(fork) },
  libraryRepo: { path: slash(path.resolve(__dirname, '..')), commit: git(path.resolve(__dirname, '..')) },
  programs,
  totals: {
    fbwFiles: fbwFiles.size,
    importedNames: importedNames.size,
    importedFromSdk: Object.keys(imports['@microsoft/msfs-sdk'] || {}).length,
    importedFromXpKit: Object.keys(imports['xp-kit'] || {}).length,
    filesImportingSdk: new Set(Object.values(imports['@microsoft/msfs-sdk'] || {}).flatMap((e) => [...e.files])).size,
    filesImportingXpKit: new Set(Object.values(imports['xp-kit'] || {}).flatMap((e) => [...e.files])).size,
    libraryDeclarationsUsed: Object.keys(library).length,
    pageGlobalsUsed: Object.keys(globals).length,
  },
  imports: importSummary,
  library: summarize(library),
  pageGlobals: summarize(globals),
  untypedGlobalAccess: plain(untyped),
};
fs.writeFileSync(out, JSON.stringify(plain(result), null, 1) + '\n');
console.error(`wrote ${out}`);
