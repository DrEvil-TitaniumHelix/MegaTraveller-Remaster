"""CG 1D0A spec + original-executable byte proof. See docs/decode/cg_1d0a_build_party.md.

MT1_ORACLE and MT1_GAMEFILES required. Outputs only below target/; never writes the
oracle. Each worker has a tick/decision cap and a parent wall-time/RSS watchdog.
"""
import argparse
import gc
import json
import os
from pathlib import Path
import random
import struct
import subprocess
import sys
import time
from unittest.mock import patch

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
SIZE = 0xF86
BRANCHES = {0x1D62: 'ne', 0x1DAB: 'e', 0x1DBB: 'e', 0x1DC3: 'l',
            0x1E7F: 'ge', 0x1ED5: 'ne', 0x1EF8: 'g', 0x1F18: 'ne',
            0x1F36: 'e', 0x1F5F: 'ne', 0x1F7D: 'e', 0x1F8D: 'e',
            0x1FC9: 'ne', 0x1FED: 'ne', 0x2013: 'e', 0x2023: 'ge', 0x2084: 'e'}


def word(data, off):
    return struct.unpack_from('<H', data, off)[0]


def put_word(data, off, value):
    struct.pack_into('<H', data, off, value & 65535)


class Rand:
    def __init__(self, state):
        self.state, self.draws = state, []

    def __call__(self):
        self.state = (self.state * 214013 + 2531011) & 0xFFFFFFFF
        value = (self.state >> 16) & 32767
        self.draws.append(value)
        return value


def block(before, party, ds, rand):
    """Transform the incoming shared block, five optional pool records, CG DS and live rand()."""
    if len(before) != SIZE or len(ds) != 65536:
        raise ValueError('expected 0xF86-byte block and 64 KiB CG DS')
    if any(r is not None and len(r) != 256 for r in party[:5]):
        raise ValueError('pool records must be 256 bytes')
    b = bytearray(before)
    glyph = lambda item: ds[(0x16BF + 3 * item) & 65535]
    b[0] = 0
    for off, value in ((0xA82, 0x56), (0xA87, 0x57), (0xA8C, 0x59)):
        b[off] = value
    count = 3
    for member in range(5):
        rec, inv = 0x102 + member * 256, 0x1CE + member * 256
        source = party[member] if member < len(party) else None
        if source is None:
            b[rec:rec + 256] = bytes(256)
            b[rec + 0x28] = 2
        else:
            b[0] |= 1 << member
            b[rec:rec + 256] = source
            icon = 0x28
            for cell in range(19):
                item = word(b, inv + 2 * cell)
                if item != 65535 and glyph(item) == 0x23:
                    put_word(b, inv + 2 * cell, word(b, inv))
                    put_word(b, inv, item)
                    icon = 0x29
                    break
            b[0xA82 + 5 * count] = icon
            count += 1
            put_word(b, rec + 0xF2, count)
            b[rec + 0x26], b[rec + 0xC9] = 45, 2
        b[member + 1], b[member + 0xA0] = member, member + 1
    for off, value in ((0xC0, 2), (0xC8, 2), (0xC1, 0x81), (0xAC, 3),
                       (0xAE, 4), (0xAF, 1), (0xB0, 5), (0xFC, 125), (0x82, 14), (0x83, 30)):
        b[off] = value
    for member in range(5):
        rec = 0x102 + member * 256
        if not b[rec + 0x25]:
            continue
        for cell in range(19):
            inv = rec + 0xCC + 2 * cell
            item = word(b, inv)
            if not 0 < item < 32768:
                continue
            obj, kind, equipped = 0xA82 + count * 5, glyph(item), False
            b[obj] = kind
            if kind in (0x23, 0x20):
                b[obj + 1:obj + 3] = bytes((5, 40 if kind == 0x23 else 30))
                if not word(b, rec + 0xF4):
                    put_word(b, rec + 0xF4, count + 1)
                    put_word(b, inv, 0)
                    equipped = True
            elif kind != 0x39:
                b[obj + 1:obj + 3] = bytes((rand() % 4 + 2, rand() % 4))
                if not word(b, rec + 0xF6):
                    put_word(b, rec + 0xF6, count + 1)
                    put_word(b, inv, 0)
                    if word(b, inv + 2) == 65535:
                        put_word(b, inv + 2, 0)
                    equipped = True
            if not equipped:
                put_word(b, inv, count + 1)
            count += 1
    b[8:0x6C] = bytes(100)
    for off, value in ((0x1EE, 1), (0x1F0, 2), (0x1F2, 3), (0x88, 198), (0x8A, 0)):
        put_word(b, off, value)
    b[0x80], b[6] = 1, 2
    b[0x8C:0x97] = ds[0x2A1B:0x2A26]
    return bytes(b)


