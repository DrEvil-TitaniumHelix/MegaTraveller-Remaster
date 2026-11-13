"""Original 10666 frames at 161EE, checked against the independent renderer."""
import sys,importlib.util,os
from pathlib import Path
sys.dont_write_bytecode=True
oracle,game,out=map(lambda p:Path(p).resolve(),sys.argv[1:4]);out.mkdir(parents=True,exist_ok=True)
sys.path[:0]=[str(oracle/'tools/spec'),str(oracle/'tools')]
def readonly(event,args):
    if event=='open' and isinstance(args[0],(str,bytes,os.PathLike)):
        mode,flags=args[1:3]
        if (mode and any(c in mode for c in 'wax+')) or flags&(os.O_WRONLY|os.O_RDWR|os.O_CREAT|os.O_TRUNC):
            p=Path(os.fsdecode(args[0])).resolve()
            if p.is_relative_to(oracle) or p.is_relative_to(game):raise RuntimeError('read-only source: '+str(p))
sys.addaudithook(readonly)
s=importlib.util.spec_from_file_location('test_render_t',oracle/'tools/spec/test_render_t.py')
t=importlib.util.module_from_spec(s);t.FIXTURE_GAME=game;sys.modules[s.name]=t
source=Path(s.origin).read_text().replace("WORK = ROOT / 'work/render_t'",'WORK = Path('+repr(str(out/'oracle-artifacts'))+')')
source=source.replace("ROOT/'orig/dos/MegaTra1'",'FIXTURE_GAME').replace("ROOT/'orig/dos/MegaTra1/DEFAULT.SAV'","FIXTURE_GAME/'DEFAULT.SAV'").replace("ROOT/'orig/dos/MegaTra1/TRAV.ITM'","FIXTURE_GAME/'TRAV.ITM'")
exec(compile(source,s.origin,'exec'),t.__dict__)
from render_t import Assets
v=t.world_vm();t.load_layout(v,'STPT',0x6DF1);v.word(0x7000,0);snap=t.Snapshot(v);assets=Assets(game)
(out/'segments.txt').write_text(' '.join(map(str,sorted(set(v.blocks)|{v.ds,0x9000,0x9100}))))
for mode in range(3):
    for remembered in (0,7):
        snap.restore(v);v.byte(0x6E2B,remembered)
        names=['Heading','food ^5$','Fuel ^50$','supplies ^100$','tools ^20$','armor ^30$','weapon ^40$']
        prices=[0,5,50,100,20,30,40];colours=[6,1,2,3,4,5,6]
        args=t.shop_fixture(v,'Supplies for purchase:',names,prices,3,1000,0,mode,colours=colours)
        d=out/f'{mode}-{remembered}';d.mkdir(exist_ok=True)
        (d/'memory.bin').write_bytes(v.rb(0,0xA0000));(d/'ds.txt').write_text(str(v.ds))
        (d/'start.bin').write_bytes(b''.join(v.ega.planes)+bytes(v.ega.palette))
        r=t.model(v,assets);r.target=r.front;r.shop_screen('Supplies for purchase:',names,prices,3,1000,0,colours=colours)
        t.call_until(v,0x10666,args,0xFF6,0x161EE);t.compare(v,r,f'shop {mode}-{remembered}')
        (d/'end-0.bin').write_bytes(b''.join(v.ega.planes)+bytes(v.ega.palette))
        current=remembered-1 if remembered else 1
        for quantity in (1,2):
            v.press(0x4E);assert v.run_to(0x107D3,max_ticks=2000),v.stop_reason
            quantities=[0]*len(names);quantities[current]=quantity
            r.shop_screen('Supplies for purchase:',names,prices,3,1000-prices[current]*quantity,0,colours=colours,current=current,first=current//3*3,quantities=quantities,initial=False)
            assert v.run_to(0x161EE,max_ticks=2000),v.stop_reason;t.compare(v,r,f'quantity {mode}-{remembered}-{quantity}')
            (d/f'end-{quantity}.bin').write_bytes(b''.join(v.ega.planes)+bytes(v.ega.palette))
print('PASS 18 original shop/quantity frames')
