"""Read-only walk oracle export. Streaming fixtures under target; no game bytes committed.
Replays all 1,800 run-5 starting cases. The 600 main_loop cases supply every owned
routine invocation; the main loop itself belongs to the harness.
Also exports isolated proof-table cases (two seeds, 1,100 cases per routine).
"""
import sys
sys.dont_write_bytecode = True
import ast, copy, inspect, os, struct, textwrap, time, threading
from pathlib import Path


def watchdog(seconds=900):
    def watch():
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if os.name == 'nt':
                import ctypes
                class Counters(ctypes.Structure):
                    _fields_ = [('cb', ctypes.c_ulong), ('faults', ctypes.c_ulong)] + [(n, ctypes.c_size_t) for n in ('peak_ws','ws','peak_paged','paged','peak_nonpaged','nonpaged','page','peak_page')]
                c = Counters(); c.cb = ctypes.sizeof(c)
                ctypes.windll.psapi.GetProcessMemoryInfo(ctypes.windll.kernel32.GetCurrentProcess(), ctypes.byref(c), c.cb)
                if c.ws > 1500 * 1024 * 1024:
                    print('PARTIAL: walk exporter memory watchdog', flush=True); os._exit(3)
            time.sleep(0.5)
        print('PARTIAL: walk exporter wall-time watchdog', flush=True); os._exit(3)
    threading.Thread(target=watch, daemon=True).start()


