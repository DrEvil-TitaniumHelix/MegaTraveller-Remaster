import os,sys,random
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import m2_dump_common as C
G=C.setup()
import numpy as np
from render_t import Surface
from render_s_cg import Renderer,copy_video
from render_palette import Palette,indexed
OPS=(0,8,16,24);T={'pixel':'apply','rect':'rectangle'};S={'rect':'rectangle'}

class Fix:
    def __init__(self,name,seed):
        self.planes=C.rand_planes(seed);self.regs=list(range(8))+list(range(16,24));self.c=C.Case(name,self.planes,self.regs)
    def t(self,page,f):
        s=Surface(indexed(self.planes,page));f(s)
        for p,d in enumerate(s.planar()):self.planes[p][page*8192:page*8192+8000]=d
    def prim(self,name,r,page,*a):
        geom,c,op=a[:-2],a[-2],a[-1];self.c.call(name,r,page,*a)
        if r=='clip':self.t(page,lambda s:getattr(s,T.get(name,name))(*geom,c,op))
        else:getattr(Renderer(planes=self.planes,page=page*8192,operation=op),S.get(name,name))(*geom,c)
    def blit(self,page,src,sw,y,x,sy,sx,h,w,tr):
        self.c.call('blit',page,bytes(src),sw,y,x,sy,sx,h,w,tr)
        a=np.frombuffer(bytes(src),dtype=np.uint8).reshape(-1,sw) if sw else np.zeros((1,0),np.uint8)
        self.t(page,lambda s:s.blit(a,y,x,sy,sx,h,w,None if tr<0 else tr))
    def copy(self,src,sy,sx,h,w,dst,dy,dx):
        self.c.call('copy',src,sy,sx,h,w,dst,dy,dx);copy_video(Renderer(planes=self.planes),src*8192,sy,sx,h,w,dst*8192,dy,dx)
    def copyrect(self,src,dst,y,x,h,w):
        self.c.call('copyrect',src,dst,y,x,h,w);other=indexed(self.planes,src);self.t(dst,lambda s:s.blit(other,y,x,y,x,h,w))
    def reg(self,i,v):
        self.c.call('reg',i,v);p=Palette(registers=self.regs);p.bios(0x1000,bx=(i&255)|((v&255)<<8));self.regs=p.registers
    def done(self):self.c.write(self.planes,self.regs)

def both(f):
    for r in ('clip','wrap'):f(r)
def pixel_edges(r):
    x=Fix(f'pixel_edges_{r}',1)
    for i,(y,px) in enumerate([(0,0),(199,319),(200,0),(0,320),(-1,5),(5,-1),(0,-1),(-1,0),(0,65535),(1700,0),(0,-65536),(100,8191)]):
        x.prim('pixel',r,i%2,y,px,0x3a if i%3 else 7,OPS[i%4])
    for op in OPS:x.prim('pixel',r,7,204,200,15,op)
    x.done()
def line_h(r):
    x=Fix(f'line_h_{r}',2)
    for i,(y,x1,x2,c) in enumerate([(10,0,7,0x12),(11,0,7,0x12),(12,3,4,0x21),(13,5,12,0x21),(14,0,319,0x9e),(15,319,0,0x9e),(16,300,330,0x45),(17,-10,10,0x45),(18,8,15,5),(19,16,16,0x12),(20,17,17,0x12)]):
        x.prim('line',r,1,y,x1,y,x2,c,OPS[i%4])
    x.prim('line',r,0,199,0,199,319,0xf0,0);x.prim('line',r,0,200,0,200,319,0x0f,0)
    x.done()
def line_v(r):
    x=Fix(f'line_v_{r}',3)
    for i,(y1,y2,px,c) in enumerate([(0,199,0,0x12),(199,0,319,3),(190,210,5,0x34),(-5,5,8,9),(50,50,20,0x12),(51,51,21,0x12),(10,20,320,6),(10,20,-1,6)]):
        x.prim('line',r,i%2,y1,px,y2,px,c,OPS[i%4])
    x.done()
def line_diag(r):
    x=Fix(f'line_diag_{r}',4);cy,cx=100,160
    for i,(dy,dx) in enumerate([(30,50),(50,30),(-30,50),(-50,30),(30,-50),(50,-30),(-30,-50),(-50,-30),(40,40),(-40,40),(40,-40),(-40,-40),(1,100),(100,1),(-1,100),(-100,1)]):
        x.prim('line',r,i%2,cy,cx,cy+dy,cx+dx,0x12 if i%5 else 0xe3,OPS[i%4])
    for i,(y1,x1,y2,x2) in enumerate([(-10,-10,210,330),(210,330,-10,-10),(-20,300,220,20),(0,0,199,319),(199,319,0,0)]):x.prim('line',r,1,y1,x1,y2,x2,4+i,OPS[i%4])
    x.done()
