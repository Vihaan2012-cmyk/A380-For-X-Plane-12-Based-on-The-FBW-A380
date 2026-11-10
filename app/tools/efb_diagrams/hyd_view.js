const HYD_USERS = {
  G: ['AILERONS', 'SPOILERS', 'L ELEVATOR', 'RUDDER', 'THS', 'SLATS/FLAPS', 'L/G', 'NORM BRAKES'],
  Y: ['AILERONS', 'SPOILERS', 'R ELEVATOR', 'RUDDER', 'THS', 'STEERING', 'CARGO DOORS', 'ALTN BRAKES'],
};
const HYD_LOW_PRIORITY = new Set(['L/G', 'STEERING', 'CARGO DOORS']);

function hydTargets(name) {
  const n = ' ' + name.toUpperCase().replace(/[_-]/g, ' ') + ' ';
  const t = { G: new Map(), Y: new Map() };
  let sys = /\bGREEN\b|\bG\b|\bGRN\b/.test(n) ? 'G' : /\bYELLOW\b|\bY\b|\bYLW\b/.test(n) ? 'Y' : null;
  const eng = n.match(/(?:EDP|ENG(?:INE)?(?: PUMP)?|PUMP)\s*(\d)\s*([AB])?/);
  if (!sys && eng) sys = +eng[1] <= 2 ? 'G' : 'Y';
  if (!sys) return null;
  const m = t[sys];
  const soft = /DEGRAD|DISPLACEMENT|OVERHEAT|OVHT|CLOG|CONTAMINATION|PRECHARGE|CRACKING|AIR (?:LEAK|INGESTION)|DRIFT|LO PR|LO LEVEL/.test(n);
  const put = (k, st) => m.set(k, st);
  if (eng && /EDP|ENG(?:INE)? PUMP|ENG \d [AB] PUMP|PUMP \d ?[AB]/.test(n)) put('EDP' + eng[1] + (eng[2] || 'A'), soft ? 'warn' : 'fail');
  else if (/ELEC(?:TRIC)? PUMP|LOCAL ELECTRIC|\bE PUMP/.test(n) || /(GREEN|YELLOW) [AB] ELEC/.test(n)) put('EP' + ((n.match(/\b([AB])\b/) || [0, 'A'])[1]), soft ? 'warn' : 'fail');
  else if (/RESERVOIR|RSVR/.test(n)) put('RSVR', soft && !/LEAK$|RESERVOIR LEAK |RSVR LEAK/.test(n) ? 'warn' : 'fail');
  else if (/ACCUMULATOR|ACCU/.test(n)) put('ACC', 'warn');
  else if (/PRIORITY/.test(n)) put('PRIO', 'fail');
  else if (/RELIEF/.test(n)) put('RELIEF', 'warn');
  else if (/FILTER/.test(n)) put('FILTER', 'warn');
  else if (/HX|HEAT EXCH/.test(n)) put('HX', 'warn');
  else if (/AIR INGESTION|SYS PRESS|SYSTEM PRESS|\bLO PR\b|OVERPRESS/.test(n)) put('MANIFOLD', /LO|INGESTION/.test(n) ? 'fail' : 'warn');
  else if (/LINE LEAK|LEAK/.test(n)) {
    const user = /BRAKE/.test(n) ? (sys === 'G' ? 'NORM BRAKES' : 'ALTN BRAKES') : /GEAR|L\/G/.test(n) ? 'L/G' : /STEER/.test(n) ? 'STEERING' : /CARGO/.test(n) ? 'CARGO DOORS' : /REVERS/.test(n) ? 'REVERSER' : null;
    if (user) put('U:' + user, 'fail');
    put('RSVR', 'warn');
  } else put('MANIFOLD', 'warn');
  return { sys, m };
}

