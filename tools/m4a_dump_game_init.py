"""78B2 cold-start fixtures from original instructions, streamed outside oracle.

Four EGA cases: intro/continuation, each with/without the mouse driver. Stop at
6923, before any simulation. At most seven run slices and 1120 ticks per case.
"""
import importlib.util
import json
import struct
import sys
from pathlib import Path

sys.dont_write_bytecode = True
root = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('gate', root/'tests/faithful/walk_golden.py')
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
oracle, output = map(Path, sys.argv[1:3])
landing = '--landing' in sys.argv[3:]
output.mkdir(parents=True, exist_ok=True)
ts, _, frames, _ = gate.load_oracle(oracle)
with gate.oracle_reads(oracle):
    for case in range(4):
        mouse, intro = bool(case & 1), bool(case & 2) and not landing
        directory = output/str(case)
        directory.mkdir(exist_ok=True)
        party = bytearray((oracle/'orig/dos/MegaTra1/DEFAULT.SAV').read_bytes())
        party[0xC1] = (party[0xC1] & 127) | (128 if intro else 0)
        if landing:
            party[6], party[7] = (2 if case < 2 else 3), 0
            party[0xFC] = 61
            party[0xF84:0xF86] = (37).to_bytes(2, 'little')
        t = ts.TSession(party=bytes(party), tail=' 9000 0000 9100 0000 '+('0nBn0' if landing else '0nAn0'), vm_class=frames.PlayedVM, mouse=mouse)
        v = t.vm
        if landing:
            v.wb(ts.SEG_A*16, party)
            items = bytearray((oracle/'orig/dos/MegaTra1/TRAV.ITM').read_bytes()[2:])
            for base in range(0, len(items), 46):
                for at in range(base, base+46):
                    if items[at] == 0: break
                    if 48 <= items[at] <= 57: items[at] += 128
            v.wb(0x91000, items)
            (directory/'input-items.bin').write_bytes(items)
            v.exec_handler = lambda name, tail: (_ for _ in ()).throw(AssertionError(f'mode B executed {name}'))
        v.root = str(oracle/'orig/dos/MegaTra1')
        v.scan_gap = 4
        v.hook(0x2037F, lambda v: v.w16(v.lin(v.r('SS'), v.r('SP')+4), 0 if landing else 1))
        checkpoints = []
        class Frames(frames.FrameChecker):
            def on(self, a):
                before = {id(fr) for fr in self.stack}
                super().on(a)
                if self.stack and id(self.stack[-1]) not in before:
                    fr = self.stack[-1]
                    checkpoints.append(('enter_' + ('redraw_view' if fr['name'] == 'redraw_view+cursor' else fr['name']), fr['tick']))

            def finish(self, fr):
                super().finish(fr)
                assert not self.fails, self.fails[0]
                if not fr['skip']:
                    checkpoints.append(('redraw_view' if fr['name'] == 'redraw_view+cursor' else fr['name'], v.ticks))
        Frames(v)
        for address, name in [(0x05C3,'show_message'), (0x1CAB8,'tileset_loaded')]:
            v.hook(address, lambda v, name=name: checkpoints.append((name,v.ticks)))
        def stop(v):
            checkpoints.append(('main_loop',v.ticks))
            v._stop('game-init-return')
        v.hook(0x6923,stop)
        reason = t.run(400)
        for _ in range(6):
            if reason == 'game-init-return': break
            assert reason == 'max_ticks', reason
            v.press(0x1C)
            reason = t.run(120)
        assert reason == 'game-init-return', (case,reason)
        (directory/'party.bin').write_bytes(party)
        (directory/'end-party.bin').write_bytes(t.block())
        (directory/'end.bin').write_bytes(gate.frame_bytes(v))
        (directory/'end-ds.bin').write_bytes(v.rb(t.ds*16,65536))
        off,seg=struct.unpack('<HH',v.rb(t.ds*16+0x6D6D,4))
        count=int.from_bytes((oracle/'orig/dos/MegaTra1/TRAV.ITM').read_bytes()[:2],'little')
        (directory/'items.bin').write_bytes(v.rb(seg*16+off,count*46))
        for name,data in gate.state_buffers(v.rb(t.ds*16,65536),v.rb).items():
            (directory/name).write_bytes(data)
        (directory/'script.txt').write_text(''.join(f'at {name} {tick}\n' for name,tick in checkpoints)+'ticks 400\n'+'key 13\nticks 120\n'*6)
        (directory/'meta.json').write_text(json.dumps(dict(mouse=mouse,intro=intro,checkpoints=checkpoints,boot_clock=[c for c in checkpoints if c[0] in ('show_message','tileset_loaded','main_loop')])))
        print(f'PASS original 78B2 case={case} mouse={mouse} intro={intro} tick={v.ticks}',flush=True)
