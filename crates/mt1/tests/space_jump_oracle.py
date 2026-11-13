"""Export independent jump.py cases and render_s_cg frames; oracle stays read-only."""
import importlib.util
import gc
import os
from pathlib import Path
import random
import struct
import sys

sys.dont_write_bytecode = True
oracle, game, out = (Path(p).resolve() for p in sys.argv[1:4])
out.mkdir(parents=True, exist_ok=True)
sys.path[:0] = [str(oracle/'tools/spec'), str(oracle/'tools')]

def readonly(event, args):
    if event == 'open' and isinstance(args[0], (str, bytes, os.PathLike)):
        mode, flags = args[1:3]
        if (mode and any(c in mode for c in 'wax+')) or flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC):
            path = Path(os.fsdecode(args[0])).resolve()
            if path.is_relative_to(oracle) or path.is_relative_to(game):
                raise RuntimeError('read-only source: '+str(path))
sys.addaudithook(readonly)

spec = importlib.util.spec_from_file_location('jump_fixture', oracle/'tools/spec/test_jump.py')
t = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = t
src = Path(spec.origin).read_text().replace("WORK = ROOT / 'work/spec_jump'", 'WORK = Path('+repr(str(out/'work'))+')')
exec(compile(src, spec.origin, 'exec'), t.__dict__)
from spec import jump

FIELDS = ('video_mode', 'world_time', 'system_encounter', 'view_heading', 'minimap_quadrant', 'thrust', 'heading', 'moving')
def words(values):
    return struct.pack('<'+'H'*len(values), *(v & 65535 for v in values))
def state_bytes(state):
    return words([getattr(state, name) for name in FIELDS])
def blob(f, value):
    f.write(struct.pack('<I', len(value))); f.write(value)

NAMES = dict(back_buffer='BackBuffer', set_cursor_mode='CursorMode', set_viewport='Viewport', draw_layout='Layout',
    set_color='Color', set_text_position='TextPosition', fill_rect='Fill', flip_with_cursor='Flip', rand='Rand',
    load_system_objects='LoadSystem', init_system_encounters='InitEncounters', draw_flight_hud='FlightHud',
    hide_cursor='HideCursor', draw_system_view='SystemView', draw_thrust='Thrust', update_minimap_quadrant='UpdateMinimap',
    copy_display_rectangle='Copy', restore_display_rectangle='Restore', animate_jump_transition='Animate',
    encounter_icons='EncounterIcons', get_ticks='Ticks', set_palette_color='Palette')
def trace(events):
    lines = []
    for name, args, regs in events:
        if name in ('struct_copy', 'memcpy', 'is_program_loaded', 'draw_text', 'draw_labels', 'select_rect'):
            continue
        if name == 'text':
            lines.append('Text('+str(list(args[0]))+')')
        elif name == 'labels':
            labels = []
            for y, x, ptr, flag, value in args:
                labels.append(f'Label {{ y: {t.signed(y)}, x: {t.signed(x)}, text: {list(value)}, flags: {flag} }}')
            lines.append('Labels(['+', '.join(labels)+'])')
        elif name == 'rectangles':
            data, keys = args
            rects = [list(struct.unpack_from('<4h', data, i)) for i in range(0, len(data), 8)]
            lines.append(f'Select {{ rects: {rects}, initial: 0, highlight: 6, background: 9, keys: {list(keys)} }}')
        else:
            title = NAMES[name]
            lines.append(title + ('('+', '.join(str(t.signed(a)) for a in args)+')' if args else ''))
    return ('\n'.join(lines)+'\n').encode()

class Export(t.Suite):
    def check(self, address, model, memory, args=(), config=None, **kwargs):
        state = jump.State(memory, self.machine.dgroup)
        for i in range(20): state.pbyte(0xAC+i, 0)
        for i, program in enumerate(sorted(config['programs'])): state.pbyte(0xAC+i, program | 0x20)
        d = state.base
        f.write(memory[0x50000:0x50000+0xF86]); f.write(state_bytes(state))
        f.write(memory[d+0x178:d+0x190]); f.write(memory[d+0x3B1:d+0x3C1])
        f.write(words([config['choice'], *config['random_values'], config['view_result'],
            config['view_changes']['heading'], config['view_changes']['thrust']]))
        services = t.Script(memory, self.machine.dgroup, config)
        model(state, services)
        f.write(memory[0x50000:0x50000+0xF86]); f.write(state_bytes(state))
        f.write(words([services.draw])); blob(f, trace(services.trace))
        self.counts['jump'] += 1
        if self.counts['jump'] % 32 == 0: gc.collect()

count = 0
with (out/'cases.bin').open('wb') as f:
    for seed in (0x5A5E1990, 0x19901003):
        suite = Export(seed, 1000)
        t.jump_cases(suite)
        count += suite.counts['jump']
print(f'exported {count} jump.py cases', flush=True)

with (out/'animation.bin').open('wb') as f:
    for ticks in (0, 65534, 0xFFFF0000):
        for period in (1, 2, 5):
            for delta in (1, 7):
                memory, state = suite.fixture()
                state.heading = 97
                config = dict(seed=1, ticks=ticks, clock_period=period, tick_step=delta)
                services = t.Script(memory, suite.machine.dgroup, config)
                jump.animate_jump_transition(state, services)
                f.write(struct.pack('<IIIh', ticks, period, delta, state.view_heading))
                f.write(struct.pack('<I', services.clock_calls)); blob(f, trace(services.trace))
    memory, state = suite.fixture()
    state.heading = 97
    services = t.Script(memory, suite.machine.dgroup, dict(seed=1))
    times = [v for frame in range(100) for v in ((0xFFFFFFFF, 1) if frame == 60 else (frame*4, frame*4+3))]
    def clock(memory, args, registers):
        value = times[services.clock_calls]; services.clock_calls += 1
        return {'AX': value & 65535, 'DX': value >> 16}
    services.handlers['get_ticks'] = clock
    jump.animate_jump_transition(state, services)
    f.write(struct.pack('<IIIhI', 0, 0, 0, state.view_heading, services.clock_calls))
    blob(f, trace(services.trace))
print('exported 19 animation clock/call traces', flush=True)

from render_s_cg_support.s_screens import Scene
from render_s_cg import Renderer, SpaceState, jump_screen
scene = Scene()
rng = random.Random(991)
start = rng.randbytes(4*65536)
(out/'frame-start.bin').write_bytes(start)
settings = [(0,2,(3,4)), (86,2,(3,4)), (87,2,(3,4)), (102,2,(3,4)), (103,2,(3,4)),
    (255,2,(3,4)), (124,0,(3,4)), (124,2,()), (124,2,(3,)), (124,2,(9,4))]
rows = []
for system in range(8):
    for page in range(2):
        for fuel, drive, programs in settings:
            r = Renderer([bytearray(start[p*65536:(p+1)*65536]) for p in range(4)])
            s = SpaceState(system, fuel, drive, frozenset(programs), page)
            jump_screen(r, scene.layouts, scene.font, scene.widths, scene.desc, s, scene.names, scene.links)
            tag = str(len(rows))
            (out/(tag+'.planes')).write_bytes(r.snapshot())
            rows.append(f'{tag} {system} {fuel} {drive} {page} '+','.join(map(str,programs)))
(out/'frames.txt').write_text('\n'.join(rows)+'\n')
(out/'font-widths.bin').write_bytes(bytes(scene.widths))
(out/'font-flags.bin').write_bytes(bytes(8 if c in scene.desc else 0 for c in range(256)))
print(f'exported {len(rows)} render_s_cg jump frames', flush=True)