def watchdog(command, seconds=120, memory_mb=1536):
    """Inherited output streams: no frame/output accumulation in the parent."""
    import psutil
    start = time.monotonic()
    with subprocess.Popen(command, cwd=ROOT, stdin=subprocess.DEVNULL) as child:
        try:
            process = psutil.Process(child.pid)
        except psutil.NoSuchProcess:
            process = None
        try:
            while child.poll() is None:
                try:
                    tree = [process] + process.children(recursive=True) if process else []
                except psutil.NoSuchProcess:
                    tree = []
                rss = 0
                for p in tree:
                    try:
                        rss += p.memory_info().rss
                    except psutil.NoSuchProcess:
                        pass
                if time.monotonic() - start > seconds or rss > memory_mb * 1024 * 1024:
                    raise RuntimeError(f'watchdog: {seconds}s / {memory_mb} MiB limit exceeded')
                time.sleep(0.1)
        finally:
            if child.poll() is None:
                try:
                    descendants = process.children(recursive=True) if process else []
                except psutil.NoSuchProcess:
                    descendants = []
                for p in reversed(descendants):
                    try:
                        p.kill()
                    except psutil.NoSuchProcess:
                        pass
                child.kill()
                child.wait()
        if child.returncode:
            raise RuntimeError(f'child exit {child.returncode}')


def load_oracle():
    oracle = Path(os.environ['MT1_ORACLE']).resolve()
    game = Path(os.environ['MT1_GAMEFILES']).resolve()
    sys.path.insert(0, str(oracle / 'tools/golden'))
    import cg_golden as golden
    base = golden.DosVM

    class ReadOnlyVM(base):
        def __init__(self, exe, *args, **kwargs):
            kwargs['root'] = str(game)
            super().__init__(str(oracle / exe), *args, **kwargs)

    return golden, ReadOnlyVM, game


def drive(golden, original, keys):
    if not keys:
        raise RuntimeError('empty key batch')
    original.vm.keys.extend(keys[:-1])
    rec = original.step(keys[-1], max_ticks=4000)
    if rec['stop'] != 'input exhausted' and original.vm.exit_code is None:
        raise RuntimeError(f'PARTIAL: {rec["stop"]}')
    return rec


