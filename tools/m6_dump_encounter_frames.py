"""Render fixtures from I2's independent EGA oracle; no writes in the oracle."""
from pathlib import Path
import random
import struct
import sys

oracle, out = map(Path, sys.argv[1:3])
sys.path[:0] = [str(oracle / 'tools'), str(oracle / 'tools/spec')]
import unicorn as uc
from render_s_encounters_support import Oracle, BASE
from render_s_cg import decode_layout
from unicorn import x86_const as x86
out.mkdir(parents=True, exist_ok=True)
o = Oracle(); rng = random.Random(0x6E11); groups = o.groups(0xAA23)
rows = []

def begin(model, method, args, seed):
    case = out / str(len(rows)); case.mkdir(exist_ok=True)
    ds = bytearray(model.state.memory[o.original.ds * 16:o.original.ds * 16 + 65536])
    struct.pack_into('<HH', ds, 0x3456, 0, 0x9000)
    struct.pack_into('<HH', ds, 0x3AAC, 0, 0x9100)
    struct.pack_into('<I', ds, 0x343A, seed)
    (case / 'input-ds.bin').write_bytes(ds)
    (case / 'input-party.bin').write_bytes(model.state.memory[0x70000:0x70F86])
    (case / 'input-vram.bin').write_bytes(model.renderer.snapshot())
    rows.append('\t'.join(map(str, [case.name, method, seed, *args])))
    return case

for kind in range(19):
    for page in range(2):
        model, _ = o.scene(rng, kind * 2 + page, 18, groups, kind)
        model.state.memory[model.state.base + 0x1BE] = 7
        o.original.byte(0x1BE, 7)
        for counter in (0, 30):
            model.state.word(0x1B6, counter); o.original.w(0x1B6, counter)
            seed = 107 + len(rows)
            case = begin(model, 'map', (), seed)
            values = []; value = seed
            for _ in range(2048):
                value = (value * 0x015A4E35 + 1) & 0xFFFFFFFF; values.append((value >> 16) & 0x7FFF)
            o.compare_tick(model, values, str(case.name))
            (case / 'end-vram.bin').write_bytes(o.original.ega.snapshot())
            (case / 'rands.txt').write_text(str(model.random_position))

for address in (0xCDFB, 0xCD80):
    o.original.machine.cpu.hook_add(uc.UC_HOOK_CODE, o.retfar, None, BASE * 16 + address, BASE * 16 + address)
for direction in range(8):
    for station in (10, 11, 12, 15):
        model, _ = o.scene(rng, direction * 4 + station, 1, groups, direction % 19)
        s = model.state
        for at, value in ((0x1C3, 1), (0x1C5, 4), (0x1C7, 4), (0x399, station), (0x34B8, direction), (0x1BA, direction), (0x1BC, (direction + 3) % 8)):
            s.word(at, value)
        s.dword(0x347E, 100000)
        s.memory[0x700A1:0x700A3] = bytes((2, 4))
        s.memory[s.base + 0x3882:s.base + 0x3882 + 225] = bytes(225)
        for i, (row, col, value) in enumerate(((2, 3, 1), (5, 4, 2), (6, 6, 100), (3, 5, 101), (4, 2, 102))):
            s.memory[s.base + 0x3882 + row * 15 + col] = value
            if value < 100:
                s.enemy(value - 1, 0, 110 + 10 * i); s.enemy(value - 1, 18, direction)
        o.original.memory = bytearray(s.memory)
        struct.pack_into('<I', o.original.memory, 0x46C, 100000)
        case = begin(model, 'radar', (99,), 1)
        o.tracing = False
        o.run(0x62CC, (99,))
        (case / 'end-vram.bin').write_bytes(o.original.ega.snapshot())
        (case / 'rands.txt').write_text('0')

def escape(cpu, address, size, user):
    cpu.reg_write(x86.UC_X86_REG_AX, 27); o.retfar(cpu, address, size, user)
o.original.machine.cpu.hook_add(uc.UC_HOOK_CODE, escape, None, BASE * 16 + 0x4707, BASE * 16 + 0x4707)
for page in range(2):
    for heading in range(8):
        model, _ = o.scene(rng, page, 1, groups, heading)
        s = model.state; s.word(0x1C3, 1); s.word(0x395, 0); s.word(0x34B8, heading)
        s.word(0x1C5, 4); s.word(0x1C7, 4); s.word(0x49A6, 0); s.word(0x49A8, 0xA600)
        s.memory[s.base + 0x3882:s.base + 0x3882 + 225] = bytes(225)
        for row, col, value in ((0, 0, 1), (9, 9, 2), (1, 3, 100), (6, 7, 101), (4, 2, 102)):
            s.memory[s.base + 0x3882 + row * 15 + col] = value
            if value < 100: s.enemy(value - 1, 0, 110); s.enemy(value - 1, 18, heading)
        o.original.memory = bytearray(s.memory); case = begin(model, 'tactical-map', (), 1)
        o.tape = []; value = 1
        for _ in range(260):
            value = (value * 0x015A4E35 + 1) & 0xFFFFFFFF; o.tape.append((value >> 16) & 0x7FFF)
        o.position = 0; o.run(0x6743)
        (case / 'end-vram.bin').write_bytes(o.original.ega.snapshot()); (case / 'rands.txt').write_text(str(o.position))

for sprite in range(110, 180, 10):
    for direction in range(8):
        model, _ = o.scene(rng, direction, 1, groups, 0)
        s = model.state; s.word(0x1C5, 4); s.word(0x1C7, 4)
        s.memory[s.base + 0x3882:s.base + 0x3882 + 225] = bytes(225)
        for i in range(4):
            row = s.word(0x29B + direction * 16 + i * 4) + 4
            col = s.word(0x29D + direction * 16 + i * 4) + 4
            if 0 <= row < 10 and 0 <= col < 10: s.memory[s.base + 0x3882 + row * 15 + col] = i + 1
            s.enemy(i, 0, sprite); s.enemy(i, 18, (direction + i) % 8)
        o.original.memory = bytearray(s.memory)
        o.original.install(decode_layout((oracle / 'orig/dos/MegaTra1/GUNS.LY').read_bytes()), segment=0x6000)
        o.original.w(0x4619, 0); o.original.w(0x461B, 0x6000)
        o.original.install(o.layouts, segment=0x5000); o.original.w(0x4622, 0); o.original.w(0x4624, 0x5000)
        s.memory = bytearray(o.original.memory)
        case = begin(model, 'gun-view', (direction,), 1)
        o.tape = []; value = 1
        for _ in range(8):
            value = (value * 0x015A4E35 + 1) & 0xFFFFFFFF; o.tape.append((value >> 16) & 0x7FFF)
        o.position = 0; o.run(0x8548, (direction,))
        (case / 'end-vram.bin').write_bytes(o.original.ega.snapshot()); (case / 'rands.txt').write_text(str(o.position))

(out / 'cases.tsv').write_text('\n'.join(rows) + '\n')
print(f'Captured {len(rows)} encounter frames (76 I2 minimap + 32 radar + 16 modal map + 56 gun view)', flush=True)
