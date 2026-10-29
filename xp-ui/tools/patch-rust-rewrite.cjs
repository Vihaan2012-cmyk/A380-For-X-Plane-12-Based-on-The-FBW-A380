'use strict';
// node patch-rust-rewrite.cjs <converted.json> <plugin src dir> [--write]
// Carries the converted serve-time patches back into the plugin's Rust source: every string literal (and the
// embedded deep_ecam_bridge.js) that spells a patch's anchor or replacement, whole or in part, gets the same
// token edits the converter made. Without --write it only reports what it would change.

const fs = require('fs');
const path = require('path');
const { tokens } = require('./patch-convert.cjs');

const [convertedFile, srcDir] = process.argv.slice(2);
const write = process.argv.includes('--write');
const converted = JSON.parse(fs.readFileSync(convertedFile, 'utf8'));

function editsBetween(oldText, newText) {
  if (oldText === newText) return [];
  const a = tokens(oldText);
  const b = tokens(newText);
  if (a.length !== b.length) return null;
  const edits = [];
  for (let i = 0; i < a.length; i++) {
    if (a[i].text !== b[i].text) edits.push({ start: a[i].pos, end: a[i].end, text: b[i].text, was: a[i].text });
  }
  return edits;
}

const jobs = [];
for (const r of converted) {
  const re = editsBetween(r.oldReplace, r.replace);
  jobs.push({ r, kind: 'replace', old: r.oldReplace, neu: r.replace, edits: re });
  if (r.find !== r.oldFind) jobs.push({ r, kind: 'find', old: r.oldFind, neu: r.find, edits: editsBetween(r.oldFind, r.find) });
}

function literals(text) {
  const out = [];
  let i = 0;
  const n = text.length;
  while (i < n) {
    const c = text[i];
    if (c === '/' && text[i + 1] === '/') {
      while (i < n && text[i] !== '\n') i++;
    } else if (c === '/' && text[i + 1] === '*') {
      let depth = 1;
      i += 2;
      while (i < n && depth) {
        if (text[i] === '/' && text[i + 1] === '*') (depth++, (i += 2));
        else if (text[i] === '*' && text[i + 1] === '/') (depth--, (i += 2));
        else i++;
      }
    } else if (c === '\'') {
      const m = /^'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'])'/.exec(text.slice(i, i + 12));
      i += m ? m[0].length : 1;
    } else if ((c === 'r' || (c === 'b' && text[i + 1] === 'r')) && /^b?r#*"/.test(text.slice(i, i + 40)) && !/[\w]/.test(text[i - 1] || '')) {
      const m = /^b?r(#*)"/.exec(text.slice(i));
      const close = '"' + m[1];
      const bodyStart = i + m[0].length;
      const bodyEnd = text.indexOf(close, bodyStart);
      const map = [];
      for (let k = bodyStart; k <= bodyEnd; k++) map.push(k);
      out.push({ start: i, end: bodyEnd + close.length, raw: true, value: text.slice(bodyStart, bodyEnd), map });
      i = bodyEnd + close.length;
    } else if (c === '"') {
      let k = i + 1;
      let value = '';
      const map = [];
      while (k < n && text[k] !== '"') {
        if (text[k] === '\\') {
          const e = text[k + 1];
          if (e === '\n' || e === '\r') {
            k += 2;
            while (k < n && /\s/.test(text[k])) k++;
            continue;
          }
          let ch;
          let len = 2;
          if (e === 'n') ch = '\n';
          else if (e === 't') ch = '\t';
          else if (e === 'r') ch = '\r';
          else if (e === '0') ch = '\0';
          else if (e === 'x') (ch = String.fromCharCode(parseInt(text.slice(k + 2, k + 4), 16))), (len = 4);
          else if (e === 'u') {
            const m = /^u\{([0-9a-fA-F]+)\}/.exec(text.slice(k + 1));
            ch = String.fromCodePoint(parseInt(m[1], 16));
            len = 1 + m[0].length;
          } else ch = e;
          for (const _ of ch) map.push(k);
          value += ch;
          k += len;
        } else {
          map.push(k);
          value += text[k];
          k++;
        }
      }
      map.push(k);
      out.push({ start: i, end: k + 1, raw: false, value, map });
      i = k + 1;
    } else i++;
  }
  return out;
}

