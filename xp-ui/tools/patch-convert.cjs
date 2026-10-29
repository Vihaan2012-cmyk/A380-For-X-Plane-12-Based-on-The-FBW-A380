'use strict';
// node patch-convert.cjs <patches.json> <old html_ui> <new html_ui> <out.json> [--map api-map.json] [--names xp_names.txt]
// Moves the plugin's serve-time patches from the old-name bundles to the converted ones: each patch's anchor is
// found again in the converted bundle by token alignment, and its replacement is renamed with what the alignment
// and the API map say.

const ts = require('typescript');
const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
const opt = (n, d) => (args.includes('--' + n) ? args[args.indexOf('--' + n) + 1] : d);
const [patchFile, oldRoot, newRoot, outFile] = args;
const map = JSON.parse(fs.readFileSync(opt('map', 'E:/fbw-int/xplane-js-private/api-map.json'), 'utf8'));
const namesFile = opt('names', 'E:/fbw-int/plugin/src/xp_names.txt');

const K = ts.SyntaxKind;
const ENDS_EXPR = new Set([K.Identifier, K.NumericLiteral, K.StringLiteral, K.CloseParenToken, K.CloseBracketToken, K.CloseBraceToken, K.ThisKeyword, K.SuperKeyword, K.TrueKeyword, K.FalseKeyword, K.NullKeyword, K.NoSubstitutionTemplateLiteral, K.TemplateTail, K.RegularExpressionLiteral, K.PlusPlusToken, K.MinusMinusToken]);

function tokens(text) {
  const sc = ts.createScanner(ts.ScriptTarget.Latest, true, ts.LanguageVariant.Standard, text);
  const out = [];
  const stack = [];
  let prev = null;
  for (;;) {
    let k = sc.scan();
    if (k === K.EndOfFileToken) break;
    if (k === K.CloseBraceToken && stack.length && stack[stack.length - 1] === 'tpl') {
      k = sc.reScanTemplateToken(false);
      if (k === K.TemplateTail) stack.pop();
    } else if (k === K.CloseBraceToken && stack.length) stack.pop();
    else if (k === K.OpenBraceToken) stack.push('{');
    if (k === K.TemplateHead) stack.push('tpl');
    if ((k === K.SlashToken || k === K.SlashEqualsToken) && !(prev && ENDS_EXPR.has(prev))) k = sc.reScanSlashToken();
    const t = { k, text: sc.getTokenText(), pos: sc.getTokenStart ? sc.getTokenStart() : sc.getTokenPos(), end: sc.getTokenEnd ? sc.getTokenEnd() : sc.getTextPos() };
    if (k === K.Identifier || ts.isKeyword(k)) t.id = sc.getTokenValue() || t.text;
    if (k === K.StringLiteral || k === K.NoSubstitutionTemplateLiteral || k === K.TemplateHead || k === K.TemplateMiddle || k === K.TemplateTail) t.str = sc.getTokenValue();
    out.push(t);
    prev = k;
  }
  return out;
}

const oldNames = new Map();
const addOld = (o, n) => {
  if (o === n) return;
  if (!oldNames.has(o)) oldNames.set(o, new Set());
  oldNames.get(o).add(n);
};
for (const s of map.symbols) {
  addOld(s.old, s.new);
  for (const m of s.members) addOld(m.old.split('.').pop(), m.new.split('.').pop());
}
for (const u of map.untyped || []) addOld(u.old, u.new);
const strMap = new Map();
const headMap = new Map();
const tailMap = new Map();
for (const s of map.strings || []) {
  if (s.old === s.new) continue;
  ({ full: strMap, head: headMap, tail: tailMap })[s.piece]?.set(s.old, s.new);
}
for (const w of map.wire || []) if (w.old !== w.new) strMap.set(w.old, w.new);
if (fs.existsSync(namesFile)) {
  for (const line of fs.readFileSync(namesFile, 'utf8').split(/\r?\n/)) {
    if (!line.startsWith('=')) continue;
    const [n, o] = line.slice(1).split('\t');
    if (o && n && o !== n && !strMap.has(o)) strMap.set(o, n);
  }
}
const strFor = (t) => (t.k === K.TemplateHead ? headMap : t.k === K.TemplateTail ? tailMap : strMap).get(t.str);
const stem = (s) => s.replace(/\d+$/, '');
const numVal = (t) => Number(t.text.replace(/_/g, ''));