def main():
    oracle, gamefiles, out = map(Path, sys.argv[1:4])
    count = 1100
    watchdog()
    sys.path[:0] = [str(oracle/'tools/spec'), str(oracle/'tools/golden'), str(oracle/'tools')]
    import walk, test_walk as tw, walk_golden as chain
    tree = ast.parse(textwrap.dedent(inspect.getsource(tw.Suite.case)))
    fn = tree.body[0]
    cut = next(i for i,n in enumerate(fn.body) if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='expected' for t in n.targets))
    setup = fn.body[:cut]
    tail = ast.parse("initial={n:rng.randrange(65536) for n in ('AX','BX','CX','DX','SI','DI','BP','ES')}\nif address==0x3676:kwargs['initial_second']=i16(initial['DI'])\nif address==0x6923:kwargs['initial_now']=int.from_bytes(m[STACK*16+0xfef6:STACK*16+0xfefa],'little')\nreturn m,cfg,args,kwargs\n").body
    fn.body = setup + tail; fn.name = 'make_case'; ast.fix_missing_locations(tree)
    ns = dict(tw.__dict__); exec(compile(tree,'<read-only Suite.case setup>','exec'),ns)
    suite = tw.Suite(); suite.default = (gamefiles/'DEFAULT.SAV').read_bytes()
    suite.areas = [a for a in tw.world_dec((gamefiles/'W.DAT').read_bytes()) if 'objects' in a]
    out.mkdir(parents=True,exist_ok=True)
    stream = (out/'walk.bin').open('wb'); stream.write(b'MTWALK1\0')
    total = 0; counts = {}; roots = 0; deadline = time.monotonic()+850
    names = set(tw.FUNCTIONS.values()) - {'main_loop'}
    name_addr = {v:k for k,v in tw.FUNCTIONS.items()}
    ptrargs = chain.POINTER_ARGS
    def u32(n): stream.write(struct.pack('<I',n & 0xffffffff))
    def blob(b): u32(len(b)); stream.write(b)
    def string(s): blob(s.encode())
    def words(a): u32(len(a)); stream.write(struct.pack('<'+'H'*len(a),*(v&65535 for v in a)))
    def ranges(addresses):
        result=[]
        for a in sorted(addresses):
            if result and result[-1][1]==a: result[-1]=(result[-1][0],a+1)
            else: result.append((a,a+1))
        return result
    class Memory:
        def __init__(self,m): self.data=m; self.open=[]; self.leaf_writes=None
        def __getitem__(self,k):
            span=range(k.start,k.stop) if isinstance(k,slice) else [k]
            for f in self.open: f['reads'].update(span)
            return self.data[k]
        def __setitem__(self,k,v):
            at=k.start if isinstance(k,slice) else k; b=bytes(v) if isinstance(k,slice) else bytes([v&255])
            for f in self.open: f['writes'].append((at,b))
            if self.leaf_writes is not None: self.leaf_writes.append((at,b))
            self.data[k]=v
    def write_sample(f,label):
        nonlocal total
        total+=1; counts[f['name']]=counts.get(f['name'],0)+1
        string(label);string(f['name']);words(f['args']);u32(f.get('initial_second',0));u32(0xffffffff if f['ret'] is None else f['ret']&65535)
        rr=ranges(f['reads']|{a+i for a,b in f['writes'] for i in range(len(b))}|set(range(walk.D+0x6d65,walk.D+0x6d69)))
        u32(len(rr))
        for lo,hi in rr: u32(lo);blob(f['before'][lo:hi])
        u32(len(f['leaves']))
        for name,args,ret,writes in f['leaves']:
            string(name);words(args);u32(ret);u32(len(writes))
            for at,b in writes:u32(at);blob(b)
        u32(len(f['frames']))
        for frame in f['frames']:
            string(frame['name']);words(frame['args']);u32(0xffffffff if frame['ret'] is None else frame['ret']&65535)
            u32(len(frame['calls']))
            for name,args,ret in frame['calls']:string(name);words(args);u32(0xffffffff if ret is None else ret)
            u32(len(frame['writes']))
            for at,b in frame['writes']:u32(at);blob(b)
    class Driver:
        def __init__(self,m,cfg,compose,label): self.mem=Memory(m);self.services=(chain.ChainServices if compose else tw.Services)(copy.deepcopy(cfg));self.compose=compose;self.label=label;self.calls=0
        def run(self,name,args,kwargs=None):
            kwargs=kwargs or {};m=self.mem
            f=dict(name=name,args=tuple(a&65535 for a in args),before=bytes(m.data),reads=set(),writes=[],calls=[],leaves=[],frames=[],initial_second=kwargs.get('initial_second',0))
            m.open.append(f)
            result=getattr(walk,name)(walk.State(m,self.call),*args,**kwargs)
            f['ret']=result;m.open.pop();f['frames'].append({k:f[k] for k in ('name','args','ret','calls','writes')})
            if m.open:m.open[-1]['frames'].extend(f['frames'])
            if not m.open or m.open[-1]['name']=='main_loop':
                if name!='main_loop':write_sample(f,self.label)
            return result
        def call(self,name,*args):
            self.calls+=1
            if self.calls>100000:raise RuntimeError('call budget exceeded')
            args=tuple(v&65535 for v in args);m=self.mem
            parent=m.open[-1]
            if self.compose and name in chain.CHAIN:
                fn,ptrs,_=chain.CHAIN[name];converted=[v if i in ptrs else walk.i16(v) for i,v in enumerate(args)]
                ret=self.run(fn.__name__,converted);parent['calls'].append((name,args,None if ret is None else ret&65535));return 0 if ret is None else ret
            m.leaf_writes=[]
            ret=tw.Services.call(self.services,name,args,m)
            writes=m.leaf_writes;m.leaf_writes=None
            parent['calls'].append((name,args,ret&0xffffffff))
            for f in m.open:f['leaves'].append((name,args,ret&0xffffffff,writes))
            return ret
    try:
        for address in (0x6923,0x4c51,0x32f0):
            for seed in (1,2):
                for index in range(300):
                    m,cfg,args,kwargs=ns['make_case'](suite,address,seed,index)
                    Driver(m,cfg,True,f'chain:{address:x}:{seed}:{index}').run(tw.FUNCTIONS[address],args,kwargs);roots+=1
                    if time.monotonic()>deadline:raise RuntimeError('wall-time budget exceeded')
            print(f'WROTE composed {tw.FUNCTIONS[address]}: 600 cases',flush=True)
        for name in sorted(names):
            address=name_addr[name]
            for seed in (1103,4709):
                for index in range(count):
                    m,cfg,args,kwargs=ns['make_case'](suite,address,seed,index)
                    Driver(m,cfg,False,f'isolated:{address:x}:{seed}:{index}').run(name,args,kwargs)
                    if time.monotonic()>deadline:raise RuntimeError('wall-time budget exceeded')
            print(f'WROTE isolated {name}: {2*count} cases',flush=True)
    except Exception:
        print('PARTIAL: fixture export failed; no cases are silently dropped',flush=True);raise
    finally:stream.close()
    (out/'summary.txt').write_text(f'composed_roots {roots}\nsamples {total}\n'+''.join(f'{n} {c}\n' for n,c in sorted(counts.items())))
    dump_runtime(oracle,gamefiles,out)
    print(f'PASS export: {roots} composed cases, {total} routine replays, 0 dropped',flush=True)
