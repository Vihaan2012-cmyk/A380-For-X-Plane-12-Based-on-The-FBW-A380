const FCTL_POWER = {
  AIL_OUT: ['G', 'Y'], AIL_MID: ['Y', 'E ESS'], AIL_IN: ['G', 'E EHA'],
  ELEV_L_IN: ['G', 'E ESS'], ELEV_L_OUT: ['G', 'E EHA'], ELEV_R_IN: ['Y', 'E EHA'], ELEV_R_OUT: ['Y', 'E ESS'],
  RUD_UP: ['G', 'E ESS', 'E EHA'], RUD_LO: ['Y', 'E ESS', 'E AC1'], THS: ['G', 'Y', 'E'], SLATS: ['G', 'E'], FLAPS: ['G', 'Y'],
};
const FCTL_CTRL = {
  AIL_L_IN: ['PRIM 1', 'SEC 1'], AIL_R_IN: ['PRIM 1', 'SEC 1'], AIL_L_MID: ['PRIM 2', 'SEC 2'], AIL_R_MID: ['PRIM 2', 'SEC 2'], AIL_L_OUT: ['PRIM 3', 'SEC 3'], AIL_R_OUT: ['PRIM 3', 'SEC 3'],
  ELEV_L_IN: ['PRIM 1', 'SEC 1'], ELEV_R_IN: ['PRIM 1', 'SEC 1'], ELEV_L_OUT: ['PRIM 2', 'SEC 2'], ELEV_R_OUT: ['PRIM 2', 'SEC 2'],
  RUD_UP: ['PRIM 1', 'SEC 1', 'BCM'], RUD_LO: ['PRIM 2', 'SEC 3', 'BCM'], THS: ['PRIM 1', 'PRIM 2', 'PRIM 3', 'SEC 1', 'SEC 2'],
  SLATS: ['SFCC 1', 'SFCC 2'], FLAPS: ['SFCC 1', 'SFCC 2'],
};
const FCTL_SPOILER_CTRL = { 1: ['PRIM 1', 'SEC 1'], 2: ['PRIM 2'], 3: ['PRIM 3'], 4: ['PRIM 1'], 5: ['PRIM 2'], 6: ['PRIM 3', 'SEC 3'], 7: ['PRIM 1'], 8: ['PRIM 2', 'SEC 2'] };
function fctlControl(key, m) {
  const sp = key.match(/^SPLR_[LR](\d)$/);
  const list = sp ? FCTL_SPOILER_CTRL[sp[1]] : FCTL_CTRL[key];
  if (!list) return { st: 'live', tag: '' };
  const alive = list.filter((c) => m.get(c) !== 'fail');
  if (!alive.length) return { st: 'warn', tag: 'LOST' };
  if (alive[0] !== list[0]) return { st: 'alt', tag: 'VIA ' + alive[0] };
  return { st: 'live', tag: '' };
}
const FCTL_COMPUTERS = ['PRIM 1', 'PRIM 2', 'PRIM 3', 'SEC 1', 'SEC 2', 'SEC 3', 'FCDC 1', 'FCDC 2', 'SFCC 1', 'SFCC 2', 'BCM'];

