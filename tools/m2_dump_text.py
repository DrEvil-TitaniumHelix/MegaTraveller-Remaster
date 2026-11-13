import sys,os,random,struct
sys.dont_write_bytecode=True
import m2_dump_common as C
import shutil
gf=C.setup()
shutil.rmtree(C.OUT,ignore_errors=True);os.makedirs(C.OUT,exist_ok=True)
def partial(why):
    """Record that real-file cases were dropped; the Rust test fails on this when MT1_GAMEFILES is set."""
    print('PARTIAL:',why);open(os.path.join(C.OUT,'PARTIAL'),'a').write(why+chr(10))
if not gf:partial('no gamefiles: real LY/PIC/DAT/STARS cases skipped')
import numpy as np
from render_s_cg import Renderer,decode_layout
from render_t import Surface,Renderer as TR,State,unpack_picture
from render_s_boot import blit_plane_rectangle
from data_formats import pic_dec,piccont_dec
REGS=[0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23]
OPS=(0,8,16,24)

def page_pixels(planes,page):
    out=np.zeros((200,320),np.uint8)
    for p in range(4):out|=(np.unpackbits(np.frombuffer(bytes(planes[p][page*8192:page*8192+8000]),np.uint8)).reshape(200,320)<<p).astype(np.uint8)
    return out
def put_page(planes,page,surf):
    for p,b in enumerate(surf.planar()):planes[p][page*8192:page*8192+8000]=b
def ly_bytes(entries):
    out=bytes([len(entries)])
    for e in entries:out+=bytes([len(e)])+b''.join(struct.pack('<BB4h',*r) for r in e)
    return out
def read(name):return open(os.path.join(gf,name),'rb').read()

class Assets:
    def __init__(s,layouts,sheets=None):s.layouts=layouts;s.sheets=sheets or {}
def tstate(widths=None,flags=None,tab=9,window_x=0,tabs=(0,0),off=b'\0'*5,off2=b'\0'*5):
    m=bytearray(0x20000)
    if widths:m[0x5128:0x5228]=widths
    if flags:m[0x5637:0x5737]=flags
    m[0x713D]=tab;struct.pack_into('<h',m,0x6E08,window_x);struct.pack_into('<hh',m,0x5228,*tabs)
    m[0x522C:0x5231]=off;m[0x5231:0x5236]=off2;m[0x52C4:0x52C8]=bytes(OPS)
    return State(bytes(m),0)
def trender(layouts,state,sheets=None):return TR(Assets(layouts,sheets),state)

def cg_tables():
    p=os.path.join(C.ORACLE,'work/mt1/CG.exe')
    if not os.path.isfile(p):return None
    d=open(p,'rb').read();off=struct.unpack_from('<H',d,8)[0]*16+0x924*16
    return d[off+0x358e:off+0x368e],d[off+0x3ce5:off+0x3de5],2
def t_tables():
    """T's own tables from the decode's booted-T RAM snapshot (tools/spec/test_render_t.py writes it; DS 0x2272): widths DS:5128,
    glyph flags DS:5637 (bit 8 = descender), tab character DS:713D, tab stops DS:5228, counted_glyph offsets DS:522C/5231."""
    p=os.path.join(C.ORACLE,'work/render_t/standalone_state_2.bin')
    if not os.path.isfile(p):return None
    m=open(p,'rb').read();ds=0x2272*16
    assert list(m[ds+0x52C4:ds+0x52C8])==list(OPS),'T op-5 table differs from the S table'
    return dict(widths=m[ds+0x5128:ds+0x5228],flags=m[ds+0x5637:ds+0x5737],bit=8,tab=m[ds+0x713D],tabs=struct.unpack_from('<hh',m,ds+0x5228),off=m[ds+0x522C:ds+0x5231],off2=m[ds+0x5231:ds+0x5236])
def synth_tables(rng):
    return bytes(rng.randrange(2,10) for _ in range(256)),bytes(rng.randrange(256) for _ in range(256)),8

def synth_ly(rng):
    return [
        [(2,3,0,0,4,9),(4,16,1,1,0,0),(5,3,0,0,0,0),(3,16,2,0,2,20),(1,5,6,0,12,12),(5,1,0,0,0,0),(2,16,8,2,10,8),(6,1,20,20,0,0),(4,16,30,30,0,0),(5,0,0,0,0,0),(3,0x12,40,0,40,30),(3,0x34,41,0,45,0),(2,9,50,0,49,5),(1,10,60,10,55,0),(2,7,70,0,70,-1)],
        [(3,5,0,0,0,7),(26,0,0,0,0,0),(4,16,1,0,0,0),(6,5,3,3,0,0),(2,16,5,0,6,3)],
        [],
        [(4,12,0,0,0,0),(0,0,0,0,0,0),(2,12,0,0,100,100)],
        [(2,8,-10,-10,-1,-1),(3,8,-5,-5,5,5),(1,8,190,300,210,330),(4,8,199,319,0,0),(4,8,200,320,0,0),(3,8,100,-20,100,340)],
        [(255,1,0,0,0,0),(84,2,0,0,0,0),(200,3,0,0,5,5),(3,16,0,0,0,3)],
        [(6,2,0,0,0,0),(6,9,0,0,0,0),(6,0,10,10,0,0)],
    ]+[[(3,rng.randrange(256),rng.randrange(-5,200),rng.randrange(-5,320),rng.randrange(-5,200),rng.randrange(-5,320)) for _ in range(rng.randrange(1,6))] for _ in range(8)]

