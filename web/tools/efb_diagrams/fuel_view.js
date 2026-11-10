const FUELV_TANKS = [
  ['OUTR_L', 'L OUTER', 18, 'outer'], ['FEED1', 'FEED 1', 114, 'feed'], ['MID_L', 'L MID', 206, 'mid'], ['FEED2', 'FEED 2', 302, 'feed'], ['INR_L', 'L INNER', 394, 'inner'],
  ['INR_R', 'R INNER', 518, 'inner'], ['FEED3', 'FEED 3', 610, 'feed'], ['MID_R', 'R MID', 702, 'mid'], ['FEED4', 'FEED 4', 794, 'feed'], ['OUTR_R', 'R OUTER', 890, 'outer'],
];
const FUELV_CAP_T = { OUTR_L: 8.9, FEED1: 21.6, MID_L: 27.9, FEED2: 23.2, INR_L: 36.5, INR_R: 36.5, FEED3: 23.2, MID_R: 27.9, FEED4: 21.6, OUTR_R: 8.9 };
const FUELV_PUMPS = { feed: ['MAIN', 'STBY'], inner: ['FWD', 'AFT'], mid: ['FWD', 'AFT'], outer: ['XFR'] };

function fuelTargets(name) {
  const n = ' ' + name.toUpperCase().replace(/[_-]/g, ' ') + ' ';
  const m = new Map();
  const side = /\bLEFT\b|\bL\b/.test(n) ? 'L' : /\bRIGHT\b|\bR\b/.test(n) ? 'R' : null;
  const tank = (() => {
    let f = n.match(/FEED(?: TANK)? ?(\d)|COLLECTOR CELL (\d)|TANK (\d)\b/);
    if (f) return 'FEED' + (f[1] || f[2] || f[3]);
    if (/OUTER|OUTR/.test(n) && side) return 'OUTR_' + side;
    if (/\bMID\b/.test(n) && side) return 'MID_' + side;
    if (/INNER|\bINR\b/.test(n) && side) return 'INR_' + side;
    if (/TRIM/.test(n)) return 'TRIM';
    return null;
  })();
  const soft = /DEGRAD|WEAR|DRIFT|FOULING|CONTAMINATION|BIAS/.test(n);
  let e;
  if ((e = n.match(/ENG(?:INE)? ?(\d).*(?:LP|SHUTOFF|SHUT OFF)/)) || (e = n.match(/LP (?:FUEL )?VALVE ?(\d)/))) m.set('LP' + e[1], 'fail');
  else if ((e = n.match(/CROSS ?FEED VALVE ?(\d)(?: ?(\d))?|XFEED ?(?:VALVE )?(\d)/))) { m.set('XF' + (e[1] || e[3]), 'fail'); if (e[2]) m.set('XF' + e[2], 'fail'); }
  else if (/JETT/.test(n)) m.set('JETT_' + (side || 'L'), 'fail');
  else if (/REFUEL|DEFUEL/.test(n)) m.set('REFUEL_' + (side || 'L'), 'fail');
  else if (/APU/.test(n)) m.set('APU', 'fail');
  else if (/GALLERY/.test(n)) m.set(/AFT/.test(n) ? 'GAL_AFT' : 'GAL_FWD', 'fail');
  else if (/TRIM LINE|ISOLATION/.test(n)) m.set(/AFT/.test(n) ? 'TRIMISO_AFT' : 'TRIMISO_FWD', 'fail');
  else if (tank && /INLET/.test(n)) m.set(tank + ':' + (/AFT/.test(n) ? 'IN_AFT' : 'IN_FWD'), 'fail');
  else if (tank && /PUMP/.test(n)) {
    const kind = tank.startsWith('FEED') ? (/STBY|STANDBY|\bB\b/.test(n) ? 'STBY' : 'MAIN') : tank === 'TRIM' ? (side === 'R' ? 'R' : 'L') : tank.startsWith('OUTR') ? 'XFR' : /AFT/.test(n) ? 'AFT' : 'FWD';
    m.set(tank + ':' + kind, soft ? 'warn' : 'fail');
  } else if (tank && /PUNCTUR|LEAK/.test(n)) m.set(tank, 'fail');
  else if (tank) m.set(tank, 'warn');
  return m.size ? m : null;
}