def line_point(r):
    x=Fix(f'line_point_{r}',5)
    for i in range(8):x.prim('line',r,i%2,20+i,5+i,20+i,5+i,0x12,OPS[i%4])
    x.done()
def fill(r):
    x=Fix(f'fill_{r}',6)
    for i,(y,px,h,w,c) in enumerate([(5,5,0,10,3),(10,5,-2,10,3),(15,10,3,0,7),(20,10,3,1,0x12),(25,10,3,-3,5),(30,0,10,320,0x78),(190,300,20,40,0x9a),(-5,-5,15,15,6),(100,100,1,1,0x12),(101,101,1,1,0x21),(50,40,4,8,0xf),(60,41,4,7,0xe)]):
        x.prim('fill',r,i%2,y,px,h,w,c,OPS[i%4])
    x.prim('fill',r,7,195,250,10,100,9,0)
    x.done()
def rect(r):
    x=Fix(f'rect_{r}',7)
    for i,(y1,x1,y2,x2,c) in enumerate([(10,10,30,50,5),(30,50,10,10,5),(40,10,39,50,6),(45,10,45,50,0x12),(50,10,51,50,7),(60,20,60,20,0x12),(61,21,61,21,0x12),(-5,-5,205,325,9),(70,300,90,330,0x34),(100,100,110,110,0xa)]):
        x.prim('rect',r,i%2,y1,x1,y2,x2,c,OPS[i%4])
    x.prim('rect',r,0,120,20,140,60,0xf,24);x.prim('rect',r,0,120,20,140,60,0xf,24)
    x.done()
def blit():
    x=Fix('blit',8);rnd=random.Random(8);src=[rnd.choice([1,1,1,0,2,5,9,15]) for _ in range(24*15)]
    for i,(page,y,px,sy,sx,h,w,tr) in enumerate([(0,0,0,0,0,15,24,-1),(0,10,10,0,0,15,24,1),(1,-5,-7,0,0,15,24,-1),(1,190,300,0,0,15,24,1),(0,50,50,3,4,15,24,-1),(0,70,50,0,0,20,30,1),(0,90,50,0,0,-1,24,-1),(0,90,50,0,0,15,-2,-1),(1,100,-30,0,-10,15,24,-1),(1,100,10,0,-10,15,24,-1),(1,100,310,0,0,15,24,1),(0,195,0,10,0,15,24,-1),(0,120,100,0,0,0,0,-1)]):
        x.blit(page,src,24,y,px,sy,sx,h,w,tr)
    x.done()
def copy():
    x=Fix('copy',9)
    for a in [(2,0,0,16,2,1,12,1),(2,96,6,16,2,1,28,15),(1,100,16,16,2,2,171,0),(0,10,10,5,4,0,10,11),(0,20,10,5,4,0,20,9),(7,204,30,3,20,0,0,0),(0,0,0,3,20,7,204,30),(1,5,-3,4,6,1,50,-2),(0,0,0,0,5,1,0,0),(0,0,0,5,0,1,0,0)]:x.copy(*a)
    x.done()
def copyrect():
    x=Fix('copyrect',10)
    for a in [(1,0,0,0,200,320),(0,1,169,0,31,192),(0,1,54,0,112,192),(1,0,0,0,53,320),(1,0,190,300,20,40),(1,0,-5,-5,10,10)]:x.copyrect(*a)
    x.done()
def regs():
    x=Fix('regs',11)
    for i,v in enumerate([63,64,127,255,0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23,40,41,42]):x.reg(i%16,v)
    x.done()
def random_mix(r,seed):
    x=Fix(f'random_{r}_{seed}',seed);rnd=random.Random(seed)
    for _ in range(300):
        c=rnd.randrange(16) if rnd.random()<0.6 else rnd.randrange(16,256);op=rnd.choice(OPS);page=rnd.randrange(8)
        y,px=rnd.randrange(-30,230),rnd.randrange(-40,360)
        k=rnd.choice(('pixel','line','fill','rect','line','fill','blit','copy','copyrect'))
        if k=='blit':x.blit(page,[rnd.randrange(16) for _ in range(48)],8,y,px,rnd.randrange(-2,4),rnd.randrange(-2,6),rnd.randrange(-1,8),rnd.randrange(-1,10),rnd.choice((-1,-1,1,0)))
        elif k=='copy':x.copy(page,y,rnd.randrange(-3,42),rnd.randrange(0,20),rnd.randrange(0,6),rnd.randrange(8),rnd.randrange(-30,230),rnd.randrange(-3,42))
        elif k=='copyrect':x.copyrect(page,rnd.randrange(8),y,px,rnd.randrange(0,40),rnd.randrange(0,40))
        elif k=='pixel':x.prim(k,r,page,y,px,c,op)
        elif k=='fill':x.prim(k,r,page,y,px,rnd.randrange(-2,60),rnd.randrange(-2,120),c,op)
        else:x.prim(k,r,page,y,px,rnd.randrange(-30,230),rnd.randrange(-40,360),c,op)
    x.done()
