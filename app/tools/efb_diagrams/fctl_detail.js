function fctlDetailMode(name) {
  const n = name.toUpperCase();
  const rules = [
    [/SEPARATION/, 'separated'], [/DISCONNECT/, 'disconnect'], [/HARDOVER/, 'hardover'], [/JAM/, 'jam'],
    [/SUPPLY LOSS|HYDRAULIC\/ELECTRICAL/, 'supply'], [/FLUTTER|DAMPER/, 'damper'], [/SERVO VALVE|INTERNAL LEAK/, 'servovalve'],
    [/SEAL|PISTON/, 'seal'], [/TRANSDUCER|LVDT|POSITION|DRIFT/, 'lvdt'], [/OPEN CIRCUIT|SHORT|WIRING|INTERMITTENT|CHANNEL|COMMAND/, 'wiring'],
  ];
  const hit = rules.find(([re]) => re.test(n));
  return hit ? hit[1] : 'fault';
}

function fctlDetailSurface(name) {
  const n = ' ' + name.toUpperCase() + ' ';
  const side = /\bLEFT\b|\bL\b/.test(n) ? 'L' : /\bRIGHT\b|\bR\b/.test(n) ? 'R' : 'L';
  const sideName = side === 'L' ? 'LEFT' : 'RIGHT';
  if (/ELEVATOR/.test(n)) {
    const pos = /OUTBOARD|OUTWARD|OUTER/.test(n) ? 'OUT' : 'IN';
    const key = `ELEV_${side}_${pos}`;
    return { key, box: pos === 'OUT' ? 'l_outer_elevator_deflection' : 'l_inner_elevator_deflection', side, title: `${sideName} ${pos === 'OUT' ? 'OUTBOARD' : 'INBOARD'} ELEVATOR`, power: FCTL_POWER[key] };
  }
  if (/AILERON/.test(n)) {
    const pos = /OUTWARD|OUTER/.test(n) ? 'OUT' : /MIDDLE|\bMID\b/.test(n) ? 'MID' : 'IN';
    return { key: `AIL_${side}_${pos}`, box: `l_aileron${pos === 'IN' ? 1 : pos === 'MID' ? 2 : 3}_percent_key`, side, title: `${sideName} ${pos === 'OUT' ? 'OUTWARD' : pos === 'MID' ? 'MIDDLE' : 'INWARD'} AILERON`, power: FCTL_POWER['AIL_' + pos] };
  }
  if (/RUDDER/.test(n)) {
    const up = !/LOWER/.test(n);
    return { key: up ? 'RUD_UP' : 'RUD_LO', box: null, side: 'C', title: up ? 'UPPER RUDDER' : 'LOWER RUDDER', power: FCTL_POWER[up ? 'RUD_UP' : 'RUD_LO'] };
  }
  const sp = n.match(/SPOILER ?(\d)/);
  if (sp) return { key: `SPLR_${side}${sp[1]}`, box: `l_spoiler${sp[1]}_key`, side, title: `${sideName} SPOILER ${sp[1]}`, power: sp[1] === '6' ? ['G', 'E ESS'] : [+sp[1] % 2 ? 'Y' : 'G'] };
  return null;
}

function fctlSurfacePoly(boxKey, side) {
  const b = DX_DATA.model.surfaces[boxKey];
  if (!b) return null;
  const k = side === 'R' ? 1 : -1;
  const xa = Math.abs(b[0]), xb = Math.abs(b[1]);
  const tail = /elevator/.test(boxKey);
  const slope = tail ? 0.36 : (DX_DATA.model.te[DX_DATA.model.te.length - 1][1] - DX_DATA.model.te[0][1]) / (DX_DATA.model.te[DX_DATA.model.te.length - 1][0] - DX_DATA.model.te[0][0]);
  const [xi, xo] = [Math.min(xa, xb), Math.max(xa, xb)];
  const chord = Math.max(0.8, b[3] - b[2] - slope * (xo - xi));
  const aftIn = b[3] - slope * (xo - xi);
  return [[k * xi, aftIn - chord], [k * xo, aftIn - chord + slope * (xo - xi)], [k * xo, b[3]], [k * xi, aftIn]];
}

