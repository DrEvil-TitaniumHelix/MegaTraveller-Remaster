// Read-only session 01 replay. Dump every completed T redraw, without changing the oracle.
// Usage: node tools/m4c0_viewport_frames.mjs LAB KEYS OUTPUT
import { pathToFileURL } from 'node:url';
import { resolve, join } from 'node:path';
import { mkdirSync, writeFileSync } from 'node:fs';
const [lab, keys, output] = process.argv.slice(2);
const { MT1, loadKeys } = await import(pathToFileURL(resolve(lab, 'mt.ts')).href);
const g = new MT1(false), cpu = g.m.cpu, previous = cpu.onStep;
const frames = []; let pending = null, step = 0;
mkdirSync(output, {recursive:true});
cpu.onStep = () => {
    const prog = g.prog;
    if (prog?.name === 'T') {
        const address = (cpu.s[1] << 4) + cpu.ip;
        if (pending && address === pending.returnAddress && cpu.r[4] === pending.sp + 2) {
            if (frames.length >= 512) throw Error('frame budget exceeded');
            const ds = g.ds(), d = ds << 4, id = String(frames.length).padStart(4, '0');
            const folder = join(output, id); mkdirSync(folder, {recursive:true});
            writeFileSync(join(folder, 'memory.bin'), g.m.mem.slice(0, 0xa0000));
            const page = g.m.rw(d + 0x52f4) & 1, pixels = Buffer.alloc(192 * 112);
            for (let y=0;y<112;y++) for (let x=0;x<192;x++) {
                const at = page * 8192 + (y + 54) * 40 + (x >> 3), mask = 0x80 >> (x & 7);
                for (let p=0;p<4;p++) if (g.m.ega.planes[p][at] & mask) pixels[y * 192 + x] |= 1 << p;
            }
            writeFileSync(join(folder, 'map.bin'), pixels);
            // Cursor exclusion = the original cursor's 16x16 area, when its saved-under state is active.
            const meta = { id, ds, step, page, phase: pending.phase, moving: g.m.rw(d + 0xe3) !== 0,
                cursor: [g.m.rw(d + 0x700c), g.m.rw(d + 0x700e)], cursorVisible: g.m.rb(d + 0x7011) !== 0 };
            writeFileSync(join(folder, 'meta.json'), JSON.stringify(meta)); frames.push(meta); pending = null;
        }
        if (address - prog.base === 0x1a63 && !pending) {
            const sp = cpu.r[4], stack = (cpu.s[2] << 4) + sp;
            pending = { returnAddress: (cpu.s[1] << 4) + g.m.rw(stack), sp, phase: g.m.rw((g.ds()<<4) + 0x641a) };
        }
    }
    if (previous) previous();
};
const script = loadKeys(keys);
if (!script.length || script.length > 65536 || script.reduce((n,s)=>n+s.wait,0) > 200000) throw Error('script budget');
for (const s of script) { g.step(s); step++; }
writeFileSync(join(output, 'manifest.json'), JSON.stringify(frames, null, 2));
if (!frames.length) throw Error('no completed map frames');
console.log(`session 01: ${frames.length} map frames, ${step} steps`);
