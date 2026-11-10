const GM_DOORS = [
  ['M1L', -3.05, -28.85], ['M1R', 3.05, -28.85], ['M2L', -3.58, -18.65], ['M2R', 3.58, -18.65], ['M3L', -3.71, -2.44], ['M3R', 3.71, -2.44],
  ['M4L', -3.57, 9.66], ['M4R', 3.57, 9.66], ['M5L', -3.35, 18.6], ['M5R', 3.35, 18.6], ['U1L', -3.38, -14.2], ['U1R', 3.38, -14.2],
  ['U2L', -3.36, 5.21], ['U2R', 3.36, 5.21], ['U3L', -3.37, 14.1], ['U3R', 3.37, 14.1], ['CARGO_FWD', 3.4, -23.45], ['CARGO_AFT', 3.37, 13.03],
];
const GM_CHOCKS = ['nose', 'lwing', 'rwing', 'lbody', 'rbody'];
const GM_ENGINES = [[-25.77, -1.14], [-14.87, -9.01], [14.87, -9.01], [25.76, -1.14]];
const GM_OPERATORS = [['0', 'AUTO'], ['dnata', 'DNATA'], ['swissport', 'SWISSPORT'], ['menzies', 'MENZIES']];
const GM_GROUPS = [
  ['POWER AND AIR', [
    ['gpu', 'Ground power unit', { cmd: 'fbw/efb/ground/gpu', svc: 'gpu' }],
    ['asu', 'Air start unit', { cmd: 'fbw/gse/asu/toggle', svc: 'asu' }],
    ['pca', 'Preconditioned air', { cmd: 'fbw/gse/pca/toggle', svc: 'pca' }],
  ]],
  ['CATERING', [
    ['cat_main', 'Main deck highloader at M2R', { places: ['cat_m2r'] }],
    ['cat_upper', 'Upper deck highloader at U1R', { places: ['udcat_u1r'] }],
  ]],
  ['BAGGAGE AND CARGO', [
    ['hold_fwd', 'Forward hold: loader and ULD train', { places: ['ldcl_fwd', 'uld_fwd'] }],
    ['hold_aft', 'Aft hold: loader and ULD train', { places: ['ldcl_aft', 'uld_aft'] }],
    ['hold_bulk', 'Bulk hold: belt loader, tractor and carts', { places: ['belt_bulk', 'bag_tractor', 'bulk_cart'] }],
  ]],
  ['FUEL', [
    ['fuel_hydrant', 'Hydrant dispenser', { places: ['fuel_hydrant'] }],
    ['fuel_tanker', 'Fuel tanker', { places: ['fuel_tanker'] }],
  ]],
  ['STAIRS', [
    ['stairs_m1l', 'Main deck stairs at M1L', { places: ['stairs_m1l'] }],
    ['stairs_m2l', 'Main deck stairs at M2L', { places: ['stairs_m2l'] }],
    ['stairs_u1l', 'Upper deck stairs at U1L', { places: ['stairs_u1l'] }],
  ]],
  ['CABIN SERVICES', [
    ['water', 'Potable water truck', { cmd: 'fbw/gse/water/toggle', svc: 'water' }],
    ['lav', 'Lavatory service truck', { cmd: 'fbw/gse/lav/toggle', svc: 'lav' }],
    ['clean', 'Cabin cleaning', { cmd: 'fbw/gse/clean/toggle', svc: 'clean' }],
    ['deice', 'De-icing truck', { cmd: 'fbw/gse/deice/toggle', svc: 'deice' }],
  ]],
];
const gm = { gse: {}, busy: false };

function gmState(spec) {
  if (spec.places) return Math.max(0, ...spec.places.map((p) => +gm.gse[`place/${p}/state`] || 0));
  if (spec.svc === 'gpu' && (+gm.gse['connected/gpu'] || 0) > 0.5) return 2;
  return +gm.gse[`${spec.svc}/state`] || 0;
}

function gmStateText(st) {
  return ['', 'On the way', 'In place', 'Leaving'][st] || '';
}

async function gmCommand(command) {
  try { await studyPost('/study/action', { kind: 'command', command }); } catch { /* offline */ }
}

async function gmToggle(spec) {
  if (spec.cmd) { await gmCommand(spec.cmd); return; }
  const present = spec.places.filter((p) => [1, 2].includes(+gm.gse[`place/${p}/state`] || 0));
  const targets = present.length ? present : spec.places;
  for (const p of targets) await gmCommand(`fbw/gse/place/${p}/toggle`);
}

