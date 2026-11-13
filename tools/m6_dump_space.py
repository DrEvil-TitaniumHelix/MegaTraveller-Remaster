"""Original S.exe instruction captures for crates/mt1/tests/space_original.rs (read-only oracle, no oracle writes).

  python -B tools/m6_dump_space.py ORACLE OUT [systems|scheduler|render ...]

systems.bin   300 D0D/39AF calls over the eight STP files, random radii/positions/dates (travel_golden.Original).
scheduler.bin 632 5EE5/5E3C/5E49 calls with scripted clocks and cursor-injected keys (spec.test_space_tick.Suite).
render.bin    160 4B30/3E49 pixel cases (spec.test_render_s_spaceview.Proof); the star tiles are the Borland LCG's own
              replies from the seed written to DS:343A, so the native generator reproduces them.
"""
import importlib.util
from pathlib import Path
import random
import struct
import sys

sys.dont_write_bytecode = True
root = Path(__file__).resolve().parents[1]
loader = importlib.util.spec_from_file_location('gate', root/'tests/faithful/walk_golden.py')
gate = importlib.util.module_from_spec(loader)
loader.loader.exec_module(gate)
oracle, output = map(Path, sys.argv[1:3])
which = set(sys.argv[3:]) or {'systems', 'scheduler', 'render'}
output.mkdir(parents=True, exist_ok=True)
gate.load_oracle(oracle)


def words(values):
    return struct.pack('<'+'H'*len(values), *[v & 65535 for v in values])


def lcg_stars(seed, n):
    out = []
    for _ in range(n):
        seed = (seed * 22695477 + 1) & 0xFFFFFFFF
        out.append(((seed >> 16) & 0x7FFF) % 20)
    return out


with gate.oracle_reads(oracle):
    if 'systems' in which:
        from travel_golden import Original, ARR, VAR
        original = Original()
        original.vm.root = str(oracle/'orig/dos/MegaTra1')
        rng = random.Random(20261011)
        with (output/'systems.bin').open('wb') as out:
            for case in range(300):
                system = case % 8
                world = rng.choice([255, *range(12)])
                clock = rng.choice([0, 1, 15, 16, 17, -1, -2, -17, -32768, rng.randrange(-32768, 32768)])
                radius = [rng.randrange(6) for _ in range(90)]
                x = [rng.randrange(-300, 300) for _ in range(80)]
                y = [rng.randrange(-300, 300) for _ in range(80)]
                expected = original.enter(system, world, clock, radius, x, y)
                out.write(words([system, world, clock, *radius, *x, *y]))
                out.write(words([expected[k] for k in VAR]))
                for name in ARR:
                    out.write(words(expected[name]))
                out.write(words([expected['name_char']]))
        print('Captured 300 original D0D/39AF calls', flush=True)

    if 'scheduler' in which:
        from spec.test_space_tick import Suite, Context, CALLEES, FUNCTIONS
        suite = Suite(20261011, 120)
        ids = {'update_cursor': 1, 'hide_cursor': 2, 'show_cursor': 3,
               'redraw_tactical': 4, 'redraw_active_station': 5,
               'select_draw_page': 6, 'back_buffer': 7, 'fill_rect': 8}
        total = 0
        with (output/'scheduler.bin').open('wb') as out:
            for address, generator in [(0x5EE5, suite.task_cases), (0x5E3C, suite.clock_cases), (0x5E49, suite.wait_cases)]:
                for memory, args, config in generator(address):
                    ctx = Context(None, suite.machine.dgroup, config)
                    size = FUNCTIONS[address][1]
                    composed = (0x5E3C,) if address == 0x5E49 else ()
                    expected = suite.machine.run(memory, address, args, far=True, segment=0,
                        extent=(address, address+size), callees={a: c for a, c in CALLEES.items() if a != address and a not in composed},
                        services=ctx, composed=composed, limit=300000)
                    trace = []
                    for event in ctx.trace:
                        if event[0] in ('get_ticks', 'snapshot_clock'):
                            continue
                        if event[0] == 'clock_reply':
                            trace.append((0, [event[1] & 65535, event[1] >> 16]))
                        else:
                            assert event[0] in ids, event
                            trace.append((ids[event[0]], event[1]))
                    out.write(words([address, args[0] if args else 0, config.get('cursor_after', 65535),
                                     config.get('cursor_key', 1), config.get('radar_result', 0), len(config['clocks'])]))
                    out.write(struct.pack('<'+'I'*len(config['clocks']), *config['clocks']))
                    base = suite.machine.dgroup*16
                    for image in (memory, expected):
                        ds = bytearray(image[base:base+65536])
                        ds[0x3458:0x345A] = words([0x9000])
                        out.write(ds)
                        out.write(image[0x50000:0x50000+0xF86])
                    out.write(words([suite.machine.reg('AX'), len(trace)]))
                    for code, values in trace:
                        out.write(words([code, len(values), *values]))
                    total += 1
        print(f'Captured {total} original scheduler/clock/wait calls', flush=True)

    if 'render' in which:
        from spec.test_render_s_spaceview import Proof, State, game_bytes
        from spec import jump
        proof = Proof(); rng = random.Random(20261011)
        with (output/'render.bin').open('wb') as out:
            for i in range(160):
                hud = i < 32
                kind = ('hud_dock', 'hud_gas')[i % 2] if hud else ('dock', 'gas')[i % 2]
                s, r, target = proof.fixture(rng, i, kind)
                seed = rng.randrange(1 << 32)
                s.dword(0x343A, seed)
                stars = lcg_stars(seed, 4000)
                if hud:
                    mode = 55 if kind == 'hud_gas' else (1, 99, -1, -2)[(i//2) % 4]; entry = 0x4B30
                    if i % 3 == 0: jump.select_draw_page(s, None)
                else:
                    size = s.get('object_size', target)
                    delta = ((-6, 0), (-5, 0), (-1, 0), (0, 0), (size, 0), (size+1, 0), (size+5, 0), (size+6, 0), (0, -6), (0, -5), (0, -1), (0, size), (0, size+1), (0, size+5), (0, size+6))
                    dx, dy = delta[(i//4) % len(delta)]
                    s.player_x = s.get('object_x', target)+dx; s.player_y = s.get('object_y', target)+dy
                    mode = (i//4) % 2; s.current_object = target if mode else -1
                    if i % 7 == 0: s.departed_object = -target
                    if i % 11 == 0: s.heading = 0
                    entry = 0x3E49
                ds = bytearray(s.memory[s.base:s.base+65536]); ds[0x3458:0x345A] = words([0x9000])
                out.write(words([entry, mode])); out.write(ds); out.write(s.memory[0x70000:0x70F86]); out.write(r.snapshot())
                proof.run(s, r, entry, (mode,), stars, label=f'case {i}')
                final = State(bytearray(proof.cpu.mem_read(0, len(s.memory))), proof.ds)
                out.write(proof.original.ega.snapshot()); out.write(game_bytes(final)); out.write(words([proof.reg('AX')]))
                if i % 16 == 15: print(f'PASS original render {i+1}/160', flush=True)
