import fs from 'node:fs';
import path from 'node:path';

const PORT = Number(process.env.XPHFBW_DEVTOOLS_PORT || 9333);
const OUT = process.argv[2] || 'E:/golden-tour/rasterlab';
const MATCH = new RegExp(process.argv[3] || '/PFD/.*Index=1');
fs.mkdirSync(OUT, { recursive: true });

const shapes = [];
for (let i = 0; i < 6; i++) shapes.push(`<circle cx="${70 + i * 115}" cy="80" r="${20 + i * 7.3}" fill="none" stroke="#fff" stroke-width="${[1, 1.5, 2, 2.5, 3, 4][i]}"/>`);
for (let i = 0; i < 12; i++) {
  const a = (i * 15 * Math.PI) / 180;
  const x = 60 + i * 58;
  shapes.push(`<line x1="${x}" y1="220" x2="${(x + Math.cos(a) * 45).toFixed(3)}" y2="${(220 - Math.sin(a) * 45).toFixed(3)}" stroke="#0f0" stroke-width="${i % 2 ? 2 : 1.5}"/>`);
}
for (let i = 0; i < 8; i++) shapes.push(`<rect x="${40 + i * 88 + i * 0.125}" y="${300 + (i % 4) * 0.25}" width="60.5" height="30.25" fill="none" stroke="#0ff" stroke-width="${[1, 2, 3, 1.5][i % 4]}"/>`);
for (let i = 0; i < 8; i++) shapes.push(`<rect x="${40 + i * 88 + i * 0.125}" y="${360 + (i % 4) * 0.25}" width="60.5" height="30.25" fill="#ffa500"/>`);
for (let i = 0; i < 6; i++) shapes.push(`<path d="M ${50 + i * 115} 470 A 40 40 0 0 1 ${130 + i * 115} 470" fill="none" stroke="#fff" stroke-width="${[1, 2, 3, 4, 2.5, 1.5][i]}" stroke-linecap="${['butt', 'round', 'square'][i % 3]}"/>`);
for (let i = 0; i < 4; i++) shapes.push(`<line x1="40" y1="${540 + i * 30}" x2="720" y2="${540 + i * 30}" stroke="#fff" stroke-width="2" stroke-dasharray="${['10 6', '4 4', '20 10 4 10', '1 3'][i]}"/>`);
shapes.push(`<polygon points="100,700 160,760 60,780" fill="#f0f"/><polygon points="300,700 380,705 330,790" fill="none" stroke="#fff" stroke-width="2" stroke-linejoin="miter"/>`);
shapes.push(`<text x="420" y="720" fill="#fff" font-family="Ecam" font-size="22">ABC 123 g</text><text x="420.5" y="760.25" fill="#0f0" font-family="Ecam" font-size="22" text-anchor="start">ABC 123 g</text>`);
const page = `<!doctype html><html><head><style>html,body{margin:0;background:#000;overflow:hidden}</style></head><body><svg width="768" height="1024" viewBox="0 0 768 1024" xmlns="http://www.w3.org/2000/svg">${shapes.join('')}</svg></body></html>`;
fs.writeFileSync(path.join(OUT, 'lab.svg.html'), page);

const targets = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
const t = targets.find((x) => x.type === 'page' && MATCH.test(x.url));
if (!t) throw new Error('no view matches ' + MATCH);
const ws = new WebSocket(t.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener('open', r, { once: true }));
let id = 0;
const pending = new Map();
ws.addEventListener('message', (e) => {
  const m = JSON.parse(e.data);
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
});
const call = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (expr) => (await call('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true })).result?.result?.value;
const dpr = await ev('window.devicePixelRatio');
await ev(`(() => { window.__TOUR && window.__TOUR.freeze && window.__TOUR.freeze(); document.open(); document.write(${JSON.stringify(page)}); document.close(); return 1; })()`);
await new Promise((r) => setTimeout(r, 1500));
const shot = await call('Page.captureScreenshot', { format: 'png', fromSurface: true });
fs.writeFileSync(path.join(OUT, 'lab.png'), Buffer.from(shot.result.data, 'base64'));
fs.writeFileSync(path.join(OUT, 'lab.json'), JSON.stringify({ view: t.url, dpr, css: [768, 1024] }));
console.log('rasterlab captured', path.join(OUT, 'lab.png'), 'dpr', dpr);
process.exit(0);
