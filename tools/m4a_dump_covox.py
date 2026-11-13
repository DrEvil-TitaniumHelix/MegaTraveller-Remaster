"""Finite DS-state + port-write proof against the decoded, CPU-verified 1E09E model.

All nineteen player-owned VMFs plus raw/header/rate/device/abort edge cases. Each case
also records the model's port writes in order (PIT 42h/43h, speaker 61h, DAC/parallel
ports); the PIC mask 21h writes are dropped: their value is the harness stub's read, not
game state. Runtime-only fixtures under target; MT1_ORACLE is read-only.
"""
import os, random, struct, sys
from pathlib import Path
sys.dont_write_bytecode=True
oracle,gamefiles,out=map(Path,sys.argv[1:4])
sys.path[:0]=[str(oracle/'tools/beh_t'),str(oracle/'tools')]
import equivalents as model
from engine import Machine, D, MEM_SIZE, w16
from services import Services

class Ports:
    def __init__(self,refresh):self.refresh=refresh;self.writes=[]
    def read(self,port):
        if port==0x61:
            self.refresh^=0x10
            return self.refresh
        return 0
    def write(self,port,value):
        if port!=0x21:self.writes.append((port&0xffff,value&255))

class SpeakerServices(Services):
    def __init__(self,ports):super().__init__();self.ports=ports
    def call(self,name,args,regs,memory):
        if name=='speaker_begin':return {'AX':model.speaker_begin(memory,self.ports)},0
        if name=='speaker_end':model.speaker_end(memory,self.ports);return {},0
        return super().call(name,args,regs,memory)

machine=Machine(oracle/'work/mt1/T.exe')
signature=bytes(machine.template[D+0x536e:D+0x5376])
rng=random.Random(0x1E09E)
cases=[]
for path in sorted(gamefiles.glob('*.VMF')):
    cases.append((path.name,path.read_bytes()[128:],132,8,False,False))
assert len(cases)==19,'PARTIAL: expected nineteen VMFs'
for i in range(120):
    length=(0,1,2,8,15,16,17,18,19,32,256)[i%11]
    blob=rng.randbytes(length)
    if i%4 and length:
        header=signature[:7]+bytes([signature[7]^(255 if i%4==2 else 0),7 if i%4==3 else 8,(0,1,132,255)[i%4]])+bytes(6)
        blob=(header+blob)[:length]
    cases.append((f'edge-{i}',blob,(0,1,132,255,256)[i%5],i%10,bool(i%2),i%7==0))
out.mkdir(parents=True,exist_ok=True)
with (out/'cases.bin').open('wb') as f:
    f.write(struct.pack('<I',len(cases)))
    for name,data,rate,device,diff,abort in cases:
        memory=bytearray(MEM_SIZE)
        memory[D:D+65536]=machine.template[D:D+65536]
        memory[D+0x5588:D+0x55a4]=rng.randbytes(28)
        memory[0x50000:0x50000+len(data)]=data
        w16(memory,0x41a,0x1e);w16(memory,0x41c,0x20 if abort else 0x1e)
        before=bytes(memory[D:D+65536])
        refresh=rng.choice((0,16));ports=Ports(refresh)
        result=model.play_covox(memory,SpeakerServices(ports),ports,(0,0x5000),len(data),rate,device,diff)
        encoded=name.encode('ascii')
        f.write(struct.pack('<I',len(encoded))+encoded+before)
        f.write(struct.pack('<I',len(data))+data)
        f.write(struct.pack('<7H',rate,device,diff,abort,result,refresh,ports.refresh))
        f.write(memory[D:D+65536])
        f.write(struct.pack('<I',len(ports.writes))+b''.join(struct.pack('<HB',p,v) for p,v in ports.writes))
print(f'PASS Covox fixture: {len(cases)} cases, nineteen VMFs; oracle beh_t play_covox (1E09E)')
