// vars_watch.mjs <out dir> [port]: once a minute, 20 reads of /vars over 5 s; what moved and by how much, to find noise the dead band misses.
import { mkdirSync, writeFileSync } from 'node:fs';
const [dir, port = 8390] = process.argv.slice(2);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const read = async () => {
  const reply = await fetch(`http://127.0.0.1:${port}/vars`, { signal: AbortSignal.timeout(3000) });
  return (await reply.json()).vars;
};
const words = new Uint32Array(1);
const floats = new Float32Array(words.buffer);
const shown = (v) => {
  if (Math.abs(v) < 2 ** 32 || !Number.isInteger(v)) return v;
  words[0] = v % 2 ** 32;
  return floats[0];
};
mkdirSync(dir, { recursive: true });
let missed = 0;
while (missed < 10) {
  await sleep(55000);
  try {
    const seen = new Map();
    for (let i = 0; i < 20; i++) {
      for (const [name, value] of Object.entries(await read())) {
        const s = seen.get(name) ?? { lo: Infinity, hi: -Infinity, moves: 0, last: value };
        const v = shown(value);
        s.lo = Math.min(s.lo, v);
        s.hi = Math.max(s.hi, v);
        if (value !== s.last) s.moves += 1;
        s.last = value;
        seen.set(name, s);
      }
      await sleep(250);
    }
    const rows = [...seen.entries()].filter(([, s]) => s.moves > 0).sort((a, b) => b[1].moves - a[1].moves);
    writeFileSync(`${dir}/${Date.now()}.txt`, rows.map(([n, s]) => `${s.moves}\t${(s.hi - s.lo).toPrecision(3)}\t${n}`).join('\n'));
    missed = 0;
  } catch {
    missed += 1;
  }
}
