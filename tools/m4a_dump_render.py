"""Stream T renderer fixtures to disk (all planes + registers, real Efate state).

Usage: m4a_dump_render.py ORACLE GAMEFILES OUTPUT. No writes or cwd in the oracle.
The original boot is capped at 14,000 ticks; the parent enforces wall/RSS limits.
Expected pixels are computed by the independent Python renderers, from entry RAM.
"""
import sys
sys.dont_write_bytecode = True
import importlib.util
import json
import os
import random
from pathlib import Path
import struct

oracle, game, out = map(lambda p: Path(p).resolve(), sys.argv[1:4])
out.mkdir(parents=True, exist_ok=True)
sys.path[:0] = [str(oracle/'tools/spec'), str(oracle/'tools')]
spec = importlib.util.spec_from_file_location('test_render_t', oracle/'tools/spec/test_render_t.py')
trt = importlib.util.module_from_spec(spec)
trt.FIXTURE_GAME=game
sys.modules[spec.name] = trt
source = Path(spec.origin).read_text().replace("WORK = ROOT / 'work/render_t'", 'WORK = Path('+repr(str(out/'oracle-artifacts'))+')')
source=source.replace("ROOT/'orig/dos/MegaTra1'",'FIXTURE_GAME').replace("ROOT/'orig/dos/MegaTra1/DEFAULT.SAV'","FIXTURE_GAME/'DEFAULT.SAV'").replace("ROOT/'orig/dos/MegaTra1/TRAV.ITM'","FIXTURE_GAME/'TRAV.ITM'")
exec(compile(source, spec.origin, 'exec'), trt.__dict__)
from render_t import Assets, Renderer, State
from render_t_cursor import CursorVideo, SelectionBlink
from render_t_menu import MenuVideo
from render_t_calls import CallVideo
from render_t_services import ServiceSurface
from combat import Far

def readonly(event, args):
    if event == 'open' and isinstance(args[0], (str, bytes, os.PathLike)):
        mode, flags = args[1:3]
        if (mode and any(c in mode for c in 'wax+')) or flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC):
            path = Path(os.fsdecode(args[0])).resolve()
            if path.is_relative_to(oracle) or path.is_relative_to(game):
                raise RuntimeError('oracle/gamefiles write rejected: '+str(path))
sys.addaudithook(readonly)
assets = Assets(game)
vm = trt.world_vm()
memory = vm.rb(0, 0xA0000)
ds = vm.ds
planes = tuple(bytes(p) for p in vm.ega.planes)
palette = vm.ega.palette[:]
dac = vm.ega.dac[:]
count = 0
names = []
(out/'segments.txt').write_text(' '.join(map(str, sorted(set(vm.blocks) | {ds, 0x9000, 0x9100}))))

def create(kind=CursorVideo):
    return kind(assets, memory, ds, planes, palette, dac, vm.ega.display_page)

def snapshot(m): return b''.join(m.planes)+bytes(m.palette)

def emit(name, model, operations, body, state=False):
    global count
    count += 1
    names.append(name)
    assert count <= 256, 'fixture step cap'
    folder = out/name
    folder.mkdir(exist_ok=True)
    (folder/'start.bin').write_bytes(snapshot(model))
    (folder/'memory.bin').write_bytes(model.memory)
    (folder/'ds.txt').write_text(str(ds))
    (folder/'calls.json').write_text('[\n'+',\n'.join(json.dumps(op) for op in operations)+'\n]\n')
    body(model)
    (folder/'end.bin').write_bytes(snapshot(model))
    if state:
        fields = ([(a,2) for a in (0x700C,0x700E,0x6FF8,0x6FFC,0x6FFE,0x52F4,0x52B3,0x503A,0x503C,0x7198)] + [(a,1) for a in (0x7010,0x7011,0x7012)]) if state is True else state
        (folder/'state.txt').write_text('\n'.join(f'{a} {w} {int.from_bytes(model.memory[ds*16+a:ds*16+a+w],"little")}' for a,w in fields))
    elif (folder/'state.txt').exists(): (folder/'state.txt').unlink()
    print('CASE', name, flush=True)

def renderer_main(m, clear):
    page = m.word(0x52F4)
    r = Renderer(assets, State(m.memory, ds), m.pixels(page*8192), m.pixels((page^1)*8192))
    r.main_screen(clear)
    for page, surface in ((page,r.front),(page^1,r.back)):
        for p,b in zip(m.planes,surface.planar()): p[page*8192:page*8192+8000] = b
    m.palette = r.palette

