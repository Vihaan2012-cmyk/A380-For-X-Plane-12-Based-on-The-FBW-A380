// Lists member accesses in converted FlyByWire sources that still use a library member's old
// name on a value the checker types as any, which the type-aware rename cannot reach.
//   node scan-any.ts [--fork <converted fork>] [--out <report.json>]

import { Project, ts } from 'ts-morph';
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const { compilerOptions, entries, slash } = require('./lib/project.cjs');

const argv = process.argv.slice(2);
const opt = (name: string, def: string): string => (argv.includes('--' + name) ? argv[argv.indexOf('--' + name) + 1] : def);
const fork = slash(path.resolve(opt('fork', 'E:/fbw-int/fbw-own-api')));
const out = opt('out', '');
const library = slash(path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/(\w:)/, '$1')), '..'));
const map = JSON.parse(fs.readFileSync('E:/fbw-int/xplane-js-private/api-map.json', 'utf8'));

const renamed = new Map<string, Set<string>>();
for (const s of map.symbols) {
  for (const m of s.members || []) {
    if (m.old === m.new) continue;
    if (!renamed.has(m.old)) renamed.set(m.old, new Set());
    renamed.get(m.old)!.add(`${s.new}.${m.new}`);
  }
}

const libModules = { 'xplane-js/ui': path.join(library, 'lib/src/index.ts'), 'xplane-js/data': path.join(library, 'spec/data-api.d.ts') };
const modulesDir = path.join(fork, 'node_modules');
const e = entries(fork);
const hits: { file: string; line: number; name: string; to: string[]; text: string }[] = [];
const seen = new Set<string>();
for (const [jsx, roots] of [
  [['UI.node', 'UI.Group'], e.fs.map((x: any) => x.file)],
  ['react', e.react.map((x: any) => x.file)],
] as const) {
  const project = new Project({ compilerOptions: compilerOptions(ts, { fork, modulesDir, libModules, jsx, typings: 'xplane' }), skipAddingFilesFromTsConfig: true });
  for (const r of roots) project.addSourceFileAtPath(r);
  project.resolveSourceFileDependencies();
  const checker = project.getProgram().compilerObject.getTypeChecker();
  for (const sf of project.getProgram().compilerObject.getSourceFiles()) {
    const f = slash(sf.fileName);
    if (!f.toLowerCase().startsWith(fork.toLowerCase() + '/') || f.includes('/node_modules/') || f.includes('/xplane/')) continue;
    const record = (name: string, receiver: ts.Node, at: ts.Node): void => {
      if (!renamed.has(name) || !(checker.getTypeAtLocation(receiver).flags & ts.TypeFlags.Any)) return;
      const { line } = sf.getLineAndCharacterOfPosition(at.getStart(sf));
      const key = `${f}:${line}:${name}`;
      if (seen.has(key)) return;
      seen.add(key);
      hits.push({ file: path.relative(fork, f).split(path.sep).join('/'), line: line + 1, name, to: [...renamed.get(name)!], text: at.parent.getText(sf).split('\n')[0].slice(0, 120) });
    };
    const visit = (node: ts.Node): void => {
      if (ts.isPropertyAccessExpression(node)) record(node.name.text, node.expression, node.name);
      else if (ts.isElementAccessExpression(node) && ts.isStringLiteralLike(node.argumentExpression)) record(node.argumentExpression.text, node.expression, node.argumentExpression);
      else if (ts.isBindingElement(node) && ts.isObjectBindingPattern(node.parent)) {
        const key = node.propertyName ?? node.name;
        if (ts.isIdentifier(key) || ts.isStringLiteralLike(key)) record(key.text, node.parent, node);
      }
      ts.forEachChild(node, visit);
    };
    visit(sf);
  }
}
hits.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line);
console.log(`${hits.length} accesses on any-typed values use an old member name`);
const byName = new Map<string, number>();
for (const h of hits) byName.set(h.name, (byName.get(h.name) ?? 0) + 1);
console.log([...byName].sort((a, b) => b[1] - a[1]).map(([n, c]) => `${n} ${c}`).join(', '));
if (out) fs.writeFileSync(out, JSON.stringify(hits, null, 1));
