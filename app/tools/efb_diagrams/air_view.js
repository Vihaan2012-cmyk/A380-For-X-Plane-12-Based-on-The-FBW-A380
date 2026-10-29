function airTargets(name) {
  const n = ' ' + name.toUpperCase().replace(/[_-]/g, ' ').replace(/FOWARD/g, 'FORWARD') + ' ';
  const m = new Map();
  const soft = /DEGRAD|FOULING|DRIFT|SENSOR|INSULATION|BLOCKAGE|CLOG|FAULT$/.test(n.trim()) && !/STUCK|LEAK|RUPTURE|FAILURE/.test(n);
  const put = (k) => m.set(k, soft ? 'warn' : 'fail');
  const eng = (n.match(/ENG(?:INE)? ?(\d)/) || [])[1];
  const side = /\bLEFT\b|\bL\b/.test(n) ? 'L' : /\bRIGHT\b|\bR\b/.test(n) ? 'R' : /CENT(?:RE|ER)/.test(n) ? 'C' : null;
  const pack = (n.match(/PACK ?(\d)/) || [])[1];
  let k;
  if (eng && /START DUCT/.test(n)) put('START' + eng);
  else if (eng && /DUCT|RUPTURE|INSULATION/.test(n) && /BLEED|PYLON|ENGINE/.test(n)) put('DUCT' + eng);
  else if (eng && /HP (?:BLEED )?VALVE|HP VALVE/.test(n)) put('HP' + eng);
  else if (eng && /IP8|IP CHECK/.test(n)) put('IPC' + eng);
  else if (eng && /FAN AIR VALVE|\bFAV\b/.test(n)) put('FAV' + eng);
  else if (eng && /PRECOOLER/.test(n)) put('PCLR' + eng);
  else if (eng && /NACELLE|ANTI ICE/.test(n)) put('NAI' + eng);
  else if (eng && /PR |SHUTOFF|BLEED VALVE|PRV|BLEED/.test(n)) put('PRV' + eng);
  else if (/CROSS ?BLEED|XBLEED|CROSS\b/.test(n)) put('XB' + (side || 'C'));
  else if (/APU BLEED|APU .*BLEED/.test(n)) put('APUB');
  else if (/WING/.test(n) && /ANTI ICE|A\/I/.test(n)) put('WAI' + (side || 'L'));
  else if ((k = n.match(/(?:PACK )?FCV ?(\d) ?(\d)|PACK FCV (\d) (\d)/))) put('FCV' + (k[1] || k[3]) + (k[2] || k[4]));
  else if (pack && /RAM AIR/.test(n)) put('RAM' + pack);
  else if (pack) put('PACK' + pack);
  else if ((k = n.match(/HOT AIR VALVE ?(\d)/))) put('HAV' + k[1]);
  else if (/HOT AIR/.test(n)) put('HAV1');
  else if (/ODLS|OVERHEAT DETECTION/.test(n)) put('ODLS' + ((n.match(/LOOP ([AB])/) || [0, 'A'])[1]));
  else if ((k = n.match(/CAB(?:IN)? FAN ?(\d)/))) put('CF' + k[1]);
  else if (/OUTFLOW|\bOFV\b/.test(n)) put('OFV' + ((n.match(/(\d)/) || [0, '1'])[1]));
  else if (/\bCPC\b|PRESSURE CONTROLLER|CABIN .*PRESS|DECOMPRESSION|OCSM/.test(n)) put('CPC');
  else if (/AVIONICS/.test(n) && /FAN|EXTRACT/.test(n)) put('AV' + (/AFT/.test(n) ? 'AFT' : /MAIN|UPPER/.test(n) ? 'MAIN' : 'FWD') + (/STANDBY/.test(n) ? 'S' : 'P'));
  else if (/EXTRACT VALVE|INLET VALVE|SKIN/.test(n)) put('AVVALVE');
  else if (/BULK/.test(n) && /HEATER/.test(n)) put('BULKHEAT');
  else if (/CARGO/.test(n) && /ISOLATION/.test(n)) put(/BULK|AFT/.test(n) ? 'CISO_AFT' : 'CISO_FWD');
  else if (/CARGO/.test(n) && /FAN|EXTRACT/.test(n)) put(/BULK|AFT/.test(n) ? 'CFAN_AFT' : 'CFAN_FWD');
  else if (/VCM|TADD|FDAC|TRIM AIR|ZONE|TEMP/.test(n)) put('TRIM');
  else if (/MIXER/.test(n)) put('MIXER');
  else return null;
  return m;
}