const escape = (s, raw) => (raw ? s : s.replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n').replace(/\t/g, '\\t'));

function piecesOf(lit) {
  if (!(lit.value.includes('{{') || lit.value.includes('}}'))) return [{ at: 0, text: lit.value, fmt: false }];
  const pieces = [];
  let cur = '';
  let at = 0;
  const offs = [];
  for (let k = 0; k < lit.value.length; k++) {
    const two = lit.value.slice(k, k + 2);
    if (two === '{{' || two === '}}') {
      cur += two[0];
      offs.push(k);
      k++;
    } else if (lit.value[k] === '{') {
      const close = lit.value.indexOf('}', k);
      if (cur) pieces.push({ at, text: cur, offs: offs.splice(0), fmt: true });
      cur = '';
      k = close;
      at = k + 1;
    } else {
      if (!cur) at = k;
      cur += lit.value[k];
      offs.push(k);
    }
  }
  if (cur) pieces.push({ at, text: cur, offs: offs.splice(0), fmt: true });
  return pieces;
}

const files = [];
const walk = (d) => {
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) {
      if (path.relative(srcDir, p).split(path.sep)[0] === 'js') continue;
      walk(p);
    } else if (/\.(rs|js)$/.test(e.name)) files.push(p);
  }
};
walk(srcDir);
const PATCH_SOURCES = ['lib.rs', 'source_patch.rs', 'ecam_patches.rs', 'ecam_patches/ecl.rs', 'cds_reconf.rs', 'aircraft_options.rs', 'surveillance.rs', 'wxr/mod.rs', 'extra_backend_fbw.rs', 'xphfbw_bridge_views.rs', 'deep/ecam/patches.rs', 'deep/ecam/codegen.rs', 'deep/ecam/fbw_codegen.rs', 'procedure_pack.rs', 'native_shadow_patches.rs', 'native_systems_host_patches.rs', 'datalink_native_patches.rs'];
const HAND = (process.env.PATCH_HAND || 'deep ECAM bridge: install and step').split('|');
files.splice(0, files.length, ...files.filter((f) => PATCH_SOURCES.includes(path.relative(srcDir, f).split(path.sep).join('/'))));
jobs.splice(0, jobs.length, ...jobs.filter((j) => !HAND.some((h) => j.r.reason.startsWith(h))));

const report = [];
const touchedJobs = new Set();
let changedFiles = 0;
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lits = file.endsWith('.js') ? [{ start: 0, end: text.length, raw: true, value: text, map: [...Array(text.length + 1).keys()], whole: true }] : literals(text);
  const rawEdits = [];
  for (const lit of lits) {
    for (const piece of piecesOf(lit)) {
      if (piece.text.length < 12) continue;
      for (const job of jobs) {
        if (!job.old.includes(piece.text)) continue;
        if (job.edits === null) {
          report.push(`NEEDS HAND: ${path.relative(srcDir, file)}: ${job.kind} of "${job.r.reason.slice(0, 60)}" (token counts differ)`);
          continue;
        }
        const off = job.old.indexOf(piece.text);
        if (job.old.indexOf(piece.text, off + 1) >= 0 && piece.text.length < 40) continue;
        touchedJobs.add(job);
        for (const e of job.edits) {
          if (e.end <= off || e.start >= off + piece.text.length) continue;
          if (e.start < off || e.end > off + piece.text.length) {
            report.push(`NEEDS HAND: ${path.relative(srcDir, file)}: edit ${e.was} -> ${e.text} crosses a literal boundary`);
            continue;
          }
          const ds = piece.fmt ? piece.offs[e.start - off] : e.start - off + piece.at;
          const de = piece.fmt ? piece.offs[e.end - off - 1] + 1 : e.end - off + piece.at;
          let rep = escape(e.text, lit.raw);
          if (piece.fmt) rep = rep.replace(/\{/g, '{{').replace(/\}/g, '}}');
          rawEdits.push({ start: lit.map[ds], end: lit.map[de], text: rep, was: e.was });
        }
      }
    }
  }
  const uniq = [...new Map(rawEdits.map((e) => [`${e.start}:${e.end}`, e])).values()].sort((x, y) => y.start - x.start);
  if (!uniq.length) continue;
  let out = text;
  for (const e of uniq) out = out.slice(0, e.start) + e.text + out.slice(e.end);
  changedFiles++;
  const counts = {};
  for (const e of uniq) counts[`${e.was} -> ${e.text}`] = (counts[`${e.was} -> ${e.text}`] || 0) + 1;
  report.push(`${path.relative(srcDir, file)}: ${uniq.length} edits ${JSON.stringify(counts).slice(0, 300)}`);
  if (write) fs.writeFileSync(file, out);
}
for (const job of jobs) if (job.edits && job.edits.length && !touchedJobs.has(job)) report.push(`NOT LOCATED: ${job.kind} of "${job.r.reason.slice(0, 70)}" (${job.r.path.split('/').pop()})`);
console.log(report.join('\n'));
console.log(`${changedFiles} files ${write ? 'rewritten' : 'would change'}`);
