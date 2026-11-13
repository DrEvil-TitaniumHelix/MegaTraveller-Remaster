import sys,os,json,random,shutil
ORACLE=GAMEFILES=OUT=None
def setup():
    """Parse argv, put the oracle on sys.path, clear <out dir> (a case the helper no longer writes must not replay stale); returns the gamefiles dir or None."""
    global ORACLE,GAMEFILES,OUT
    ORACLE,g,OUT=sys.argv[1:4];GAMEFILES=None if g=='-' else g
    shutil.rmtree(OUT,ignore_errors=True)
    for d in ('tools','tools/spec/render_s_cg_support','tools/spec'):sys.path.insert(0,os.path.join(ORACLE,d))
    return GAMEFILES
def rand_planes(seed):return [bytearray(random.Random(seed*4+p).randbytes(65536)) for p in range(4)]
def snap(planes,regs):
    assert len(planes)==4 and all(len(p)==65536 for p in planes) and len(regs)==16
    return b''.join(bytes(p) for p in planes)+bytes(regs)
class Case:
    """Records calls while the caller applies them to the oracle; write() once the end state is final."""
    def __init__(self,name,planes,regs):self.name=name;self.start=snap(planes,regs);self.calls=[];self.extra={}
    def call(self,*args):self.calls.append([a.hex() if isinstance(a,(bytes,bytearray)) else a for a in args])
    def write(self,planes,regs):
        d=os.path.join(OUT,self.name);os.makedirs(d,exist_ok=True)
        open(os.path.join(d,'start.bin'),'wb').write(self.start)
        open(os.path.join(d,'end.bin'),'wb').write(snap(planes,regs))
        open(os.path.join(d,'calls.json'),'w').write('[\n'+',\n'.join(json.dumps(c) for c in self.calls)+'\n]\n')
        for k,v in self.extra.items():open(os.path.join(d,k),'wb').write(v)
        print('WROTE',self.name,len(self.calls),'calls')