function fctlLocator(surf, x0, y0, w, h) {
  const s = Math.min(w / 82, h / 74);
  const p = (x, z) => [x0 + w / 2 + x * s, y0 + (z + 35.4) * s];
  const out = [`<rect x="${x0}" y="${y0}" width="${w}" height="${h}" rx="8" fill="#1f2226" stroke="${NS.grey}" stroke-width="1"/>`];
  out.push(exPath(DX_DATA.sil.top.map(([x, z]) => p(x, z)), { fill: 'rgba(255,255,255,0.03)', stroke: NS.grey, sw: 1 }));
  const poly = surf.box ? fctlSurfacePoly(surf.box, surf.side) : [[-0.8, 28], [0.8, 28], [0.8, 37], [-0.8, 37]];
  out.push(`<path d="M${poly.map(([x, z]) => p(x, z).map((v) => v.toFixed(1)).join(',')).join(' L')} Z" fill="${NS.red}" stroke="#fff" stroke-width="1"/>`);
  const c = poly.reduce((a, q) => [a[0] + q[0] / poly.length, a[1] + q[1] / poly.length], [0, 0]);
  const [cx, cy] = p(c[0], c[1]);
  out.push(`<circle cx="${cx.toFixed(1)}" cy="${cy.toFixed(1)}" r="9" fill="none" stroke="${NS.red}" stroke-width="1.5"/>`);
  return out.join('');
}

