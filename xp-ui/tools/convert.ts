// JS Library for X-Plane: one-time, type-aware conversion of FlyByWire's sources (and of
// this library) onto the library's own API, and the exact reverse used to check it.
//
//   node convert.ts lib  forward|reverse --src <repo> --out <dir> [--fork <fork>] [--map <api-map.json>]
//   node convert.ts fbw  forward|reverse --src <fork> --out <dir> --lib <lib/src> [--data <data d.ts>] [--extra <file,file relative to src>] [--map ...] [--update-map]
//   node convert.ts compare <a-dir> <b-dir> <list-file>

import { Project, ts } from 'ts-morph';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { compilerOptions, entries, slash } = require('./lib/project.cjs');

type Edit = { start: number; end: number; text: string; why: string };
type Sym = { half: string; old: string; new: string; kind: string; declaredIn: string | null; members: { old: string; new: string; declaredIn: string | null }[] };
type StringEntry = { piece: 'full' | 'head' | 'middle' | 'tail'; old: string; new: string; internal?: string };
type ApiMap = {
  modules: { old: string; new: string; reverse: string }[];
  dataModule: string;
  jsx: { old: string; new: string }[];
  files: { old: string; new: string }[];
  symbols: Sym[];
  strings: StringEntry[];
  wire: { old: string; new: string }[];
  untyped: { old: string; new: string }[];
  libraryText: { file: string; imports: string[]; pairs: [string, string][] }[];
  plugin?: { names: Record<string, string> };
};

const PRIVATE = 'E:/fbw-int/xplane-js-private';
let LIB_DIR = '';
const argv = process.argv.slice(2);
const opt = (name: string, def?: string): string | undefined => {
  const i = argv.indexOf('--' + name);
  return i >= 0 ? argv[i + 1] : def;
};
const flag = (name: string): boolean => argv.includes('--' + name);
const lower = (f: string) => slash(f).toLowerCase();

function readText(file: string): { text: string; eol: string; bom: string } {
  const raw = fs.readFileSync(file, 'utf8');
  const bom = raw.charCodeAt(0) === 0xfeff ? '﻿' : '';
  const text = bom ? raw.slice(1) : raw;
  return { text, eol: text.includes('\r\n') ? '\r\n' : '\n', bom };
}

function applyEdits(text: string, edits: Edit[], file: string): string {
  const removals = edits.filter((e) => e.why === 'data import' || e.why === 'libtext');
  const kept = edits.filter((e) => e.why === 'data import' || e.why === 'libtext' || !removals.some((r) => e.start >= r.start && e.end <= r.end));
  const sorted = [...kept].sort((a, b) => b.start - a.start || b.end - a.end);
  const seen = new Map<string, Edit>();
  const unique: Edit[] = [];
  for (const e of sorted) {
    const k = `${e.start}:${e.end}`;
    const prev = seen.get(k);
    if (prev) {
      if (prev.text !== e.text) throw new Error(`${file}: conflicting edits at ${e.start}: "${prev.text}" (${prev.why}) vs "${e.text}" (${e.why})`);
      continue;
    }
    seen.set(k, e);
    unique.push(e);
  }
  for (let i = 1; i < unique.length; i++) {
    if (unique[i].end > unique[i - 1].start && unique[i].start !== unique[i].end) throw new Error(`${file}: overlapping edits at ${unique[i].start}..${unique[i].end} (${unique[i].why}) and ${unique[i - 1].start} (${unique[i - 1].why})`);
  }
  let out = text;
  for (const e of unique) out = out.slice(0, e.start) + e.text + out.slice(e.end);
  return out;
}

// ---------------------------------------------------------------------------
// Map views per direction.

type Target = { half: string; from: string; to: string; owner?: string; member?: string; declaredIn: string | null };

function targetsFor(map: ApiMap, reverse: boolean): Target[] {
  const out: Target[] = [];
  for (const c of (map as any).structural ?? []) {
    for (const [o, n] of Object.entries(c.members as Record<string, string>)) {
      const [from, to] = reverse ? [n, o] : [o, n];
      out.push({ half: 'fbw', from, to, owner: c.class, member: from, declaredIn: c.file });
    }
  }
  for (const s of map.symbols) {
    const [from, to] = reverse ? [s.new, s.old] : [s.old, s.new];
    out.push({ half: s.half, from, to, declaredIn: s.declaredIn });
    const seg = new Map(s.members.filter((m) => !m.old.includes('.')).map((m) => [m.old, m.new]));
    for (const m of s.members) {
      const parts = m.old.split('.');
      const newPath = [...parts.slice(0, -1).map((p) => seg.get(p) ?? p), m.new].join('.');
      const oldLast = parts[parts.length - 1];
      const [mf, mt] = reverse ? [newPath, oldLast] : [m.old, m.new];
      out.push({ half: s.half, from: mf, to: mt, owner: from, member: mf, declaredIn: m.declaredIn });
    }
  }
  return out;
}

// ---------------------------------------------------------------------------
// Projects.

type Mode = 'lib' | 'fbw';

function makeProjects(mode: Mode, reverse: boolean, src: string, fork: string, lib: string, data: string | undefined): { project: Project; jsx: string }[] {
  const modulesDir = path.join(fork, 'node_modules');
  const libEntry = path.join(lib, 'index.ts');
  const libModules: Record<string, string> = reverse ? { 'xplane-js/ui': libEntry, 'xplane-js/data': data! } : { '@microsoft/msfs-sdk': libEntry, 'xp-kit': libEntry, 'xp-ui': libEntry };
  if (!reverse && data) libModules['xplane-js/data'] = data;
  const typings = reverse ? 'xplane' : 'msfs';
  const mk = (jsx: any, roots: string[]) => {
    const options = compilerOptions(ts, { fork, modulesDir, libModules, jsx, typings });
    const project = new Project({ compilerOptions: options, skipAddingFilesFromTsConfig: true });
    for (const r of roots) project.addSourceFileAtPath(r);
    project.resolveSourceFileDependencies();
    for (const sf of project.getSourceFiles()) {
      const n = lower(sf.getFilePath());
      if (n.includes('/@microsoft/') || n.includes('/xp-kit/') || n.includes('/xp-avionics/')) throw new Error('forbidden file in project');
    }
    return project;
  };
  const fsJsx = reverse ? ['UI.node', 'UI.Group'] : ['FSComponent.buildComponent', 'FSComponent.Fragment'];
  if (mode === 'lib') {
    const files = walk(lib).filter((f) => f.endsWith('.ts'));
    return [{ project: mk(fsJsx, files), jsx: 'fs' }];
  }
  const e = entries(src);
  return [
    { project: mk(fsJsx, [...e.fs.map((x: any) => x.file), ...(opt('extra') ? opt('extra')!.split(',').map((f) => slash(path.resolve(src, f))) : [])]), jsx: 'fs' },
    { project: mk('react', e.react.map((x: any) => x.file)), jsx: 'react' },
  ];
}

