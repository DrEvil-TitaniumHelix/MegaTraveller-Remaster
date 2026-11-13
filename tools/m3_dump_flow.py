import sys,os,json
sys.dont_write_bytecode=True
ORACLE,GF,OUT=[os.path.abspath(a) if a!='-' else a for a in sys.argv[1:4]]
want=int(os.environ.get('MT3_FLOW_RUNS','8'))
marker=os.path.join(OUT,'done')
g=os.path.join(ORACLE,'tools','golden');sys.path.insert(0,g)
def stamp():
    fs=[os.path.join(g,n) for n in ('cg_golden.py','cg_spec.py','dosvm.py')]+[os.path.abspath(__file__)]
    return '%d %s'%(want,' '.join('%d:%d'%(int(os.path.getmtime(f)),os.path.getsize(f)) for f in fs if os.path.isfile(f)))
if os.path.isfile(marker) and open(marker).read().strip()==stamp():print('CACHED',OUT);sys.exit(0)
if not(os.path.isfile(os.path.join(ORACLE,'work','mt1','CG.exe')) and os.path.isfile(os.path.join(ORACLE,'orig','dos','MegaTra1','DCHARS.DAT'))):
    print('PARTIAL: oracle lacks work/mt1/CG.exe or orig DCHARS.DAT; no flow fixtures');sys.exit(0)
os.chdir(ORACLE)
import cg_golden
os.makedirs(OUT,exist_ok=True)
for f in os.listdir(OUT):os.remove(os.path.join(OUT,f))
def run(n,seed,cs,draft,disk):
    groups=[];inner=cg_golden.make_script(cs,draft)
    def choose(rec,i,o):
        k=inner(rec,i,o)
        groups.append(k if k is None else k);return k
    orig=cg_golden.make_script;cg_golden.make_script=lambda a,b:choose
    try:o,steps,cap,rec=cg_golden.run_original(seed,cs,draft,record=False,disk=disk,frames=False)
    finally:cg_golden.make_script=orig
    hx=lambda b:bytes(b).hex()
    L=['run %d %s %d %d'%(o.seed,'-' if cs is None else cs,int(draft),int(disk))]
    for i,s in enumerate(steps):
        k=groups[i-1] if i else None
        if k is None and i:k=[cg_golden.ENTER]
        L.append('step %d keys %s state %d'%(i,'-' if not k else ','.join(map(str,k)),s['rand_state']))
        L.append('rand '+' '.join(map(str,s['rand'])));L.append('work '+s['work'])
        for kind,v in s['text']:
            b=v.encode('latin1') if kind!='menu' else b'\0'.join(x.encode('latin1') for x in v)
            L.append('text %s %s'%(kind,b.hex()))
    L.append('widths '+hx(o.vm.rb(o.vm.lin(o.vm.r('DS'),0x358e),256)))
    if cap:
        L.append('pool %d %s'%(cap['slot'],hx(cap['record'])));L.append('skills '+','.join('%d:%d'%x for x in cap['skills']))
        f=o.vm.vfs.get('UCHARS.DAT')
        if f is not None:L.append('uchars '+hx(f))
    open(os.path.join(OUT,'run_%02d.txt'%n),'w').write('\n'.join(L)+'\n')
    print('WROTE run_%02d: seed %d, %d steps, finished %s'%(n,o.seed,len(steps),bool(cap)))
for n,(seed,cs,draft,*more) in enumerate(cg_golden.RUNS[:want]):run(n,seed,cs,draft,bool(more and more[0]))
open(marker,'w').write(stamp())