function fctlDetailView(name, title) {
  const surf = fctlDetailSurface(name);
  if (!surf) return null;
  const mode = fctlDetailMode(name);
  const W = 1000, H = 660;
  const wires = [], fg = [];
  const comps = { G: 'PRIM 1', Y: 'PRIM 2' };
  fg.push(exT(24, 32, surf.title, { anchor: 'start', size: 17, fill: NS.green }));
  const modeText = { separated: 'SURFACE SEPARATION', disconnect: 'SURFACE DISCONNECT', hardover: 'SERVO HARDOVER', jam: 'ACTUATOR JAM', supply: 'POWER SUPPLY LOSS', damper: 'FLUTTER DAMPER LOSS', servovalve: 'SERVO VALVE INTERNAL LEAKAGE', seal: 'PISTON SEAL WEAR', lvdt: 'POSITION TRANSDUCER FAULT', wiring: 'COMMAND / MONITOR WIRING FAULT', fault: 'ACTUATOR FAULT' }[mode];
  fg.push(exT(24, 52, modeText, { anchor: 'start', size: 11, fill: NS.red }));
  fg.push(fctlLocator(surf, W - 214, 14, 196, 150));
  const hingeX = 800, hingeY = 380;
  surf.power.forEach((p, i) => {
    const primary = i === 0;
    const isHyd = p === 'G' || p === 'Y';
    const y = 120 + i * (surf.power.length > 2 ? 160 : 230);
    const ctrl = isHyd ? comps[p] : i === 1 ? 'SEC 1' : 'SEC 2';
    const bad = (on, worse = 'fail') => (primary && on ? worse : 'live');
    fg.push(exT(24, y - 10, ctrl, { size: 11, anchor: 'start', fill: NS.greyText }));
    fg.push(nsBox(24, y, 104, 34, 'COM LANE', bad(mode === 'wiring'), { size: 10, r: 7 }), nsBox(24, y + 48, 104, 34, 'MON LANE', 'live', { size: 10, r: 7 }));
    const ax = 176, aw = 520, ah = 150;
    fg.push(`<path d="${nsSquircle(ax, y - 16, aw, ah, 14)}" fill="none" stroke="${NS.grey}" stroke-width="1.2" stroke-dasharray="3 4"/>`);
    fg.push(exT(ax + aw, y - 24, isHyd ? `HYDRAULIC SERVO CONTROL · ${p === 'G' ? 'GREEN' : 'YELLOW'}` : `ELECTRO-HYDROSTATIC ACTUATOR · ${p.replace('E ', 'AC ')}`, { size: 10, anchor: 'end', fill: NS.greyText }));
    const row = y + 4;
    const chain = isHyd
      ? [['SOV', 'SHUTOFF V', bad(mode === 'supply')], ['FILTER', 'FILTER', 'live'], ['CHECK', 'CHECK V', 'live'], ['SV', 'SERVO V', bad(mode === 'hardover' || mode === 'servovalve', mode === 'hardover' ? 'fail' : 'warn')], ['MV', 'MODE V', bad(mode === 'fault', 'warn')]]
      : [['CB', 'C/B', bad(mode === 'supply')], ['PE', 'POWER ELEC', bad(mode === 'fault', 'warn')], ['MOT', 'MOTOR', 'live'], ['PUMP', 'PUMP', bad(mode === 'servovalve', 'warn')], ['ACC', 'ACCUM', 'live']];
    chain.forEach(([k, label, st], j) => {
      const x = ax + 12 + j * 74;
      if (j) wires.push(nsWire([[x - 8, row + 15], [x, row + 15]], st === 'fail' ? 'fail' : 'live'));
      fg.push(nsBox(x, row, 64, 30, label, st, { size: 9, r: 7 }));
    });
    const pistonSt = bad(mode === 'jam' || mode === 'seal', mode === 'jam' ? 'fail' : 'warn');
    const px = ax + 12 + 5 * 74;
    wires.push(nsWire([[px - 8, row + 15], [px, row + 15]], pistonSt === 'fail' ? 'fail' : 'live'));
    fg.push(`<path d="${nsSquircle(px, row - 6, 128, 42, 8)}" fill="${pistonSt === 'fail' ? NS.failFill : pistonSt === 'warn' ? NS.warnFill : NS.fill}" stroke="${nsStroke(pistonSt)}" stroke-width="2.2"/>`);
    fg.push(`<path d="M${px + 64},${row - 2} L${px + 64},${row + 32}" stroke="${nsStroke(pistonSt)}" stroke-width="3"/>`, exT(px + 32, row + 19, 'EXT', { size: 9, fill: NS.greyText }), exT(px + 96, row + 19, 'RET', { size: 9, fill: NS.greyText }));
    fg.push(exT(px + 64, row - 12, 'PISTON', { size: 9, fill: nsStroke(pistonSt) }));
    const damp = bad(mode === 'damper', 'warn');
    fg.push(nsValve(px + 64, row + 56, damp, true, 'DAMPING ORIFICE'));
    wires.push(nsWire([[px + 64, row + 36], [px + 64, row + 47]], damp));
    const lvdtCom = bad(mode === 'lvdt'), lvdtMon = 'live';
    fg.push(nsBox(ax + 12, row + 60, 90, 26, 'LVDT COM', lvdtCom, { size: 9, r: 6 }), nsBox(ax + 112, row + 60, 90, 26, 'LVDT MON', lvdtMon, { size: 9, r: 6 }));
    fg.push(nsBox(ax + 212, row + 60, 90, 26, 'MODE SOL', bad(mode === 'wiring', 'warn'), { size: 9, r: 6 }));
    const cmd = bad(mode === 'wiring');
    wires.push(nsWire([[128, y + 17], [150, y + 17], [150, row - 30], [ax + 12 + 3 * 74 + 32, row - 30], [ax + 12 + 3 * 74 + 32, row]], cmd, cmd === 'fail' ? '5 5' : ''));
    fg.push(exT(160, row - 34, 'SERVO COMMAND', { size: 8, anchor: 'start', fill: cmd === 'fail' ? NS.red : NS.greyText }));
    wires.push(nsWire([[128, y + 22], [158, y + 22], [158, row + 100], [ax + 257, row + 100], [ax + 257, row + 86]], cmd === 'fail' ? 'fail' : 'live', cmd === 'fail' ? '5 5' : ''));
    wires.push(nsWire([[ax + 57, row + 86], [ax + 57, row + 108], [140, row + 108], [140, y + 28], [128, y + 28]], lvdtCom === 'fail' ? 'fail' : 'live', '6 4'));
    wires.push(nsWire([[ax + 157, row + 86], [ax + 157, row + 116], [136, row + 116], [136, y + 66], [128, y + 66]], 'live', '6 4'));
    fg.push(exT(ax + 300, row + 112, 'FEEDBACK COM / MON', { size: 8, anchor: 'start', fill: lvdtCom === 'fail' ? NS.red : NS.greyText }));
    const supplySt = bad(mode === 'supply');
    wires.push(nsWire([[ax + 44, row + 30], [ax + 44, row + 46], [ax - 30, row + 46], [ax - 30, y + ah + 6]], supplySt, supplySt === 'fail' ? '5 5' : ''));
    fg.push(exT(ax - 24, y + ah + 2, isHyd ? `${p === 'G' ? 'GREEN' : 'YELLOW'} SYSTEM 5000 PSI` : `${p.replace('E ', 'AC ')} BUS 115 VAC`, { size: 9, anchor: 'start', fill: supplySt === 'fail' ? NS.red : NS.greyText }));
    const rod = bad(mode === 'disconnect');
    wires.push(nsWire([[px + 128, row + 15], [hingeX - 30, hingeY + (i ? 26 : -26)]], rod, rod === 'fail' ? '7 6' : ''));
  });
  const defl = mode === 'hardover' ? 25 : mode === 'disconnect' ? -12 : 0;
  const surfSt = mode === 'separated' || mode === 'jam' || mode === 'hardover' || mode === 'disconnect' ? 'fail' : mode === 'fault' || mode === 'supply' || mode === 'damper' ? 'warn' : 'live';
  const c = nsStroke(surfSt);
  const poly = surf.box ? fctlSurfacePoly(surf.box, 'R') : [[0, 0], [1.6, 0], [1.6, 9], [0, 9]];
  const xs = poly.map((q) => q[0]), zs = poly.map((q) => q[1]);
  const sx = 150 / Math.max(1, Math.max(...xs) - Math.min(...xs)), sz = 160 / Math.max(1, Math.max(...zs) - Math.min(...zs));
  const sc = Math.min(sx, sz);
  const pts = poly.map(([x, z]) => [hingeX + (x - Math.min(...xs)) * sc, hingeY - 80 + (z - Math.min(...zs)) * sc]);
  fg.push(`<g transform="rotate(${defl} ${hingeX} ${hingeY})"><path d="M${pts.map((q) => q.map((v) => v.toFixed(1)).join(',')).join(' L')} Z" fill="${surfSt === 'fail' ? 'rgba(255,61,99,0.15)' : NS.fill}" stroke="${c}" stroke-width="2.4"${mode === 'separated' ? ' stroke-dasharray="7 5"' : ''}/></g>`);
  fg.push(`<path d="M${hingeX - 10},${hingeY - 100} L${hingeX - 10},${hingeY + 100}" stroke="${NS.grey}" stroke-width="1.4" stroke-dasharray="8 5"/>`, exT(hingeX - 10, hingeY + 116, 'HINGE LINE', { size: 9, fill: NS.greyText }));
  const status = { separated: 'SURFACE SEPARATED', disconnect: 'DISCONNECTED - FLOATING', hardover: 'HARDOVER - FULL DEFLECTION', jam: 'JAMMED - SURFACE FROZEN', supply: 'ACTUATOR IN DAMPING MODE', damper: 'FLUTTER DAMPING LOST', servovalve: 'SERVO VALVE LEAKING', seal: 'PISTON SEAL WEAR', lvdt: 'POSITION FEEDBACK LOST', wiring: 'COMMAND / MONITOR WIRING FAULT', fault: 'ACTUATOR FAULT - OTHER ACTUATOR ACTIVE' }[mode];
  fg.push(exT(hingeX + 70, hingeY + 140, status, { size: 11, fill: surfSt === 'live' ? NS.orange : c }));
  if (title) fg.push(exT(W - 16, H - 8, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  return nsSvg(W, H, [...wires, ...fg]);
}