def ly_cases():
    files=['FONT','CG','STATIONS','STPT','GROUND','TOP','CH','LED','GUNS','SPTOP','TITLE'] if gf else []
    for k,name in enumerate(files):
        raw=read(name+'.LY');entries=decode_layout(raw);rng=random.Random(100+k)
        for raster in ('wrap','clip'):
            planes=C.rand_planes(200+k*2+(raster=='clip'));case=C.Case(f'ly_{raster}_{name}',planes,REGS)
            case.call('lyfile',name,name+'.LY')
            idx=list(range(len(entries)))+[len(entries),len(entries)+7,255,1000,-1]
            if raster=='wrap':
                r=Renderer(planes)
                for i in idx:
                    page,y,x,c=rng.randrange(8),rng.randrange(-20,210),rng.randrange(-20,330),rng.randrange(256) if rng.random()<0.2 else rng.randrange(16)
                    r.page=page*8192;r.color=c;r.operation=rng.choice(OPS)
                    r.layout(entries,i,y,x);case.call('ly','wrap',page,name,i,y,x,c)
                case.write(r.planes,REGS)
            else:
                r=trender({name:entries},tstate())
                for i in idx:
                    page,y,x,c=rng.randrange(2),rng.randrange(-20,210),rng.randrange(-20,330),rng.randrange(256) if rng.random()<0.2 else rng.randrange(16)
                    r.target=Surface(page_pixels(planes,page));r.color=c;r.mode=rng.choice(OPS)
                    r.ly(i,y,x,name);put_page(planes,page,r.target);case.call('ly','clip',page,name,i,y,x,c)
                case.write(planes,REGS)
    rng=random.Random(7);entries=synth_ly(rng);raw=ly_bytes(entries)
    for raster in ('wrap','clip'):
        planes=C.rand_planes(300+(raster=='clip'));case=C.Case(f'ly_{raster}_synth',planes,REGS);case.call('lyhex','syn',raw)
        for i in list(range(len(entries)))*2+[len(entries),99]:
            page,y,x,c=rng.randrange(8 if raster=='wrap' else 2),rng.randrange(-10,200),rng.randrange(-10,320),rng.choice([16,16,0,5,0x27,rng.randrange(16)])
            if raster=='wrap':
                r=Renderer(planes);r.page=page*8192;r.color=c;r.operation=rng.choice(OPS);r.layout(entries,i,y,x)
            else:
                r=trender({'syn':entries},tstate());r.target=Surface(page_pixels(planes,page));r.color=c;r.mode=rng.choice(OPS);r.ly(i,y,x,'syn');put_page(planes,page,r.target)
            case.call('ly',raster,page,'syn',i,y,x,c)
        case.write(planes,REGS)

