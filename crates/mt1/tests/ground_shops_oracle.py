"""Read-only services.py fixture export; no oracle ledger/artifact writes."""
import sys,struct
from pathlib import Path
sys.dont_write_bytecode=True
sys.path.insert(0,str(Path(sys.argv[1])/'tools/spec'))
import services as spec
from trade import ds,advance

class Memory:
    def __init__(self): self.data=bytearray(0x100000)
    def read(self,p,n):
        at=p[1]*16+p[0]; return self.data[at:at+n]
    def write(self,p,b):
        at=p[1]*16+p[0]; self.data[at:at+len(b)]=b
    def byte(self,p): return self.read(p,1)[0]
    def word(self,p): return int.from_bytes(self.read(p,2),'little')
    def pointer(self,p): return struct.unpack('<HH',self.read(p,4))
    def put_byte(self,p,v): self.write(p,bytes([v&255]))
    def put_word(self,p,v): self.write(p,struct.pack('<H',v&65535))
class Context:
    def __init__(self): self.mem=Memory()
    def copy(self,src,dst,n): self.mem.write(dst,self.mem.read(src,n))

ranges=((0,9),(10,12),(13,21),(22,26),(28,29),(30,33),(34,36),(37,45),(46,47),(48,59),(60,64),(70,78))
c=Context()
for i,(lo,hi) in enumerate(ranges): c.mem.write(ds(0x209e+4*i),struct.pack('<hh',lo,hi))
for v in range(-32,121): print('R',v,spec.reel_symbol(c,v))
symbols=[spec.reel_symbol(c,v) for v in range(78)]
for a in range(78):
    values=[]
    for b in range(78):
        for d in range(78):
            base,mul=spec.casino_award((a,b,d),(symbols[a],symbols[b],symbols[d]))
            values.append(base*mul)
    print('P',a,struct.pack('<'+str(len(values))+'I',*values).hex())
for row in spec.casino_odds(): print('D',row['skill'],row['denominator'],row['winning_numerator'],row['payout_numerator'])
for mode in range(3):
    for first in [0,1,127,255,256]:
        for item in [0,29,34,45,255]:
            c=Context();m=c.mem
            m.write(ds(0x6d65),struct.pack('<HH',0,0x5000));m.write(ds(0x6d6d),struct.pack('<HH',0,0x7000))
            for i in range(256):m.write(spec.object_record(c,i),bytes([255 if i>=first else 7,0xa5,0x3c,0xde,0xad]))
            m.put_byte(spec.item(c,item,0x27),item*31);m.put_byte(spec.item(c,item,0x28),item*7)
            ret=[spec.alloc_object,spec.alloc_object_full,spec.alloc_object_s][mode](c,item)
            print('A',mode,first,item,ret,m.read((0xa82,0x5000),1280).hex())
print('PASS')
