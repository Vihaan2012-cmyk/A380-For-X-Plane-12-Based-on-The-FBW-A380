const fs = require('fs');
const path = require('path');

const root = process.argv[2];
const ts = require(path.join(root, 'node_modules', 'typescript'));
const dir = path.join(root, 'fbw-a380x/src/systems/systems-host/CpiomC/FlightWarningSystem');

const FILES = [
  ['FwsAbnormalSensed.ts', ['ewdAbnormalSensed', 'ewdDeferredProcs']],
  ['FwsAbnormalNonSensed.ts', ['ewdAbnormalNonSensed', 'ewdDeferredProcs']],
];
const LISTS = [
  'inopSysAllPhases',
  'inopSysApprLdg',
  'redundLoss',
  'limitationsAllPhases',
  'limitationsApprLdg',
  'limitationsPfd',
  'info',
];

function propName(p) {
  const n = p.name;
  if (ts.isIdentifier(n) || ts.isStringLiteral(n) || ts.isNumericLiteral(n)) return n.text;
  return null;
}

function squash(text) {
  return text.replace(/\s+/g, ' ').trim();
}

function collect(node, pred, out) {
  if (pred(node)) out.push(node);
  ts.forEachChild(node, (c) => {
    collect(c, pred, out);
  });
  return out;
}

function fwsRefs(node, sf) {
  const names = [];
  for (const n of collect(node, ts.isPropertyAccessExpression, [])) {
    const t = n.getText(sf);
    const m = /^this\.fws\.([A-Za-z0-9_]+)$/.exec(t);
    if (m && !names.includes(m[1])) names.push(m[1]);
  }
  return names;
}

function simvarNames(node) {
  const names = [];
  for (const n of collect(node, ts.isStringLiteralLike, [])) {
    if (/^[LA]:/.test(n.text) && !names.includes(n.text)) names.push(n.text);
  }
  return names;
}

function arrowBody(init) {
  if (!init || !(ts.isArrowFunction(init) || ts.isFunctionExpression(init))) return null;
  return init.body;
}

function arrayShape(init, sf) {
  const body = arrowBody(init);
  if (!body) return { kind: 'expr', src: squash(init.getText(sf)) };
  let arr = body;
  while (ts.isParenthesizedExpression(arr)) arr = arr.expression;
  if (!ts.isArrayLiteralExpression(arr)) return { kind: 'dynamic', src: squash(body.getText(sf)) };
  const hasSpread = arr.elements.some((e) => ts.isSpreadElement(e));
  const literal = arr.elements.every((e) => e.kind === ts.SyntaxKind.TrueKeyword || e.kind === ts.SyntaxKind.FalseKeyword);
  return {
    kind: 'array',
    len: hasSpread ? null : arr.elements.length,
    literal: literal ? arr.elements.map((e) => e.kind === ts.SyntaxKind.TrueKeyword) : null,
  };
}

function idList(init, sf) {
  const body = arrowBody(init);
  if (!body) return null;
  const ids = [];
  for (const n of collect(body, ts.isStringLiteralLike, [])) {
    if (/^\d{9}$/.test(n.text) && !ids.includes(n.text)) ids.push(n.text);
  }
  let arr = body;
  while (ts.isParenthesizedExpression(arr)) arr = arr.expression;
  const plain = ts.isArrayLiteralExpression(arr) && arr.elements.every((e) => ts.isStringLiteralLike(e));
  return { ids, conditional: !plain };
}

function aural(init, sf) {
  const src = squash(init.getText(sf));
  const consts = [];
  for (const n of collect(init, ts.isPropertyAccessExpression, [])) {
    const m = /^FwcAuralWarning\.(\w+)$/.exec(n.getText(sf));
    if (m && !consts.includes(m[1])) consts.push(m[1]);
  }
  const refs = fwsRefs(init, sf);
  return { src, refs, consts };
}

