"""S Covox PCM oracle samples for space_sounds.rs; all oracle files read-only."""
from pathlib import Path
import struct
import sys

sys.dont_write_bytecode = True
oracle, gamefiles, output = map(Path, sys.argv[1:])
sys.path.insert(0, str(oracle / 'tools/beh_covox'))
from equivalents import PLAYERS, Playback, MAGIC

exe = (oracle / 'work/mt1/S.exe').read_bytes()
image = exe[struct.unpack_from('<H', exe, 8)[0] * 16:]
calls = [i for i in range(65533) if image[i] == 0xE8
         and (i + 3 + struct.unpack_from('<h', image, i + 1)[0]) & 65535 == 0x1FB]
assert calls == [0x8B8D, 0x9A09, 0x9F1D], calls
far_calls = [i for i in range(len(image) - 5) if image[i] == 0x9A
             and struct.unpack_from('<H', image, i + 1)[0]
             + 16 * struct.unpack_from('<H', image, i + 3)[0] == 0x1FB]
assert not far_calls, far_calls
output.parent.mkdir(parents=True, exist_ok=True)
count = 0
with output.open('wb') as out:
    for device in (8, 0x222):
        memory = bytearray(0x90000)
        ds = 0x20000
        memory[ds + 0x2D9A:ds + 0x2DB6] = image[0x161D0 + 0x2D9A:0x161D0 + 0x2DB6]
        for index in (0, 1, 2, 0, 2, 1):
            name = ('MA.VMF', 'LGUN.VMF', 'BOOM.VMF')[index]
            payload = (gamefiles / name).read_bytes()[128:]
            assert not payload.startswith(MAGIC)
            memory[0x50000:0x50000 + len(payload)] = payload
            spec = Playback(memory, ds, PLAYERS[8], io_values=(0x10, 0, 0), delays=(0,))
            spec.run((0, 0x5000), len(payload), 18, device)
            assert spec.result == 0
            levels = spec.samples
            if device == 8:
                level = 0
                levels = []
                for sample in spec.samples:
                    if sample >= 132: level = 0
                    elif sample <= 124: level = 255
                    levels.append(level)
            else:
                assert bytes(levels) == payload[:-1]
            out.write(struct.pack('<HHI', device, index, len(levels)))
            out.write(bytes(levels))
            count += 1
print(f'PASS S audio oracle: {count} sequential clips, PC speaker and Covox; 001FB callers 8B8D/9A09/9F1D')