const memo = new Map();
const tokFile = (root, rel) => {
  const key = root + rel;
  if (!memo.has(key)) {
    const file = path.join(root, rel.replace(/^\//, ''));
    const text = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
    memo.set(key, { text, toks: tokens(text) });
  }
  return memo.get(key);
};

const suffixed = (x, y) => stem(x) === stem(y) && stem(x).length >= 4;

function same(a, b, pairs, member = false) {
  if (a.id !== undefined && b.id !== undefined && (a.k === K.Identifier || b.k === K.Identifier)) {
    const key = (member ? '.' : '') + a.id;
    if (a.id === b.id) {
      if (!pairs.has(key)) pairs.set(key, a.id);
      return pairs.get(key) === a.id;
    }
    const known = pairs.get(key);
    if (known !== undefined) return known === b.id;
    const ok = (oldNames.has(a.id) && [...oldNames.get(a.id)].some((n) => n === b.id || suffixed(n, b.id))) || suffixed(a.id, b.id);
    if (ok) pairs.set(key, b.id);
    return ok;
  }
  if (a.str !== undefined && b.str !== undefined && a.k === b.k) {
    if (a.str === b.str) return true;
    return strFor(a) === b.str;
  }
  if (a.k === K.NumericLiteral && b.k === K.NumericLiteral) return numVal(a) === numVal(b);
  return a.k === b.k && a.text === b.text;
}

function align(find, newToks) {
  const f = tokens(find);
  if (!f.length) return { hits: [] };
  const hits = [];
  for (let i = 0; i + f.length <= newToks.length; i++) {
    if (!same(f[0], newToks[i], new Map())) continue;
    const pairs = new Map();
    let ok = true;
    for (let j = 0; j < f.length && ok; j++) ok = same(f[j], newToks[i + j], pairs, j > 0 && f[j - 1].k === K.DotToken);
    if (ok) hits.push({ at: i, n: f.length, pairs });
  }
  return { hits, f };
}

const declared = (text, name) => {
  const re = new RegExp(`(?:^|[\\s;{(,])(?:var|let|const|class|function)\\s+(${name}\\d*)\\b`, 'gm');
  const found = new Set();
  for (const m of text.matchAll(re)) found.add(m[1]);
  return [...found];
};

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const used = (text, receiver, name) => new RegExp(`${receiver.map(esc).join('\\s*')}\\s*\\.\\s*${esc(name)}\\b`).test(text);

function receiverOf(toks, conv, dot) {
  const parts = [];
  let j = dot - 1;
  while (j >= 0) {
    const t = toks[j];
    if (t.k === K.CloseParenToken && toks[j - 1] && toks[j - 1].k === K.OpenParenToken) {
      parts.unshift('(', ')');
      j -= 2;
    } else if (t.id !== undefined) {
      parts.unshift(conv[j]);
      if (toks[j - 1] && toks[j - 1].k === K.DotToken) {
        parts.unshift('.');
        j -= 2;
      } else break;
    } else return [];
    if (parts[0] === '(' && !(toks[j] && toks[j].id !== undefined)) return [];
  }
  return parts;
}

function convertCode(code, ownPairs, filePairs, newText, overrides = {}) {
  const toks = tokens(code);
  const conv = toks.map((t) => t.text);
  let out = '';
  let last = 0;
  const left = [];
  const renames = [];
  const absent = [];
  const own = new Set(toks.filter((t) => t.id !== undefined).map((t) => t.id));
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i];
    let rep = null;
    if (t.k === K.Identifier) {
      const prevDot = i > 0 && toks[i - 1].k === K.DotToken;
      const isKey = !prevDot && toks[i + 1] && toks[i + 1].k === K.ColonToken && i > 0 && (toks[i - 1].k === K.OpenBraceToken || toks[i - 1].k === K.CommaToken);
      if (isKey) {
      } else if (prevDot) {
        const cands = new Set([ownPairs.get('.' + t.id), filePairs.get('.' + t.id), ...(oldNames.get(t.id) || [])].filter((c) => c && c !== t.id));
        if (cands.size && ownPairs.get('.' + t.id) !== t.id) {
          const recv = receiverOf(toks, conv, i - 1);
          const key = `${recv.join('')}.${t.id}`;
          if (overrides[key] !== undefined) rep = overrides[key];
          else if (recv.length && used(newText, recv, t.id)) rep = null;
          else {
            const hits = recv.length ? [...cands].filter((c) => used(newText, recv, c)) : [];
            if (hits.length === 1) rep = hits[0];
            else if (ownPairs.has('.' + t.id) && ownPairs.get('.' + t.id) !== t.id) rep = ownPairs.get('.' + t.id);
            else left.push(key);
          }
        }
      } else if (ownPairs.has(t.id)) rep = ownPairs.get(t.id);
      else if (oldNames.has(t.id)) {
        const cands = [...oldNames.get(t.id)];
        const local = cands.flatMap((c) => declared(newText, c));
        if (local.length === 1) rep = local[0];
        else if (cands.length === 1 && local.length === 0 && !declared(newText, t.id).length) rep = cands[0];
        else if (!declared(newText, t.id).length) left.push(t.id);
      }
    } else if (t.str !== undefined && strFor(t) !== undefined) {
      rep = t.text.replace(t.str, strFor(t));
    }
    if (rep !== null) conv[i] = rep;
    if (rep !== null && rep !== t.text) {
      renames.push(`${t.text} -> ${rep} @ ${code.slice(Math.max(0, t.pos - 40), t.end + 30).replace(/\s+/g, ' ')}`);
      out += code.slice(last, t.pos) + rep;
      last = t.end;
    }
    const name = rep !== null && t.k === K.Identifier ? rep : t.k === K.Identifier ? t.id : null;
    if (name && !own.has(name) && !new RegExp(`\\b${name.replace(/\$/g, '\\$')}\\b`).test(newText)) absent.push(name);
  }
  return { code: out + code.slice(last), left, renames, absent: [...new Set(absent)] };
}