def generated(golden, vm_class, game, index):
    seed, choices, draft, *disk = golden.RUNS[index]
    with patch.object(golden, 'DosVM', vm_class):
        original = golden.Original(seed)
    t = vm_class('work/mt1/T.exe', tail=' 9000 0000 9100 0000 0nAn0')
    t.blocks[0x9000], t.blocks[0x9100] = 0xFA, 0x1B6
    initial = []

    def exec_cg(name, tail):
        if name != 'CG.DAT':
            raise RuntimeError(f'unexpected T child {name}')
        initial.append(t.rb(0x90000, SIZE))
        t.stop_reason = 'captured CG hand-over'
        t.mu.emu_stop()
        return 2

    t.exec_handler = exec_cg
    t.run(max_ticks=4000)
    if len(initial) != 1:
        raise RuntimeError(f'PARTIAL: T did not EXEC CG: {t.stop_reason}')
    expected_initial = bytearray(SIZE)
    expected_initial[0xA82:0xF82:5] = b'\xff' * 256
    if initial[0] != expected_initial:
        raise AssertionError('T 822C initial party block differs')
    original.vm.wb(0x90000, initial[0])
    if disk and disk[0]:
        original.vm.vfs['UCHARS.DAT'] = bytearray((game / 'DCHARS.DAT').read_bytes())
    choose = golden.make_script(choices, draft)
    rec = original.step(max_ticks=4000)
    for n in range(400):
        if rec['stop'] != 'input exhausted':
            raise RuntimeError(f'PARTIAL generation: {rec["stop"]}')
        keys = choose(rec, n, original)
        if keys == 'stop':
            break
        rec = drive(golden, original, keys or [13])
    else:
        raise RuntimeError('PARTIAL: generation decision cap')
    saved = original.vm.vfs.get('UCHARS.DAT')
    if not saved or saved[0] < 1:
        raise RuntimeError('PARTIAL: generated pool not saved')
    menu = golden.last_menu(rec)
    rec = drive(golden, original, [golden.DOWN] * menu.index('Add to Party') + [13])
    menu = golden.last_menu(rec)
    if not menu or 'GOLDEN' not in menu[-1]:
        raise RuntimeError(f'generated character absent from selection: {menu}')
    rec = drive(golden, original, [golden.DOWN] * (len(menu) - 1) + [13])
    for _ in range(index % 5):
        menu = golden.last_menu(rec)
        rec = drive(golden, original, [golden.DOWN] * menu.index('Add to Party') + [13])
        rec = drive(golden, original, [13])
    vm, captured = original.vm, {}

    def entry(v):
        ds = v.rb(v.lin(v.r('DS'), 0), 65536)
        slots = ds[0x29EE:0x29F3]
        captured.update(before=v.rb(0x90000, SIZE), ds=ds,
                        party=[ds[0x1740 + 256*s:0x1840 + 256*s] if s else None for s in slots],
                        rng=struct.unpack_from('<I', ds, 0x3DE8)[0],
                        memory=v.rb(0, 0x110000), cpu=v.mu.context_save(), blocks=dict(v.blocks), draws=[], edges=set())

    vm.hook(0x1D0A, entry)
    vm.hook(0x9095, lambda v: captured['draws'].append(v.r('AX')) if captured else None)
    def branch(v, address, condition):
        flags = v.r('EFLAGS')
        z, s, o = bool(flags & 64), bool(flags & 128), bool(flags & 2048)
        taken = {'e': z, 'ne': not z, 'l': s != o, 'ge': s == o, 'g': not z and s == o}[condition]
        captured['edges'].add(f'{address:04X}:{int(taken)}')
    for address, condition in BRANCHES.items():
        vm.hook(address, lambda v, a=address, c=condition: branch(v, a, c))
    menu = golden.last_menu(rec)
    rec = drive(golden, original, [golden.DOWN] * menu.index('Play Game') + [13])
    for _ in range(12):
        if vm.exit_code is not None:
            break
        rec = drive(golden, original, [13])
    if vm.exit_code != 2 or not captured:
        raise RuntimeError(f'PARTIAL: Play Game did not reach 1D0A/exit(2): {rec}')
    return original, captured


def check_case(out, name, original, captured, fault=False):
    rng = Rand(captured['rng'])
    want = block(captured['before'], captured['party'], captured['ds'], rng)
    got = original.vm.rb(0x90000, SIZE)
    if fault:
        want = bytes([want[0] ^ 1]) + want[1:]
    differences = [i for i, (a, b) in enumerate(zip(want, got)) if a != b]
    actual_state = struct.unpack('<I', original.vm.rb(original.vm.lin(original.vm.r('DS'), 0x3DE8), 4))[0]
    if differences or rng.state != actual_state or rng.draws != captured['draws']:
        raise AssertionError(f'{name}: {len(differences)} bytes differ; first={differences[:8]}; '
                             f'RNG {rng.state:08x}/{actual_state:08x}')
    filename = captured['ds'][0x4140:].split(b'\0', 1)[0].decode('latin1')
    if filename:
        filename = (filename if '.' in filename else filename + '.sav').upper()
        if original.vm.vfs.get(filename) != got:
            raise AssertionError(f'{name}: original 1C60 saved bytes differ')
    d = out / name
    d.mkdir(parents=True, exist_ok=True)
    for filename, data in (('before.bin', captured['before']), ('ds.bin', captured['ds']),
                           ('party.bin', got), ('pool.bin', b''.join(r or bytes(256) for r in captured['party']))):
        (d / filename).write_bytes(data)
    mask = sum(1 << i for i, r in enumerate(captured['party']) if r is not None)
    (d / 'state.txt').write_text(f'{mask} {captured["rng"]} {rng.state}\n' + ' '.join(map(str, rng.draws)) + '\n', encoding='ascii')
    (d / 'branches.txt').write_text('\n'.join(sorted(captured['edges'])) + '\n', encoding='ascii')
    print(f'| {name} | {SIZE} | {len(rng.draws)} | 0 |', flush=True)
    return name


