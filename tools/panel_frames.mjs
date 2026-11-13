// Replay read-only lockstep scripts and retain the original EGA frames for the Classic composition proof.
// node tools/panel_frames.mjs LAB REFERENCES OUTPUT
import {readFileSync, readdirSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve, join, basename} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const [labArg, refsArg, outArg] = process.argv.slice(2);
if (!labArg || !refsArg || !outArg) throw Error('LAB REFERENCES OUTPUT required');
const lab = resolve(labArg), refs = resolve(refsArg), out = resolve(outArg);
const {MT1, loadKeys} = await import(pathToFileURL(join(lab, 'mt.ts')));
mkdirSync(out, {recursive:true});
let total = 0; const manifest = [];
for (const name of readdirSync(refs).filter(n => /^0[1-6].*\.keys$/.test(n)).sort()) {
  const script = loadKeys(join(refs, name));
  if (script.length > 65536 || script.reduce((n, s) => n + s.wait, 0) > 200000) throw Error('replay budget');
  const game = new MT1(false);
  for (let i = 0; i < script.length; i++) {
    game.step(script[i]);
    if (game.m.videoMode !== 0x0d) continue;
    const frame = new Uint8Array(64016);
    game.m.ega.indices320(frame.subarray(0, 64000)); frame.set(game.m.ega.attr.subarray(0, 16), 64000);
    const file = `${basename(name, '.keys')}-${String(i).padStart(5, '0')}.ega`;
    writeFileSync(join(out, file), frame);
    manifest.push({file, sha256:createHash('sha256').update(frame).digest('hex'), script:name, step:i, prog:game.prog?.name}); total++;
  }
  console.log(`${name}: ${script.length} steps; ${total} frames total`);
}
if (!total) throw Error('no reference frames');
writeFileSync(join(out, 'manifest.json'), JSON.stringify(manifest, null, 2));
console.log(`Captured ${total} original lockstep frames`);
