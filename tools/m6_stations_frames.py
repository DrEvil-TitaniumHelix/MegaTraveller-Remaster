"""Export S11–S20 raster calls and independent full-plane expected frames.

The existing oracle screen matrix executes original drawing decisions; its
render_s_stations.Raster draws independent expectations and Observer checks
those against original VRAM. This adapter writes solely to --output's folder.
No oracle log, ledger, image, notes or game files are changed.
"""
import argparse
from collections import Counter
import os
from pathlib import Path
import struct
import sys
sys.dont_write_bytecode=True
p=argparse.ArgumentParser();p.add_argument('output',type=Path);a=p.parse_args()
oracle=Path(os.environ['MT1_ORACLE']);sys.path[:0]=[str(oracle/'tools'),str(oracle/'tools/spec')]
import test_render_s_stations as matrix
from render_s_stations import Observer
from render_s_cg import Renderer

def n(v):return struct.pack('<I',v&0xffffffff)
def string(s):b=s.encode();return n(len(b))+b
def patches(old,new):
    chunks=[(i,new[i:i+256])for i in range(0,len(new),256)if old[i:i+256]!=new[i:i+256]]
    return n(len(chunks))+b''.join(n(i)+n(len(b))+b for i,b in chunks)

a.output.parent.mkdir(parents=True,exist_ok=True)
matrix.WORK=a.output.parent
matrix.log=lambda line:print(line,flush=True)
Renderer.save_png=lambda *args,**kwargs:None
counts=Counter()
with a.output.open('wb')as out:
    out.write(b'SFRAME01')
    class Export(Observer):
        def __init__(self,vm):
            super().__init__(vm);original=self.raster.draw
            def draw(address,args,memory,ds):
                if self.recording:
                    out.write(b'D'+n(address)+n(len(args))+b''.join(n(x)for x in args)+patches(self.previous,memory));self.previous=memory
                original(address,args,memory,ds)
            self.raster.draw=draw;self.recording=False
        def reset(self):
            super().reset();self.recording=True;self.previous=self.vm.rb(0,0xA0000)
            out.write(b'S'+n(self.vm.ds)+n(len(self.previous))+self.previous+self.raster.renderer.snapshot())
        def compare(self,label):
            super().compare(label)
            if isinstance(label,tuple)and isinstance(label[0],str)and label[0]in matrix.SCENES:
                counts[label[0]]+=1
                out.write(b'F'+string(f'{label[0]}-{label[1]}')+self.raster.renderer.snapshot());self.recording=False
    matrix.Observer=Export
    scenes=','.join(x for x in matrix.SCENES if x not in ('quit','death'))
    for video in ('0','3'):matrix.matrix(0x4C342026,video,1,scenes)
    out.write(b'E')
for scene,count in sorted(counts.items()):print(f'FRAMES {scene}: {count}',flush=True)
print(f'Exported {sum(counts.values())} full-plane frames',flush=True)
