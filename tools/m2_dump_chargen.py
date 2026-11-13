import sys,os
sys.dont_write_bytecode=True
import m2_dump_common as C
import shutil
C.setup()
shutil.rmtree(C.OUT,ignore_errors=True);os.makedirs(C.OUT,exist_ok=True)
sys.path.insert(0,os.path.join(C.ORACLE,'tools/golden'))
os.chdir(C.ORACLE)
from render_s_cg import Renderer
from render_s_cg_support.cg_screens import Observer
from cg_golden import Original,make_script
SEED,CHOICES=4711,1
REGS=[0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23]

class Rec(Renderer):
    """render_s_cg.Renderer that records its outer calls (the Observer's), not the primitives those expand into."""
    def __init__(s):super().__init__();s.rec=[];s.nest=0;s.kinds={};s.tables=None
    def _c(s,*a):
        if s.nest==0:
            assert s.page%8192==0,s.page
            s.rec.append([a[0],s.page//8192,*a[1:]])
    def _run(s,f,*a):
        s.nest+=1
        try:return f(*a)
        finally:s.nest-=1
    def layout(s,entries,index,y=0,x=0):s._c('layout',s.kinds[id(entries)],index,y,x,s.color);return s._run(super().layout,entries,index,y,x)
    def text(s,font,widths,value,y,x,color,shadow=None,descenders=()):
        t=(bytes(widths),bytes(int(c in descenders) for c in range(256)))
        assert s.tables in (None,t),'font tables changed during the session';s.tables=t
        s._c('text',bytes(value),y,x,color,-1 if shadow is None else shadow);return s._run(super().text,font,widths,value,y,x,color,shadow,descenders)
    def fill(s,y,x,h,w,color):s._c('fill',y,x,h,w,color,s.operation);return s._run(super().fill,y,x,h,w,color)
    def line(s,y1,x1,y2,x2,color):s._c('line',y1,x1,y2,x2,color,s.operation);return s._run(super().line,y1,x1,y2,x2,color)
    def pixel(s,y,x,color):s._c('pixel',y,x,color,s.operation);return s._run(super().pixel,y,x,color)

original=Original(SEED);obs=Observer(original.vm);r=obs.renderer=Rec();r.kinds={id(obs.font):'font',id(obs.cg):'cg'}
choose=make_script(CHOICES);rec=original.step();n=0;start=[bytearray(p) for p in r.planes]
while rec['stop']=='input exhausted' and n<400:
    obs.check(f'input {n}')
    case=C.Case(f'frame_{n:02}',start,REGS)
    if r.tables:case.call('font',*r.tables)
    for c in r.rec:case.call(*c)
    case.write(r.planes,REGS);r.rec=[];start=[bytearray(p) for p in r.planes]
    keys=choose(rec,n,original)
    if keys=='stop':break
    if keys is None:keys=[13]
    original.vm.keys.extend(keys[:-1]);rec=original.step(keys[-1]);n+=1
assert not obs.active and not obs.unknown_writes
obs.check('session end')
print('FRAMES',n+1 if rec['stop']=='input exhausted' else n,'phases',dict(obs.frames),'calls',dict(obs.calls))
