import sys,os,random,shutil
sys.dont_write_bytecode=True
ORACLE,GF,OUT=sys.argv[1:4]
sys.path.insert(0,os.path.join(ORACLE,'tools','golden'))
from cg_spec import MsRand,roll_attributes,add_to_pool,save_pool,Tables
from cg_golden import RUNS
shutil.rmtree(OUT,ignore_errors=True);os.makedirs(OUT)
SEEDS=[r[0] for r in RUNS if r[0] is not None]+[0xFFFF]
with open(os.path.join(OUT,'rng.txt'),'w') as f:
    for s in SEEDS:
        r=MsRand(s);v=[r() for _ in range(10000)];f.write('%d %d %s\n'%(s,r.state,' '.join(map(str,v))))
with open(os.path.join(OUT,'attr.txt'),'w') as f:
    for s in SEEDS:f.write('%d %s\n'%(s,' '.join(map(str,roll_attributes(MsRand(s))))))
print('WROTE rng/attr for seeds',SEEDS)
exe=os.path.join(ORACLE,'work','mt1','CG.exe');dch=os.path.join(ORACLE,'orig','dos','MegaTra1','DCHARS.DAT')
if not(os.path.isfile(exe) and os.path.isfile(dch)):print('PARTIAL: no work/mt1/CG.exe or orig DCHARS.DAT in the oracle; pool cases skipped');sys.exit(0)
t=Tables(exe);open(os.path.join(OUT,'tables.bin'),'wb').write(bytes(t.ds[0x1184:0x1A00]))
base=open(dch,'rb').read();recs=[base[1+256*k:257+256*k] for k in range(base[0])]
first=None
for k in range(24):
    rng=random.Random(k);w=bytearray(57)
    w[0]=rng.randrange(3);n=rng.choice([1,6,14,15]);w[1:1+n]=bytes(rng.randrange(65,91) for _ in range(n))
    w[16:22]=bytes(rng.randrange(2,16) for _ in range(6));w[22]=rng.randrange(5);w[23]=rng.randrange(7);w[24]=rng.randrange(18,70)
    w[25]=rng.randrange(256);w[26]=rng.randrange(256);w[27]=rng.randrange(256);w[28]=rng.randrange(256);w[29]=rng.randrange(256)
    w[30:46]=bytes((rng.randrange(0x60) if rng.random()<0.6 else -1)&0xFF for _ in range(16))
    w[46:50]=rng.randrange(1<<32).to_bytes(4,'little');w[50:52]=rng.randrange(30001).to_bytes(2,'little');w[52]=rng.randrange(2)
    w[53:55]=rng.randrange(1<<16).to_bytes(2,'little');w[55]=rng.randrange(256);w[56]=rng.randrange(256)
    skills=[(rng.randrange(95),rng.randrange(8)) for _ in range(rng.randrange(13))]
    before=recs[k%len(recs)];rec=add_to_pool(bytes(w),skills,t,before)
    d=os.path.join(OUT,'pool_%02d'%k);os.makedirs(d)
    open(os.path.join(d,'work.bin'),'wb').write(bytes(w));open(os.path.join(d,'before.bin'),'wb').write(before);open(os.path.join(d,'record.bin'),'wb').write(rec)
    open(os.path.join(d,'skills.txt'),'w').write(''.join('%d %d\n'%s for s in skills))
    if first is None:first=rec
open(os.path.join(OUT,'uchars.bin'),'wb').write(save_pool(recs+[first]))
print('WROTE 24 pool cases over',len(recs),'DCHARS records')