function fctlTargets(name) {
  const n = ' ' + name.toUpperCase().replace(/[_-]/g, ' ') + ' ';
  const m = new Map();
  const side = /\bLEFT\b|\bL\b/.test(n) ? 'L' : /\bRIGHT\b|\bR\b/.test(n) ? 'R' : null;
  const soft = /SUPPLY LOSS|TRANSDUCER|DRIFT|BIAS|WEAR|LEAKAGE|DAMPER|CHANNEL|MONITOR|INTERMITTENT/.test(n);
  const st = soft ? 'warn' : 'fail';
  let k;
  if ((k = n.match(/\b(PRIM|SEC|FCDC|SFCC) ?(\d)/))) return m.set(`${k[1]} ${k[2]}`, st), m;
  if (/BCM|BACKUP CONTROL/.test(n)) return m.set('BCM', st), m;
  const sides = side ? [side] : ['L', 'R'];
  if ((k = n.match(/SPOILER ?(\d)/))) sides.forEach((s) => m.set(`SPLR_${s}${k[1]}`, st));
  else if (/SPOILERS?|SPLR/.test(n)) sides.forEach((s) => { for (let i = 1; i <= 8; i++) m.set(`SPLR_${s}${i}`, st); });
  else if (/AILERON|\bAIL\b/.test(n)) {
    const pos = /OUTWARD|OUTER|\bOUTR\b|OUTBOARD/.test(n) ? ['OUT'] : /MIDDLE|\bMID\b/.test(n) ? ['MID'] : /INWARD|INNER|\bINR\b|INBOARD/.test(n) ? ['IN'] : ['IN', 'MID', 'OUT'];
    sides.forEach((s) => pos.forEach((p) => m.set(`AIL_${s}_${p}`, st)));
  } else if (/ELEVATOR|\bELEV\b/.test(n)) {
    const pos = /OUTBOARD|OUTWARD|OUTER|\bOUTR\b/.test(n) ? ['OUT'] : /INBOARD|INWARD|INNER|\bINR\b/.test(n) ? ['IN'] : ['IN', 'OUT'];
    sides.forEach((s) => pos.forEach((p) => m.set(`ELEV_${s}_${p}`, st)));
  } else if (/RUDDER|\bRUD\b/.test(n)) (/UPPER/.test(n) ? ['RUD_UP'] : /LOWER/.test(n) ? ['RUD_LO'] : ['RUD_UP', 'RUD_LO']).forEach((r) => m.set(r, st));
  else if (/THS|PITCH TRIM|STABILI[SZ]ER|HORIZONTAL/.test(n)) m.set('THS', st);
  else if (/SLAT|DROOP/.test(n)) m.set('SLATS', st);
  else if (/FLAP/.test(n)) m.set('FLAPS', st);
  else return null;
  return m;
}

function fctlLaw(m) {
  const prims = [1, 2, 3].filter((i) => m.get('PRIM ' + i) === 'fail').length;
  const secs = [1, 2, 3].filter((i) => m.get('SEC ' + i) === 'fail').length;
  if (prims === 3 && secs === 3) return 'BACKUP (BCM)';
  if (prims === 3) return 'DIRECT LAW';
  if (prims >= 2) return 'ALTN LAW';
  return null;
}

