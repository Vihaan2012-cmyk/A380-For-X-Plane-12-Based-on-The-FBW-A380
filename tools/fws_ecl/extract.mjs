import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const PLUGIN = path.resolve(HERE, '..', '..');
const FBW = path.resolve(PLUGIN, '..', 'fbw-aircraft');
const FWS = path.join(FBW, 'fbw-a380x/src/systems/systems-host/CpiomC/FlightWarningSystem');
const OUT = path.join(PLUGIN, 'crates/fws_ecl/src');
const requireFbw = createRequire(path.join(FBW, 'package.json'));
const ts = requireFbw('typescript');
const esbuild = requireFbw('esbuild');

function runEntry() {
  const dir = mkdtempSync(path.join(tmpdir(), 'fws_ecl-'));
  try {
    const out = path.join(dir, 'ecl.cjs');
    esbuild.buildSync({
      entryPoints: [path.join(HERE, 'entry.ts')],
      bundle: true,
      platform: 'node',
      format: 'cjs',
      logLevel: 'warning',
      outfile: out,
    });
    const run = spawnSync(process.execPath, [out], { encoding: 'utf8', maxBuffer: 1 << 28 });
    if (run.status !== 0) throw new Error(run.stderr);
    return JSON.parse(run.stdout);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function parse(file) {
  const text = readFileSync(path.join(FWS, file), 'utf8');
  return ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
}

function propertyObject(sf, name) {
  let found = null;
  const visit = (node) => {
    if (found) return;
    if (ts.isPropertyDeclaration(node) && node.name.getText(sf) === name && node.initializer) {
      let init = node.initializer;
      while (ts.isAsExpression(init) || ts.isParenthesizedExpression(init)) init = init.expression;
      if (ts.isObjectLiteralExpression(init)) found = init;
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  if (!found) throw new Error(`property ${name} not found in ${sf.fileName}`);
  return found;
}

function keyText(name) {
  if (ts.isNumericLiteral(name) || ts.isStringLiteral(name) || ts.isIdentifier(name)) return name.text;
  throw new Error(`unsupported key ${name.getText()}`);
}

function clean(node, sf) {
  let text = node.getText(sf);
  text = text.replace(/\/\/[^\n]*/g, '');
  return text.replace(/\s+/g, ' ').trim();
}

function inputsOf(text) {
  const found = [];
  const add = (name) => {
    if (!found.includes(name)) found.push(name);
  };
  const re = /this\.fws\.(\w+)(?![\w(])|this\.(partSplrs|mostSplrs)\b|SimVar\.GetSimVarValue\(\s*'([^']+)'/g;
  let m;
  while ((m = re.exec(text))) {
    if (m[1]) add(`fws.${m[1]}`);
    else if (m[2]) add(`inop.${m[2]}`);
    else add(m[3]);
  }
  return found;
}

function entries(obj, sf) {
  const out = [];
  for (const p of obj.properties) {
    if (!ts.isPropertyAssignment(p)) throw new Error(`unsupported entry ${p.getText(sf)}`);
    const fields = {};
    if (!ts.isObjectLiteralExpression(p.initializer)) throw new Error(`entry ${keyText(p.name)} is not an object`);
    for (const f of p.initializer.properties) {
      if (!ts.isPropertyAssignment(f)) throw new Error(`unsupported field ${f.getText(sf)}`);
      fields[f.name.getText(sf)] = f.initializer;
    }
    out.push({ key: keyText(p.name), fields });
  }
  return out;
}

function arrowBody(node, sf) {
  if (!ts.isArrowFunction(node)) throw new Error(`not an arrow function: ${clean(node, sf)}`);
  let body = node.body;
  while (ts.isParenthesizedExpression(body)) body = body.expression;
  return body;
}

function numberArray(node) {
  return node.elements.map((e) => Number(e.text));
}

function stringArray(node) {
  return node.elements.map((e) => e.text);
}

function boolField(f, name) {
  const n = f[name];
  if (!n) return false;
  if (n.kind === ts.SyntaxKind.TrueKeyword) return true;
  if (n.kind === ts.SyntaxKind.FalseKeyword) return false;
  throw new Error(`${name} is not a literal bool`);
}

function selectionOf(key, fields, sf) {
  const body = arrowBody(fields.whichCodeToReturn, sf);
  const text = clean(body, sf);
  const codesText = clean(fields.codesToReturn, sf);
  let m;
  if (key === '0000010') return { kind: 'ToMemo', expr: text };
  if (key === '0000020') return { kind: 'LdgMemo', expr: text };
  if ((m = /^\[ ?this\.fws\.adirsMessage1\(/.exec(text))) return { kind: 'AdirsAlignOver3Min', expr: text };
  if ((m = /^\[ ?this\.fws\.adirsMessage2\(/.exec(text))) return { kind: 'AdirsAlignUnder3Min', expr: text };
  if (key === '350000001' && /excessCabinAltitude/.test(codesText))
    return { kind: 'OxyPaxSys', expr: `${text} / ${codesText}` };
  if (text === '[0]') return { kind: 'Fixed0', expr: text };
  if ((m = /^\[ ?this\.fws\.(\w+)\.get\(\) \? (\d+) : (\d+) ?\]$/.exec(text)))
    return { kind: 'Ternary', input: `fws.${m[1]}`, t: Number(m[2]), f: Number(m[3]), expr: text };
  if ((m = /^this\.fws\.(\w+)\.get\(\) \? \[(\d+)\] : \[(\d+)\]$/.exec(text)))
    return { kind: 'Ternary', input: `fws.${m[1]}`, t: Number(m[2]), f: Number(m[3]), expr: text };
  if ((m = /^\[ ?this\.fws\.(\w+)\.get\(\) \? (\d+) : (\d+) ?\]$/.exec(text.replace(/\s+/g, ' '))))
    return { kind: 'Ternary', input: `fws.${m[1]}`, t: Number(m[2]), f: Number(m[3]), expr: text };
  if (/engine1Running[\s\S]*engine4Running[\s\S]*filter\(Boolean\)\.length > 1 \? 0 : 1/.test(text))
    return { kind: 'MoreThanOneEngineRunning', t: 0, f: 1, expr: text };
  return { kind: 'Custom', expr: text };
}

function inhibitOf(fields, sf) {
  const text = clean(arrowBody(fields.memoInhibit, sf), sf);
  if (text === 'false') return { kind: 'Never', expr: text };
  if (text === 'this.fws.toMemo.get() === 1 || this.fws.ldgMemo.get() === 1')
    return { kind: 'ToOrLdgMemoActive', expr: text };
  return { kind: 'Custom', expr: text };
}

function memoRules(sf, prop) {
  return entries(propertyObject(sf, prop), sf).map(({ key, fields }) => {
    const active = clean(fields.simVarIsActive, sf);
    const select = selectionOf(key, fields, sf);
    const codesText = clean(fields.codesToReturn, sf);
    const codes = [...codesText.matchAll(/'(\d+)'/g)].map((m) => m[1]);
    const inhibit = inhibitOf(fields, sf);
    return {
      key,
      phaseInhib: fields.flightPhaseInhib ? numberArray(fields.flightPhaseInhib) : [],
      activeExpr: active,
      activeInputs: inputsOf(active),
      select,
      selectInputs: inputsOf(select.expr + ' ' + codesText),
      codes,
      inhibit,
      inhibitInputs: inputsOf(inhibit.expr),
      leftSide: boolField(fields, 'leftSide'),
    };
  });
}

function suppressable(fields, sf) {
  const active = clean(fields.simVarIsActive, sf);
  return {
    activeExpr: active,
    activeInputs: inputsOf(active),
    notActiveWhen: fields.notActiveWhenItemActive ? stringArray(fields.notActiveWhenItemActive) : [],
    monitor: fields.monitorConfirmTime ? Number(fields.monitorConfirmTime.text) : null,
  };
}

function phaseOf(fields) {
  const t = fields.phase.getText();
  const m = /\.(AllPhases|ApprLdg)$/.exec(t);
  if (!m) throw new Error(`phase ${t}`);
  return m[1];
}

function statusRules(file, prop, withPhase) {
  const sf = parse(file);
  return entries(propertyObject(sf, prop), sf).map(({ key, fields }) => ({
    key,
    phase: withPhase ? phaseOf(fields) : null,
    pfd: boolField(fields, 'pfd'),
    redundancyLoss: boolField(fields, 'redundancyLoss'),
    ...suppressable(fields, sf),
  }));
}

function sensedChecks() {
  const sf = parse('FwsNormalChecklists.ts');
  const out = {};
  for (const { key, fields } of entries(propertyObject(sf, 'sensedItems'), sf)) {
    const body = arrowBody(fields.whichItemsChecked, sf);
    if (!ts.isArrayLiteralExpression(body)) throw new Error(`checks ${key} not an array`);
    out[key] = body.elements.map((e) => {
      if (e.kind === ts.SyntaxKind.NullKeyword) return null;
      const expr = clean(e, sf);
      return { expr, inputs: inputsOf(expr) };
    });
  }
  return out;
}

function rs(s) {
  let out = '"';
  for (const ch of String(s)) {
    const c = ch.codePointAt(0);
    if (ch === '"') out += '\\"';
    else if (ch === '\\') out += '\\\\';
    else if (c < 0x20 || c > 0x7e) out += `\\u{${c.toString(16)}}`;
    else out += ch;
  }
  return out + '"';
}

const strs = (a) => `&[${a.map(rs).join(', ')}]`;
const opt = (v, f) => (v === null || v === undefined ? 'None' : `Some(${f(v)})`);
const HEADER = '// Generated by tools/fws_ecl/extract.mjs from FlyByWire\'s A380X sources. Do not edit.\n\n';

function emitNormal(d, checks) {
  let o = HEADER + 'use crate::types::*;\n\n';
  o += `pub const WD_NUM_LINES: usize = ${d.wdNumLines};\npub const WD_LINE_CHARACTERS: usize = ${d.wdLineCharacters};\n\n`;
  o += `pub const DEFERRED_PROCEDURE_IDS: &[u32] = &[${d.deferredNormal.join(', ')}];\n\n`;
  const ids = Object.keys(d.normal).map(Number).sort((a, b) => a - b);
  o += 'pub static NORMAL_PROCEDURES: &[NormalProcedure] = &[\n';
  for (const id of ids) {
    const p = d.normal[id];
    const c = checks[id];
    if (!c || (!d.deferredNormal.includes(id) && c.length !== p.items.length)) throw new Error(`sensed list for ${id} does not match its items`);
    o += `    NormalProcedure {\n        id: ${id},\n        title: ${rs(p.title)},\n        only_activated_by_request: ${!!p.onlyActivatedByRequest},\n        deferred: ${d.deferredNormal.includes(id)},\n        items: &[\n`;
    for (const it of p.items) {
      if (it.time !== undefined) throw new Error('timed items are not expected in normal procedures');
      let kind = 'ItemKind::Special';
      if (it.condition) kind = 'ItemKind::Condition';
      else if (it.labelNotCompleted !== undefined)
        kind = `ItemKind::Action { label_not_completed: ${rs(it.labelNotCompleted)}, label_completed: ${opt(it.labelCompleted, rs)}, colon_if_completed: ${it.colonIfCompleted !== false} }`;
      o += `            Item { name: ${rs(it.name)}, sensed: ${it.sensed}, level: ${it.level ?? 0}, style: ${opt(it.style, (s) => `LineStyle::${s}`)}, kind: ${kind} },\n`;
    }
    o += '        ],\n        checks: &[\n';
    for (const ck of c) o += `            ${opt(ck, (x) => `Check { expr: ${rs(x.expr)}, inputs: ${strs(x.inputs)} }`)},\n`;
    o += '        ],\n    },\n';
  }
  o += '];\n';
  return o;
}

function emitMessages(d) {
  let o = HEADER + 'use crate::types::Message;\n\n';
  const table = (name, obj) => {
    o += `pub static ${name}: &[Message] = &[\n`;
    for (const [id, raw] of Object.entries(obj)) o += `    Message { id: ${rs(id)}, raw: ${rs(raw)} },\n`;
    o += '];\n\n';
  };
  table('MEMO_MESSAGES', d.memos);
  table('INFO_MESSAGES', d.infos);
  table('LIMITATION_MESSAGES', d.limitations);
  table('INOP_SYS_MESSAGES', d.inop);
  o += `pub static PFD_MEMO_DISPLAY: &[&str] = ${strs(d.pfdMemoDisplay)};\n\n`;
  o += `pub const A_THR_OFF_MEMO_KEY: &str = ${rs(d.aThrOffMemoKey)};\n`;
  return o;
}

const SELECT = {
  Fixed0: () => 'MemoSelect::Fixed0',
  ToMemo: () => 'MemoSelect::ToMemo',
  LdgMemo: () => 'MemoSelect::LdgMemo',
  AdirsAlignOver3Min: () => 'MemoSelect::AdirsAlignOver3Min',
  AdirsAlignUnder3Min: () => 'MemoSelect::AdirsAlignUnder3Min',
  OxyPaxSys: () => 'MemoSelect::OxyPaxSys',
  Custom: () => 'MemoSelect::Custom',
  Ternary: (s) => `MemoSelect::Ternary { input: ${rs(s.input)}, if_true: ${s.t}, if_false: ${s.f} }`,
  MoreThanOneEngineRunning: (s) => `MemoSelect::MoreThanOneEngineRunning { if_true: ${s.t}, if_false: ${s.f} }`,
};

function emitMemoRules(name, rules) {
  let o = `pub static ${name}: &[MemoRule] = &[\n`;
  for (const r of rules) {
    o += `    MemoRule {\n        key: ${rs(r.key)},\n        flight_phase_inhib: &[${r.phaseInhib.join(', ')}],\n        active_expr: ${rs(r.activeExpr)},\n        active_inputs: ${strs(r.activeInputs)},\n        select: ${SELECT[r.select.kind](r.select)},\n        select_expr: ${rs(r.select.expr)},\n        select_inputs: ${strs(r.selectInputs)},\n        codes: ${strs(r.codes)},\n        inhibit: MemoInhibit::${r.inhibit.kind},\n        inhibit_inputs: ${strs(r.inhibitInputs)},\n        left_side: ${r.leftSide},\n    },\n`;
  }
  return o + '];\n\n';
}

function emitStatus(name, ty, rules, fieldsOf) {
  let o = `pub static ${name}: &[${ty}] = &[\n`;
  for (const r of rules) {
    o += `    ${ty} {\n        id: ${rs(r.key)},\n${fieldsOf(r)}        active_expr: ${rs(r.activeExpr)},\n        active_inputs: ${strs(r.activeInputs)},\n        not_active_when_item_active: ${strs(r.notActiveWhen)},\n        monitor_confirm_time: ${opt(r.monitor, (v) => `${v}f32`)},\n    },\n`;
  }
  return o + '];\n\n';
}

function emitRules(memos, toLdg, limits, infos, inops) {
  let o = HEADER + 'use crate::types::*;\n\n';
  o += emitMemoRules('EWD_MEMOS', memos);
  o += emitMemoRules('TO_LDG_MEMOS', toLdg);
  o += emitStatus('LIMITATION_RULES', 'LimitationRule', limits, (r) => `        phase: RulePhase::${r.phase},\n        pfd: ${r.pfd},\n`);
  o += emitStatus('INFO_RULES', 'InfoRule', infos, () => '');
  o += emitStatus('INOP_SYS_RULES', 'InopRule', inops, (r) => `        phase: RulePhase::${r.phase},\n        redundancy_loss: ${r.redundancyLoss},\n`);
  return o.replace(/\n\n$/, '\n');
}

const d = runEntry();
const memoSf = parse('FwsMemos.ts');
const memos = memoRules(memoSf, 'ewdMemos');
const toLdg = memoRules(memoSf, 'ewdToLdgMemos');
const limits = statusRules('FwsLimitations.ts', 'limitations', true);
const infos = statusRules('FwsInformation.ts', 'info', false);
const inops = statusRules('FwsInopSys.ts', 'inopSys', true);
const checks = sensedChecks();

writeFileSync(path.join(OUT, 'normal_data.rs'), emitNormal(d, checks));
writeFileSync(path.join(OUT, 'message_data.rs'), emitMessages(d));
writeFileSync(path.join(OUT, 'rule_data.rs'), emitRules(memos, toLdg, limits, infos, inops));

const counts = {
  normalProcedures: Object.keys(d.normal).length,
  normalItems: Object.values(d.normal).reduce((n, p) => n + p.items.length, 0),
  sensedLists: Object.keys(checks).length,
  deferredIds: d.deferredNormal.length,
  memoMessages: Object.keys(d.memos).length,
  infoMessages: Object.keys(d.infos).length,
  limitationMessages: Object.keys(d.limitations).length,
  inopSysMessages: Object.keys(d.inop).length,
  pfdMemoDisplay: d.pfdMemoDisplay.length,
  ewdMemoRules: memos.length,
  toLdgMemoRules: toLdg.length,
  limitationRules: limits.length,
  infoRules: infos.length,
  inopSysRules: inops.length,
  customSelections: [...memos, ...toLdg].filter((r) => r.select.kind === 'Custom').map((r) => r.key),
  customInhibits: [...memos, ...toLdg].filter((r) => r.inhibit.kind === 'Custom').map((r) => r.key),
};
process.stdout.write(JSON.stringify(counts, null, 2) + '\n');