const core = (() => {
  const text = fs.readFileSync(path.join(dir, 'FwsCore.ts'), 'utf8');
  const sf = ts.createSourceFile('FwsCore.ts', text, ts.ScriptTarget.Latest, true);
  const lists = {};
  for (const decl of collect(sf, ts.isPropertyDeclaration, [])) {
    if (decl.initializer && ts.isArrayLiteralExpression(decl.initializer) && decl.initializer.elements.every(ts.isNumericLiteral)) {
      lists[decl.name.getText(sf)] = decl.initializer.elements.map((x) => Number(x.text));
    }
  }
  return lists;
})();

function phaseList(init, sf) {
  if (ts.isArrayLiteralExpression(init)) return init.elements.map((x) => Number(x.text));
  const m = /^this\.fws\.(\w+)$/.exec(init.getText(sf));
  if (m && core[m[1]]) return core[m[1]];
  throw new Error('unresolved flightPhaseInhib ' + init.getText(sf));
}

function entry(prop, sf) {
  const obj = prop.initializer;
  const first = obj.properties[0];
  const head = sf.text.slice(obj.getStart(sf) + 1, first ? first.getStart(sf) : obj.getEnd());
  const cm = /\/\/\s*(.*)/.exec(head);
  const e = { id: propName(prop), comment: cm ? cm[1].trim() : '' };
  for (const p of obj.properties) {
    if (!ts.isPropertyAssignment(p) && !ts.isMethodDeclaration(p)) continue;
    const name = propName(p);
    const init = ts.isPropertyAssignment(p) ? p.initializer : null;
    if (!init) continue;
    if (name === 'flightPhaseInhib') {
      e.phases = phaseList(init, sf);
    } else if (name === 'failure') {
      e.failure = Number(init.getText(sf));
    } else if (name === 'sysPage') {
      e.sysPage = init.getText(sf).replace(/^SdPages\./, '');
    } else if (name === 'auralWarning') {
      e.aural = aural(init, sf);
    } else if (name === 'simVarIsActive') {
      e.active = { src: squash(init.getText(sf)), refs: fwsRefs(init, sf), vars: simvarNames(init) };
    } else if (name === 'notActiveWhenItemActive') {
      e.suppressedBy = ts.isArrayLiteralExpression(init) ? init.elements.map((x) => x.text) : [];
    } else if (name === 'monitorConfirmTime') {
      e.monitorConfirmTime = Number(init.text);
    } else if (name === 'cancel') {
      e.cancel = init.kind === ts.SyntaxKind.TrueKeyword;
    } else if (name === 'nonProcedureKey') {
      e.nonProcedureKey = init.kind === ts.SyntaxKind.TrueKeyword;
    } else if (name === 'whichItemsToShow') {
      e.show = arrayShape(init, sf);
    } else if (name === 'whichItemsChecked') {
      e.checked = arrayShape(init, sf);
    } else if (name === 'whichItemsActive') {
      e.active_items = arrayShape(init, sf);
    } else if (name === 'whichItemsTimer') {
      e.timer = arrayShape(init, sf);
    } else if (LISTS.includes(name)) {
      e[name] = idList(init, sf);
    } else {
      e.unknown = (e.unknown || []).concat(name);
    }
  }
  return e;
}

const out = {};
for (const [file, props] of FILES) {
  const text = fs.readFileSync(path.join(dir, file), 'utf8');
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  for (const decl of collect(sf, ts.isPropertyDeclaration, [])) {
    const dn = decl.name.getText(sf);
    if (!props.includes(dn) || !decl.initializer || !ts.isObjectLiteralExpression(decl.initializer)) continue;
    const list = [];
    for (const p of decl.initializer.properties) {
      if (ts.isPropertyAssignment(p) && ts.isObjectLiteralExpression(p.initializer)) {
        list.push(entry(p, sf));
      } else {
        list.push({ id: null, unsupported: squash(p.getText(sf)) });
      }
    }
    out[`${file}:${dn}`] = list;
  }
}
process.stdout.write(JSON.stringify(out));