function fuelView(m, title) {
  const W = 1000, H = 600;
  const wires = [], fg = [];
  const TY = 214, TH = 64, TW = 88;
  const at = new Map(FUELV_TANKS.map(([k, , x]) => [k, x + TW / 2]));
  const valve = (x, y, k, label, open = true) => {
    const s = m.get(k) || (open ? 'live' : 'off');
    fg.push(nsValve(x, y, s, s === 'fail' ? false : open, label));
    return s;
  };
  const feedDead = (n) => ['MAIN', 'STBY'].every((p) => m.get(`FEED${n}:${p}`) === 'fail') || m.get('FEED' + n) === 'fail';
  const xfeedUsed = [1, 2, 3, 4].some(feedDead);
  for (let n = 1; n <= 4; n++) {
    const cx = at.get('FEED' + n);
    const lpFail = m.get('LP' + n) === 'fail';
    const engSt = lpFail ? 'warn' : feedDead(n) ? 'alt' : 'live';
    wires.push(nsWire([[cx, TY], [cx, 160]], feedDead(n) ? 'warn' : lpFail ? 'fail' : 'live'), nsWire([[cx, 160], [cx, 74]], lpFail ? 'fail' : 'live'));
    fg.push(nsBox(cx - 42, 40, 84, 34, `ENG ${n}`, engSt === 'alt' ? 'live' : engSt, { tag: lpFail ? 'FUEL CUT' : feedDead(n) ? 'ON XFEED' : '' }));
    valve(cx, 112, 'LP' + n, 'LP');
    wires.push(nsWire([[cx, 160], [cx + 30, 160], [cx + 30, 176]], xfeedUsed && (feedDead(n) || !m.get('XF' + n)) ? 'live' : 'off', xfeedUsed ? '7 5' : ''));
    valve(cx + 30, 176, 'XF' + n, 'XFEED', xfeedUsed);
  }
  wires.push(nsWire([[at.get('FEED1') + 30, 186], [at.get('FEED4') + 30, 186]], xfeedUsed ? 'alt' : 'off', xfeedUsed ? '7 5' : '', { arrow: false }));
  fg.push(exT(500, 182, 'CROSSFEED MANIFOLD', { size: 10, fill: xfeedUsed ? NS.green : NS.greyText }));
  const GF = 372, GA = 404;
  for (const [y, k, label] of [[GF, 'GAL_FWD', 'FWD TRANSFER GALLERY'], [GA, 'GAL_AFT', 'AFT TRANSFER GALLERY']]) {
    const s = m.get(k) || 'live';
    wires.push(nsWire([[40, y], [960, y]], s, s === 'fail' ? '4 5' : ''));
    fg.push(exT(500, y - 5, label, { size: 9, fill: s === 'live' ? NS.greyText : nsStroke(s) }));
  }
  for (const [k, label, x, kind] of FUELV_TANKS) {
    const cx = x + TW / 2;
    const pumps = FUELV_PUMPS[kind];
    const pumpFails = pumps.filter((p) => m.get(`${k}:${p}`) === 'fail').length;
    const tankSt = m.get(k) || (pumpFails === pumps.length ? 'warn' : 'live');
    fg.push(nsBox(x, TY, TW, TH, `${label}
${FUELV_CAP_T[k]} T`, tankSt, { tag: m.get(k) === 'fail' ? 'LEAK' : m.get(k) ? 'FAULT' : pumpFails === pumps.length ? (kind === 'feed' ? 'NO PRESS' : 'UNUSABLE') : '' }));
    pumps.forEach((p, i) => {
      const px = pumps.length === 1 ? cx - 20 : x + i * 46;
      const s = m.get(`${k}:${p}`) || (m.get(k) === 'fail' ? 'warn' : 'live');
      wires.push(nsWire([[px + 21, TY + TH], [px + 21, 296]], s === 'fail' ? 'fail' : s));
      if (kind !== 'feed') wires.push(nsWire([[px + 21, 326], [px + 21, i ? GA : GF]], s === 'fail' ? 'fail' : s));
      fg.push(nsBox(px, 296, 42, 30, p, s, { size: 10, r: 7 }));
    });
    if (kind === 'feed') {
      for (const [dir, y, dx] of [['FWD', GF, -16], ['AFT', GA, 16]]) {
        const s = m.get(`${k}:IN_${dir}`) || 'live';
        wires.push(nsWire([[cx + dx, y], [cx + dx, 344]], s === 'fail' ? 'warn' : 'live'));
        valve(cx + dx, 352, `${k}:IN_${dir}`, '');
      }
    }
  }
  const trimSt = m.get('TRIM') || 'live';
  fg.push(nsBox(420, 500, 160, 52, 'TRIM TANK', trimSt, { tag: m.get('TRIM') === 'fail' ? 'LEAK' : '' }));
  for (const [k, x, label] of [['TRIM:L', 440, 'L'], ['TRIM:R', 520, 'R']]) {
    const s = m.get(k) || 'live';
    wires.push(nsWire([[x + 20, 500], [x + 20, 470]], s === 'fail' ? 'fail' : s));
    fg.push(nsBox(x, 440, 40, 30, label, s, { size: 10, r: 7 }));
  }
  for (const [k, y, x] of [['TRIMISO_FWD', GF, 470], ['TRIMISO_AFT', GA, 530]]) {
    const s = m.get(k) || 'live';
    wires.push(nsWire([[x, y], [x, 440]], s === 'fail' ? 'warn' : 'live'));
    valve(x, 424, k, '');
    fg.push(exT(k.endsWith('FWD') ? x - 14 : x + 14, 428, k.endsWith('FWD') ? 'ISO FWD' : 'ISO AFT', { size: 9, anchor: k.endsWith('FWD') ? 'end' : 'start', fill: NS.greyText }));
  }
  for (const [k, x, label] of [['JETT_L', 40, 'JETT L'], ['JETT_R', 960, 'JETT R']]) {
    wires.push(nsWire([[x, GF], [x, GA + 40]], m.get(k) ? 'warn' : 'off'));
    valve(x, GA + 30, k, label, false);
  }
  for (const [k, x, label] of [['REFUEL_L', 120, 'REFUEL L'], ['REFUEL_R', 880, 'REFUEL R']]) {
    wires.push(nsWire([[x, GA], [x, GA + 70]], m.get(k) ? 'warn' : 'off'));
    valve(x, GA + 54, k, label, false);
  }
  const apu = m.get('APU') || 'live';
  wires.push(nsWire([[at.get('FEED4') + 30, 186], [960, 186], [960, 520], [900, 520]], apu === 'fail' ? 'warn' : 'live'));
  fg.push(nsBox(820, 504, 80, 32, 'APU', apu === 'fail' ? 'warn' : 'live', { tag: apu === 'fail' ? 'NO FEED' : '' }));
  if (title) fg.push(exT(W - 16, H - 8, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  const xs = [];
  for (const k of m.keys()) {
    const base = k.split(':')[0];
    const tank = FUELV_TANKS.find((tk) => tk[0] === base);
    if (tank) xs.push([tank[2], 30, TW, 380]);
    else if (/^(LP|XF)(\d)/.test(k)) { const n = k.match(/(\d)/)[1]; xs.push([at.get('FEED' + n) - 60, 30, 150, 300]); }
    else if (/^TRIM/.test(k)) xs.push([400, 360, 200, 200]);
    else if (/^GAL/.test(k)) xs.push([30, 340, 940, 90]);
    else if (/JETT_L|REFUEL_L/.test(k)) xs.push([10, 340, 200, 160]);
    else if (/JETT_R|REFUEL_R/.test(k)) xs.push([790, 340, 200, 160]);
    else if (k === 'APU') xs.push([780, 160, 210, 390]);
  }
  for (let n = 1; n <= 4; n++) if (feedDead(n)) xs.push([at.get('FEED1') - 40, 20, at.get('FEED4') - at.get('FEED1') + 120, 300]);
  return nsSvg(W, H, [...wires, ...fg], nsBounds(xs));
}

function fuelDiagram(name, title) {
  const m = fuelTargets(name);
  return m ? fuelView(m, title) : null;
}