function main() {
const overridesFile = opt('overrides', path.join(__dirname, 'patch-overrides.json'));
const overrides = fs.existsSync(overridesFile) ? JSON.parse(fs.readFileSync(overridesFile, 'utf8')) : {};
const patches = JSON.parse(fs.readFileSync(patchFile, 'utf8')).filter((p) => !(p.source || '').includes('oans'));
const rows = [];
const filePairs = new Map();
for (const p of patches) {
  const old = tokFile(oldRoot, p.path);
  const nw = tokFile(newRoot, p.path);
  const exactNew = nw.text.split(p.find).length - 1;
  let newFind = null;
  let pairs = new Map();
  let how = '';
  const fo = Object.entries(overrides.finds || {}).find(([k]) => { const [file, why] = k.split('|'); return p.path.endsWith('/' + file) && p.reason.startsWith(why); });
  if (fo) {
    newFind = fo[1] === 'keep' ? p.find : fo[1];
    how = fo[1] === 'keep' ? 'kept (plugin fuzzy match)' : 'override';
  } else if (exactNew === 1) {
    newFind = p.find;
    how = 'unchanged';
  } else {
    const { hits } = align(p.find, nw.toks);
    if (hits.length === 1) {
      const h = hits[0];
      newFind = nw.text.slice(nw.toks[h.at].pos, nw.toks[h.at + h.n - 1].end);
      pairs = h.pairs;
      how = 'aligned';
    } else how = `aligned ${hits.length} times`;
  }
  if (!filePairs.has(p.path)) filePairs.set(p.path, new Map());
  const fp = filePairs.get(p.path);
  for (const [a, b] of pairs) {
    if (!fp.has(a)) fp.set(a, b);
    else if (fp.get(a) !== b) fp.set(a, null);
  }
  rows.push({ p, newFind, how, pairs });
}
const out = rows.map(({ p, newFind, how, pairs }) => {
  const nw = tokFile(newRoot, p.path);
  const fp = new Map([...filePairs.get(p.path)].filter(([, b]) => b !== null));
  const conv = convertCode(p.replace, pairs, fp, nw.text, overrides[p.path.split('/').pop()] || {});
  const findConv = newFind === p.find ? { code: p.find, left: [] } : { code: newFind, left: [] };
  return { path: p.path, reason: p.reason, how, oldFind: p.find, find: newFind, oldReplace: p.replace, replace: conv.code, leftInReplace: [...new Set(conv.left)], renames: conv.renames, absent: conv.absent, findLeft: findConv.left };
});
fs.writeFileSync(outFile, JSON.stringify(out, null, 1) + '\n');
const moved = out.filter((r) => r.find !== null).length;
console.log(`${moved}/${out.length} anchors found in the converted bundles (${out.filter((r) => r.how === 'unchanged').length} unchanged, ${out.filter((r) => r.how === 'aligned').length} aligned)`);
for (const r of out.filter((x) => x.find === null)) console.log(`  NOT FOUND (${r.how}) ${r.path.split('/').pop()}: ${r.reason.slice(0, 90)}`);
const leftover = out.filter((r) => r.leftInReplace.length);
console.log(`${leftover.length} replacements still name old API members it could not resolve`);
for (const r of leftover.slice(0, 30)) console.log(`  ${r.path.split('/').pop()}: ${r.leftInReplace.join(', ')} | ${r.reason.slice(0, 70)}`);
}

if (require.main === module) main();
else module.exports = { tokens, same, align, convertCode, tokFile, oldNames, strFor };
