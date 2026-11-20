const ts = require('E:/A380/fbw-int/fbw-aircraft-ts/node_modules/typescript');
const fs = require('fs'), path = require('path'), vm = require('vm');
const root = 'E:/A380/fbw-int/fbw-aircraft-ts/fbw-a380x/src/systems/instruments/src/MsfsAvionicsCommon/EcamMessages';
const cache = {};
const idxSrc = fs.readFileSync(path.join(root, 'index.ts'), 'utf8');
const pre = (idxSrc.match(/export enum \w+ \{[\s\S]*?\n\}/g) || []).concat(idxSrc.match(/export const FMS_PRED_UNRELIABLE[\s\S]*?\n\};/));
const enumMod = { exports: {} };
vm.runInNewContext(ts.transpileModule(pre.join('\n'), { compilerOptions: { module: 1, target: 7 } }).outputText, { module: enumMod, exports: enumMod.exports });
const wrap = (ex) => new Proxy(ex, { get: (t, k) => (k in enumMod.exports ? enumMod.exports[k] : t[k]) });
function load(file) { file = path.resolve(file);
  if (cache[file]) return cache[file].exports;
  let src = fs.readFileSync(file, 'utf8');
  if (file.endsWith('EcamMessages/index.ts')) {
    const e = src.match(/export enum ChecklistLineStyle \{[\s\S]*?\n\}/)[0];
    src = e + '\n' + src.replace(e, '');
  }
  const out = ts.transpileModule(src, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 } }).outputText;
  const m = { exports: {} };
  cache[file] = m;
  const req = (p) => { 
    if (!p.startsWith('.')) return {};
    const f = path.resolve(path.dirname(file), p);
    for (const c of [f + '.ts', path.join(f, 'index.ts')]) if (fs.existsSync(c)) return wrap(load(c));
    return new Proxy({}, { get: () => undefined });
  };
  vm.runInNewContext(out, { module: m, exports: m.exports, require: req, console });
  return m.exports;
}
const idx = load(path.join(root, 'index.ts'));
const strip = (o) => JSON.parse(JSON.stringify(o, (k, v) => (typeof v === 'function' ? undefined : v)));
const np = load(path.join(root, 'NormalProcedures.ts'));
const res = {};
res.EcamNormalProcedures = strip(np.EcamNormalProcedures);
res.deferredProcedureIds = strip(np.deferredProcedureIds);
for (const k of Object.keys(idx)) if (k.startsWith('Ecam')) res[k] = strip(idx[k]);
console.error(Object.keys(res).map((k) => k + ':' + Object.keys(res[k]).length).join(' '));
fs.writeFileSync(process.argv[2], JSON.stringify(res));