ALPHA=bytes(range(32,127))+bytes(range(0xA1,0xC0))
def words(rng,n,alpha=ALPHA):return b' '.join(bytes(rng.choice(alpha) for _ in range(rng.randrange(1,8))) for _ in range(n))
def text_cases():
    fonts=[]
    if gf:
        t=cg_tables()
        if t:fonts.append(('real','FONT',decode_layout(read('FONT.LY')),None)+t)
        else:partial('oracle work/mt1/CG.exe missing: text_s_real / text_t_real_* skipped')
        tt=t_tables()
        if tt:fonts.append(('treal','FONT',decode_layout(read('FONT.LY')),None,tt['widths'],tt['flags'],tt['bit']))
        else:partial('oracle work/render_t/standalone_state_2.bin missing: text_*_treal_* skipped')
    rng=random.Random(11);syn=synth_ly(rng)
    glyphs=[[] for _ in range(255)]
    for c in range(255):glyphs[c]=syn[c%len(syn)] if c>=32 else []
    glyphs[ord('a')]=[(2,16,0,0,3,2)];glyphs[ord('b')]=[(3,7,0,0,4,0),(4,16,0,1,0,0)];glyphs[ord('c')]=[(6,ord('a'),1,1,0,0)];glyphs[ord(' ')]=[]
    fonts.append(('synth','synfont',glyphs,ly_bytes(glyphs))+synth_tables(rng))
    for kind,lyname,entries,raw,widths,flags,bit in fonts:
        desc=[c for c in range(256) if flags[c]&bit or 0xA1<=c<=0xBF];rng=random.Random(21 if kind=='real' else 22)
        def declare(case,bit=bit):
            case.call('lyfile',lyname,lyname+'.LY') if raw is None else case.call('lyhex',lyname,raw)
            case.call('font','F',lyname,widths,flags,bit)
        planes=C.rand_planes(400+(kind=='synth'));case=C.Case(f'text_s_{kind}',planes,REGS);declare(case);r=Renderer(planes)
        split=os.environ.get('M2_SPLIT')
        for i in range(60 if kind!='treal' else 0):
            s=words(rng,rng.randrange(1,5));page=rng.randrange(8);y=rng.randrange(-10,205);x=rng.randrange(-20,330)
            c=rng.randrange(256) if i%7==0 else rng.randrange(16);sh=None if i%3 else rng.randrange(16)
            r.page=page*8192;r.operation=rng.choice(OPS)
            if split:case=C.Case(f'text_s_{kind}_{i:02}',r.planes,REGS);declare(case)
            r.text(entries,widths,s,y,x,c,sh,desc);case.call('stext',page,'F',s,y,x,c,-1 if sh is None else sh)
            if split:case.write(r.planes,REGS)
        if not split:case.write(r.planes,REGS)
        for variant in range(2):
            rng2=random.Random(31+variant+10*(kind=='synth'));planes=C.rand_planes(410+variant+2*(kind=='synth'))
            case=C.Case(f'text_t_{kind}_{variant}',planes,REGS);declare(case,8)
            tab=rng2.choice([9,1,ord('~')]);window_x=rng2.randrange(0,200);tabs=(rng2.randrange(0,160),rng2.randrange(160,320))
            off=bytes(rng2.randrange(0,12) for _ in range(5));off2=bytes(rng2.randrange(0,12) for _ in range(5))
            if kind=='treal':tt=t_tables();tab,tabs,off,off2=tt['tab'],tt['tabs'],tt['off'],tt['off2']
            r=trender({'FONT':entries},tstate(widths,flags,tab,window_x,tabs,off,off2))
            page=variant;r.target=Surface(page_pixels(planes,page))
            pen,colour,shadow,bg=rng2.randrange(16),rng2.randrange(16),variant,rng2.randrange(16)
            y,x=rng2.randrange(0,190),rng2.randrange(0,300)
            r.text_y,r.text_x,r.color,r.text_color,r.text_flags,r.text_bg=y,x,pen,colour,shadow,bg
            case.call('tcursor',page,y,x,pen,colour,shadow,bg,tab,tabs[0],tabs[1],window_x)
            for i in range(40):
                k=rng2.randrange(10)
                if k<6:
                    s=words(rng2,rng2.randrange(1,4));s=bytearray(s)
                    for j in range(len(s)):
                        if rng2.random()<0.08:s[j]=tab
                        elif rng2.random()<0.05:s[j]=ord('|')
                    s=bytes(s)
                    if rng2.random()<0.3:r.text_y,r.text_x=rng2.randrange(-5,200),rng2.randrange(-20,320);case.call('tcursor',page,r.text_y,r.text_x,r.color,r.text_color,r.text_flags&1,r.text_bg,tab,tabs[0],tabs[1],window_x)
                    r.text(s);case.call('ttext','F',s)
                elif k<8:
                    s=words(rng2,rng2.randrange(3,14),bytes(range(97,123))+b'ABC');yy,xx=rng2.randrange(0,150),rng2.randrange(0,200)
                    w,h,lh=rng2.randrange(30,140),rng2.randrange(10,60),rng2.choice([6,8,9]);c,b=rng2.randrange(16),rng2.randrange(16)
                    r.wrapped(s,yy,xx,w,h,lh,c,b);case.call('twrapped','F',s,yy,xx,w,h,lh,c,b)
                else:
                    c,g,n,yy,xx=rng2.randrange(14),rng2.randrange(5),rng2.randrange(4),rng2.randrange(0,190),rng2.randrange(0,300)
                    r.counted_glyph(c,g,n,yy,xx);case.call('tcounted','F',c,g,n,yy,xx,off,off2)
            put_page(planes,page,r.target);case.write(planes,REGS)

def packbits_literal(data):
    out=b''
    for i in range(0,len(data),128):chunk=data[i:i+128];out+=bytes([len(chunk)-1])+chunk
    return out
def pic_bytes(h,wb,px):
    packed=bytes((px[i]<<4)|px[i+1] for i in range(0,len(px),2));pk=packbits_literal(packed)
    return struct.pack('<HH',h,wb)+bytes(16)+struct.pack('<HH',len(packed),len(pk))+pk
