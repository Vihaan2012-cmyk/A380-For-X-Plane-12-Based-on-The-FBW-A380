const FO_LISTS = new Map();
const FO_SOLO = new Map();
const FO_SOLO_STEP_MS = 3400;
const FO_SOLO_WAIT_MS = 2000;
const FO_SOLO_REMIND_MS = 20000;
const FO_AUTO_LISTS = { 1: 'BEFORE START', 2: 'AFTER START', 3: 'TAXI', 4: 'LINE-UP', 5: 'APPROACH', 6: 'LANDING', 7: 'PARKING' };
let foAutoSeq = null;

function foSlug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/_+/g, '_').replace(/^_|_$/g, '').slice(0, 60);
}

function foCopilotMode() {
  return settings['xphfbw.copilotMode'] === true || settings['xphfbw.copilotMode'] === 'true';
}

function foMode() {
  if (foCopilotMode()) return 'solo';
  const m = settings['xphfbw.copilotChecklist'];
  return m === 'response' || m === 'off' || m === 'solo' ? m : 'challenge';
}

function foSay(keys, interrupt) {
  if (!keys.length) return;
  studyPost('/study/action', { kind: 'copilotSay', keys, interrupt: !!interrupt }).catch(() => {});
}

function foPlain(lines) {
  const out = [];
  const walk = (ns) => ns.forEach((n) => { if (!n.ln[3].includes('c') && !n.ln[3].includes('h') && !n.ln[3].includes('b')) out.push(n); walk(n.kids); });
  walk(docBlocks(lines));
  return out;
}

function foResponse(value) {
  return value && !value.includes('__') && foSlug(value) ? 'r_' + foSlug(value) : 'checked';
}

function foAfterLines(id, lines, current) {
  const st = FO_LISTS.get(id);
  if (!st || foMode() === 'off' || foMode() === 'solo') return;
  const ticks = new Set(docTicks.get(id) || []);
  const plain = foPlain(lines);
  const keys = [];
  if (foMode() === 'response') {
    for (const n of plain) if (ticks.has(n.j) && !st.ticks.has(n.j)) keys.push(foResponse(n.ln[1]));
  } else if (current >= 0 && current !== st.current) {
    const n = plain.find((p) => p.j === current);
    if (n) keys.push('c_' + foSlug(n.ln[0]));
  }
  const done = plain.length > 0 && plain.every((n) => ticks.has(n.j));
  if (done && !st.done) {
    keys.push('checklist_complete');
    FO_LISTS.delete(id);
  } else {
    st.ticks = ticks;
    st.current = current;
    st.done = done;
  }
  foSay(keys, false);
}

function foRedraw() {
  if (document.getElementById('tab-checklists')?.classList.contains('active') && typeof renderDocs === 'function') renderDocs();
}

async function foSensed(title, item) {
  const conds = typeof CL_CONDITIONS === 'object' && CL_CONDITIONS[title] && CL_CONDITIONS[title][item];
  if (!conds || typeof clVarName !== 'function') return true;
  const names = [...new Set(conds.flatMap((c) => { const n = clVarName(c.v); return /:\d+$/.test(n) ? [n] : [n, n + ':1']; }))];
  let vars;
  try { vars = ((await studyGet('/vars?names=' + names.map(encodeURIComponent).join(','))) || {}).vars || {}; } catch { return true; }
  return conds.every((c) => { const v = clValue(vars, c); return v === null || clCompare(v, c); });
}

async function foSoloStep(id) {
  const run = FO_SOLO.get(id);
  if (!run || foMode() !== 'solo') { FO_SOLO.delete(id); return; }
  const c = DOCS.normal[+id.slice(2)];
  const ticks = docTicks.get(id) || new Set();
  const next = foPlain(c.i).find((n) => !ticks.has(n.j));
  if (!next) {
    FO_SOLO.delete(id);
    foSay(['checklist_complete'], false);
    show(`F/O: ${c.t} checklist complete`);
    return;
  }
  const [name, value] = next.ln;
  if (!(await foSensed(c.t, name))) {
    if (run.waiting !== next.j || Date.now() - run.remindedAt > FO_SOLO_REMIND_MS) {
      run.waiting = next.j;
      run.remindedAt = Date.now();
      foSay(['c_' + foSlug(name), 'not_set'], false);
      show(`F/O: ${name} not ${value.replace(/_+/g, '').trim() || 'set'}, waiting`);
    }
    run.timer = setTimeout(() => foSoloStep(id), FO_SOLO_WAIT_MS);
    return;
  }
  run.waiting = null;
  foSay(['c_' + foSlug(name), foResponse(value)], false);
  ticks.add(next.j);
  docTicks.set(id, ticks);
  foRedraw();
  run.timer = setTimeout(() => foSoloStep(id), FO_SOLO_STEP_MS);
}

function foSoloStart(id) {
  const old = FO_SOLO.get(id);
  if (old) clearTimeout(old.timer);
  const c = DOCS.normal[+id.slice(2)];
  FO_SOLO.set(id, { timer: null, waiting: null, remindedAt: 0 });
  foSay(['title_' + foSlug(c.t)], true);
  FO_SOLO.get(id).timer = setTimeout(() => foSoloStep(id), 1800);
}

function foStart(id) {
  const c = DOCS.normal[+id.slice(2)];
  if (!c) return;
  if (foMode() === 'solo') { foSoloStart(id); return; }
  const ticks = docTicks.get(id) || new Set();
  const next = foPlain(c.i).find((n) => !ticks.has(n.j));
  const keys = ['title_' + foSlug(c.t)];
  if (foMode() === 'challenge' && next) keys.push('c_' + foSlug(next.ln[0]));
  if (!next) keys.push('checklist_complete');
  else FO_LISTS.set(id, { ticks: new Set(ticks), current: next.j, done: false });
  foSay(keys, true);
}

function foButton(id) {
  const label = { response: 'F/O RESPONDS', solo: FO_SOLO.has(id) ? 'F/O RUNNING…' : 'F/O RUNS IT' }[foMode()] || 'F/O READS';
  const b = el('button', { type: 'button', text: label });
  b.title = {
    response: 'You read each challenge, the first officer answers as you tick it.',
    solo: 'The first officer reads, checks and answers every line, ticking each one, and waits on anything not set.',
  }[foMode()] || 'The first officer reads each challenge; answer it and tick the line.';
  b.addEventListener('click', (e) => { e.stopPropagation(); foStart(id); });
  if (foMode() === 'off') b.disabled = true;
  return b;
}

async function foAutoPoll() {
  if (FIXTURE || study.preview) return;
  let v;
  try { v = ((await studyGet('/vars?names=A32NX_COPILOT_CL_SEQ,A32NX_COPILOT_CL_CODE')) || {}).vars || {}; } catch { return; }
  const seq = v.A32NX_COPILOT_CL_SEQ;
  if (seq === undefined) return;
  const first = foAutoSeq === null;
  if (seq === foAutoSeq) return;
  foAutoSeq = seq;
  const auto = foCopilotMode() || settings['xphfbw.copilotAutoChecklists'] === true || settings['xphfbw.copilotAutoChecklists'] === 'true';
  if (first || foMode() === 'off' || !auto) return;
  const title = FO_AUTO_LISTS[Math.round(v.A32NX_COPILOT_CL_CODE || 0)];
  const i = (DOCS.normal || []).findIndex((c) => c.t === title);
  if (i < 0) return;
  show(`F/O: ${title} checklist`);
  foStart('n:' + i);
}

setInterval(foAutoPoll, 2000);
