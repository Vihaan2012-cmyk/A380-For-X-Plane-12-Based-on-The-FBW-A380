const SB_POS = ['PLANE LATITUDE', 'PLANE LONGITUDE'];

function sbDistanceNm(lat1, lon1, lat2, lon2) {
  const r = Math.PI / 180;
  const a = Math.sin((lat2 - lat1) * r / 2) ** 2 + Math.cos(lat1 * r) * Math.cos(lat2 * r) * Math.sin((lon2 - lon1) * r / 2) ** 2;
  return 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a)) * 3440.065;
}

function sbProgress(org, dst, lat, lon) {
  const flown = sbDistanceNm(org.lat, org.lon, lat, lon);
  const togo = sbDistanceNm(lat, lon, dst.lat, dst.lon);
  return { frac: flown + togo > 0 ? flown / (flown + togo) : 0, togo };
}

function sbBar() {
  const st = document.querySelector('.efb-topbar .st');
  if (!st) return null;
  let bar = st.querySelector('.sb-prog');
  if (!bar) {
    bar = el('div', { class: 'sb-prog' }, el('span', { class: 'sb-org' }), el('div', { class: 'sb-track' }, el('i'), el('b')), el('span', { class: 'sb-dst' }));
    st.prepend(bar);
  }
  return bar;
}

async function sbUpdate() {
  const bar = sbBar();
  if (!bar) return;
  const o = (typeof lastOfp === 'object' && lastOfp) || null;
  const org = o && o.origin, dst = o && o.destination;
  const on = settings.EFB_SHOW_STATUSBAR_FLIGHTPROGRESS !== '0' && org && dst && org.pos_lat !== undefined && dst.pos_lat !== undefined;
  bar.hidden = !on;
  if (!on) return;
  bar.querySelector('.sb-org').textContent = org.icao_code || '';
  bar.querySelector('.sb-dst').textContent = dst.icao_code || '';
  if (FIXTURE) return;
  let v;
  try { v = (await studyGet('/vars?names=' + SB_POS.map(encodeURIComponent).join(','))).vars || {}; } catch { return; }
  if (v['PLANE LATITUDE'] === undefined) return;
  const p = sbProgress({ lat: Number(org.pos_lat), lon: Number(org.pos_long) }, { lat: Number(dst.pos_lat), lon: Number(dst.pos_long) }, v['PLANE LATITUDE'], v['PLANE LONGITUDE']);
  const pct = Math.round(Math.max(0, Math.min(1, p.frac)) * 1000) / 10;
  bar.querySelector('.sb-track i').style.width = pct + '%';
  bar.querySelector('.sb-track b').style.left = pct + '%';
  bar.title = `${Math.round(p.togo)} nm to go (great circle)`;
}

setInterval(sbUpdate, 5000);
setTimeout(sbUpdate, 1500);

(function () {
  const style = document.createElement('style');
  style.textContent = `
.sb-prog[hidden],.efb-clock [hidden]{display:none !important;}
.sb-prog{display:flex;align-items:center;gap:8px;font:600 12px 'Cascadia Mono',monospace;color:#C4CBD4;}
.sb-track{position:relative;width:150px;height:4px;border-radius:2px;background:rgba(255,255,255,0.14);}
.sb-track i{position:absolute;left:0;top:0;bottom:0;border-radius:2px;background:linear-gradient(90deg,#f0703a,#ff3f7a);}
.sb-track b{position:absolute;top:50%;width:9px;height:9px;margin:-4.5px 0 0 -4.5px;border-radius:50%;background:#fff;}
`;
  document.head.append(style);
})();
