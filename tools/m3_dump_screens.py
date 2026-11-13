import sys,os,re,struct,json
sys.dont_write_bytecode=True
import m2_dump_common as C
C.setup()
sys.path.insert(0,os.path.join(C.ORACLE,'tools/golden'))
os.chdir(C.ORACLE)
from render_s_cg import Renderer
import render_cg_paths as RP
from cg_golden import ENTER,DOWN,UP,RIGHT,ESC
LEFT=0x14B
SEED=int(sys.argv[sys.argv.index('--seed')+1]) if '--seed' in sys.argv else 4711
MATCH=sys.argv[sys.argv.index('--match')+1] if '--match' in sys.argv else None

def hexs(b):return (b.encode('latin1') if isinstance(b,str) else bytes(b)).hex()
class Rec(Renderer):
    """render_s_cg.Renderer that records its outer calls (the Observer's), not the primitives those expand into (as m2_dump_chargen)."""
    def begin(s):s.rec=[];s.nest=0
    def _c(s,*a):
        if s.nest==0:s.rec.append([a[0],s.page//8192,*a[1:]])
    def _run(s,f,*a):
        s.nest+=1
        try:return f(*a)
        finally:s.nest-=1
    def layout(s,entries,index,y=0,x=0):s._c('layout','font' if len(entries)>=200 else 'cg',index,y,x,s.color);return s._run(super().layout,entries,index,y,x)
    def text(s,font,widths,value,y,x,color,shadow=None,descenders=()):s._c('text',bytes(value),y,x,color,-1 if shadow is None else shadow);return s._run(super().text,font,widths,value,y,x,color,shadow,descenders)
    def fill(s,y,x,h,w,color):s._c('fill',y,x,h,w,color,s.operation);return s._run(super().fill,y,x,h,w,color)
    def line(s,y1,x1,y2,x2,color):s._c('line',y1,x1,y2,x2,color,s.operation);return s._run(super().line,y1,x1,y2,x2,color)
    def pixel(s,y,x,color):s._c('pixel',y,x,color,s.operation);return s._run(super().pixel,y,x,color)
h=RP.Harness(SEED)
r=Rec();r.__dict__.update(h.pixels.renderer.__dict__);h.pixels.renderer=r;r.begin()
v=h.vm
def finish_direct():
    """A primitive called directly returns at the harness trampoline before the Observer's return hook: close its frame by hand."""
    px=h.pixels
    if px.active:
        if px.return_hook is not None:
            try:v.mu.hook_del(px.return_hook)
            except Exception:pass
            px.return_hook=None
        px.active=False;px.check('direct call')
REGS=[h.word(0x3692+2*i)&63 for i in range(16)]
def ds_str(p):return v.cstr(v.r('DS'),p&0xFFFF)
def w(a):return h.word(a)
def b(a):return h.byte(a)
def s16(x):return x-0x10000 if x&0x8000 else x
def put_words(a,words):
    for i,x in enumerate(words):h.wordput(a+2*i,x)

def fmt_c(fmt,args):
    """C printf subset the CG messages use: %s (DS string pointer), %d / %ld (ints)."""
    out=bytearray();i=0;ai=0;fmt=bytes(fmt)
    while i<len(fmt):
        ch=fmt[i]
        if ch==37 and i+1<len(fmt):
            spec=fmt[i+1:i+3]
            if fmt[i+1]==ord('s'):out+=ds_str(args[ai]);ai+=1;i+=2;continue
            if fmt[i+1]==ord('d'):out+=str(s16(args[ai]&0xFFFF)).encode();ai+=1;i+=2;continue
            if spec==b'ld':out+=str(args[ai]).encode();ai+=1;i+=3;continue
        out.append(ch);i+=1
    return bytes(out)
def state_lines():
    L=[]
    L.append('color %d'%b(0x4360));L.append('pen %d'%b(0x396c));L.append('cursor %d %d'%(s16(w(0x4368)),s16(w(0x4362))))
    L.append('origin %d %d'%(s16(w(0x435e)),s16(w(0x4156))));L.append('flags %d %d'%(b(0x4158),b(0x4159)))
    L.append('center %d'%s16(w(0x436a)));L.append('lineh %d %d'%(s16(w(0x415a)),s16(w(0x4150))))
    L.append('curwin %d'%s16(w(0x37ca)));L.append('initial %d'%s16(w(0x38d6)));L.append('changed %d'%b(0x39c4))
    L.append('msgdelay %d'%s16(w(0x0626)));L.append('msgwin %d'%s16(w(0x1824)));L.append('skillswin %d %d'%(struct.unpack('b',bytes([b(0x2d6c)]))[0],struct.unpack('b',bytes([b(0x2d6d)]))[0]))
    L.append('totals %d %d'%(s16(w(0x053a)),s16(w(0x053c))))
    for i in range(10):
        f=[s16(w(0x36b2+28*i+2*k)) for k in range(13)]
        L.append('win %d '%i+' '.join(str(x) for x in f))
    return L

class Case:
    def __init__(s,name):s.name=name;s.lines=[];s.calls=[];s.data=[]
    def call(s,addr,args,keys=None,setup=None,resolve=None):
        """One original call; `keys` = list of key lists, one per input step; `resolve` = list of (argindex,'str') to resolve DS strings."""
        s.calls.append((addr,args,keys or [],setup,resolve or []))
    def data_line(s,*a):s.data.append(' '.join(str(x) for x in a))

results=[];partial=0;last_result=[0]
def run_case(c,**reset):
    global partial
    if MATCH and not re.search(MATCH,c.name):return
    h.reset(**reset);h.pixels.renderer.begin()
    if not getattr(h.pixels,'_wrapped',False):
        orig=h.pixels.checkpoint;h.pixels.checkpoint=lambda label:(finish_direct(),orig(label))[1];h.pixels._wrapped=True
    first=True;start=None;lines=[];texts=[];result=None;calls=[]
    for addr,args,keys,setup,resolve in c.calls:
        if setup:setup()
        if first:
            lines=state_lines();start=C.snap([bytearray(p) for p in h.pixels.renderer.planes],REGS);first=False
        rargs=list(args)
        shown=[]
        for i,a in enumerate(args):
            kind=dict(resolve).get(i)
            shown.append(('s:'+hexs(ds_str(a))) if kind=='str' else str(s16(a&0xFFFF)))
        step_keys=list(keys)
        def choose(rec,step,sk=step_keys):
            assert step<len(sk),'%s: more input steps than keys (%d)'%(c.name,step)
            return sk[step]
        try:
            result,records=h.call(addr,tuple(a&0xFFFF for a in rargs),choose if step_keys else None,allow_exit=True)
            finish_direct()
        except AssertionError as e:
            print('FAIL',c.name,'%04X'%addr,repr(e)[:200]);partial+=1;finish_direct();return
        if addr==0x0B7A:shown.append(str(s16(w(0x4ca))))
        if addr==0x12B0:shown=['s:'+hexs(fmt_c(ds_str(args[0]),args[1:]))]
        c.lines.append('rand '+' '.join(str(x) for x in h.rand_used));h.rand_used=[]
        c.lines.append('call %04X '%addr+' '.join(shown))
        for si,k in enumerate(step_keys):c.lines.append('keys %d '%si+' '.join(str(x) for x in k))
        for rr in records:
            for t in rr['text']:
                texts.append('text %s %s'%(t[0],hexs(('|'.join(t[1]) if isinstance(t[1],list) else t[1]))))
        c.lines.append('result %d'%s16(result&0xFFFF));last_result[0]=result&0xFFFF
    case=C.Case(c.name,[bytearray(65536)]*4,REGS);case.start=start
    for cl in h.pixels.renderer.rec:case.call(*cl)
    c.data_line('work '+hexs(h.original.work()))
    case.extra['state.txt']=('\n'.join(lines)+'\n').encode()
    case.extra['case.txt']=('\n'.join(c.lines+c.data+texts)+'\n').encode()
    case.write(h.pixels.renderer.planes,REGS);results.append(c.name)

SERVICES=0x14EE;YESNO=0x1826;MAIN=0x29D8;DISK=0x29E0;TABLES=0x2E28;MUSTER=0x300E;EDITOR=0x620;REENLIST=0x3012;ENLISTQ=0x28B0
V=lambda items,n,row,col,allow,border,title=0:(items,n,4,row,col,allow,0,0,border,0,1,0,3)+((title,0) if border==3 else ())
def menu_items(items,n):return 'items '+' '.join(hexs(ds_str(w(items+2*i))) for i in range(n))

def vertical():
    for name,keys,extra in [('accept',[[ENTER]],()),('cancel',[[ESC]],()),('wrap_up',[[UP,ENTER]],()),('wrap_down',[[DOWN]*5+[ENTER]],()),
                            ('initial_unique',[[ord('a'),ENTER]],()),('initial_ambiguous',[[ord('m'),ENTER]],()),('ignore',[[ord('z'),0x149,ENTER]],()),
                            ('two_steps',[[DOWN],[DOWN,ENTER]],())]:
        c=Case('menu.vertical.'+name);c.call(0x519A,V(SERVICES,5,110,150,0,3,0x27EC),keys,resolve=[(13,'str')]);c.data_line(menu_items(SERVICES,5));run_case(c)
    c=Case('menu.vertical.border2.main');c.call(0x519A,V(MAIN,4,0x50,0x1e,0,2),[[DOWN,ENTER]]);c.data_line(menu_items(MAIN,4));run_case(c)
    c=Case('menu.vertical.border2.initial');c.call(0x519A,V(DISK,3,0x50,0x1e,0,2),[[ENTER]],setup=lambda:h.wordput(0x38d6,2));c.data_line(menu_items(DISK,3));run_case(c)
    c=Case('menu.vertical.unique_allowed');c.call(0x519A,V(TABLES,4,0x67,0xaa,1,2),[[ord('E')]]);c.data_line(menu_items(TABLES,4));run_case(c)
    for choice in (0,1):
        c=Case('menu.confirm.%d'%choice);c.call(0x1AA2,(),[[DOWN]*choice+[ENTER]]);c.data_line(menu_items(YESNO,2));run_case(c)
    for key in (ESC,ord('y'),ord('n')):
        c=Case('menu.confirm.key%d'%key);c.call(0x1AA2,(),[[key]]);c.data_line(menu_items(YESNO,2));run_case(c)

def skill_rows(service):
    """2E36's six rows per table for `service`: (short name, descriptor type)."""
    out=[]
    for table in range(4):
        rows=[]
        for die in range(6):
            skill=b(w(0x2D48+2*table)+die*5+service);d=0x1184+skill*8
            rows.append((ds_str(w(d+2)),b(d+6)))
        out.append(rows)
    return out
def skill_table_menu():
    for service in (0,3):
        for name,keys in [('enter',[[ENTER]]),('down2',[[DOWN,DOWN,ENTER]]),('up',[[UP,ENTER]])]:
            c=Case('menu.skill_tables.s%d.%s'%(service,name))
            def setup(service=service):h.put(0x2d6a,[service]);h.put(0x2d6c,[0xFF,0xFF])
            c.call(0x519A,(TABLES,4,4,0x67,0xaa,0,0x2e36,0,2,0,1,0,3),keys,setup=setup)
            c.data_line(menu_items(TABLES,4))
            for t,rows in enumerate(skill_rows(service)):c.data_line('rows %d '%t+' '.join('%s:%d'%(hexs(n),k) for n,k in rows))
            run_case(c)

import cg_replay
REPLAY=cg_replay.replay
def no_replay_56a6(on):cg_replay.replay=(lambda fr:None if fr['func']==0x56A6 else REPLAY(fr)) if on else REPLAY
def horizontal():
    for name,keys,initial in [('enter',[[ENTER]],1),('right',[[RIGHT,ENTER]],-1),('left',[[LEFT,ENTER]],1),('letter_n',[[ord('n'),ENTER]],-1),('letter_y',[[ord('y'),ENTER]],1),('edges',[[LEFT,RIGHT,RIGHT,ENTER]],-1)]:
        c=Case('menu.horizontal.'+name);c.call(0x56A6,(YESNO,2,4,0x76,0xdc,0,0,0,2,0,1,0,3),keys,setup=lambda i=initial:h.wordput(0x38d6,i));c.data_line(menu_items(YESNO,2))
        no_replay_56a6(name.startswith('letter'));run_case(c);no_replay_56a6(False) if name.startswith('letter') else None
    c=Case('menu.horizontal.three');c.call(0x56A6,(EDITOR,3,4,0x7a,100,0,0,0,2,0,1,0,3),[[RIGHT,RIGHT,ENTER]]);c.data_line(menu_items(EDITOR,3));run_case(c)
    labels=[w(0x1184+8*k+2) for k in range(7,13)]
    for name,keys in [('enter',[[ENTER]]),('right',[[RIGHT,ENTER]]),('back',[[RIGHT,LEFT,ENTER]])]:
        c=Case('menu.horizontal.help.'+name)
        def setup():put_words(0x2e5e,labels);put_words(0x2e6c,labels);h.put(0x2d6c,[0xFF,0xFF])
        c.call(0x56A6,(MUSTER,2,4,100,0xaf,0,0x374a,0,2,0,1,0,3),keys,setup=setup)
        c.data_line(menu_items(MUSTER,2));c.data_line('labels '+' '.join(hexs(ds_str(p)) for p in labels))
        run_case(c)
    for name,keys in [('yes',[[ENTER]]),('no_then_yes',[[RIGHT,ENTER],[LEFT,ENTER]])]:
        c=Case('menu.horizontal.reroll.'+name)
        c.call(0x0A36,(0x4be,6,0x76,100,2,0xc,0,1,0,3),setup=lambda:h.put(0x456,[0]))
        c.call(0x56A6,(YESNO,2,4,0x76,0xdc,0,0,0x900,2,0,1,0,3),keys,setup=lambda:(h.wordput(0x38d6,1),h.wordput(0x454,last_result[0])))
        c.data_line(menu_items(YESNO,2))
        c.data_line('labels '+' '.join(hexs(ds_str(w(0x4be+2*i))) for i in range(6)))
        run_case(c)

def messages():
    for name,addr,args,res in [('plain',0x12B0,(0x2791,),[(0,'str')]),('wrap',0x12B0,(0x27f5,w(SERVICES)),[(0,'str'),(1,'str')]),('int',0x12B0,(0x2daf,2),[(0,'str')]),
                               ('show',0x1158,(0x2791,),[(0,'str')]),('long',0x1158,(0x28be,),[(0,'str')]),('banner',0x1BD0,(0x2bbd,),[(0,'str')])]:
        c=Case('message.'+name);c.call(addr,args,resolve=res);run_case(c)
    c=Case('message.twice');c.call(0x12B0,(0x2791,),resolve=[(0,'str')]);c.call(0x12B0,(0x27dc,),resolve=[(0,'str')]);run_case(c)
    c=Case('message.after_key');c.call(0x1AA2,(),[[ENTER]]);c.call(0x12B0,(0x27dc,),resolve=[(0,'str')]);c.data_line(menu_items(YESNO,2));run_case(c)

def inputs():
    for name,keys in [('golden',[[ord(k) for k in 'GOLDEN']+[ENTER]]),('erase',[[ord(k) for k in 'I5X']+[8]+[ord('Y'),ENTER]]),('esc',[[ord('A'),ESC]]),
                      ('max',[[ord(k) for k in 'ABCDEFGHIJKLMNOPQRST']+[ENTER]]),('empty',[[ENTER]]),('stepwise',[[ord('A')],[ord('b')],[ENTER]])]:
        c=Case('input.'+name);c.call(0x5CC2,(0x2ac4,15,100,125,80,1,0,3),keys);run_case(c)
    c=Case('input.filename');c.call(0x5CC2,(0x2ac4,13,80,30,80,1,0,3),[[ord(k) for k in 'I5PARTY']+[ENTER]]);run_case(c)

def windows():
    for border in (0,1,2,3):
        c=Case('window.border%d'%border)
        args=(40,60,50,120,1,border,0,1,0,3)+((0x27EC,0) if border==3 else ())
        c.call(0x4FFA,args,resolve=[(10,'str')] if border==3 else None)
        run_case(c)
    c=Case('window.open_close');c.call(0x4FFA,(40,60,50,120,1,2,0,1,0,3));c.call(0x501E,(h.word(0x37ca)+1,) if False else (1,));run_case(c)
    c=Case('window.two_then_close_first');c.call(0x4FFA,(40,60,50,120,1,2,0,1,0,3));c.call(0x4FFA,(90,20,30,200,1,1,7,0));c.call(0x501E,(2,));run_case(c)
    c=Case('panel.attributes');c.call(0x0A36,(0x4be,6,0x76,100,2,0xc,0,1,0,3));c.data_line('labels '+' '.join(hexs(ds_str(w(0x4be+2*i))) for i in range(6)));run_case(c)
    c=Case('panel.values');c.call(0x0A36,(0x4be,6,0x76,100,2,0xc,0,1,0,3));c.call(0x0B1C,(2,0));c.data_line('labels '+' '.join(hexs(ds_str(w(0x4be+2*i))) for i in range(6)));run_case(c,stats=(7,12,3,15,9,2))

def drawing():
    for idx in (0,2,3):
        c=Case('flash.%d'%idx);c.call(0x2DCC,(0x93,0xab,8,0x61,4,idx));run_case(c)
    c=Case('totals.first');c.call(0x0C9C,(12,15));run_case(c)
    c=Case('totals.change');c.call(0x0C9C,(12,15));c.call(0x0C9C,(13,15));c.call(0x0C9C,(13,15));run_case(c)
    for sel in (0,5):
        c=Case('skill_cursor.%d'%sel);c.call(0x0B7A,(),setup=lambda s=sel:h.wordput(0x4ca,s));run_case(c)
    c=Case('party.list')
    def setup():h.put(0x29ee,[1,3,0,5,0]);h.put(0x29f3,[3])
    c.call(0x1B72,(),setup=setup);c.data_line('names '+' '.join(hexs(ds_str(0x1840+256*(m-1))) if m else '-' for m in (1,3,0,5,0)));run_case(c)
    n_entries=b(w(0x182a))
    for i in range(min(n_entries,24)):
        c=Case('layout.%02d'%i);c.call(0x6416,(i,0,0),setup=lambda:h.put(0x4360,[2]) or h.put(0x396c,[2]));run_case(c)
    c=Case('layout.offset');c.call(0x6416,(2,0x37,0x6e));run_case(c)
    for s in range(5):
        c=Case('emblem.%d'%s);c.call(0x481A,(0,));c.call(0x1416,(0x1e,0xf,s));run_case(c)
    c=Case('text.shadow');c.call(0x4792,(1,));c.call(0x47AC,(0,));c.call(0x481A,(3,));c.call(0x10E8,(0,50,50,8,5,0x2791),resolve=[(5,'str')]);c.call(0x479E,(1,));run_case(c)
    c=Case('text.plain');c.call(0x481A,(4,));c.call(0x10E8,(0,60,10,8,5,0x27dc),resolve=[(5,'str')]);run_case(c)
    c=Case('text.centered');c.call(0x481A,(0,));c.call(0x49B8,(0x2bbd,10),resolve=[(0,'str')]);run_case(c)

def text_list(p):
    out=[]
    if struct.unpack('b',bytes([b(p+6)]))[0]==-1:return out
    while True:
        row,col,txt,color=s16(w(p)),s16(w(p+2)),w(p+4),b(p+6)
        if txt:out.append((row,col,ds_str(txt),color))
        p+=7
        if b(p+6)==255:break
    return out
def sheet(index):
    r=0x1840+256*index;rec=v.rb(v.lin(v.r('DS'),r),256)
    service=rec[0x20];rank=rec[0x21]
    service_name=ds_str(w(0x14f8+2*service))
    rank_name=ds_str(w(b(0x1800+5*rank+service)*2+0x1138))
    skills=[];d=0x11b4;level=0x2a
    for k in range(79):
        while struct.unpack('b',bytes([b(d+7)]))[0]==-1:d+=8
        if rec[level]:skills.append((ds_str(w(d+2)),rec[level]))
        level+=1;d+=8
    items=[]
    for k in range(19):
        cell=struct.unpack_from('<h',rec,0xcc+2*k)[0]
        if cell>0:items.append(ds_str(w(0x16c0+cell*3)))
    return rec,service_name,rank_name,skills,items
def sheets():
    for index in range(9):
        c=Case('sheet.%d'%index)
        c.call(0x6416,(0x13,0,0));c.call(0x4A50,(0x2cea,0,0));c.call(0x251E,(index,))
        rec,sn,rn,skills,items=sheet(index)
        c.data_line('record '+hexs(rec));c.data_line('service %s'%hexs(sn));c.data_line('rank %s'%hexs(rn))
        c.data_line('skills '+' '.join('%s:%d'%(hexs(n),l) for n,l in skills));c.data_line('items '+' '.join(hexs(n) for n in items))
        for row,col,txt,color in text_list(0x2cea):c.data_line('textlist %d %d %d %s'%(row,col,color,hexs(txt)))
        run_case(c)
    c=Case('sheet.textlist.generator')
    for row,col,txt,color in text_list(0x2cc0):c.data_line('textlist %d %d %d %s'%(row,col,color,hexs(txt)))
    c.call(0x6416,(0,0,0));c.call(0x4A50,(0x2cc0,0,0));run_case(c)

vertical();skill_table_menu();horizontal();messages();inputs();windows();drawing();sheets()
print('CASES',len(results),'seed',SEED)
if partial:print('PARTIAL',partial,'cases failed in the oracle')