for page in (0,1):
    for clear in (0,1):
        m=create();m.word(0x52F4,page);m.back()
        emit(f'main-{page}-{clear}',m,[['main',clear]],lambda m:renderer_main(m,clear))
    for row,col in ((0,0),(1,0),(0,1),(1,1),(10,20),(11,21)):
        for phase in (0,1,2):
            m=create();m.word(0x52F4,page);m.back();m.word(0x6415,row);m.word(0x6266,col)
            m.word(0x6B6C,row+6);m.word(0x63F5,col+10);m.word(0x641A,phase);m.word(0xE1,(row+col+phase)%8)
            m.word(0x6B73,65535);m.word(0x6413,65535)
            emit(f'efate-{page}-{row}-{col}-{phase}',m,[['ticks',100],['redraw']],lambda m:m.redraw(100))

for n,text in enumerate((b'Hello, Efate.',b'A sign nearby says: "No littering in the Starport area! Oxygen supplied by SuSAG, LIC MegaCorp."', b'Word '*60)):
    for inhibit in (0,1):
        m=create(CallVideo);m.byte(0x6C3C,inhibit)
        emit(f'message-{n}-{inhibit}',m,[['ticks',100],['message',text.hex()]],lambda m:m.help_message(text))

m=create(CallVideo);m.byte(0x5169,255);m.byte(0x713D,ord('A'))
m.word(0x5228,30);m.word(0x522A,60);m.byte(0x6C3C,0)
emit('message-wide-word',m,[['message_repeat',ord('A'),8_421_505,'2042']],
     lambda m:m.help_message(b'A'*8_421_505+b' B'),[(0x7139,2),(0x713B,2)])