function walk(dir: string): string[] {
  const out: string[] = [];
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, d.name);
    if (d.isDirectory()) out.push(...walk(p));
    else out.push(slash(p));
  }
  return out.sort();
}

// ---------------------------------------------------------------------------
// Finding declarations for rename targets.

function declarationNode(program: ts.Program, t: Target, ctx: { lib: string; data?: string; fork: string; reverse: boolean }): ts.Node | null {
  const checker = program.getTypeChecker();
  const alias = (s: ts.Symbol | undefined) => (s && s.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(s) : s);
  let root: ts.Symbol | undefined;
  const top = t.owner ?? t.from;
  if (t.half === 'fbw') {
    const sf = program.getSourceFiles().find((x) => lower(x.fileName).endsWith('/' + lower(t.declaredIn!)));
    const cls = sf?.statements.find((x) => ts.isClassDeclaration(x) && x.name?.text === t.owner) as ts.ClassDeclaration | undefined;
    const m = cls?.members.find((x) => x.name && ts.isIdentifier(x.name) && x.name.text === t.member);
    return m ? (m.name as ts.Node) : null;
  }
  if (t.half === 'ui' || t.half === 'ui-alias') {
    const sf = program.getSourceFile(path.join(ctx.lib, 'index.ts')) ?? program.getSourceFile(slash(path.join(ctx.lib, 'index.ts')));
    if (!sf) return null;
    const mod = checker.getSymbolAtLocation(sf)!;
    const exp = checker.getExportsOfModule(mod).find((s) => s.name === top);
    if (!exp) return null;
    if (t.half === 'ui-alias' && !t.owner) {
      const d = exp.declarations?.find((x) => ts.isExportSpecifier(x));
      return d ? (d as ts.ExportSpecifier).name : null;
    }
    root = alias(exp);
  } else if (ctx.reverse) {
    const sf = program.getSourceFile(ctx.data!) ?? program.getSourceFiles().find((s) => lower(s.fileName) === lower(ctx.data!));
    if (!sf) return null;
    const mod = checker.getSymbolAtLocation(sf)!;
    root = checker.getExportsOfModule(mod).find((s) => s.name === top);
  } else {
    const typings = lower(ctx.fork + '/fbw-common/src/typings/');
    for (const sf of program.getSourceFiles()) {
      if (!lower(sf.fileName).startsWith(typings)) continue;
      const found = checker.getSymbolsInScope(sf, ts.SymbolFlags.Value | ts.SymbolFlags.Type | ts.SymbolFlags.Namespace).find((s) => s.name === top && s.declarations?.some((d) => lower(d.getSourceFile().fileName).startsWith(typings)));
      if (found) {
        root = found;
        break;
      }
    }
  }
  if (!root) return null;
  let sym: ts.Symbol | undefined = root;
  if (t.owner) {
    for (const part of t.member!.split('.')) {
      const s = alias(sym)!;
      let next = s.members?.get(part as ts.__String) ?? s.exports?.get(part as ts.__String);
      if (!next && s.declarations) {
        const dt = checker.getDeclaredTypeOfSymbol(s);
        next = dt.getProperty(part);
        if (!next) next = checker.getTypeOfSymbolAtLocation(s, s.declarations[0]).getProperty(part);
      }
      if (!next) return null;
      sym = next;
    }
  }
  const d = sym!.declarations?.[0] as any;
  if (!d) return null;
  return d.name ?? d;
}

// ---------------------------------------------------------------------------
// Data flow: which string pieces reach a variable-name or wire-name position.

type Sink = { kind: 'var' | 'wire'; callee: string; arg: number };

