function hrChecklistRows() {
  const rows = [];
  (DOCS.normal || []).forEach((c, i) => {
    if (/DEFERRED PROCEDURE|^<</.test(c.t)) return;
    const items = c.i.map((ln, j) => [ln, j]).filter(([ln]) => !/[chb]/.test(ln[3] || ''));
    if (!items.length) return;
    const ticks = docTicks.get('n:' + i) || new Set();
    const done = items.filter(([, j]) => ticks.has(j)).length;
    rows.push({ title: c.t, done, total: items.length });
  });
  return rows;
}

function hrOpen(tab) {
  document.querySelector(`.tab-btn[data-tab="${tab}"]`)?.click();
}

function homeReminders() {
  const box = el('div', { class: 'box home-rem' });
  const lists = hrChecklistRows();
  const next = lists.find((r) => r.done < r.total);
  const cl = el('div', { class: 'home-rem-col' }, el('div', { class: 'home-sec', text: 'Checklists' }));
  for (const r of lists) {
    const state = r.done === r.total ? 'done' : r === next ? 'next' : '';
    const row = el('button', { type: 'button', class: 'home-rem-row ' + state },
      el('span', { class: 'home-rem-dot' }), el('span', { class: 'home-rem-t', text: r.title }), el('span', { class: 'mono dim', text: `${r.done}/${r.total}` }));
    row.addEventListener('click', () => hrOpen('checklists'));
    cl.append(row);
  }
  const mx = el('div', { class: 'home-rem-col' }, el('div', { class: 'home-sec', text: 'Maintenance' }));
  const mxBody = el('div', { class: 'label dim', text: FIXTURE ? 'Maintenance status needs X-Plane running.' : 'Reading the tech log…' });
  mx.append(mxBody);
  const pins = (typeof docPins === 'function' ? docPins() : []).map((id) => (typeof docLocate === 'function' ? docLocate(id) : null)).filter(Boolean);
  if (pins.length) {
    mx.append(el('div', { class: 'home-sec home-rem-sub', text: 'Pinned documents' }));
    for (const hit of pins.slice(0, 8)) {
      const row = el('button', { type: 'button', class: 'home-rem-row pin' }, el('span', { class: 'home-rem-dot' }), el('span', { class: 'home-rem-t', text: hit.leaf.t }));
      row.addEventListener('click', () => { docSel = hit.leaf.id; hrOpen('checklists'); });
      mx.append(row);
    }
  }
  box.append(cl, mx);
  if (!FIXTURE) {
    studyGet('/study/maintenance').then((d) => {
      if (!d) return;
      mxBody.remove();
      const legal = !d.dispatch || d.dispatch.legal !== false;
      const go = el('button', { type: 'button', class: 'home-rem-row ' + (legal ? 'done' : 'nogo') },
        el('span', { class: 'home-rem-dot' }), el('span', { class: 'home-rem-t', text: legal ? 'Dispatch: GO' : 'Dispatch: NO GO' }));
      go.addEventListener('click', () => hrOpen('maintenance'));
      mx.append(go);
      for (const b of (d.dispatch && d.dispatch.blocking) || []) mx.append(el('div', { class: 'label home-rem-note', text: b }));
      for (const [label, n] of [['Open defects', (d.defects || []).length], ['MEL deferrals', (d.deferrals || []).length], ['Tech log entries', (d.techLog || []).length]]) {
        const row = el('button', { type: 'button', class: 'home-rem-row' + (n && label === 'Open defects' ? ' next' : '') },
          el('span', { class: 'home-rem-dot' }), el('span', { class: 'home-rem-t', text: label }), el('span', { class: 'mono dim', text: String(n) }));
        row.addEventListener('click', () => hrOpen('maintenance'));
        mx.append(row);
      }
      if (d.airframeHours) mx.append(el('div', { class: 'label dim home-rem-note', text: `Airframe ${Math.round(d.airframeHours).toLocaleString()} h` }));
    }).catch(() => { mxBody.textContent = 'Maintenance status unavailable.'; });
  }
  return box;
}

(function () {
  const style = document.createElement('style');
  style.textContent = `
.home-rem{flex:none;display:grid;grid-template-columns:minmax(0,1.3fr) minmax(0,1fr);gap:28px;align-items:start;}
.home-rem-col{display:flex;flex-direction:column;gap:4px;min-width:0;}
button.home-rem-row{flex:none;display:flex;align-items:center;gap:10px;width:100%;height:auto;padding:7px 10px;border-radius:8px;background:transparent;border:0 !important;color:var(--text);font:600 13px 'Manrope','Segoe UI',sans-serif;text-align:left;}
button.home-rem-row:hover{background:rgba(255,255,255,0.05);}
.home-rem-t{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
.home-rem-dot{width:8px;height:8px;border-radius:50%;background:rgba(255,255,255,0.25);flex:none;}
.home-rem-row.done .home-rem-dot{background:#4cc36a;}
.home-rem-row.next .home-rem-dot{background:#ff4d6a;}
.home-rem-row.next .home-rem-t{color:#fff;}
.home-rem-row.nogo .home-rem-dot{background:#ff4d4d;}
.home-rem-row.nogo .home-rem-t{color:#ff6b6b;}
.home-rem-note{padding:2px 10px 2px 28px;}
.home-rem-sub{margin-top:14px;}
.home-rem-row.pin .home-rem-dot{background:#3fb0ff;}
`;
  document.head.append(style);
})();