function gmChocks(vars) {
  return GM_CHOCKS.filter((c) => +vars['A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()] > 0.5).length;
}

async function gmSetChocks(vars) {
  const want = gmChocks(vars) === 0;
  for (const c of GM_CHOCKS) {
    if ((+vars['A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()] > 0.5) !== want) await gmCommand(`fbw/walkaround/chocks_${c}_toggle`);
  }
}

function gmCowl(vars, e) {
  return +vars[`A32NX_ENGINE_COWL_OPEN_${e + 1}`] || 0;
}

function gmNames() {
  return [...GM_DOORS.map((_, i) => `INTERACTIVE POINT OPEN:${i}`), ...GM_CHOCKS.map((c) => 'A32NX_WALKAROUND_CHOCKS_' + c.toUpperCase()), ...GM_ENGINES.map((_, e) => `A32NX_ENGINE_COWL_OPEN_${e + 1}`), 'PUSHBACK ATTACHED'];
}

async function gmPoll() {
  if (gm.busy) return;
  gm.busy = true;
  try { gm.gse = ((await studyGet('/gse/state')) || {}).values || {}; } catch { /* offline */ } finally { gm.busy = false; }
}

function gmPlan(vars) {
  const W = 640, H = 720, s = 7.6, cx = W / 2, top = 70 + 35.2 * s;
  const P = (x, z) => [cx + x * s, top + z * s];
  const sil = (DX_DATA.sil.top || []).map(([x, z], i) => `${i ? 'L' : 'M'}${P(x, z).map((v) => v.toFixed(1)).join(',')}`).join('') + 'Z';
  const svg = svgEl('svg', { viewBox: `0 0 ${W} ${H}`, class: 'gm-svg' });
  svg.append(svgEl('path', { d: sil, class: 'gm-ac' }));
  const chip = (x, y, label, on, moving, act, wide) => {
    const g = svgEl('g', { class: 'gm-chip' + (on ? ' on' : '') + (moving ? ' moving' : ''), 'data-gm': act });
    const w = wide || Math.max(38, label.length * 7.4 + 16);
    g.append(svgEl('path', { d: nsSquircle(x - w / 2, y - 12, w, 24, 7) }));
    const t = svgEl('text', { x, y: y + 4.5, 'text-anchor': 'middle' });
    t.textContent = label;
    g.append(t);
    return g;
  };
  const leader = (x1, y1, x2, y2, on) => svg.append(svgEl('line', { x1, y1, x2, y2, class: 'gm-lead' + (on ? ' on' : '') }));
  GM_DOORS.forEach(([name, x, z], i) => {
    const open = +vars[`INTERACTIVE POINT OPEN:${i}`] || 0;
    const [px, py] = P(x, z);
    const cargo = name.startsWith('CARGO');
    const upper = name[0] === 'U';
    const side = x < 0 ? -1 : 1;
    const off = cargo ? 150 : upper ? 96 : 46;
    const lx = px + side * off;
    const label = cargo ? (name === 'CARGO_FWD' ? 'FWD' : 'AFT') : name;
    leader(px, py, lx - side * 18, py, open > 2);
    svg.append(chip(lx, py, label, open > 98, open > 2 && open <= 98, `door:${name}`));
  });
  const bulkOn = gmState({ places: ['belt_bulk'] });
  const [bx, by] = P(3.25, 15.92 + 3.2);
  leader(...P(3.25, 15.92), bx + 150 - 18, by, bulkOn === 2);
  svg.append(chip(bx + 150, by, 'BULK', bulkOn === 2, bulkOn === 1 || bulkOn === 3, 'svc:hold_bulk'));
  const svcChip = (x, z, dx, dz, label, spec, act) => {
    const st = gmState(spec);
    const [px, py] = P(x, z);
    const [lx, ly] = [px + dx, py + dz];
    leader(px, py, lx, ly, st === 2);
    svg.append(chip(lx, ly, label, st === 2, st === 1 || st === 3, act));
  };
  svcChip(0.45, -29.2, 70, -34, 'GPU', { svc: 'gpu' }, 'svc:gpu');
  svcChip(-19.69, -6.1, -40, -40, 'FUEL', { places: ['fuel_hydrant', 'fuel_tanker'] }, 'svc:fuel');
  svcChip(-4.0, -8.0, -118, 52, 'PCA', { svc: 'pca' }, 'svc:pca');
  svcChip(4.0, -5.0, 118, 44, 'ASU', { svc: 'asu' }, 'svc:asu');
  svcChip(3.58, -18.65, 236, -22, 'CATERING', { places: ['cat_m2r'] }, 'svc:cat_main');
  svcChip(3.38, -14.2, 236, 22, 'CATERING UD', { places: ['udcat_u1r'] }, 'svc:cat_upper');
  svcChip(1.5, 24.0, 60, 18, 'WATER', { svc: 'water' }, 'svc:water');
  svcChip(-1.2, 26.5, -60, 18, 'LAV', { svc: 'lav' }, 'svc:lav');
  const jet = ['m1l', 'm2l', 'u1l'].filter((d) => (+gm.gse[`jetway_${d}/state`] || 0) > 1).length;
  const [jx, jy] = P(-3.05, -28.85);
  leader(jx, jy, jx - 120, jy - 30, jet > 0);
  svg.append(chip(jx - 150, jy - 30, jet > 1 ? `JETWAY x${jet}` : 'JETWAY', jet > 0, false, 'jetway'));
  GM_ENGINES.forEach(([x, z], e) => {
    const open = gmCowl(vars, e);
    const [px, py] = P(x, z - 3.2);
    leader(px, py, px, py - 26, open > 0.98);
    svg.append(chip(px, py - 38, `COWL ${e + 1}`, open > 0.98, open > 0.02 && open <= 0.98, `cowl:${e + 1}`));
  });
  const chocks = gmChocks(vars);
  const [cgx, cgy] = P(0, 1.5);
  svg.append(chip(cgx, cgy, 'CHOCKS', chocks === GM_CHOCKS.length, chocks > 0 && chocks < GM_CHOCKS.length, 'chocks'));
  const tug = +vars['PUSHBACK ATTACHED'] > 0.5;
  const [tx, ty] = P(0, -35.2);
  svg.append(chip(tx, ty - 26, 'TUG', tug, (+gm.gse['tug/state'] || 0) === 1, 'tug'));
  svg.addEventListener('click', async (e) => {
    const g = e.target.closest('[data-gm]');
    if (!g) return;
    const [kind, arg] = g.getAttribute('data-gm').split(':');
    if (kind === 'door') await gmCommand(`fbw/door/${arg}/toggle`);
    else if (kind === 'chocks') await gmSetChocks(vars);
    else if (kind === 'cowl') await gmCommand(`fbw/engine/cowl_${arg}_toggle`);
    else if (kind === 'jetway') await gmCommand('fbw/efb/ground/jetway');
    else if (kind === 'tug') await gmCommand('fbw/pushback/toggle');
    else if (kind === 'svc') {
      if (arg === 'fuel') {
        const present = ['fuel_hydrant', 'fuel_tanker'].find((p) => [1, 2].includes(+gm.gse[`place/${p}/state`] || 0));
        await gmCommand(`fbw/gse/place/${present || 'fuel_hydrant'}/toggle`);
      } else {
        const spec = GM_GROUPS.flatMap((g2) => g2[1]).find((r) => r[0] === arg);
        if (spec) await gmToggle(spec[2]);
      }
    }
    setTimeout(() => { if (typeof ground === 'object' && ground.redraw) gmPoll().then(() => ground.redraw()); }, 350);
  });
  const hint = svgEl('text', { x: W - 16, y: H - 14, 'text-anchor': 'end', class: 'gm-hint' });
  hint.textContent = 'Tap a door, a cowl or a service';
  svg.append(hint);
  return svg;
}

function gmList(vars) {
  const col = el('div', { class: 'gm-list' });
  const op = String(settings['xphfbw.gseOperator'] ?? '0');
  const seg = el('div', { class: 'gm-seg' }, ...GM_OPERATORS.map(([v, label]) => {
    const b = el('button', { type: 'button', class: 'pc-opt' + ((op === v || (v === '0' && !['dnata', 'swissport', 'menzies'].includes(op))) ? ' on' : ''), text: label });
    b.addEventListener('click', async () => {
      const r = await bridge.setSettings({ 'xphfbw.gseOperator': v });
      if (r && r.ok === false) { show(r.message || 'Could not save'); return; }
      settings = { ...settings, 'xphfbw.gseOperator': v };
      if (ground.redraw) ground.redraw();
    });
    return b;
  }));
  col.append(el('div', { class: 'gm-gh', text: 'HANDLING AGENT' }), seg);
  const row = (label, st, onClick, extra) => {
    const b = el('button', { type: 'button', class: 'pc-btn gm-act' + (st === 2 || st === 1 ? '' : st === 3 ? '' : ' pc-go'), text: st === 2 || st === 1 ? 'SEND AWAY' : st === 3 ? 'LEAVING' : 'CALL' });
    if (st === 3) b.disabled = true;
    b.addEventListener('click', async () => { await onClick(); setTimeout(() => gmPoll().then(() => ground.redraw && ground.redraw()), 350); });
    return el('div', { class: 'gm-row' }, el('div', { class: 'gm-rl' }, el('span', { text: label }), el('span', { class: 'gm-rs' + (st === 2 ? ' ok' : st ? ' chg' : ''), text: extra !== undefined ? extra : gmStateText(st) })), b);
  };
  const chocks = gmChocks(vars);
  col.append(el('div', { class: 'gm-gh', text: 'ON THE AIRCRAFT' }), row('Wheel chocks', chocks === GM_CHOCKS.length ? 2 : 0, () => gmSetChocks(vars), chocks ? `${chocks} of ${GM_CHOCKS.length} in place` : ''));
  const opened = GM_ENGINES.map((_, e) => gmCowl(vars, e));
  const cowls = el('div', { class: 'gm-seg' }, ...opened.map((open, e) => {
    const b = el('button', { type: 'button', class: 'pc-opt' + (open > 0.98 ? ' on' : ''), text: `ENG ${e + 1}` });
    b.addEventListener('click', async () => { await gmCommand(`fbw/engine/cowl_${e + 1}_toggle`); setTimeout(() => ground.redraw && ground.redraw(), 350); });
    return b;
  }));
  const moving = opened.some((o) => o > 0.02 && o <= 0.98);
  const openList = opened.map((o, e) => (o > 0.98 ? e + 1 : 0)).filter(Boolean);
  col.append(el('div', { class: 'gm-row' }, el('div', { class: 'gm-rl' }, el('span', { text: 'Fan cowl doors' }), el('span', { class: 'gm-rs' + (moving ? ' chg' : openList.length ? ' ok' : ''), text: moving ? 'Moving' : openList.length ? `Open on engine ${openList.join(', ')}` : 'Parked, engine stopped' })), cowls));
  for (const [title, rows] of GM_GROUPS) {
    col.append(el('div', { class: 'gm-gh', text: title }));
    for (const [, label, spec] of rows) col.append(row(label, gmState(spec), () => gmToggle(spec)));
  }
  const cargo = el('button', { type: 'button', class: 'gm-link', text: 'Special cargo, NOTOC and load planning in Flight › Cargo' });
  cargo.addEventListener('click', () => document.querySelector('.tab-btn[data-tab="cargo"]')?.click());
  col.append(cargo);
  return col;
}

function gmSection(vars) {
  return el('div', { class: 'pc-card gm-card' }, el('div', { class: 'pc-h', text: 'GROUND SERVICES' }), el('div', { class: 'gm-grid' }, el('div', { class: 'gm-plan' }, gmPlan(vars)), gmList(vars)));
}

(function () {
  const s = document.createElement('style');
  s.textContent = `
.gm-card{margin-bottom:16px;}
.gm-grid{display:grid;grid-template-columns:minmax(420px,0.95fr) 1fr;gap:18px;align-items:start;}
.gm-plan{background:#25272b;border:1px solid rgba(255,255,255,0.08);border-radius:14px;padding:6px;position:sticky;top:8px;}
.gm-svg{width:100%;height:min(640px,calc(100vh - 330px));display:block;font-family:'Manrope','Segoe UI',sans-serif;}
.gm-ac{fill:#2f3237;stroke:#61656c;stroke-width:1.2;}
.gm-chip{cursor:pointer;}
.gm-chip path{fill:#1f2226;stroke:#61656c;stroke-width:1.4;}
.gm-chip text{fill:#8b9097;font-weight:800;font-size:12px;letter-spacing:0.04em;pointer-events:none;}
.gm-chip.on path{fill:#3fd27f;stroke:#3fd27f;}
.gm-chip.on text{fill:#14301f;}
.gm-chip.moving path{fill:#3a2f14;stroke:#ffb000;}
.gm-chip.moving text{fill:#ffb000;}
.gm-chip:hover path{stroke:#ff4d6a;}
.gm-lead{stroke:#61656c;stroke-width:1.2;}
.gm-lead.on{stroke:#3fd27f;stroke-width:1.6;}
.gm-hint{fill:#8b9097;font-size:12px;font-weight:600;}
.gm-list{display:flex;flex-direction:column;gap:6px;}
.gm-gh{font:800 11.5px 'Manrope','Segoe UI',sans-serif;letter-spacing:0.09em;color:#9da3aa;margin:10px 0 2px;}
.gm-row{display:flex;align-items:center;justify-content:space-between;gap:12px;background:#2a2c30;border:1px solid rgba(255,255,255,0.08);border-radius:12px;padding:8px 10px 8px 14px;}
.gm-rl{display:flex;flex-direction:column;gap:2px;font:700 14px 'Manrope','Segoe UI',sans-serif;color:#f2f2f2;}
.gm-rs{font:600 12px 'Manrope','Segoe UI',sans-serif;color:#8b9097;min-height:14px;}
.gm-rs.ok{color:#3fd27f;}
.gm-rs.chg{color:#ffb000;}
#tab-ground .pc-btn.gm-act{min-width:118px;}
.gm-seg{display:flex;gap:6px;flex-wrap:wrap;}
#tab-ground button.gm-link{margin-top:10px;background:none;border:none !important;color:#ff4d6a;font:700 13px 'Manrope','Segoe UI',sans-serif;text-align:left;padding:4px 0;box-shadow:none !important;}
`;
  document.head.append(s);
})();