for n in range(8):
    m=create();row,col=n%2,(n//2)%2
    m.word(0x6415,row);m.word(0x6266,col);m.word(0x6B73,65535);m.word(0x6413,65535)
    cell=m.word(0xBC)+m.word(0xBE)*16+((n%7)*m.word(0x6D2B)+n%12)*3
    m.memory[cell]=3;m.memory[cell+2]=0xC1
    objects=m.word(0x6B6E)+m.word(0x6B70)*16
    m.memory[objects+9+2]=1;m.memory[objects+9+3]=48
    m.byte(0x6422+8,3);m.word(0x6422+32,n%2);m.word(0x6422+34,(n//2)%2)
    m.byte(0x6422+36,n);m.byte(0x6422+40,n%3-1)
    emit(f'overlays-{n}',m,[['ticks',100],['redraw']],lambda m:m.redraw(100))

for n,tick in enumerate((108,109,110,0xFFFFFFFF,0)):
    m=create();m.word(0x7000,0);m.byte(0x7011,0)
    rectangles=[tuple(struct.unpack_from('<4h',m.memory,ds*16+0x480B+i*8)) for i in range(3)]
    def blink(m,tick=tick):
        s=SelectionBlink(m,rectangles,0,6,7,100);s.poll(tick);s.choose(2,tick);s.poll((tick+10)&0xFFFFFFFF)
    emit(f'selection-{n}',m,[['ticks',100],['blink_start',0x480B,3,0,6,7],['ticks',tick],['blink_poll'],['blink_choose',2],['ticks',(tick+10)&0xFFFFFFFF],['blink_poll']],blink)
for flag in (0,1):
    for inhibit in (0,1):
        m=create(CallVideo);m.byte(0x6C3C,inhibit)
        emit(f'dismiss-{flag}-{inhibit}',m,[['dismiss',flag]],lambda m:m.dismiss_message(flag))

for index,offset in enumerate(range(0x427A,0x480B,75)):
    for top in (0,1):
        m=create(MenuVideo);m.byte(0x6C3C,top);page=index%2;m.word(0x52F4,page);m.back()
        emit(f'menu-{offset:04x}-{top}',m,[['ticks',100],['menu',offset,ds],['present_menu']],lambda m,offset=offset:(m.draw_menu(Far(offset,ds)),m.present_menu()))

for n,(x,y) in enumerate(((0,0),(7,46),(155,98),(311,192),(319,199),(-1,-1))):
    for front in (0,1):
        m=create();m.word(0x7000,65535);m.byte(0x7011,0);m.byte(0x7012,0)
        if front:m.front()
        else:m.back()
        m.sample=(0,x*2,y)
        def act(m):
            m.show();m.show();m.sample=(1,200,80);m.track();m.flip();m.hide();m.hide()
        emit(f'cursor-{n}-{front}',m,[['mouse',x,y,0],['show'],['show'],['mouse',100,80,1],['track'],['flip'],['hide'],['hide']],act,True)

for phase in (1,2):
    for tick in (100,109,110):
        m=create();m.word(0x6D0E,1);m.byte(0x6D11,0);m.byte(0x6C42,phase);m.dword(0x6D19,109)
        emit(f'target-{phase}-{tick}',m,[['ticks',tick],['redraw']],lambda m,tick=tick:m.redraw(tick))

for n in range(4):
    m=create();m.word(0x7000,65535);m.byte(0x7011,0);m.byte(0x7012,n%2)
    if n>=2:
        rng=random.Random(4711+n);m.planes=[bytearray(rng.randbytes(65536)) for _ in range(4)]
    m.word(0x6B73,65535);m.word(0x6413,65535)
    m.sample=(0,300,20);m.show();m.sample=(0,100,100)
    emit(f'mouse-redraw-{n}',m,[['mouse',50,100,0],['ticks',100],['redraw']],lambda m:m.redraw(100),True)

for person,room in ((1,0),(24,7),(54,-1),(0,3),(30,2)):
    m=create();page=m.word(0x52F4)^1
    def portrait(m,person=person,room=room):
        r=Renderer(assets,State(m.memory,ds),m.pixels((page^1)*8192),m.pixels(page*8192));r.shop_portrait(person,room)
        for p,b in zip(m.planes,r.back.planar()):p[page*8192:page*8192+8000]=b
    emit(f'portrait-{person}-{room}',m,[['portrait',person,room]],portrait)

def intro_expected(m,index):
    if index==0:
        for a,v in ((0x6E0A,2),(0x6E0E,57),(0x6E08,0),(0x6E00,192),(0x6E06,141),(0x6E04,15)):m.word(a,v)
        m.back()
    if index in (0,4):
        r=m.painter()
        if index==0:r.ly(1,54,0,'GROUND')
        r.target.fill(55,192,146,128,7)
        surface=ServiceSurface(m.planes,m.video_offset(0x503A))
        pixels=assets.pictures['START']
        surface.blit(pixels,60,200,0,112 if index==4 else 0,pixels.shape[0],pixels.shape[1]//16*8)
        m.finish_paint(r);m.present(54,0,146,40)
    m.say_message(index,0)

cspec=importlib.util.spec_from_file_location('test_render_t_cursor',oracle/'tools/spec/test_render_t_cursor.py')
tc=importlib.util.module_from_spec(cspec);sys.modules[cspec.name]=tc
csource=Path(cspec.origin).read_text().replace("WORK=ROOT/'work/render_t_cursor'",'WORK=Path('+repr(str(out/'oracle-artifacts-cursor'))+')')
exec(compile(csource,cspec.origin,'exec'),tc.__dict__)
vm=tc.boot()
vm.irq_gate=lambda:False
def onward(stop):
    vm.mu.emu_start(vm.lin(vm.r('CS'),vm.r('IP')),0x100000,count=1)
    assert vm.run_to(stop,max_ticks=2000),vm.stop_reason
for i in range(5):
    if i:
        returned=vm.r16(vm.lin(vm.r('SS'),vm.r('SP')))
        onward(0xD506);vm.byte(0x6FE4,0x1C);onward(returned)
    m=CallVideo(assets,vm.rb(0,0xA0000),ds,vm.ega.planes,vm.ega.palette,vm.ega.dac,vm.ega.display_page)
    if i==0:
        sp=(vm.r('SP')-2)&65535;vm.w16(vm.lin(vm.r('SS'),sp),vm.load_seg)
        sp=(sp-2)&65535;vm.w16(vm.lin(vm.r('SS'),sp),0xFFF8)
        vm.w('SP',sp);vm.w('CS',vm.load_seg);vm.w('IP',0x7651)
        assert vm.run_to(0xD2C3,max_ticks=2000),vm.stop_reason
    else:
        onward(0xD2C3)
    emit(f'intro-{i}',m,[['intro',i]],lambda m,i=i:intro_expected(m,i))
    actual=b''.join(vm.ega.planes)+bytes(vm.ega.palette)
    expected=snapshot(m)
    assert actual==expected,('intro spec/original',i,next((j for j,(a,b) in enumerate(zip(actual,expected)) if a!=b),None))

(out/'manifest.txt').write_text('\n'.join(names)+'\n')
print(f'PASS m4a_dump_render: {count} complete real-file cases; 4 planes + regs',flush=True)
