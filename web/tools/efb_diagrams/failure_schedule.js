const FS_WHEN = [['phase', 'On flight phase'], ['aboveSpeed', 'Above IAS (kt)'], ['aboveAlt', 'Climbing through (ft)'], ['belowAlt', 'Descending through (ft)'], ['minutes', 'After (minutes)']];
const FS_PHASES = [['Taxi', 'Taxi'], ['Takeoff', 'Take-off'], ['Climb', 'Climb'], ['Cruise', 'Cruise'], ['Descent', 'Descent'], ['Approach', 'Approach'], ['GoAround', 'Go-around']];
const FS_SCENARIOS = [
  ['Tyre burst on the take-off roll', [['Left wing gear tyre burst', 'aboveSpeed', 110]]],
  ['Bird strike in the initial climb', [['Engine 2 N1 bird-strike imbalance', 'aboveSpeed', 175]]],
  ['Generator 1 lost in the climb', [['Generator 1', 'phase', 'Climb']]],
  ['Bleed leak in cruise', [['Engine 1 bleed duct leak', 'phase', 'Cruise']]],
  ['Rapid decompression in cruise', [['Rapid Decompression', 'phase', 'Cruise']]],
];
const fs = { open: false, pick: '', when: 'phase', value: 'Cruise', pending: [] };

function fsById(name) {
  return (fail.list || []).find((f) => f.name === name) || null;
}

function fsLabel(t) {
  switch (t.when) {
    case 'phase': return `on ${(FS_PHASES.find(([k]) => k === t.value) || [0, t.value])[1].toLowerCase()}`;
    case 'aboveSpeed': return `above ${Math.round(t.value)} kt`;
    case 'aboveAlt': return `climbing through ${Math.round(t.value)} ft`;
    case 'belowAlt': return `descending through ${Math.round(t.value)} ft`;
    default: return `at ${Number(t.value).toFixed(2)} airframe hours`;
  }
}

async function fsRefresh() {
  if (FIXTURE) return;
  try { fs.pending = await (await fetch(PLUGIN_ORIGIN + '/study/scheduled', { cache: 'no-store' })).json(); } catch { fs.pending = []; }
  if (document.getElementById('tab-failures')?.classList.contains('active')) drawFailures();
}

async function fsSchedule(id, when, value) {
  let v = value;
  if (when === 'minutes') {
    let hours = 0;
    try { hours = Number((await studyGet('/study/maintenance')).airframeHours) || 0; } catch {}
    v = hours + Number(value) / 60;
    when = 'airframeHours';
  } else if (when !== 'phase') {
    v = Number(value);
    if (!Number.isFinite(v)) { show('Enter a number'); return false; }
  }
  const r = await studyPost('/study/action', { kind: 'scheduleFailure', id, when, value: v });
  return !(r && r.ok === false);
}

function failSchedulePanel() {
  const box = el('div', { class: 'box fs-box' });
  const head = el('button', { type: 'button', class: 'fs-head' }, el('span', { class: 'section-title', style: 'margin:0', text: `Scheduled failures / ${fs.pending.length}` }), el('span', { class: 'label dim', text: fs.open ? 'hide' : 'schedule, scenarios' }));
  head.addEventListener('click', () => { fs.open = !fs.open; drawFailures(); });
  box.append(head);
  for (const t of fs.pending) {
    const cancel = el('button', { type: 'button', class: 'small grey', text: 'Cancel' });
    cancel.addEventListener('click', async () => { await studyPost('/study/action', { kind: 'cancelScheduledFailure', id: t.id }); setTimeout(fsRefresh, 400); });
    box.append(el('div', { class: 'fs-row' }, el('span', { class: 'fs-name', text: t.name }), el('span', { class: 'label dim', text: fsLabel(t) }), cancel));
  }
  if (!fs.open) return box;
  const names = el('datalist', { id: 'fs-names' });
  for (const f of fail.list || []) names.append(el('option', { value: `${f.name} [${f.id}]` }));
  const pick = el('input', { type: 'text', list: 'fs-names', placeholder: 'Failure (type to search)', value: fs.pick });
  pick.addEventListener('change', () => { fs.pick = pick.value; });
  const when = el('select', {});
  for (const [k, label] of FS_WHEN) when.append(el('option', { value: k, text: label }));
  when.value = fs.when;
  when.addEventListener('change', () => { fs.when = when.value; fs.value = fs.when === 'phase' ? 'Cruise' : ''; drawFailures(); });
  let value;
  if (fs.when === 'phase') {
    value = el('select', {});
    for (const [k, label] of FS_PHASES) value.append(el('option', { value: k, text: label }));
    value.value = fs.value || 'Cruise';
  } else {
    value = el('input', { type: 'text', placeholder: fs.when === 'minutes' ? '20' : fs.when === 'aboveSpeed' ? '120' : '10000', value: fs.value });
  }
  value.addEventListener('change', () => { fs.value = value.value; });
  const go = el('button', { type: 'button', class: 'small', text: 'Schedule' });
  go.addEventListener('click', async () => {
    fs.pick = pick.value; fs.value = value.value;
    const m = fs.pick.match(/\[(\d+)\]\s*$/);
    if (!m) { show('Pick a failure from the list'); return; }
    if (await fsSchedule(Number(m[1]), fs.when, fs.value)) { show('Failure scheduled'); fs.pick = ''; setTimeout(fsRefresh, 400); }
  });
  box.append(names, el('div', { class: 'fs-form' }, pick, when, value, go));
  const sc = el('div', { class: 'fs-scen' });
  for (const [title, items] of FS_SCENARIOS) {
    const found = items.map(([name, w, v]) => [fsById(name), w, v]);
    const b = el('button', { type: 'button', class: 'tile', title: items.map(([n]) => n).join(', ') }, el('span', { text: title }), el('span', { class: 'tile-state', text: found.every(([f]) => f) ? items.map(([, w, v]) => fsLabel({ when: w, value: v })).join(', ') : 'not in this failure list' }));
    b.disabled = !found.every(([f]) => f);
    b.addEventListener('click', async () => {
      for (const [f, w, v] of found) if (!(await fsSchedule(f.id, w, v))) return;
      show(`${title}: scheduled`);
      setTimeout(fsRefresh, 400);
    });
    sc.append(b);
  }
  box.append(el('div', { class: 'label dim', style: 'margin:12px 0 6px', text: 'Training scenarios: each arms its failures once, when its trigger comes. Scheduled failures last until they fire or X-Plane closes; cancel above to take one back.' }), sc);
  return box;
}

(function () {
  const style = document.createElement('style');
  style.textContent = `
.fs-box{margin-bottom:14px;}
button.fs-head{display:flex;align-items:center;justify-content:space-between;width:100%;height:auto;padding:0;background:none;border:0 !important;}
.fs-row{display:flex;align-items:center;gap:14px;padding:8px 0;border-top:1px solid rgba(255,255,255,0.06);}
.fs-row .fs-name{flex:1;font-weight:700;}
.fs-row button.small{flex:none;width:90px;}
.fs-form{display:grid;grid-template-columns:minmax(0,2fr) minmax(0,1fr) minmax(0,1fr) 110px;gap:10px;margin-top:12px;}
.fs-scen{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:10px;}
`;
  document.head.append(style);
})();