def dump_runtime(oracle, gamefiles, out):
    sys.path[:0]=[str(oracle/'tools/spec'),str(oracle/'tools')]
    import unicorn as uc
    from walk_support.base_engine import Machine, BASE, DGROUP, STACK, REGS
    m=Machine(oracle/'work/mt1/T.exe');D=DGROUP*16
    def setv(mem,off,value,n=2):mem[D+off:D+off+n]=(value&((1<<(8*n))-1)).to_bytes(n,'little')
    rows=[]
    for seed in (0,1,1103,4709,0xffffffff):
        for bound in (0,1,3,5,10,20,30,32767,-1,-5,-32768):
            mem=m.template.copy();setv(mem,0x5c20,seed,4)
            actual=m.run(mem,0x1b52c,[bound],far=True,segment=0x1b4b,limit=1000)
            rows.append(f'random {seed} {bound} {m.reg("AX")} {int.from_bytes(actual[D+0x5c20:D+0x5c24],"little")}')
    for x,y in ((0,0),(22,6),(1,1),(32767,-32768),(-1,32767),(100,100)):
        for high,stride in ((1,20),(3,60),(0,0),(255,32767)):
            mem=m.template.copy();setv(mem,0x5c3a,high,1);setv(mem,0x6d2b,stride)
            actual=m.run(mem,0x1b54c,[x,y],far=True,segment=0x1b4b,limit=1000)
            values=[int.from_bytes(actual[D+o:D+o+2],'little') for o in (0x6b6c,0x63f5,0x6415,0x6266,0x6c27,0x6420)]
            rows.append('position '+' '.join(map(str,[x,y,high,stride,*values])))
    import walk
    terrain_rows=[]
    for flags in range(256):
        for terrain_class in (-32768,-1,0,1,2,3,4,5,6,7,8,32767):
            mem=m.template.copy();setv(mem,0x2000,flags,1)
            args=[0x2000,DGROUP,terrain_class]
            expected=walk.foot_terrain(walk.State(mem,lambda *args: 0),*args)
            m.run(mem,0x1bfce,args,far=True,segment=0x1b4b,limit=1000)
            actual=m.reg('AX')
            assert actual==expected,('terrain oracle mismatch',flags,terrain_class,actual,expected)
            terrain_rows.append(f'{flags} {terrain_class} {actual}')
    (out/'terrain.txt').write_text('\n'.join(terrain_rows)+'\n')
    print(f'WROTE {len(terrain_rows)} foot_terrain cases: walk.py == original 1BFCE (test at 1BFD4)',flush=True)
    world=(gamefiles/'W.DAT').read_bytes();party=(gamefiles/'DEFAULT.SAV').read_bytes()
    (out/'runtime-before.bin').write_bytes(m.template[D:D+65536]+party)
    sizes={int(a,16):int(n) for a,n,*_ in (l.split() for l in (oracle/'work/fstarts_T.tsv').read_text().splitlines())}
    bases=(0,4,7,13,16,18,21,24,28)
    for w in range(8):
        for area in range(bases[w+1]-bases[w]):
            m=Machine(oracle/'work/mt1/T.exe')
            mem=m.template.copy();mem[0x60000:0x60000+len(party)]=party;setv(mem,0x6d65,0);setv(mem,0x6d67,0x6000)
            pos=0;seed=1;draws=[]
            def outside(cpu,at,size,user):
                nonlocal pos,seed
                target=at-BASE*16
                if target==0xfff0:return
                if any(lo<=target<lo+sizes[lo] for lo in (0xff4f,0xfe4d,0x1c2d0)):return
                hooks={0x1e9c1:('open',2),0x1eee0:('read',4),0x1f1b6:('seek',4),
                       0x1dfc4:('pic_free',2),0x1dd8e:('pic_load',5),
                       0x1f3d5:('memcpy',5),0x1d726:('setfile',3),0x1b52c:('random',1)}
                assert target in hooks,('unhooked init_world',hex(target))
                name,n=hooks[target];sp=m.reg('SP');stack=m.reg('SS')*16+sp
                args=struct.unpack('<'+'H'*n,bytes(cpu.mem_read(stack+4,n*2)));ret=0
                if name=='open':ret=5
                elif name=='seek':pos=args[1]|args[2]<<16;ret=pos
                elif name=='read':b=world[pos:pos+args[3]];cpu.mem_write(args[2]*16+args[1],b);pos+=len(b);ret=len(b)
                elif name=='memcpy':
                    cpu.mem_write(args[1]*16+args[0],bytes(cpu.mem_read(args[3]*16+args[2],args[4])))
                elif name=='random':
                    if args[0]:seed=(seed*22695477+1)&0xffffffff;draw=(seed>>16)&32767;draws.append(draw);ret=draw%args[0]
                ip,cs=struct.unpack('<HH',bytes(cpu.mem_read(stack,4)));cpu.reg_write(REGS['AX'],ret&65535);cpu.reg_write(REGS['DX'],ret>>16);cpu.reg_write(REGS['SP'],sp+4);cpu.reg_write(REGS['CS'],cs);cpu.reg_write(REGS['IP'],ip)
            lo=BASE*16+0xbe15;hi=lo+sizes[0xbe15]
            hooks=[m.cpu.hook_add(uc.UC_HOOK_CODE,outside,None,0,lo-1),m.cpu.hook_add(uc.UC_HOOK_CODE,outside,None,hi,0x10ffff)]
            try:actual=m.run(mem,0xbe15,[w,area],limit=100000)
            finally:
                for h in hooks:m.cpu.hook_del(h)
            filename=f'world-{w}-{area}.bin';(out/filename).write_bytes(actual[D:D+65536]+actual[0x60000:0x60000+len(party)])
            rows.append(f'world {w} {area} {seed} {filename} '+','.join(map(str,draws)))
    (out/'runtime.txt').write_text('\n'.join(rows)+'\n')
    print(f'WROTE {len(rows)} original-instruction RNG/position/world cases',flush=True)
if __name__=='__main__':
    if '--runtime-only' in sys.argv:
        watchdog(60);dump_runtime(*map(Path,sys.argv[1:4]))
    else:main()