def span_invalid_op():
    """S span with an op outside {0,8,16,24}: interior bytes XOR, end bytes replace; per-pixel paths replace."""
    x=Fix('line_h_wrap_invalid_op',13)
    for i,(y,x1,x2,c,op) in enumerate([(5,0,23,1,1),(6,0,23,0x12,23),(7,3,4,7,9),(8,8,31,0x3c,5),(9,-10,10,1,1),(10,8,8,9,1),(11,300,330,0xa5,2),(12,16,23,6,255)]):
        x.prim('line','wrap',i%2,y,x1,y,x2,c,op)
    x.prim('line','wrap',7,204,200,204,300,9,1);x.prim('fill','wrap',0,20,4,3,30,0x78,3);x.prim('rect','wrap',1,30,4,40,60,5,7)
    x.prim('pixel','wrap',0,50,50,5,3);x.prim('line','wrap',0,60,0,70,50,6,7);x.prim('line','wrap',0,60,80,60,80,0x12,1)
    x.done()
def extreme():
    """Argument extremes the oracle completes quickly (Rust must return and draw the same)."""
    M=2**31-1;m=-2**31;x=Fix('extreme',14)
    x.prim('line','clip',0,M,1,M,1,0x12,0);x.prim('line','clip',0,0,M-1,1,M,1,0);x.prim('line','clip',0,M-1,0,M,1,1,0)
    x.prim('fill','clip',0,M,0,2,1,1,0);x.prim('fill','clip',0,0,M,1,1,1,0);x.prim('fill','clip',0,0,m,1,0,1,0);x.prim('rect','clip',0,M-1,M-1,M,M,1,0)
    x.prim('pixel','wrap',0,M,M,5,0);x.prim('pixel','wrap',0,m,m,5,16);x.prim('line','wrap',0,M,0,M,10,3,0);x.prim('line','wrap',1,m,m,m,m+5,0x12,0)
    x.prim('line','wrap',1,m,M-3,m,M,6,0);x.prim('fill','wrap',0,M-1,-5,2,10,0x9a,24)
    x.blit(0,b'',0,0,0,0,0,1,0,-1);x.blit(0,b'',1,M,0,0,0,2,1,-1);x.blit(0,b'',1,0,0,M,0,2,1,-1);x.blit(0,b'',1,0,m,0,0,1,1,-1)
    x.blit(0,b'',1,0,m+1,0,0,1,1,-1);x.blit(0,b'',1,0,0,0,m,1,1,-1);x.blit(1,b'',1,5,5,0,0,2,M,-1);x.blit(1,b'',1,6,6,m,m,1,1,-1)
    x.copy(0,M,0,2,1,0,0,0);x.copy(0,0,M,1,2,0,0,0);x.copy(0,0,0,1,1,0,M,m);x.copy(0,m,m,2,2,7,M,M)
    x.copyrect(1,0,M,0,2,320);x.copyrect(1,0,0,m,1,5)
    x.reg(17,1);x.reg(16,5);x.reg(255,9)
    x.done()
def clip_page4():
    x=Fix('clip_page4',15)
    x.prim('fill','clip',4,0,0,200,320,0x12,0);x.prim('line','clip',4,-10,-10,210,330,5,24);x.prim('rect','clip',3,10,10,190,310,7,16);x.blit(5,bytes(range(16))*4,8,100,150,0,0,8,8,3)
    x.done()
def palette():
    import data_formats
    vals=data_formats.pal_dec(open(os.path.join(G,'MAP.PAL'),'rb').read())['vals']
    x=Fix('palette',12)
    for i,v in enumerate(vals[:16]):x.reg(i,v)
    p=Palette(registers=x.regs)
    for page in (0,1):x.c.extra[f'rgb{page}.bin']=p.rgb(indexed(x.planes,page)).tobytes()
    x.c.extra['colors.bin']=p.colors().tobytes()
    x.done()

for f in (pixel_edges,line_h,line_v,line_diag,line_point,fill,rect):both(f)
blit();copy();copyrect();regs();span_invalid_op();extreme();clip_page4()
for seed in (101,102):
    for r in ('clip','wrap'):random_mix(r,seed)
if G:palette()
