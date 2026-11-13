"""Read-only fixtures from the proven input/render_t specs; no game outputs saved.

Invoked by ground_input_oracle.rs. Fixed case budgets and a 45-second watchdog.
"""
import os
from pathlib import Path
import random
import struct
import sys
import threading

watchdog = threading.Timer(45, lambda: os._exit(124))
watchdog.daemon = True
watchdog.start()
sys.dont_write_bytecode = True
oracle, game = map(Path, sys.argv[1:3])
sys.path[:0] = [str(oracle / "tools/spec"), str(oracle / "tools")]
import input as spec
from combat import Far
from render_t import decode_dialogue
from data_formats import msg_dec, dict_dec
from unexepack import unpack

class Memory:
    def __init__(self): self.data = bytearray(1 << 20)
    def u8(self,p): return self.data[p.linear]
    def i16(self,p): return struct.unpack_from('<h',self.data,p.linear)[0]
    def put8(self,p,v): self.data[p.linear] = v & 255
    def put16(self,p,v): struct.pack_into('<H',self.data,p.linear,v & 65535)

packed = unpack((game/'T.DAT').read_bytes())
exe = packed[0] if isinstance(packed, tuple) else packed
header = struct.unpack_from('<H',exe,8)[0]*16
data = bytearray(exe[header+0x20620:]); data.extend(bytes(max(0,65536-len(data))))
for i in range(struct.unpack_from('<H',exe,6)[0]):
    at = struct.unpack_from('<H',exe,0x18)[0]+4*i
    off,seg = struct.unpack_from('<HH',exe,at)
    at=off+seg*16-0x20620
    if 0<=at<len(data)-1: struct.pack_into('<H',data,at,(struct.unpack_from('<H',data,at)[0]+0x1000)&65535)
m=Memory();m.data[0x30620:0x40620]=data[:65536]
s=spec.State(m,None,spec.T)
for code in range(256): print('K',code,spec.translated_key(s,code))
for selected in range(8):
    for code in range(256):
        action,key=spec.inventory_action('T',code,selected)
        print('I',selected,code,action,key)

class Services:
    def __init__(self,keys=()): self.keys=iter(keys);self.ticks=0;self.polls=0
    def call(self,s,name,args):
        if name=='memcpy':
            dest,src,n=args;m=s.m;m.data[dest.linear:dest.linear+n]=m.data[src.linear:src.linear+n]
        elif name=='strchr':
            p,ch=args;start=p.linear;stop=s.m.data.index(0,start)
            i=s.m.data[start:stop+1].find(bytes([ch&255]))
            return Far((p.offset+i)&65535,p.segment) if i>=0 else Far(0,0)
        elif name=='ticks': return self.ticks
        elif name=='poll_mouse':
            self.polls+=1
            assert self.polls<=20,'PARTIAL: selector did not consume a bounded key script'
            code,delta=next(self.keys);self.ticks=(self.ticks+delta)&0xffffffff;s.b(spec.T.key,code)
        elif name=='mouse_hotspots': spec.mouse_hotspots(s,args[0])
        elif name not in ('hide_cursor','show_cursor','front_buffer','back_buffer','outline'):
            raise AssertionError(name)

rng=random.Random(4711)
for case in range(1400):
    count=rng.randrange(1,8);records=[]
    for i in range(count):
        values=[rng.randrange(-100,340),rng.randrange(-100,340),rng.randrange(-10,400),rng.randrange(-10,400)]
        if case%11==0: values=[32000,0,1000,40]
        records.append(struct.pack('<hhhhB',*values,(i+1 if case%13 else 0)))
    row,col=rng.randrange(-100,340),rng.randrange(-100,340)
    if case%3==0:
        top,left,h,w=struct.unpack('<hhhh',records[0][:8]);row,col=top,left
    rawx=(col*2)&65535
    services=Services();ss=spec.State(Memory(),services,spec.T,{'AX':4,'BX':case%4,'CX':rawx,'DX':row&65535})
    spec.mouse_callback(ss)
    packed=b''.join(records)+bytes(9);ss.m.data[ss.ds*16+0x120:ss.ds*16+0x120+len(packed)]=packed
    spec.mouse_hotspots(ss,ss.pointer(0x120))
    print('H',case,row,rawx>>1,case%4,packed.hex(),ss.b(spec.T.key),ss.b(spec.T.mouse_pending),ss.w(spec.T.mouse_event))

for n in (1,3,6):
    for code in range(256):
        keys=[(code,0),(1,0)]
        services=Services(keys);ss=spec.State(Memory(),services,spec.T)
        ss.m.data[0x30620:0x40620]=data[:65536]
        hot=ss.pointer(0x8000);ss.m.data[hot.linear:hot.linear+7]=b'QCT123\0'
        ss.w(spec.T.mouse_event+4,171);ss.w(spec.T.mouse_event+6,200)
        result=spec.rect_menu(ss,ss.pointer(0x480B),n,-2,6,7,hot)
        print('R',n,code,result)

messages=msg_dec((game/'TRAV.MSG').read_bytes())['msgs']
dictionary=dict_dec((game/'TRAV.DAT').read_bytes())['words']
for index,tokens in enumerate(messages):
    for width,mode in ((192,0),(189,0),(48,1),(6,2)):
        lines,short=decode_dialogue(tokens,dictionary,width,index,mode)
        print('D',index,width,mode,','.join(x.hex() or '_' for x in lines) or '-',short.hex() if short is not None else '-')
print('PASS',len(messages))
sys.stdout.flush()
watchdog.cancel()
