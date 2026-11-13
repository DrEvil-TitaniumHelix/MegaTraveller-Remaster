"""Original-instruction 10164 fixtures, generated from BYO files, never shipped."""
import sys
sys.dont_write_bytecode=True
import importlib.util
import os
from pathlib import Path
import struct

oracle,game,out=map(lambda s:Path(s).resolve(),sys.argv[1:4]);out.mkdir(parents=True,exist_ok=True)
sys.path[:0]=[str(oracle/'tools/spec'),str(oracle/'tools')]
def readonly(event,args):
    if event=='open' and isinstance(args[0],(str,bytes,os.PathLike)):
        mode,flags=args[1:3]
        if (mode and any(c in mode for c in 'wax+')) or flags&(os.O_WRONLY|os.O_RDWR|os.O_CREAT|os.O_TRUNC):
            p=Path(os.fsdecode(args[0])).resolve()
            if p.is_relative_to(oracle) or p.is_relative_to(game):raise RuntimeError('read-only source: '+str(p))
sys.addaudithook(readonly)
spec=importlib.util.spec_from_file_location('test_render_t',oracle/'tools/spec/test_render_t.py')
trt=importlib.util.module_from_spec(spec);trt.FIXTURE_GAME=game;sys.modules[spec.name]=trt
source=Path(spec.origin).read_text().replace("WORK = ROOT / 'work/render_t'",'WORK = Path('+repr(str(out/'oracle-artifacts'))+')')
source=source.replace("ROOT/'orig/dos/MegaTra1'",'FIXTURE_GAME').replace("ROOT/'orig/dos/MegaTra1/DEFAULT.SAV'","FIXTURE_GAME/'DEFAULT.SAV'").replace("ROOT/'orig/dos/MegaTra1/TRAV.ITM'","FIXTURE_GAME/'TRAV.ITM'")
exec(compile(source,spec.origin,'exec'),trt.__dict__)
vm=trt.world_vm();trt.load_layout(vm,'STPT',0x6DF1)
for off,v in [(0x6DF9,104),(0x6DF5,67),(0x6E02,122),(0x6DFE,112)]:vm.word(off,v)
vm.byte(0x7011,0);vm.call(0xF56C,far=True)
base=trt.Snapshot(vm);names=[]
(out/'segments.txt').write_text(' '.join(map(str,sorted(set(vm.blocks)|{vm.ds,0x9000,0x9100}))))
for world in (0,2,7,15):
    for seed in (0,0x7EE70001):
        base.restore(vm);vm.byte(0x5E14,world);vm.wb(vm.ds*16+0x5C20,struct.pack('<I',seed))
        name=f'panel-{world}-{seed}';d=out/name;d.mkdir(exist_ok=True);names.append(name)
        (d/'memory.bin').write_bytes(vm.rb(0,0xA0000));(d/'ds.txt').write_text(str(vm.ds))
        (d/'start.bin').write_bytes(b''.join(vm.ega.planes)+bytes(vm.ega.palette))
        vm.call(0x10164,seg=0xFF6)
        (d/'end.bin').write_bytes(b''.join(vm.ega.planes)+bytes(vm.ega.palette))
        (d/'rng.bin').write_bytes(vm.rb(vm.ds*16+0x5C20,4))
        print(name,flush=True)
(out/'manifest.txt').write_text('\n'.join(names)+'\n')

base.restore(vm);vm.call(0x104C7,(2,),far=True,seg=0xFF6);vm.call(0xF50B,far=True)
vm.wb(0x90000+0x1CE,bytes(46));vm.w16(0x90000+0x1CE+12,1)
vm.wb(0x90000+0xA82,bytes([0,50,0,0,0]))
d=out/'ammo-label';d.mkdir(exist_ok=True)
(d/'memory.bin').write_bytes(vm.rb(0,0xA0000));(d/'ds.txt').write_text(str(vm.ds))
(d/'start.bin').write_bytes(b''.join(vm.ega.planes)+bytes(vm.ega.palette))
trt.call_until(vm,0x1752A,(65534,0,30000,0,0),0xFF6,0x161EE)
(d/'end.bin').write_bytes(b''.join(vm.ega.planes)+bytes(vm.ega.palette))