def extra_cases(out, original, saved, fault):
    """Execute original 1D0A from its captured entry, including real rand()/exit()."""
    rng, names, vm = random.Random(0x1D0A), [], original.vm
    raw = (Path(os.environ['MT1_GAMEFILES']) / 'DCHARS.DAT').read_bytes()
    records = [raw[1 + 256*i:257 + 256*i] for i in range(raw[0])]
    for case in range(64):
        vm.wb(0, saved['memory'])
        vm.mu.context_restore(saved['cpu'])
        vm.blocks = dict(saved['blocks'])
        vm.exit_code = vm.stop_reason = None
        vm.keys.clear()
        original.text, original.rand, original.steps = [], [], []
        ds_base = vm.lin(vm.r('DS'), 0)
        before = rng.randbytes(SIZE)
        vm.wb(0x90000, before)
        mask = case if case < 32 else rng.randrange(32)
        vm.wb(ds_base + 0x29EE, bytes(i + 1 if mask >> i & 1 else 0 for i in range(5)))
        vm.wb(ds_base + 0x4140, b'PROOF.SAV\0' if case % 2 else b'\0')
        for i in range(5):
            rec = bytearray(records[(case + i) % len(records)])
            if case >= 32:
                rec = bytearray(rng.randbytes(256))
                rec[0x25] = rng.choice((0, 1, 40, 255))
                for off in (0xF4, 0xF6):
                    put_word(rec, off, rng.choice((0, 0, 0, 0x1234)))
                for cell in range(19):
                    put_word(rec, 0xCC + 2*cell, rng.choice((0, 0xFFFF, 0x8000, 2, 3, 42, 43, 47, 48, 49, 54, 57, 60, 70, 1, 0x7FFF)))
            vm.wb(ds_base + 0x1840 + 256*i, rec)
        original.step(max_ticks=4000)
        if vm.exit_code != 2:
            raise RuntimeError(f'PARTIAL direct case {case}: {vm.stop_reason}')
        name = f'mask_{case:02d}' if case < 32 else f'edge_{case - 32:02d}'
        names.append(check_case(out, name, original, saved, fault))
    return names


def worker(out, index, fault):
    golden, vm_class, game = load_oracle()
    original, captured = generated(golden, vm_class, game, index)
    names = [check_case(out, f'run_{index}', original, captured, fault)]
    if index == 0:
        names.extend(extra_cases(out, original, captured, fault))
    (out / f'run_{index}.json').write_text(json.dumps(names), encoding='utf-8')
    gc.collect()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, default=ROOT / 'target/cg-1d0a')
    parser.add_argument('--worker', type=int)
    parser.add_argument('--fault', action='store_true')
    parser.add_argument('--timeout', type=int, default=120)
    args = parser.parse_args()
    for name in ('MT1_ORACLE', 'MT1_GAMEFILES'):
        if not os.environ.get(name):
            raise RuntimeError(f'{name} is unset or empty')
    out = args.out.resolve()
    if not out.is_relative_to(ROOT / 'target'):
        parser.error('--out must be below the build repository target/')
    out.mkdir(parents=True, exist_ok=True)
    if args.worker is not None:
        worker(out, args.worker, args.fault)
        return 0
    manifest = out / 'cases.txt'
    manifest.unlink(missing_ok=True)
    print('| case | bytes | rand draws | differ |\n| --- | ---: | ---: | ---: |', flush=True)
    for index in range(8):
        command = [sys.executable, '-B', str(Path(__file__).resolve()), '--worker', str(index), '--out', str(out)]
        watchdog(command + (['--fault'] if args.fault else []), args.timeout)
    names = [n for i in range(8) for n in json.loads((out / f'run_{i}.json').read_text())]
    if len(names) != 72:
        raise RuntimeError('PARTIAL: missing cases')
    edges = set().union(*[set((out / n / 'branches.txt').read_text().splitlines()) for n in names])
    missing = {f'{a:04X}:{b}' for a in BRANCHES for b in (0, 1)} - edges
    if missing:
        raise RuntimeError(f'PARTIAL: conditional branch outcomes not exercised: {sorted(missing)}')
    manifest.write_text('\n'.join(names) + '\n', encoding='ascii')
    print(f'CG 1D0A BYTE PROOF: PASS ({len(names)} cases, {len(names)*SIZE} bytes, '
          f'{len(edges)}/34 branch outcomes, 0 differ)', flush=True)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        print(f'CG 1D0A BYTE PROOF: FAIL: {error}', file=sys.stderr, flush=True)
        sys.exit(1)
