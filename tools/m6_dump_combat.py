"""Capture unmodified S routines; oracle read-only, fixtures in caller output."""
import itertools
from pathlib import Path
import random
import struct
import sys
from collections import deque

oracle, out = map(Path, sys.argv[1:3])
sys.path[:0] = [str(oracle / 'tools'), str(oracle / 'tools/spec')]
from render_s_cg_support.original import Original
import unicorn as uc
from unicorn import x86_const as x86

out.mkdir(parents=True, exist_ok=True)
o = Original('S')
template = bytes(o.memory)
rows = []
rng = random.Random(0x6288A1)

def start(seed):
    o.memory = bytearray(template)
    o.w(0x3456, 0); o.w(0x3458, 0x7000)
    o.w(0x3AAC, 0); o.w(0x3AAE, 0x9100)
    struct.pack_into('<I', o.memory, o.ds * 16 + 0x343A, seed)
    struct.pack_into('<I', o.memory, 0x46C, 100000)
    struct.pack_into('<I', o.memory, o.ds * 16 + 0x188, 65535)
    o.byte(0x3B78, 0); o.byte(0x4627, 0)
    o.memory[0x70000:0x70F86] = bytes(0xF86)

def party(at, value): o.memory[0x70000 + at] = value & 255

def capture(method, address, args):
    case = out / str(len(rows)); case.mkdir(exist_ok=True)
    def save(prefix):
        ds = bytearray(o.memory[o.ds * 16:o.ds * 16 + 65536])
        struct.pack_into('<H', ds, 0x3458, 0x9000)
        (case / f'{prefix}-ds.bin').write_bytes(ds)
        (case / f'{prefix}-party.bin').write_bytes(o.memory[0x70000:0x70F86])
    save('input')
    o.run(address, args, far=True)
    save('end')
    rows.append('\t'.join(map(str, [case.name, method, o.machine.reg('AX'), *args])))

for weapon, gun, charge in itertools.product(range(1, 16), range(2), (-1, 0, 24, 25, 55, 100)):
    start(rng.randrange(65536)); o.w(0x1BF + 2 * gun, charge)
    party(0xA1 + gun, 1 + gun); party(0x117 + gun * 256, rng.randrange(16))
    party(0xC3 + gun, rng.choice((0, 49, 50, 100)))
    for i, p in enumerate((5, 6, 7, 8)):
        party(0xAC + i, p | (0x20 if rng.randrange(2) else 0x40))
    capture('player', 0x88A1, (weapon, gun, rng.randrange(-6, 5), rng.randrange(-6, 5)))

for weapon, pilot, damage in itertools.product(range(1, 9), (0, 1), (0, 29, 98)):
    start(rng.randrange(65536)); party(0xA0, pilot)
    for component in range(4): party(0xC2 + component, damage)
    for i, p in enumerate((10, 11, 12)): party(0xAC + i, p | (0x20 if rng.randrange(2) else 0x40))
    capture('enemy', 0x8AAA, (weapon, rng.randrange(-4, 5), rng.randrange(-4, 5)))

for n in range(96):
    start(rng.randrange(65536)); o.w(0x1C3, 1); o.w(0x1C5, 4); o.w(0x1C7, 4)
    o.w(0x34C6, -1); o.w(0x399, 11); o.w(0x36D0, 5)
    for i in range(4):
        o.w(0x37AE + i * 2, 0 if i <= n % 4 else -1)
        for off in range(0, 30, 2): o.w(0x3CB9 + 30 * i + off, 0)
        for off, value in ((0, 110), (16, rng.choice((300, 500, 750, 1500))), (20, rng.randrange(1, 9)),
                           (22, rng.randrange(1, 9)), (26, rng.choice((-1, 0, 2, 8))), (28, rng.choice((-1, 0, 2, 8)))):
            o.w(0x3CB9 + 30 * i + off, value)
    for i in range(225): o.byte(0x3882 + i, rng.choice((0, 0, 0, 0, 1, 100, 101)))
    capture('advance', 0x8C4C, ())