function hydView(t, title) {
  const W = 1000, H = 860;
  const parts = [];
  const fg = [];
  for (const sys of ['G', 'Y']) {
    const m = t && t[sys] instanceof Map ? t[sys] : t && t.sys === sys ? t.m : new Map();
    const x0 = sys === 'G' ? 20 : 520;
    const cx = x0 + 230;
    const name = sys === 'G' ? 'GREEN' : 'YELLOW';
    const engs = sys === 'G' ? [1, 2] : [3, 4];
    const sysLost = m.get('RSVR') === 'fail' || m.get('MANIFOLD') === 'fail';
    const st = (k, fallback = 'live') => m.get(k) || (sysLost && !/^(RSVR|MANIFOLD)$/.test(k) && !/^(EDP|EP)/.test(k) ? 'warn' : fallback);
    fg.push(exT(x0 + 6, 20, name + ' SYSTEM', { anchor: 'start', size: 14, fill: sysLost ? NS.red : NS.green }));
    if (m.get('NOTE')) m.get('NOTE').split('|').forEach((line, i) => fg.push(exT(x0 + 470, 46 + i * 14, line, { anchor: 'end', size: 10, fill: NS.red })));
    fg.push(nsBox(cx - 90, 30, 180, 44, m.get('RSVR') === 'fail' ? 'RESERVOIR\nQTY LO' : 'RESERVOIR\n3.5 BAR', st('RSVR'), { size: 12, tag: m.get('RSVR') === 'fail' ? 'FAILED' : m.get('RSVR') ? 'DEGRADED' : '' }));
    const pumpY = 132, epY = 214;
    const pumps = [];
    engs.forEach((e, i) => ['A', 'B'].forEach((ab, j) => pumps.push({ k: 'EDP' + e + ab, label: `EDP ${e}${ab}`, x: x0 + 8 + (i * 2 + j) * 112, y: pumpY })));
    pumps.push({ k: 'EPA', label: 'ELEC A', x: x0 + 120, y: epY }, { k: 'EPB', label: 'ELEC B', x: x0 + 232, y: epY });
    if (sys === 'G') pumps.push({ k: 'AUX', label: 'AUX PUMP', x: x0 + 344, y: epY });
    const manY = 300;
    const supply = sysLost ? 'warn' : 'live';
    const header = 88;
    parts.push(nsWire([[cx, 74], [cx, header]], supply), nsWire([[x0 + 58, header], [x0 + 394, header]], supply));
    engs.forEach((e, i) => {
      const vx = x0 + 8 + i * 224 + 106;
      parts.push(nsWire([[vx, header], [vx, 118]], supply), nsWire([[vx - 56, 118], [vx + 56, 118]], supply));
      parts.push(nsWire([[vx - 56, 118], [vx - 56, pumpY]], supply), nsWire([[vx + 56, 118], [vx + 56, pumpY]], supply));
      const c = sysLost ? NS.orange : NS.green;
      fg.push(nsValve(vx, 103, sysLost ? 'warn' : 'live', m.get('FSOV' + e) !== 'shut', ''));
      fg.push(exT(vx + 14, 99, `ENG ${e}`, { size: 9, anchor: 'start', fill: NS.greyText }), exT(vx + 14, 110, 'FSOV', { size: 9, anchor: 'start', fill: NS.greyText }));
    });
    for (const p of pumps.filter((q) => !q.k.startsWith('EDP'))) {
    }
    const headerY = 258;
    const flow = sysLost ? 'warn' : 'live';
    for (const p of pumps) {
      const s = m.get(p.k) || 'live';
      const out = s === 'fail' ? 'fail' : flow;
      parts.push(nsWire([[p.x + 50, p.y + 34], [p.x + 50, p.y + 44]], out), nsWire([[p.x + 50, p.y + 44], [p.x + 50, headerY]], s === 'fail' ? 'fail' : flow, s === 'fail' ? '5 5' : ''));
      fg.push(`<path d="M${p.x + 44},${p.y + 38} L${p.x + 56},${p.y + 38} L${p.x + 50},${p.y + 47} Z" fill="${nsStroke(out)}"/><path d="M${p.x + 43},${p.y + 48} L${p.x + 57},${p.y + 48}" stroke="${nsStroke(out)}" stroke-width="2"/>`);
    }
    parts.push(nsWire([[x0 + 40, headerY], [x0 + 434, headerY]], flow));
    fg.push(exT(x0 + 440, headerY + 4, 'HP', { size: 9, anchor: 'start', fill: NS.greyText }));
    for (const p of pumps) {
      const s = m.get(p.k) || (sysLost ? 'warn' : 'live');
      fg.push(nsBox(p.x, p.y, 100, 34, p.label, s, { tag: m.get(p.k) === 'fail' ? 'FAILED' : m.get(p.k) ? 'DEGRADED' : '' }));
    }
    const comps = [['FILTER', 'FILTER'], ['HX', 'FUEL HX'], ['ACC', 'ACCUM'], ['RELIEF', 'RELIEF V'], ['PRIO', 'PRIORITY V']];
    comps.forEach(([k, label], i) => {
      const x = x0 + 8 + i * 90;
      const cs = st(k) === 'fail' ? 'fail' : flow;
      parts.push(nsWire([[x + 40, headerY], [x + 40, 268]], cs), nsWire([[x + 40, 296], [x + 40, manY + 8]], cs));
      fg.push(nsBox(x, 268, 80, 28, label, st(k), { size: 11, tag: m.get(k) === 'fail' ? 'FAILED' : m.get(k) ? 'DEGRADED' : '' }));
    });
    const manSt = m.get('MANIFOLD') || (sysLost ? 'warn' : 'live');
    parts.push(nsWire([[x0 + 8, manY + 8], [x0 + 452, manY + 8]], manSt === 'off' ? 'off' : manSt));
    fg.push(exT(x0 + 452, manY + 26, sysLost ? '0 PSI' : m.get('MANIFOLD') || m.get('RELIEF') ? 'LOW PRESS' : '5000 PSI', { anchor: 'end', size: 11, fill: nsStroke(manSt) }));
    const branches = sys === 'G'
      ? [['PRIMARY FLIGHT CONTROLS', 'FLIGHT', ['L OUT AIL', 'L IN AIL', 'R OUT AIL', 'R IN AIL', 'L IN ELEV', 'L OUT ELEV', 'L SPLR 2', 'L SPLR 4', 'L SPLR 6', 'L SPLR 8', 'R SPLR 2', 'R SPLR 4', 'R SPLR 6', 'R SPLR 8', 'UPPER RUD']],
        ['SECONDARY FLIGHT CONTROLS', 'SECONDARY', ['THS MOTOR', 'SLAT PCU', 'FLAP PCU']],
        ['LANDING GEAR · BEHIND PRIORITY VALVE', 'L/G', ['NLG ACT', 'L WLG ACT', 'R WLG ACT', 'L BLG ACT', 'R BLG ACT', 'GEAR DOORS']],
        ['NORMAL BRAKES', 'NORM BRAKES', ['L WLG BRK', 'R WLG BRK', 'L BLG BRK', 'R BLG BRK']]]
      : [['PRIMARY FLIGHT CONTROLS', 'FLIGHT', ['L OUT AIL', 'L MID AIL', 'R OUT AIL', 'R MID AIL', 'R IN ELEV', 'R OUT ELEV', 'L SPLR 1', 'L SPLR 3', 'L SPLR 5', 'L SPLR 7', 'R SPLR 1', 'R SPLR 3', 'R SPLR 5', 'R SPLR 7', 'LOWER RUD']],
        ['SECONDARY FLIGHT CONTROLS', 'SECONDARY', ['THS MOTOR', 'FLAP PCU']],
        ['STEERING AND CARGO DOORS · BEHIND PRIORITY VALVE', 'STEERING', ['NOSE STEER', 'L BODY STEER', 'R BODY STEER', 'FWD CARGO DR', 'AFT CARGO DR']],
        ['ALTERNATE BRAKES', 'ALTN BRAKES', ['BRK ACCUM', 'L WLG BRK', 'R WLG BRK', 'L BLG BRK', 'R BLG BRK']]];
    let by = manY + 56;
    const trunk = x0 + 16;
    const trunkSt = sysLost ? 'warn' : 'live';
    let trunkEnd = by;
    branches.forEach(([label, key, items]) => {
      const prio = /PRIORITY/.test(label);
      const leak = m.get('U:' + key) || (key === 'STEERING' && (m.get('U:STEERING') || m.get('U:CARGO DOORS')));
      const bs = leak ? 'fail' : sysLost || (prio && m.get('PRIO') === 'fail') ? 'warn' : 'live';
      const st = leak ? 'warn' : bs;
      const rows = Math.ceil(items.length / 6);
      fg.push(exT(x0 + 44, by + 8, label, { size: 9, anchor: 'start', fill: bs === 'live' ? NS.greyText : nsStroke(bs) }));
      if (leak) fg.push(exT(x0 + 470, by + 8, leak === 'fail' ? 'LINE LEAK' : leak, { size: 9, anchor: 'end', fill: NS.red }));
      for (let r = 0; r < rows; r++) {
        const row = items.slice(r * 6, r * 6 + 6);
        const y = by + 26 + r * 46;
        const busY = y - 10;
        const lastX = x0 + 44 + (row.length - 1) * 70 + 32;
        parts.push(nsWire([[trunk, busY], [lastX, busY]], st));
        fg.push(`<circle cx="${trunk}" cy="${busY}" r="4" fill="${nsStroke(trunkSt)}"/>`);
        if (prio) fg.push(nsValve(trunk + 16, busY, m.get('PRIO') === 'fail' ? 'fail' : 'live', m.get('PRIO') !== 'fail', ''));
        row.forEach((u, j) => {
          const x = x0 + 44 + j * 70;
          const torn = m.get('A:' + u);
          parts.push(nsWire([[x + 32, busY], [x + 32, y]], torn ? 'fail' : st, torn ? '4 4' : ''));
          fg.push(nsBox(x, y, 64, 26, u, torn ? 'fail' : st, { size: 8, r: 6 }));
        });
        trunkEnd = busY;
      }
      by += 26 + (rows - 1) * 46 + 52;
    });
    parts.push(nsWire([[trunk, manY + 8], [trunk, trunkEnd]], trunkSt));
  }
  parts.push(`<path d="M500,20 L500,${H - 20}" stroke="${NS.grey}" stroke-width="1" stroke-dasharray="4 6"/>`);
  parts.push(...fg);
  if (title) parts.push(exT(W - 16, H - 10, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  return nsSvg(W, H, parts, t && t.m && t.m.size ? [t.sys === 'G' ? 6 : 506, 6, 488, H - 50, 0] : null);
}

function hydDiagram(name, title) {
  const t = hydTargets(name);
  return t ? hydView(t, title) : null;
}

function hydSeparationLeaks(list, firePb = []) {
  const t = { G: new Map(), Y: new Map() };
  const side = (i) => (i ? 'R' : 'L');
  const tear = (sys, label) => {
    t[sys].set('U:FLIGHT', 'LINES TORN');
    t[sys].set('RSVR', 'warn');
    if (label) t[sys].set('A:' + label, 'fail');
  };
  const ail = ['IN', 'MID', 'OUT'];
  for (const f of list) {
    const id = f.id;
    if (id >= 54000 && id < 54004) {
      const n = id - 53999, sys = n <= 2 ? 'G' : 'Y';
      t[sys].set('EDP' + n + 'A', 'fail').set('EDP' + n + 'B', 'fail');
      if (firePb[n - 1]) t[sys].set('FSOV' + n, 'shut').set('NOTE', `ENG ${n} SEPARATED|FIRE P/B OUT · LINES SHUT`);
      else t[sys].set('RSVR', 'warn').set('NOTE', `ENG ${n} SEPARATED|LINES OPEN TILL FIRE P/B`);
    } else if (id >= 57300 && id < 57306) {
      const s = side(Math.floor((id - 57300) / 3)), k = (id - 57300) % 3;
      tear('G', `${s} ${ail[k]} AIL`), tear('Y', `${s} ${ail[k]} AIL`);
    } else if (id >= 57330 && id < 57332) {
      const s = side(id - 57330);
      for (const k of [1, 2]) tear('G', `${s} ${ail[k]} AIL`), tear('Y', `${s} ${ail[k]} AIL`);
    } else if (id >= 55310 && id < 55314) {
      const i = Math.floor((id - 55310) / 2);
      tear(i ? 'Y' : 'G', `${side(i)} ${(id - 55310) % 2 ? 'OUT' : 'IN'} ELEV`);
    } else if (id >= 55320 && id < 55322) {
      const i = id - 55320;
      for (const p of ['IN', 'OUT']) tear(i ? 'Y' : 'G', `${side(i)} ${p} ELEV`);
    } else if (id >= 57310 && id < 57326) {
      const k = (id - 57310) % 8 + 1;
      tear(k % 2 ? 'Y' : 'G', `${side(Math.floor((id - 57310) / 8))} SPLR ${k}`);
    } else if (id === 55330 || id === 55300 || id === 55301) {
      const both = id === 55330;
      if (both || id === 55300) tear('G', 'UPPER RUD'), tear('Y', null);
      if (both || id === 55301) tear('Y', 'LOWER RUD'), tear('G', null);
    }
  }
  return t.G.size || t.Y.size ? t : null;
}