function bleedView(m, title) {
  const W = 1000, H = 640;
  const wires = [], fg = [];
  const s = (k, d = 'live') => m.get(k) || d;
  const engX = [110, 310, 660, 870];
  const manY = 330;
  const xbOpen = ['XBL', 'XBC', 'XBR'].some((k) => m.get(k) === 'fail') || [1, 2, 3, 4].some((e) => m.get('PRV' + e) === 'fail');
  engX.forEach((x, i) => {
    const e = i + 1;
    const dead = ['PRV', 'DUCT', 'PCLR'].some((p) => m.get(p + e) === 'fail');
    const flow = dead ? 'warn' : 'live';
    fg.push(nsBox(x - 50, 22, 100, 34, `ENG ${e}`, 'live'));
    wires.push(nsWire([[x - 22, 56], [x - 22, 92]], s('IPC' + e) === 'fail' ? 'fail' : 'live'), nsWire([[x + 22, 56], [x + 22, 92]], s('HP' + e) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x - 22, 78, s('IPC' + e), true, 'IP8', { left: true }), nsValve(x + 22, 78, s('HP' + e), m.get('HP' + e) !== 'fail', 'HP6'));
    wires.push(nsWire([[x - 22, 92], [x + 22, 92]], 'live'), nsWire([[x, 92], [x, 172]], m.get('PRV' + e) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x, 126, s('PRV' + e), m.get('PRV' + e) !== 'fail', 'PRV'));
    fg.push(nsBox(x - 46, 172, 92, 38, 'PRECOOLER', s('PCLR' + e), { size: 11, tag: m.get('PCLR' + e) ? (m.get('PCLR' + e) === 'fail' ? 'FAILED' : 'DEGRADED') : '' }));
    wires.push(nsWire([[x + 46, 191], [x + 70, 191], [x + 70, 140]], s('FAV' + e) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x + 70, 152, s('FAV' + e), m.get('FAV' + e) !== 'fail', 'FAV'));
    wires.push(nsWire([[x - 50, 39], [x - 70, 39], [x - 70, 110]], s('NAI' + e) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x - 70, 96, s('NAI' + e), false, 'NAI', { left: true }));
    const duct = s('DUCT' + e, flow);
    wires.push(nsWire([[x, 210], [x, manY]], duct === 'fail' ? 'fail' : flow, duct === 'fail' ? '5 5' : ''));
    if (m.get('DUCT' + e)) fg.push(exT(x + 8, 270, 'DUCT LEAK', { size: 10, anchor: 'start', fill: NS.red }));
    wires.push(nsWire([[x, 250], [x + 34, 250], [x + 34, 290]], s('START' + e) === 'fail' ? 'fail' : 'off'));
    fg.push(nsValve(x + 34, 276, s('START' + e, 'off'), false, 'START'));
  });
  wires.push(nsWire([[30, manY], [970, manY]], 'live'));
  fg.push(exT(500, manY - 8, 'CROSSBLEED MANIFOLD', { size: 10, fill: NS.greyText }));
  for (const [k, x, label] of [['XBL', 195, 'XBLEED L'], ['XBC', 500, 'XBLEED C'], ['XBR', 765, 'XBLEED R']]) fg.push(nsValve(x, manY, s(k, xbOpen ? 'alt' : 'off'), xbOpen && m.get(k) !== 'fail', label));
  for (const [k, x, label] of [['WAIL', 30, 'WING A/I L'], ['WAIR', 970, 'WING A/I R']]) {
    wires.push(nsWire([[x, manY], [x, manY + 70]], s(k) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x, manY + 36, s(k, 'off'), false, ''), nsBox(x === 30 ? 10 : 910, manY + 70, 80, 30, label, s(k), { size: 10 }));
  }
  wires.push(nsWire([[560, manY], [560, 460]], s('APUB') === 'fail' ? 'fail' : 'off'));
  fg.push(nsValve(560, 400, s('APUB', 'off'), false, 'APU BLEED'), nsBox(520, 460, 80, 32, 'APU', 'off'));
  wires.push(nsWire([[440, manY], [440, 460]], 'off'));
  fg.push(nsValve(440, 400, 'off', false, 'GND HP', { left: true }), nsBox(400, 460, 80, 32, 'GND CART', 'off', { size: 10 }));
  for (const [p, x] of [[1, 230], [2, 730]]) {
    const packSt = s('PACK' + p);
    for (const [j, dx] of [[1, -30], [2, 30]]) {
      const k = `FCV${p}${j}`;
      wires.push(nsWire([[x + dx, manY], [x + dx, 470]], s(k) === 'fail' ? 'fail' : 'live'));
      fg.push(nsValve(x + dx, 400, s(k), m.get(k) !== 'fail', `FCV ${p}-${j}`, { left: dx < 0 }));
    }
    fg.push(nsBox(x - 70, 470, 140, 46, `PACK ${p}\nACM`, packSt, { size: 12, tag: m.get('PACK' + p) ? 'FAULT' : '' }));
    wires.push(nsWire([[x + 70, 493], [x + 96, 493]], s('RAM' + p) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x + 96, 493, s('RAM' + p), true, 'RAM AIR'));
    wires.push(nsWire([[x, 516], [x, 560], [500, 560]], packSt === 'fail' ? 'warn' : 'live'));
  }
  fg.push(nsBox(440, 560, 120, 40, 'MIXER UNIT', s('MIXER'), { size: 12 }));
  for (const [k, x] of [['HAV1', 350], ['HAV2', 650]]) {
    wires.push(nsWire([[x, manY], [x, 430]], s(k) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x, 380, s(k), m.get(k) !== 'fail', k === 'HAV1' ? 'HOT AIR 1' : 'HOT AIR 2'), exT(x, 446, 'TRIM AIR', { size: 9, fill: NS.greyText }));
  }
  for (const [k, y] of [['ODLSA', manY + 12], ['ODLSB', manY + 18]]) wires.push(nsWire([[40, y], [960, y]], m.get(k) ? 'warn' : 'off', '2 6'));
  if (m.get('ODLSA') || m.get('ODLSB')) fg.push(exT(960, manY + 34, 'OVERHEAT DETECTION LOOP', { size: 9, anchor: 'end', fill: NS.orange }));
  if (title) fg.push(exT(W - 16, H - 8, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  const focus = [];
  for (const key of m.keys()) {
    const e = (key.match(/(\d)$/) || [])[1];
    if (/^(IPC|HP|PRV|FAV|PCLR|NAI|DUCT|START)\d$/.test(key)) focus.push([engX[e - 1] - 90, 10, 200, 330]);
    else if (/^XB/.test(key)) focus.push([150, 280, 700, 120]);
    else if (/^WAI/.test(key)) focus.push([key.endsWith('L') ? 0 : 800, 280, 200, 150]);
    else if (/^(FCV|PACK|RAM)/.test(key)) focus.push([key.match(/\d/)[0] === '1' ? 120 : 620, 300, 260, 230]);
    else focus.push([300, 300, 400, 300]);
  }
  return nsSvg(W, H, [...wires, ...fg], nsBounds(focus));
}

function ventView(m, title) {
  const W = 1000, H = 600;
  const wires = [], fg = [];
  const s = (k, d = 'live') => m.get(k) || d;
  fg.push(nsBox(440, 30, 120, 40, 'MIXER UNIT', s('MIXER'), { size: 12 }));
  wires.push(nsWire([[500, 70], [500, 110]], 'live'), nsWire([[120, 110], [880, 110]], 'live'));
  const zones = [['COCKPIT', 60], ['MAIN FWD', 190], ['MAIN MID', 320], ['MAIN AFT', 450], ['UPPER FWD', 580], ['UPPER MID', 710], ['UPPER AFT', 840]];
  const trim = s('TRIM');
  zones.forEach(([z, x]) => {
    wires.push(nsWire([[x + 50, 110], [x + 50, 160]], 'live'));
    fg.push(nsValve(x + 50, 136, trim === 'fail' ? 'warn' : 'live', true, ''), nsBox(x, 160, 100, 36, z, trim === 'fail' ? 'warn' : 'live', { size: 10 }));
  });
  fg.push(exT(500, 104, 'TRIM AIR', { size: 10, fill: NS.greyText }));
  for (let f = 1; f <= 4; f++) {
    const x = 160 + (f - 1) * 200;
    wires.push(nsWire([[x + 40, 230], [x + 40, 110]], s('CF' + f) === 'fail' ? 'fail' : 'live', s('CF' + f) === 'fail' ? '5 5' : ''));
    fg.push(nsBox(x, 230, 80, 30, `CAB FAN ${f}`, s('CF' + f), { size: 10, tag: m.get('CF' + f) ? 'FAILED' : '' }));
  }
  fg.push(exT(500, 278, 'RECIRCULATION', { size: 10, fill: NS.greyText }));
  wires.push(nsWire([[60, 320], [940, 320]], m.get('CPC') ? 'warn' : 'live'));
  for (let o = 1; o <= 4; o++) {
    const x = 120 + (o - 1) * 240;
    wires.push(nsWire([[x, 320], [x, 360]], s('OFV' + o) === 'fail' ? 'fail' : 'live'));
    fg.push(nsValve(x, 340, s('OFV' + o), true, `OFV ${o}`));
  }
  wires.push(nsWire([[880, 320], [880, 376]], m.get('CPC') ? 'warn' : 'live'));
  fg.push(nsBox(820, 376, 120, 30, 'CPC 1 / 2', s('CPC'), { size: 11, tag: m.get('CPC') ? 'FAULT' : '' }), exT(60, 312, 'CABIN PRESSURE', { size: 10, anchor: 'start', fill: NS.greyText }));
  fg.push(exT(40, 410, 'AVIONICS VENTILATION', { size: 11, anchor: 'start', fill: NS.greyText }));
  [['FWD', 60], ['MAIN', 300], ['AFT', 540]].forEach(([bay, x]) => {
    fg.push(nsBox(x, 430, 100, 36, `${bay} BAY`, ['P', 'S'].every((k) => m.get(`AV${bay}${k}`) === 'fail') ? 'warn' : 'live', { size: 11 }));
    ['P', 'S'].forEach((ps, j) => {
      const k = `AV${bay}${ps}`;
      wires.push(nsWire([[x + 100, 448], [x + 120 + j * 64, 448], [x + 120 + j * 64, 480]], s(k) === 'fail' ? 'fail' : ps === 'P' ? 'live' : 'off'));
      fg.push(nsBox(x + 104 + j * 64, 480, 60, 28, ps === 'P' ? 'PRIM' : 'STBY', s(k, ps === 'P' ? 'live' : 'off'), { size: 9, r: 7 }));
    });
  });
  fg.push(nsValve(820, 448, s('AVVALVE'), true, 'EXTRACT V'));
  fg.push(exT(40, 568, 'CARGO', { size: 11, anchor: 'start', fill: NS.greyText }));
  [['FWD', 120, 'CISO_FWD', 'CFAN_FWD'], ['BULK', 420, 'CISO_AFT', 'CFAN_AFT']].forEach(([c, x, iso, fan]) => {
    fg.push(nsValve(x, 562, s(iso), m.get(iso) !== 'fail', `${c} ISOL V`), nsBox(x + 90, 548, 80, 28, `${c} FAN`, s(fan), { size: 9, r: 7 }));
  });
  fg.push(nsBox(720, 548, 110, 28, 'BULK HEATER', s('BULKHEAT'), { size: 9, r: 7 }));
  if (title) fg.push(exT(W - 16, H - 8, title, { anchor: 'end', size: 12, fill: NS.greyText, weight: 500 }));
  return nsSvg(W, H, [...wires, ...fg]);
}

function airDiagram(name, title) {
  const m = airTargets(name);
  if (!m) return null;
  const vent = [...m.keys()].some((k) => /^(CF|OFV|CPC|AV|CISO|CFAN|BULKHEAT|TRIM|MIXER)/.test(k));
  return vent ? ventView(m, title) : bleedView(m, title);
}
