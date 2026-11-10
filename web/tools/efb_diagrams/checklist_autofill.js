const CL_AUTO = new Map();
const CL_DECLINED = new Set();

function clVarName(v) {
  return String(v).replace(/^[AL]:/, '').split(',')[0].trim();
}

function clCompare(value, cond) {
  const r = Number(cond.r);
  switch (String(cond.c || 'EQ').toUpperCase()) {
    case 'NE': return Math.abs(value - r) > 1e-3;
    case 'LT': return value < r;
    case 'LE': return value <= r + 1e-3;
    case 'GE': return value >= r - 1e-3;
    case 'GT': return value > r;
    default: return Math.abs(value - r) <= 1e-3;
  }
}

function clConditionNames() {
  const names = new Set();
  for (const items of Object.values(typeof CL_CONDITIONS === 'object' ? CL_CONDITIONS : {})) {
    for (const conds of Object.values(items)) for (const c of conds) { const n = clVarName(c.v); names.add(n); if (!/:\d+$/.test(n)) names.add(n + ':1'); }
  }
  return [...names];
}

function clValue(vars, cond) {
  const n = clVarName(cond.v);
  if (n in vars) return vars[n];
  if (!/:\d+$/.test(n) && (n + ':1') in vars) return vars[n + ':1'];
  return null;
}

async function clAutofillPoll() {
  if (FIXTURE || study.preview) return;
  if (settings['xphfbw.autofillChecklists'] === false || settings['xphfbw.autofillChecklists'] === 'false') return;
  if (typeof CL_CONDITIONS !== 'object' || typeof DOCS !== 'object' || !DOCS.normal) return;
  const names = clConditionNames();
  if (!names.length) return;
  let vars;
  try { vars = ((await studyGet('/vars?names=' + names.map(encodeURIComponent).join(','))) || {}).vars; } catch { return; }
  if (!vars) return;
  let changed = false;
  DOCS.normal.forEach((c, i) => {
    const conds = CL_CONDITIONS[c.t];
    if (!conds) return;
    const id = 'n:' + i;
    if (!docTicks.has(id)) { CL_AUTO.delete(id); for (const k of [...CL_DECLINED]) if (k.startsWith(id + '#')) CL_DECLINED.delete(k); }
    const ticks = docTicks.get(id) || new Set();
    const auto = CL_AUTO.get(id) || new Set();
    c.i.forEach((ln, j) => {
      const list = conds[ln[0]];
      if (!list) return;
      const values = list.map((cond) => clValue(vars, cond));
      if (values.some((v) => v === null)) return;
      const ok = list.every((cond, k) => clCompare(values[k], cond));
      const key = id + '#' + j;
      if (!ok) CL_DECLINED.delete(key);
      if (ok && auto.has(j) && !ticks.has(j)) { auto.delete(j); CL_DECLINED.add(key); }
      if (ok && !ticks.has(j) && !CL_DECLINED.has(key)) { ticks.add(j); auto.add(j); changed = true; }
      else if (!ok && auto.has(j)) { if (ticks.has(j)) { ticks.delete(j); changed = true; } auto.delete(j); }
    });
    docTicks.set(id, ticks);
    CL_AUTO.set(id, auto);
  });
  if (changed && document.getElementById('tab-checklists')?.classList.contains('active') && typeof renderDocs === 'function') renderDocs();
}

setInterval(clAutofillPoll, 1500);
