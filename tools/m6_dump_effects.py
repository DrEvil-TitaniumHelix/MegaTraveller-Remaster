"""S 13E30 full private-state and speaker-event captures, including persistence."""
from pathlib import Path
import struct
import sys

oracle, out = map(Path, sys.argv[1:3])
sys.path[:0] = [str(oracle / 'tools/beh_covox')]
from verify import CovoxMachine
from equivalents import Playback, PLAYERS
from engine import compare_memory

class Speaker(Playback):
    refresh = 0
    def inport(self, port, value):
        if port == 0x61:
            self.refresh ^= 16; value = self.refresh
        elif port == 0x21: value = 0
        return super().inport(port, value)

out.mkdir(parents=True, exist_ok=True)
m = CovoxMachine(); memory = m.template.copy(); ds = m.dgroup * 16
refresh = 0
rows = []
for n, index in enumerate((1, 2, 1, 0, 2, 0)):
    name = ('MA.VMF', 'LGUN.VMF', 'BOOM.VMF')[index]
    data = (oracle / 'orig/dos/MegaTra1' / name).read_bytes()[128:]
    memory[0x50000:0x50000 + len(data)] = data
    struct.pack_into('<HH', memory, 0x41A, 30, 30)
    memory[ds + 0x3B78] = memory[ds + 0x4627] = 1
    struct.pack_into('<H', memory, ds + 0x1A28, 8)
    case = out / str(n); case.mkdir(exist_ok=True)
    (case / 'input-ds.bin').write_bytes(memory[ds:ds + 65536])
    expected = memory.copy(); p = Speaker(expected, ds, PLAYERS[8]); p.refresh = refresh
    p.run((0, 0x5000), len(data), 18, 8)
    actual = m.run(memory, 0x13E30, (0, 0x5000, len(data), 0, 18, 8, 0), far=True,
        segment=0x13E3, reads=p.reads, limit=12000000, preserved=('BP','SI','DI','DS','ES','SS'))
    compare_memory(actual, expected); assert m.ports.trace == p.trace
    struct.pack_into('<H', actual, ds + 0x1A26, (len(data) * 182 + 4999) // 5000)
    writes = [(event[1], event[3]) for event in p.trace if event[0] == 'out' and event[1] != 0x21]
    levels = []; level = 0
    for i, (port, value) in enumerate(writes):
        if port == 0x42 and value == 255: levels.append(level)
        elif port == 0x61:
            level = 255 if value & 2 else 0
            if levels and not (i + 1 < len(writes) and writes[i + 1][0] == 0x43): levels[-1] = level
    (case / 'end-ds.bin').write_bytes(actual[ds:ds + 65536])
    (case / 'levels.bin').write_bytes(bytes(levels))
    rows.append(f'{n}\t{index}\t{level}')
    memory = actual; refresh = p.refresh
(out / 'cases.tsv').write_text('\n'.join(rows) + '\n')
print('PASS six composed S effects: machine/model DS and all port events', flush=True)