function flowStrings(program: ts.Program, files: Set<string>, sinks: Sink[], propertySinks: string[], resolveQualified: (s: ts.Symbol) => string | null, tableOwners: string[] = []) {
  const checker = program.getTypeChecker();
  const alias = (s: ts.Symbol | undefined) => (s && s.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(s) : s);
  const result = new Map<ts.Node, 'var' | 'wire'>();
  const visited = new Set<string>();
  const paramSinks = new Map<ts.ParameterDeclaration, 'var' | 'wire'>();
  const callsByDecl = new Map<ts.Node, ts.CallLikeExpression[]>();
  const writesByProp = new Map<ts.Symbol, ts.Expression[]>();
  const callbackArrays = new Map<ts.Node, ts.Expression>();

  const inScope = (sf: ts.SourceFile) => files.has(lower(sf.fileName));
  const allFiles = program.getSourceFiles().filter((sf) => !lower(sf.fileName).includes('/node_modules/'));
  for (const sf of allFiles) {
    const visit = (n: ts.Node) => {
      if (ts.isCallExpression(n) || ts.isNewExpression(n)) {
        try {
          const sig = checker.getResolvedSignature(n);
          const decl = sig?.declaration as ts.SignatureDeclaration | undefined;
          if (decl) {
            const named = (decl as any).name ? checker.getSymbolAtLocation((decl as any).name) : undefined;
            const all = named?.declarations?.length ? named.declarations : [decl];
            for (const d of all) {
              const list = callsByDecl.get(d) ?? [];
              list.push(n);
              callsByDecl.set(d, list);
            }
          }
        } catch {
          // ignore
        }
        if (ts.isCallExpression(n) && ts.isPropertyAccessExpression(n.expression) && /^(map|forEach|filter|some|every|find|reduce|flatMap)$/.test(n.expression.name.text)) {
          for (const a of n.arguments) if (ts.isArrowFunction(a) || ts.isFunctionExpression(a)) callbackArrays.set(a, n.expression.expression);
        }
      }
      if (ts.isBinaryExpression(n) && n.operatorToken.kind === ts.SyntaxKind.EqualsToken && ts.isPropertyAccessExpression(n.left)) {
        const s = checker.getSymbolAtLocation(n.left.name);
        if (s) {
          const list = writesByProp.get(s) ?? [];
          list.push(n.right);
          writesByProp.set(s, list);
        }
      }
      if ((ts.isPropertyAssignment(n) || ts.isShorthandPropertyAssignment(n)) && ts.isObjectLiteralExpression(n.parent)) {
        const ct = checker.getContextualType(n.parent);
        const name = n.name.getText();
        for (const t of ct ? (ct.isUnion() ? ct.types : [ct]) : []) {
          const p = t.getProperty(name);
          if (p) {
            const list = writesByProp.get(p) ?? [];
            list.push(ts.isPropertyAssignment(n) ? n.initializer : n.name);
            writesByProp.set(p, list);
          }
        }
      }
      ts.forEachChild(n, visit);
    };
    visit(sf);
  }

  const key = (n: ts.Node) => `${n.getSourceFile().fileName}:${n.pos}:${n.end}:${n.kind}`;

  function value(e: ts.Expression | undefined, kind: 'var' | 'wire'): void {
    if (!e) return;
    const k = key(e) + kind;
    if (visited.has(k)) return;
    visited.add(k);
    if (ts.isStringLiteral(e) || ts.isNoSubstitutionTemplateLiteral(e) || ts.isTemplateExpression(e)) {
      if (inScope(e.getSourceFile())) result.set(e, kind);
      return;
    }
    if (ts.isParenthesizedExpression(e) || ts.isAsExpression(e) || ts.isNonNullExpression(e) || ts.isTypeAssertionExpression(e) || ts.isSatisfiesExpression(e)) return value(e.expression, kind);
    if (ts.isConditionalExpression(e)) {
      value(e.whenTrue, kind);
      value(e.whenFalse, kind);
      return;
    }
    if (ts.isBinaryExpression(e)) {
      if (e.operatorToken.kind === ts.SyntaxKind.PlusToken) {
        value(e.left, kind);
        if (ts.isStringLiteral(e.right) || ts.isNoSubstitutionTemplateLiteral(e.right)) value(e.right, kind);
      } else if (e.operatorToken.kind === ts.SyntaxKind.BarBarToken || e.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken) {
        value(e.left, kind);
        value(e.right, kind);
      }
      return;
    }
    if (ts.isIdentifier(e)) return fromSymbol(checker.getSymbolAtLocation(e), kind);
    if (ts.isPropertyAccessExpression(e)) {
      const s = checker.getSymbolAtLocation(e.name);
      fromSymbol(s, kind);
      return;
    }
    if (ts.isElementAccessExpression(e)) {
      const s = alias(checker.getSymbolAtLocation(e.expression));
      for (const d of s?.declarations ?? []) {
        if (ts.isEnumDeclaration(d)) for (const m of d.members) value(m.initializer, kind);
        if (ts.isVariableDeclaration(d) && d.initializer) elements(d.initializer, kind);
        if (ts.isPropertyDeclaration(d) && d.initializer) elements(d.initializer, kind);
      }
      return;
    }
    if (ts.isCallExpression(e)) {
      try {
        const decl = checker.getResolvedSignature(e)?.declaration as ts.SignatureDeclaration | undefined;
        if (decl && 'body' in decl && decl.body) returns(decl.body as ts.Node, kind);
      } catch {
        // ignore
      }
    }
  }

  function table(e: ts.Expression | undefined, depth = 0): void {
    if (!e || depth > 8) return;
    const k = key(e) + 'table';
    if (visited.has(k)) return;
    visited.add(k);
    if (ts.isParenthesizedExpression(e) || ts.isAsExpression(e) || ts.isSatisfiesExpression(e) || ts.isNonNullExpression(e)) return table(e.expression, depth + 1);
    if (ts.isNewExpression(e) || ts.isCallExpression(e)) {
      for (const a of e.arguments ?? []) table(a, depth + 1);
      if (ts.isCallExpression(e) && ts.isPropertyAccessExpression(e.expression)) table(e.expression.expression, depth + 1);
      return;
    }
    if (ts.isArrayLiteralExpression(e)) {
      for (const el of e.elements) {
        if (ts.isSpreadElement(el)) table(el.expression, depth + 1);
        else if (ts.isArrayLiteralExpression(el)) for (const x of el.elements) entry(x as ts.Expression);
        else table(el as ts.Expression, depth + 1);
      }
      return;
    }
    if (ts.isIdentifier(e) || ts.isPropertyAccessExpression(e)) {
      const s = alias(checker.getSymbolAtLocation(ts.isPropertyAccessExpression(e) ? e.name : e));
      for (const d of s?.declarations ?? []) {
        if ((ts.isVariableDeclaration(d) || ts.isPropertyDeclaration(d)) && d.initializer) table(d.initializer, depth + 1);
        if (ts.isParameter(d)) {
          const fn = d.parent;
          const i = fn.parameters.indexOf(d);
          for (const call of callsByDecl.get(fn) ?? []) table(((call as ts.CallExpression).arguments ?? [])[i], depth + 1);
        }
      }
      for (const w of s ? writesByProp.get(s) ?? [] : []) table(w, depth + 1);
    }
  }

  function entry(x: ts.Expression) {
    const o = ts.isParenthesizedExpression(x) || ts.isAsExpression(x) || ts.isSatisfiesExpression(x) ? x.expression : x;
    if (!ts.isObjectLiteralExpression(o)) return;
    for (const p of o.properties) if (ts.isPropertyAssignment(p) && p.name.getText() === 'name') value(p.initializer, 'var');
  }

  function elements(init: ts.Expression, kind: 'var' | 'wire') {
    const x = ts.isAsExpression(init) || ts.isSatisfiesExpression(init) ? init.expression : init;
    if (ts.isArrayLiteralExpression(x)) for (const el of x.elements) value(el as ts.Expression, kind);
    if (ts.isObjectLiteralExpression(x)) for (const p of x.properties) if (ts.isPropertyAssignment(p)) value(p.initializer, kind);
  }

  function returns(body: ts.Node, kind: 'var' | 'wire') {
    if (!ts.isBlock(body)) return value(body as ts.Expression, kind);
    const visit = (n: ts.Node) => {
      if (ts.isReturnStatement(n)) value(n.expression, kind);
      if (ts.isFunctionLike(n)) return;
      ts.forEachChild(n, visit);
    };
    ts.forEachChild(body, visit);
  }

  function fromSymbol(s0: ts.Symbol | undefined, kind: 'var' | 'wire') {
    const s = alias(s0);
    if (!s) return;
    const k = 'sym:' + (s.declarations?.[0] ? key(s.declarations[0]) : s.name) + kind;
    if (visited.has(k)) return;
    visited.add(k);
    for (const d of s.declarations ?? []) {
      if (ts.isVariableDeclaration(d)) {
        value(d.initializer, kind);
        if (d.initializer && (ts.isArrayLiteralExpression(d.initializer) || ts.isObjectLiteralExpression(d.initializer))) elements(d.initializer, kind);
      } else if (ts.isParameter(d)) {
        paramSink(d, kind);
      } else if (ts.isEnumMember(d)) {
        value(d.initializer, kind);
      } else if (ts.isPropertyDeclaration(d) || ts.isPropertySignature(d)) {
        if (ts.isPropertyDeclaration(d)) value(d.initializer, kind);
      } else if (ts.isGetAccessor(d) && d.body) {
        returns(d.body, kind);
      } else if (ts.isBindingElement(d)) {
        // destructured names are not followed
      }
    }
    for (const w of writesByProp.get(s) ?? []) value(w, kind);
  }

  /** `new Sub(x)` on an inherited constructor, where Sub only uses the parameter property inside another category's name (`L:..${this.name}`). */
  function fragmentOnly(call: ts.Node, p: ts.ParameterDeclaration): boolean {
    if (!ts.isNewExpression(call) || !ts.isIdentifier(p.name)) return false;
    const owner = p.parent.parent;
    const s = alias(checker.getSymbolAtLocation(call.expression));
    const sub = s?.declarations?.find(ts.isClassDeclaration);
    if (!sub || sub === owner || sub.members.some(ts.isConstructorDeclaration)) return false;
    const field = p.name.text;
    let uses = 0;
    let fragments = 0;
    const visit = (n: ts.Node) => {
      if (ts.isPropertyAccessExpression(n) && n.expression.kind === ts.SyntaxKind.ThisKeyword && n.name.text === field) {
        uses++;
        const span = n.parent;
        if (ts.isTemplateSpan(span) && OTHER_CATEGORY.test(span.parent.head.text)) fragments++;
      }
      ts.forEachChild(n, visit);
    };
    visit(sub);
    return uses > 0 && uses === fragments;
  }

  function paramSink(p: ts.ParameterDeclaration, kind: 'var' | 'wire') {
    if (paramSinks.has(p)) return;
    paramSinks.set(p, kind);
    value(p.initializer, kind);
    const fn = p.parent;
    const index = fn.parameters.indexOf(p);
    if (ts.isConstructorDeclaration(fn) && p.modifiers?.some((m) => m.kind === ts.SyntaxKind.PublicKeyword || m.kind === ts.SyntaxKind.PrivateKeyword || m.kind === ts.SyntaxKind.ProtectedKeyword || m.kind === ts.SyntaxKind.ReadonlyKeyword)) {
      // parameter property: its writes are the constructor arguments, already followed below
    }
    for (const call of callsByDecl.get(fn) ?? []) {
      if (fragmentOnly(call, p)) continue;
      const args = (call as ts.CallExpression).arguments ?? [];
      if (p.dotDotDotToken) for (const a of args.slice(index)) value(a, kind);
      else value(args[index], kind);
    }
    const arr = callbackArrays.get(fn);
    if (arr && index === 0) {
      const s = alias(checker.getSymbolAtLocation(arr));
      for (const d of s?.declarations ?? []) if ((ts.isVariableDeclaration(d) || ts.isPropertyDeclaration(d)) && d.initializer) elements(d.initializer, kind);
      if (ts.isArrayLiteralExpression(arr)) elements(arr, kind);
    }
  }

  for (const sf of allFiles) {
    const visit = (n: ts.Node) => {
      if (ts.isCallExpression(n) || ts.isNewExpression(n)) {
        const callee = n.expression;
        const id = ts.isPropertyAccessExpression(callee) ? callee.name : callee;
        const s = alias(checker.getSymbolAtLocation(id));
        const q = s ? resolveQualified(s) : null;
        if (q) for (const sk of sinks) if (sk.callee === q) value(n.arguments?.[sk.arg], sk.kind);
        if (ts.isCallExpression(n) && n.expression.kind === ts.SyntaxKind.SuperKeyword) {
          const ct = checker.getTypeAtLocation(n.expression);
          const cs = ct.getConstructSignatures?.() ?? [];
          for (const sig of cs) {
            const d = sig.getDeclaration?.();
            const owner = d && d.parent && ts.isClassDeclaration(d.parent) && d.parent.name ? checker.getSymbolAtLocation(d.parent.name) : undefined;
            if (owner && tableOwners.includes(resolveQualified(owner) ?? '')) table(n.arguments[0]);
          }
        }
        if (ts.isNewExpression(n)) {
          const s2 = alias(checker.getSymbolAtLocation(n.expression));
          if (s2 && tableOwners.includes(resolveQualified(s2) ?? '')) table(n.arguments?.[0]);
        }
      }
      if ((ts.isPropertyAssignment(n) || ts.isShorthandPropertyAssignment(n)) && ts.isObjectLiteralExpression(n.parent)) {
        const ct = checker.getContextualType(n.parent);
        const name = n.name.getText();
        for (const t of ct ? (ct.isUnion() ? ct.types : [ct]) : []) {
          const p = t.getProperty(name);
          const q = p ? resolveQualified(p) : null;
          if (q && propertySinks.includes(q)) {
            value(ts.isPropertyAssignment(n) ? n.initializer : n.name, 'var');
            break;
          }
        }
      }
      ts.forEachChild(n, visit);
    };
    visit(sf);
  }
  return result;
}