function fctlView(m, title) {
  const W = 1000, H = 520;
  const wires = [], fg = [];
  const s = (k) => m.get(k) || 'live';
  FCTL_COMPUTERS.forEach((c, i) => {
    const x = 20 + i * 88;
    wires.push(nsWire([[x + 40, 64], [x + 40, 92]], s(c) === 'fail' ? 'fail' : 'live', s(c) === 'fail' ? '5 5' : ''));
    fg.push(nsBox(x, 28, 80, 36, c, s(c), { size: 11, tag: m.get(c) === 'fail' ? 'FAILED' : m.get(c) ? 'FAULT' : '' }));
  });
  wires.push(nsWire([[8, 92], [992, 92]], 'live'));
  fg.push(exT(500, 86, 'FLIGHT CONTROL COMMAND BUSES', { size: 9, fill: NS.greyText }));
  const law = fctlLaw(m);
  if (law) fg.push(exT(980, 128, law, { size: 14, anchor: 'end', fill: NS.orange }));
  const surface = (key, label, x, y, w, power) => {
    const ctl = fctlControl(key, m);
    const st = m.get(key) || ctl.st;
    wires.push(nsWire([[x + w / 2, y - 22], [x + w / 2, y]], st === 'fail' ? 'fail' : 'live'));
    fg.push(nsBox(x, y, w, 30, label, st, { size: 10, r: 7, tag: m.get(key) === 'fail' ? 'FAILED' : m.get(key) ? 'DEGRADED' : ctl.tag }));
    if (power && power.length) fg.push(exT(x + w / 2, y + 42, power.map((p) => p.replace('E ', 'E·')).join(' '), { size: 8, fill: st === 'fail' ? NS.red : '#8fd9b0', weight: 700 }));
  };
  const LR = 10, RR = 990;
  const rowBus = (y, xa, xb) => wires.push(nsWire([[xa, y - 22], [xb, y - 22]], 'live'));
  rowBus(166, LR, 486); rowBus(256, LR, 486); rowBus(166, 514, RR); rowBus(256, 514, RR); rowBus(380, LR, RR);
  wires.push(nsWire([[LR, 92], [LR, 358]], 'live'), nsWire([[RR, 92], [RR, 358]], 'live'));
  for (const [side, x0, dir] of [['L', 20, 1], ['R', 520, -1]]) {
    fg.push(exT(x0 + 6, 128, side === 'L' ? 'LEFT WING' : 'RIGHT WING', { size: 11, anchor: 'start', fill: NS.greyText }));
    const row = side === 'L' ? [['AIL_L_OUT', 'OUT AIL', 'AIL_OUT'], ['AIL_L_MID', 'MID AIL', 'AIL_MID'], ['AIL_L_IN', 'IN AIL', 'AIL_IN'], ['SLATS', 'SLATS', 'SLATS'], ['FLAPS', 'FLAPS', 'FLAPS']]
      : [['FLAPS', 'FLAPS', 'FLAPS'], ['SLATS', 'SLATS', 'SLATS'], ['AIL_R_IN', 'IN AIL', 'AIL_IN'], ['AIL_R_MID', 'MID AIL', 'AIL_MID'], ['AIL_R_OUT', 'OUT AIL', 'AIL_OUT']];
    row.forEach(([k, label, pk], i) => surface(k, label, x0 + i * 92, 166, 84, FCTL_POWER[pk]));
    const spl = [...Array(8).keys()].map((i) => i + 1);
    (side === 'L' ? spl.slice().reverse() : spl).forEach((n, i) => {
      const pw = n === 6 ? ['G', 'E ESS'] : [n % 2 ? 'Y' : 'G'];
      surface(`SPLR_${side}${n}`, `SPLR ${n}`, x0 + i * 58, 256, 52, pw);
    });
  }
  fg.push(exT(26, 344, 'EMPENNAGE', { size: 11, anchor: 'start', fill: NS.greyText }));
  [['ELEV_L_OUT', 'L OUT ELEV', 'ELEV_L_OUT'], ['ELEV_L_IN', 'L IN ELEV', 'ELEV_L_IN'], ['RUD_UP', 'UPPER RUDDER', 'RUD_UP'], ['RUD_LO', 'LOWER RUDDER', 'RUD_LO'], ['THS', 'THS', 'THS'], ['ELEV_R_IN', 'R IN ELEV', 'ELEV_R_IN'], ['ELEV_R_OUT', 'R OUT ELEV', 'ELEV_R_OUT']].forEach(([k, label, pk], i) => {
    surface(k, label, 26 + i * 136, 380, 120, FCTL_POWER[pk]);
  });
  fg.push(exT(26, 462, 'POWER', { size: 10, anchor: 'start', fill: NS.greyText }));
  fg.push(exT(26, 480, 'G / Y  HYDRAULIC SERVO   E·ESS  AC ESS EHA/EBHA   E·EHA  AC EHA (247XP)   E·AC1  AC 1   E  ELECTRIC MOTOR', { size: 9, anchor: 'start', fill: NS.greyText, weight: 500 }));
  if (title) fg.push(exT(W - 16, H - 8, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  return nsSvg(W, H, [...wires, ...fg]);
}

function fctlDiagram(name, title) {
  const m = fctlTargets(name);
  return m ? fctlView(m, title) : null;
}