keys, selections = deque(), deque()
def boundary(cpu, address, size, info):
    routine, far = info
    ss, sp = cpu.reg_read(x86.UC_X86_REG_SS), cpu.reg_read(x86.UC_X86_REG_SP)
    stack = ss * 16 + sp
    if routine == 'cursor' and cpu.mem_read(o.ds * 16 + 0x483A, 1) == b'\0':
        assert keys, 'control key tape exhausted'
        cpu.mem_write(o.ds * 16 + 0x483A, bytes((keys.popleft(),)))
    elif routine == 'select':
        assert selections, 'selection tape exhausted'
        cpu.reg_write(x86.UC_X86_REG_AX, selections.popleft() & 65535)
    elif routine == 'handle': cpu.reg_write(x86.UC_X86_REG_AX, 0)
    ip = int.from_bytes(cpu.mem_read(stack, 2), 'little')
    if far: cpu.reg_write(x86.UC_X86_REG_CS, int.from_bytes(cpu.mem_read(stack + 2, 2), 'little'))
    cpu.reg_write(x86.UC_X86_REG_SP, sp + (4 if far else 2)); cpu.reg_write(x86.UC_X86_REG_IP, ip)

for address, name, far in [(0xCE48, 'cursor', True), (0xCDFB, 'draw', True), (0xCD80, 'draw', True),
    (0x1C1C, 'select', True), (0xD7B4, 'select', True), (0x11D6, 'handle', True),
    (0x8672, 'draw', True), (0xAF04, 'draw', True), (0x2DE, 'draw', True),
    (0xE3B5, 'draw', False), (0xD4E3, 'draw', True), (0xE099, 'draw', True),
    (0x1ED, 'draw', True), (0xE411, 'draw', True), (0xEF1E, 'draw', True),
    (0xEE61, 'draw', True), (0xEEC2, 'draw', True)]:
    o.machine.cpu.hook_add(uc.UC_HOOK_CODE, boundary, (name, far), 0x10000 + address, 0x10000 + address)

def control_start(seed, raw_keys, choices=()):
    start(seed); keys.clear(); keys.extend(raw_keys); selections.clear(); selections.extend(choices)
    for slot in range(15): o.w(0x34BA + slot * 2, -1)
    for at in (0x37A2, 0x37AE, 0x37BC, 0x37CA):
        for slot in range(4): o.w(at + slot * 2, -1)
    o.w(0x395, 0); o.w(0x397, 1); o.w(0x399, 11); o.w(0x1C3, 0)
    struct.pack_into('<I', o.memory, o.ds * 16 + 0x347E, 100000)
    o.byte(0x483A, 0)

for gun, direction, command in itertools.product(range(2), (0, 7), (0x4B, 0x4D, 0x14)):
    control_start(410 + len(rows), [command, 0x1C, 1] if command == 0x14 else [command, 1])
    o.w(0x399, gun + 11); o.w(0x1BA + 2 * gun, direction); o.w(0x1BF + 2 * gun, 100)
    party(0xC8 + gun, 1); party(0xA1 + gun, gun + 1)
    case = out / str(len(rows)); case.mkdir(exist_ok=True)
    (case / 'keys.txt').write_text(' '.join(map(str, keys)))
    (case / 'selections.txt').write_text('')
    capture('gun', 0x9FDB, (gun,))

for capacity, choices in itertools.product((2, 5, 8, 12, 20), ((0, 0), (-1,), (-2,), (0, -2))):
    control_start(811 + len(rows), [0x14, 1], choices)
    o.w(0x3CBB, capacity); o.w(0x3CD1, 0); party(0xCA, 7)
    case = out / str(len(rows)); case.mkdir(exist_ok=True)
    (case / 'keys.txt').write_text(' '.join(map(str, keys)))
    (case / 'selections.txt').write_text(' '.join(map(str, selections)))
    capture('board', 0x236C, (0,))

o.machine.cpu.hook_add(uc.UC_HOOK_CODE, boundary, ('draw', True), 0x1B706, 0x1B706)
for tubes, free in itertools.product(((0, 0, 0, 0), (1, 2, 3, 4), (0, 7, 0, 9)), (0, 1, 3, 20)):
    start(77)
    for i, value in enumerate(tubes): o.byte(0x387E + i, value)
    for i in range(20): party(0xDE + i, 0 if i < free else 30)
    capture('land-missiles', 0xB6C0, ())

(out / 'cases.tsv').write_text('\n'.join(rows) + '\n')
print(f'Captured {len(rows)} original S combat cases', flush=True)
