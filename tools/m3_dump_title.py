import sys,os,random
sys.dont_write_bytecode=True
import m2_dump_common as C
C.setup()
sys.path.insert(0,os.path.join(C.ORACLE,'tools/spec'))
os.chdir(C.ORACLE)
import test_render_startup as T
SEED=int(sys.argv[sys.argv.index('--seed')+1]) if '--seed' in sys.argv else 1105
holder=[None]
class VM(T.StartupVM):
    def __init__(s,*a,**k):super().__init__(*a,**k);holder[0]=s
T.StartupVM=VM
snaps=[]
ORIG_EQUAL=T.equal
def equal(a,b,label):
    ORIG_EQUAL(a,b,label)
    snaps.append((label,bytes(b),holder[0].display_page,list(holder[0].palette)))
T.equal=equal
SCRIPTS={'idle':(None,)*16+(ord('x'),),
         'migraine':tuple(ord(c) for c in 'migraine')+(None,)*8+(ord('x'),),
         'early_exit':(None,ord('q')),
         'noise':(None,)*6+(ord('x'),)}
for name,script in SCRIPTS.items():
    snaps.clear()
    noise=name=='noise'
    n=T.graphics_case(SEED,script=script,video=2,noise=noise)
    assert n==len(snaps),(n,len(snaps))
    regs=list(holder[0].palette)
    if noise:
        rng=random.Random(SEED^0x9876);planes=[bytearray(rng.randbytes(65536)) for _ in range(4)]
    else:planes=[bytearray(65536) for _ in range(4)]
    case=C.Case('title.'+name,planes,regs)
    values=random.Random(SEED)
    rv=[values.randrange(32768) for _ in range(10000)][316:316+600]
    case.extra['rand.txt']=('\n'.join(str(x) for x in rv)+'\n').encode()
    case.extra['keys.txt']=('\n'.join('-' if k is None else str(k) for k in script)+'\n').encode()
    case.extra['checks.txt']=('\n'.join('%02d %s %d'%(i,label.replace(' ','_'),page) for i,(label,_,page,_) in enumerate(snaps))+'\n').encode()
    for i,(label,planes_bytes,page,pal) in enumerate(snaps):
        assert pal==regs,'palette changed mid-title'
        case.extra['%02d.bin'%i]=planes_bytes
    case.write([bytearray(snaps[-1][1][k*65536:(k+1)*65536]) for k in range(4)],regs)
print('CASES',len(SCRIPTS),'seed',SEED)