// ---------------------------------------------------------------------------
// MSFS variable names -> plugin names.

const OTHER_CATEGORY = /^(L|K|H|B|O|Z|I|C|M|P|R|S|T|W|X):/i;
const SIM_FULL = /^(A:|E:)?[A-Z][A-Z0-9 _]*[A-Z0-9_](:(\d+|#index#)?)?$/;
const SIM_HEAD = /^(A:|E:)?[A-Z][A-Z0-9 _]*[ :_]?$/;
const sanitise = (s: string) => s.replace(/[^A-Za-z0-9_/#]/g, '_');

function loadPluginMapping(plugin: string): Map<string, string> {
  const lib = fs.readFileSync(path.join(plugin, 'src/lib.rs'), 'utf8');
  const block = lib.slice(lib.indexOf('fn mapping(name: &str)'));
  const body = block.slice(0, block.indexOf('\n    })\n'));
  const mapped = new Map<string, string>();
  for (const m of body.matchAll(/^\s*"([^"]+)" => \(?\s*"([^"]+)"/gm)) mapped.set(m[1], m[2]);
  return mapped;
}

function newVarName(text: string, piece: 'full' | 'head' | 'middle' | 'tail', mapped: Map<string, string>, uniqueDataref: Set<string>): string {
  if (piece === 'middle' || piece === 'tail') return sanitise(text);
  const m = text.match(/^(A:|E:)?(.*)$/)!;
  const prefix = m[1] ?? '';
  const rest = m[2];
  if (piece === 'full' && mapped.has(rest) && uniqueDataref.has(rest)) return prefix + mapped.get(rest)!;
  return prefix + 'fbw/' + sanitise(rest);
}

// ---------------------------------------------------------------------------
// Main conversion.

function convert(mode: Mode, direction: 'forward' | 'reverse') {
  const reverse = direction === 'reverse';
  const src = slash(path.resolve(opt('src')!));
  const out = slash(path.resolve(opt('out')!));
  const fork = slash(path.resolve(opt('fork', mode === 'fbw' ? src : 'E:/fbw-int/fbw-own-api')!));
  const lib = slash(path.resolve(opt('lib', mode === 'lib' ? path.join(src, 'lib/src') : '')!));
  const data = opt('data') ? slash(path.resolve(opt('data')!)) : undefined;
  LIB_DIR = lower(lib) + '/';
  const plugin = slash(path.resolve(opt('plugin', 'E:/fbw-int/plugin')!));
  const mapFile = opt('map', path.join(PRIVATE, 'api-map.json'))!;
  const map: ApiMap = JSON.parse(fs.readFileSync(mapFile, 'utf8'));
  const t0 = Date.now();
  const log = (s: string) => console.log(`[${((Date.now() - t0) / 1000).toFixed(0)}s] ${s}`);

  const projects = makeProjects(mode, reverse, src, fork, lib, data);
  log(`projects ready: ${projects.map((p) => p.project.getSourceFiles().length).join(' + ')} files`);

  const isOutput = (f: string) => {
    const n = lower(f);
    if (mode === 'lib') return n.startsWith(lower(lib) + '/');
    return n.startsWith(lower(src) + '/') && !n.includes('/node_modules/') && !n.startsWith(lower(src + '/fbw-common/src/typings/')) && !n.startsWith(lower(src + '/xplane/'));
  };

  const edits = new Map<string, Edit[]>();
  const addEdit = (file: string, e: Edit) => {
    const k = lower(file);
    if (!edits.has(k)) edits.set(k, []);
    edits.get(k)!.push(e);
  };
  const dataTopByFile = new Map<string, Set<string>>();
  const targets = targetsFor(map, reverse);
  const dataTop = new Set(map.symbols.filter((s) => s.half === 'data').map((s) => (reverse ? s.new : s.old)));
  const ctx = { lib, data, fork, reverse };
  const missing: string[] = [];
  const fileTexts = new Map<string, string>();

  for (const { project, jsx } of projects) {
    const program = project.getProgram().compilerObject;
    const ls = project.getLanguageService().compilerObject;
    for (const sf of program.getSourceFiles()) if (isOutput(sf.fileName)) fileTexts.set(lower(sf.fileName), sf.fileName);
    let n = 0;
    for (const t of targets) {
      const node = declarationNode(program, t, ctx);
      if (!node) {
        if (jsx === 'fs') missing.push(`${t.half} ${t.owner ? t.owner + '.' + t.member : t.from}`);
        continue;
      }
      const sf = node.getSourceFile();
      const locs = ls.findRenameLocations(sf.fileName, node.getStart(), false, false, { providePrefixAndSuffixTextForRename: false });
      if (!locs) {
        missing.push(`rename refused: ${t.from}`);
        continue;
      }
      for (const l of locs) {
        if (!isOutput(l.fileName)) continue;
        const oldText = program.getSourceFile(l.fileName)!.text.slice(l.textSpan.start, l.textSpan.start + l.textSpan.length);
        const last = (t.member ?? t.from).split('.').pop()!;
        if (oldText !== last && !(oldText === `'${last}'` || oldText === `"${last}"`)) continue;
        const to = t.to.split('.').pop()!;
        addEdit(l.fileName, { start: l.textSpan.start, end: l.textSpan.start + l.textSpan.length, text: oldText.startsWith("'") || oldText.startsWith('"') ? oldText[0] + to + oldText[0] : to, why: `${t.from}->${t.to}` });
        if (!t.owner && t.half === 'data') {
          const k = lower(l.fileName);
          if (!dataTopByFile.has(k)) dataTopByFile.set(k, new Set());
          dataTopByFile.get(k)!.add(t.to);
        }
      }
      if (++n % 50 === 0) log(`${jsx}: ${n}/${targets.length} renames`);
    }
    const libSpecs = reverse ? new Set([map.modules[0].new]) : new Set(map.modules.map((m) => m.old));
    const uiOld = new Map(map.symbols.filter((s) => s.half === 'ui').map((s) => [s.old, s.new]));
    const uiNew = new Map(map.symbols.filter((s) => s.half === 'ui').map((s) => [s.new, s.old]));
    const aliasNew = new Set(map.symbols.filter((s) => s.half === 'ui-alias').map((s) => s.new));
    for (const sf of program.getSourceFiles()) {
      if (!isOutput(sf.fileName)) continue;
      for (const st of sf.statements) {
        if (!ts.isImportDeclaration(st) || !ts.isStringLiteral(st.moduleSpecifier) || !libSpecs.has(st.moduleSpecifier.text)) continue;
        const nb = st.importClause?.namedBindings;
        if (!nb || !ts.isNamedImports(nb)) continue;
        for (const el of nb.elements) {
          if (!el.propertyName) continue;
          const local = el.name.text;
          const imported = el.propertyName.text;
          const to = reverse ? (aliasNew.has(imported) ? uiNew.get(local) : undefined) : uiOld.get(local);
          if (!to || imported === local) continue;
          for (const l of ls.findRenameLocations(sf.fileName, el.name.getStart(sf), false, false, { providePrefixAndSuffixTextForRename: false }) ?? []) {
            if (!isOutput(l.fileName)) continue;
            addEdit(l.fileName, { start: l.textSpan.start, end: l.textSpan.start + l.textSpan.length, text: to, why: `local ${local}->${to}` });
          }
        }
      }
    }
    log(`${jsx}: renames collected`);

    if (mode === 'fbw') stringEdits(program, project, map, reverse, isOutput, addEdit, plugin, flag('update-map') && !reverse, jsx);
    log(`${jsx}: strings collected`);
  }
  if (missing.length) {
    console.log(`targets without a declaration (${missing.length}):`);
    for (const m of [...new Set(missing)]) console.log('  ' + m);
  }

  const written: string[] = [];
  const moduleOld = new Map(map.modules.map((m) => [m.old, m.new]));
  const uiAliasOld = new Set(map.symbols.filter((s) => s.half === 'ui-alias' || s.old.match(/^(cell|derive|list|feed)$/)).map((s) => s.new));
  for (const [k, orig] of fileTexts) {
    const { text, eol, bom } = readText(orig);
    const sf = ts.createSourceFile(orig, text, ts.ScriptTarget.Latest, true, orig.endsWith('x') ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
    const fe = [...(edits.get(k) ?? [])];
    for (const st of sf.statements) {
      if ((ts.isImportDeclaration(st) || ts.isExportDeclaration(st)) && st.moduleSpecifier && ts.isStringLiteral(st.moduleSpecifier)) {
        const spec = st.moduleSpecifier.text;
        let to: string | undefined;
        if (!reverse && moduleOld.has(spec)) to = moduleOld.get(spec);
        if (reverse && spec === map.modules[0].new) to = mode === 'lib' ? 'xp-ui' : reverseModule(st, uiAliasOld, fe, text);
        if (to) fe.push({ start: st.moduleSpecifier.getStart(sf) + 1, end: st.moduleSpecifier.getEnd() - 1, text: to, why: 'module' });
        if (reverse && spec === map.dataModule && ts.isImportDeclaration(st)) {
          let end = st.getEnd();
          if (text.slice(end, end + eol.length) === eol) end += eol.length;
          fe.push({ start: st.getStart(sf), end, text: '', why: 'data import' });
        }
      }
    }
    const needData = new Set<string>();
    const textPairs = mode === 'lib' ? map.libraryText : (map as any).fbwText ?? [];
    for (const lt of textPairs) {
      const ltFiles = [lt.file, ...map.files.filter((f) => f.old === lt.file).map((f) => f.new)];
      if (!ltFiles.some((f) => slash(orig).endsWith('/' + f))) continue;
      for (const [o, nw] of lt.pairs) {
        const [from, to] = reverse ? [nw, o] : [o, nw];
        const a = from.split('\n').join(eol);
        if (!a) throw new Error(`${orig}: empty text pair`);
        let at = text.indexOf(a);
        if (at < 0) throw new Error(`${orig}: text pair not found: ${from.slice(0, 60)}`);
        while (at >= 0) {
          fe.push({ start: at, end: at + a.length, text: to.split('\n').join(eol), why: 'libtext' });
          at = text.indexOf(a, at + a.length);
        }
      }
      if (!reverse) for (const i of lt.imports) needData.add(i);
    }
    const pairRanges = fe.filter((e) => e.why === 'libtext');
    for (const name of dataTopByFile.get(k) ?? []) {
      if (fe.some((e) => e.why.endsWith('->' + name) && !pairRanges.some((r) => e.start >= r.start && e.end <= r.end))) needData.add(name);
    }
    let body = applyEdits(text, fe, orig);
    for (const j of map.jsx) {
      const [a, b] = reverse ? [j.new, j.old] : [j.old, j.new];
      body = body.split(`@jsx ${a}`).join(`@jsx ${b}`).split(`@jsxFrag ${a}`).join(`@jsxFrag ${b}`);
    }
    if (!reverse && needData.size) body = addDataImport(body, [...needData].sort(), map.dataModule, eol);
    let rel = slash(orig).slice(slash(src).length + 1);
    for (const f of map.files) {
      const [a, b] = reverse ? [f.new, f.old] : [f.old, f.new];
      if (rel === a) rel = b;
    }
    if (mode === 'lib') body = renameFileImports(body, map, reverse);
    if (mode === 'lib' && /_check\.ts$/.test(orig)) body = checkFileNames(body, map, reverse);
    const dest = path.join(out, rel);
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    body = bom + body;
    if (!fs.existsSync(dest) || fs.readFileSync(dest, 'utf8') !== body) fs.writeFileSync(dest, body);
    written.push(rel);
  }
  if (mode === 'fbw') written.push(...convertConfigs(src, out, map, reverse));
  fs.writeFileSync(path.join(out, reverse ? '.reverse-files.txt' : '.forward-files.txt'), written.sort().join('\n') + '\n');
  if (flag('update-map') && !reverse) fs.writeFileSync(mapFile, JSON.stringify(map, null, 1) + '\n');
  log(`wrote ${written.length} files to ${out}`);
}

function checkFileNames(body: string, map: ApiMap, reverse: boolean): string {
  const pairs = new Map<string, Set<string>>();
  const add = (a: string, b: string) => {
    if (a === b) return;
    if (!pairs.has(a)) pairs.set(a, new Set());
    pairs.get(a)!.add(b);
  };
  for (const s of map.symbols) {
    add(reverse ? s.new : s.old, reverse ? s.old : s.new);
    for (const m of s.members) {
      const [a, b] = reverse ? [m.new, m.old] : [m.old, m.new];
      if (!a.includes('.')) add(a, b);
    }
  }
  const targets = new Set([...pairs.values()].flatMap((v) => [...v]));
  for (const [a, set] of pairs) {
    if (set.size === 1 && !targets.has(a)) body = body.split(`'${a}'`).join(`'${[...set][0]}'`);
    if (set.size !== 1 || targets.has(a) || a.length < 4 || /^(number|string|boolean|symbol|bigint|object|unknown|never|void|any|null|undefined|this|type)$/.test(a)) continue;
    const b = [...set][0];
    body = body.replace(new RegExp(`(?<![\\w.$])${a}(?![\\w$])`, 'g'), (m, off) => {
      const before = body.slice(Math.max(0, off - 7), off);
      return /(Api|Mine|Impl)\.$/.test(before) ? m : b;
    });
  }
  return body;
}

function reverseModule(st: ts.ImportDeclaration | ts.ExportDeclaration, aliasOld: Set<string>, _fe: Edit[], _text: string): string {
  const named: string[] = [];
  if (ts.isImportDeclaration(st)) {
    const nb = st.importClause?.namedBindings;
    if (nb && ts.isNamedImports(nb)) for (const el of nb.elements) named.push((el.propertyName ?? el.name).text);
  }
  return named.length && named.every((n) => aliasOld.has(n)) ? 'xp-kit' : '@microsoft/msfs-sdk';
}

function addDataImport(body: string, names: string[], module: string, eol: string): string {
  const sf = ts.createSourceFile('x.tsx', body, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const imports = sf.statements.filter((s) => ts.isImportDeclaration(s));
  const line = `import { ${names.join(', ')} } from '${module}';`;
  if (imports.length) {
    const last = imports[imports.length - 1];
    return body.slice(0, last.getEnd()) + eol + line + body.slice(last.getEnd());
  }
  const first = sf.statements[0];
  const at = first ? first.getStart(sf) : body.length;
  return body.slice(0, at) + line + eol + body.slice(at);
}

function renameFileImports(body: string, map: ApiMap, reverse: boolean): string {
  for (const f of map.files) {
    const [a, b] = reverse ? [f.new, f.old] : [f.old, f.new];
    const from = path.basename(a).replace(/\.tsx?$/, '');
    const to = path.basename(b).replace(/\.tsx?$/, '');
    const dir = path.basename(path.dirname(a));
    body = body.split(`'./${from}'`).join(`'./${to}'`).split(`'../${dir}/${from}'`).join(`'../${dir}/${to}'`);
  }
  return body;
}

function convertConfigs(src: string, out: string, map: ApiMap, reverse: boolean): string[] {
  const done: string[] = [];
  const roots = ['fbw-a380x/src', 'fbw-common/src', 'fbw-a32nx/src'];
  for (const r of roots) {
    for (const f of walk(path.join(src, r))) {
      if (!/tsconfig[^/]*\.json$/.test(f) || f.includes('/node_modules/')) continue;
      let t = fs.readFileSync(f, 'utf8');
      const before = t;
      for (const j of map.jsx) {
        const [a, b] = reverse ? [j.new, j.old] : [j.old, j.new];
        t = t.split(`"${a}"`).join(`"${b}"`);
      }
      if (t !== before) {
        const rel = slash(f).slice(slash(src).length + 1);
        fs.mkdirSync(path.dirname(path.join(out, rel)), { recursive: true });
        fs.writeFileSync(path.join(out, rel), t);
        done.push(rel);
      }
    }
  }
  return done;
}

// ---------------------------------------------------------------------------
// String edits (variable names and wire names).

function stringEdits(program: ts.Program, project: Project, map: ApiMap, reverse: boolean, isOutput: (f: string) => boolean, addEdit: (f: string, e: Edit) => void, plugin: string, update: boolean, jsx: string) {
  const checker = program.getTypeChecker();
  const files = new Set(program.getSourceFiles().filter((s) => isOutput(s.fileName)).map((s) => lower(s.fileName)));
  const q = (oldName: string) => {
    if (!reverse) return oldName;
    const [top, ...rest] = oldName.split('.');
    const s = map.symbols.find((x) => x.old === top);
    if (!s) return oldName;
    if (!rest.length) return s.new;
    const seg = new Map(s.members.filter((x) => !x.old.includes('.')).map((x) => [x.old, x.new]));
    const m = s.members.find((x) => x.old === rest.join('.'));
    const path = rest.slice(0, -1).map((p) => seg.get(p) ?? p);
    return `${s.new}.${[...path, m ? m.new : rest[rest.length - 1]].join('.')}`;
  };
  const varSinks = ['SimVar.GetSimVarValue', 'SimVar.SetSimVarValue', 'SimVar.GetGameVarValue', 'SimVar.GetGlobalVarValue', 'SimVar.GetRegisteredId', 'SimVar.GetSimVarValueFast', 'SimVar.SimVarBatch.add'];
  const wireSinks = ['Coherent.call', 'Coherent.on', 'Coherent.off', 'Coherent.trigger', 'RegisterViewListener', 'ViewListener.ViewListener.on', 'ViewListener.ViewListener.off', 'ViewListener.ViewListener.call', 'ViewListener.ViewListener.trigger', 'ViewListener.ViewListener.triggerToAllSubscribers', 'LaunchFlowEvent'];
  const sinks: Sink[] = [...varSinks.map((c) => ({ kind: 'var' as const, callee: q(c), arg: 0 })), ...wireSinks.map((c) => ({ kind: 'wire' as const, callee: q(c), arg: 0 }))];
  const propertySinks = [q('SimVarDefinition.name')];
  const typingsDir = lower(projectRoot(project) + '/fbw-common/src/typings/');
  const resolveQualified = (s: ts.Symbol): string | null => {
    const d = s.declarations?.[0];
    if (!d) return null;
    const f = lower(d.getSourceFile().fileName);
    const ownLib = f.includes('/clean-avionics/') || (LIB_DIR !== '' && f.startsWith(LIB_DIR));
    if (!ownLib && !f.startsWith(typingsDir)) return null;
    const names: string[] = [];
    for (let p: ts.Node | undefined = d; p && !ts.isSourceFile(p); p = p.parent) {
      const nm = (p as any).name;
      if ((ts.isClassDeclaration(p) || ts.isInterfaceDeclaration(p) || ts.isModuleDeclaration(p) || ts.isFunctionDeclaration(p) || ts.isMethodDeclaration(p) || ts.isMethodSignature(p) || ts.isPropertySignature(p) || ts.isPropertyDeclaration(p) || ts.isVariableDeclaration(p) || ts.isFunctionDeclaration(p)) && nm && ts.isIdentifier(nm) && !(ts.isModuleDeclaration(p) && nm.text === 'global')) names.unshift(nm.text);
    }
    return names.join('.');
  };
  const found = flowStrings(program, files, sinks, propertySinks, resolveQualified, [q('SimVarPublisher')]);

  const wireMap = new Map(map.wire.map((w) => (reverse ? [w.new, w.old] : [w.old, w.new])));
  const mapped = loadPluginMapping(plugin);
  const datarefCount = new Map<string, number>();
  for (const v of mapped.values()) datarefCount.set(v, (datarefCount.get(v) ?? 0) + 1);
  const uniqueDataref = new Set([...mapped.entries()].filter(([, v]) => datarefCount.get(v) === 1).map(([k]) => k));
  const byOld = new Map(map.strings.map((s) => [s.piece + '\u0000' + s.old, s]));
  const byNew = new Map(map.strings.map((s) => [s.piece + '\u0000' + s.new, s]));
  const quoteEdit = (node: ts.Node, sfText: string, start: number, end: number, newInner: string, why: string) => addEdit(node.getSourceFile().fileName, { start, end, text: newInner, why });

  for (const [node, kind] of found) {
    const sf = node.getSourceFile();
    if (kind === 'wire') {
      if (!(ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node))) continue;
      const to = wireMap.get(node.text);
      if (to) quoteEdit(node, sf.text, node.getStart(sf) + 1, node.getEnd() - 1, to, 'wire');
      continue;
    }
    const pieces: { piece: StringEntry['piece']; text: string; start: number; end: number }[] = [];
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
      pieces.push({ piece: 'full', text: node.text, start: node.getStart(sf) + 1, end: node.getEnd() - 1 });
    } else if (ts.isTemplateExpression(node)) {
      pieces.push({ piece: 'head', text: node.head.text, start: node.head.getStart(sf) + 1, end: node.head.getEnd() - 2 });
      node.templateSpans.forEach((sp, i) => {
        const last = i === node.templateSpans.length - 1;
        pieces.push({ piece: last ? 'tail' : 'middle', text: sp.literal.text, start: sp.literal.getStart(sf) + 1, end: sp.literal.getEnd() - (last ? 1 : 2) });
      });
    }
    if (!pieces.length) continue;
    if (sf.text.slice(pieces[0].start, pieces[0].end) !== pieces[0].text) continue;
    if (!reverse) {
      const head = pieces[0];
      if (OTHER_CATEGORY.test(head.text) || head.text.startsWith('GAME:')) continue;
      const ok = head.piece === 'full' ? SIM_FULL.test(head.text) || SIM_HEAD.test(head.text) : SIM_HEAD.test(head.text) && head.text.length > 0;
      if (!ok || !/[A-Z]/.test(head.text)) continue;
      for (const p of pieces) {
        if (p.piece !== 'full' && p.piece !== 'head' && !/^[A-Z0-9 :_#]*$/.test(p.text)) continue;
        const k = p.piece + '\u0000' + p.text;
        let e = byOld.get(k);
        if (!e) {
          e = { piece: p.piece, old: p.text, new: newVarName(p.text, p.piece, mapped, uniqueDataref) };
          if (p.piece === 'full') e.internal = p.text.replace(/^A:/, '');
          map.strings.push(e);
          byOld.set(k, e);
        }
        if (e.new !== p.text) addEdit(sf.fileName, { start: p.start, end: p.end, text: e.new, why: 'var' });
      }
    } else {
      const head = byNew.get(pieces[0].piece + '\u0000' + pieces[0].text);
      if (!head) continue;
      for (const p of pieces) {
        const e = byNew.get(p.piece + '\u0000' + p.text);
        if (e && e.old !== p.text) addEdit(sf.fileName, { start: p.start, end: p.end, text: e.old, why: 'var' });
      }
    }
  }
  if (update) {
    const seen = new Map<string, string>();
    for (const s of map.strings) {
      const k = s.piece + '\u0000' + s.new;
      if (seen.has(k) && seen.get(k) !== s.old) throw new Error(`two variable names map to ${s.new}: ${seen.get(k)} and ${s.old}`);
      seen.set(k, s.old);
    }
  }
  void checker;
  void jsx;
}

function projectRoot(project: Project): string {
  const anyFile = project.getSourceFiles().find((s) => /\/fbw-(a380x|common)\//.test(slash(s.getFilePath())));
  if (!anyFile) return '';
  const p = slash(anyFile.getFilePath());
  return p.slice(0, p.search(/\/fbw-(a380x|common)\//));
}

// ---------------------------------------------------------------------------
// Compare two trees after formatting.

async function compare(a: string, b: string, listFile: string) {
  const prettier = await import('prettier');
  const files = fs.readFileSync(listFile, 'utf8').split('\n').filter(Boolean);
  let diffs = 0;
  const report: string[] = [];
  for (const rel of files) {
    const fa = path.join(a, rel);
    const fb = path.join(b, rel);
    if (!fs.existsSync(fa) || !fs.existsSync(fb)) {
      diffs++;
      report.push(`missing: ${rel}`);
      continue;
    }
    const parser = rel.endsWith('.json') ? 'json5' : 'typescript';
    const fmt = async (f: string) => {
      const t = fs.readFileSync(f, 'utf8').replace(/\r\n/g, '\n');
      try {
        return await prettier.format(t, { parser, printWidth: 120, singleQuote: true, endOfLine: 'lf' });
      } catch {
        return t;
      }
    };
    const [x, y] = [await fmt(fa), await fmt(fb)];
    if (x !== y) {
      diffs++;
      const xl = x.split('\n');
      const yl = y.split('\n');
      let i = 0;
      while (i < xl.length && xl[i] === yl[i]) i++;
      report.push(`differs: ${rel}:${i + 1}\n  - ${xl[i]}\n  + ${yl[i]}`);
    }
  }
  console.log(report.slice(0, 60).join('\n'));
  console.log(`${files.length} files compared, ${diffs} differ`);
  process.exitCode = diffs ? 1 : 0;
}

const [cmd, dir] = argv;
if (cmd === 'compare') await compare(argv[1], argv[2], argv[3]);
else if ((cmd === 'lib' || cmd === 'fbw') && (dir === 'forward' || dir === 'reverse')) convert(cmd, dir);
else {
  console.log('usage: node convert.ts lib|fbw forward|reverse --src <dir> --out <dir> [...] | compare <a> <b> <list>');
  process.exitCode = 2;
}