def pic_cases():
    rng=random.Random(41);pics={}
    def pix(raw):return unpack_picture(pic_dec(raw))
    planes=C.rand_planes(500);case=C.Case('pic_t_synth',planes,REGS)
    for n,(h,wb) in enumerate([(2,1),(5,3),(17,2),(1,1)]):
        px=[rng.randrange(16) for _ in range(h*wb*8)];raw=pic_bytes(h,wb,px);pics[f's{n}']=pix(raw);case.call('pichex',f's{n}',raw)
    small=[rng.randrange(16) for _ in range(45*288)];big=[rng.randrange(16) for _ in range(30*288)]
    for name,h,px in (('small',45,small),('big',30,big)):raw=pic_bytes(h,36,px);pics[name]=pix(raw);case.call('pichex',name,raw)
    blits(case,planes,pics,rng,'t')
    if gf:
        planes=C.rand_planes(501);case=C.Case('pic_t_real',planes,REGS);pics={}
        for f in ('TOP.PIC','START.PIC','TIT.PIC','ZOD.PIC','A0.PIC'):
            if os.path.isfile(os.path.join(gf,f)):pics[f]=pix(read(f));case.call('picfile',f,f,-1)
        for f,n in (('OBJ.DAT',5),('C.DAT',2),('X.DAT',2),('Y.DAT',3),('Z.DAT',4)):
            chunks=piccont_dec(read(f))
            for i in range(min(n,len(chunks))):pics[f'{f}{i}']=unpack_picture(chunks[i]);case.call('picfile',f'{f}{i}',f,i)
        blits(case,planes,pics,rng,'t',small='OBJ.DAT0',big='OBJ.DAT1')
    planes=C.rand_planes(510);case=C.Case('pic_upload',planes,REGS);rng=random.Random(42)
    for i in range(24):
        rows,width=rng.randrange(1,12),rng.randrange(1,50);sstride=width+rng.randrange(0,5);dstride=rng.choice([40,41,39,width])
        src=bytes(rng.randrange(256) for _ in range(rows*sstride+8));dst=rng.choice([rng.randrange(65536),0xffff-rng.randrange(0,200),0x4000])
        op=OPS[i%4];mask=rng.randrange(1,16)
        if i==23:width=0;op=0
        blit_plane_rectangle(planes,src+bytes(65536),0,0,dst,0xA000,0,0,dstride,0,0,sstride,rows,width,op,mask)
        case.call('upload',src,sstride,dst,dstride,rows,width,op,mask)
    case.write(planes,REGS)
    if gf and os.path.isfile(os.path.join(gf,'STARS.PIC')):
        planes=C.rand_planes(511);case=C.Case('pic_atlas',planes,REGS);case.call('picfile','STARS','STARS.PIC',-1)
        px=pix(read('STARS.PIC'));h,w=px.shape
        for p in range(4):
            packed=np.packbits((px>>p)&1,axis=1).tobytes()
            blit_plane_rectangle(planes,packed+bytes(65536),0,0,0x4000,0xA000,0,0,40,0,0,w//8,h,w//8,0,1<<p)
        case.call('atlas','STARS',0x4000);case.write(planes,REGS)
def blits(case,planes,pics,rng,kind,small='small',big='big'):
    names=sorted(pics);surfs={}
    for i in range(50):
        page=rng.randrange(2);surf=Surface(page_pixels(planes,page));name=rng.choice(names);src=pics[name];h,w=src.shape
        tr=rng.choice([None,None,1,14,src[0,0]])
        if i%3==0:
            y,x=rng.randrange(-h,200),rng.randrange(-w,320);surf.blit(src,y,x,transparent=tr);case.call('pic',page,name,y,x,-1 if tr is None else int(tr))
        elif i%3==1:
            y,x=rng.randrange(-20,200),rng.randrange(-20,320);sy,sx=rng.randrange(-3,h),rng.randrange(-3,w);hh,ww=rng.randrange(0,h+4),rng.randrange(0,w+4)
            surf.blit(src,y,x,sy,sx,hh,ww,tr);case.call('picrect',page,name,y,x,sy,sx,hh,ww,-1 if tr is None else int(tr))
        else:
            icon=rng.choice([rng.randrange(0,36),rng.randrange(100,112),rng.randrange(0,24)]);y,x=rng.randrange(-5,200),rng.randrange(-5,320)
            r=trender({},tstate(),{'OBJ':[pics[small],pics[big]]});r.target=surf;r.item_icon(icon,y,x);case.call('icon',page,small,big,icon,y,x)
        put_page(planes,page,surf)
    case.write(planes,REGS)

ly_cases();text_cases();pic_cases()
