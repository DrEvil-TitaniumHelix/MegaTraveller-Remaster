"""Export independent Python station contracts to the Rust differential driver.

No original-machine execution/output is used as expected data. Oracle modules
and assets are read-only; generated memory fixtures live only under target/.
Every service boundary checks caller writes/arguments and supplies only that
callee's effects. No expected memory is loaded by the shipping game.
"""
import argparse
from collections import Counter
import os
from pathlib import Path
import struct
import sys

sys.dont_write_bytecode = True
p = argparse.ArgumentParser()
p.add_argument('output', type=Path)
p.add_argument('--count', type=int, default=32)
a = p.parse_args()
oracle = Path(os.environ['MT1_ORACLE'])
sys.path[:0] = [str(oracle/'tools'), str(oracle/'tools/spec')]
import test_s_stations as stations
import test_space_tick as timing


def u32(n): return struct.pack('<I', n & 0xffffffff)
def string(s):
    b = s.encode(); return u32(len(b)) + b
def values(xs): return u32(len(xs)) + b''.join(u32(x) for x in xs)
def patches(memory, addresses):
    addresses = sorted(addresses)
    return u32(len(addresses)) + b''.join(u32(i)+bytes([memory[i]]) for i in addresses)
def changes(base, memory):
    for start in range(0, len(base), 256):
        if base[start:start+256] != memory[start:start+256]:
            yield from (i for i in range(start, min(start+256,len(base))) if base[i] != memory[i])


def recorder(cls):
    class Recorder(cls):
        def __init__(self, *args):
            super().__init__(*args); self.events = []
        def take(self):
            data = patches(self.memory, self.memory.written)
            self.memory.written.clear()
            return data
        def call(self, name, *args, registers=None):
            before = self.take()
            result = super().call(name, *args, registers=registers)
            normalized = [x & 65535 for x in args]
            if name == 'struct_copy': normalized.append(registers['CX'])
            if name == 'draw_character': normalized[0] &= 255
            if name == 'shift_string': normalized[2] &= 255
            event = string(name)+values(normalized)+before
            event += u32(result.get('AX',0)&65535)+u32(result.get('DX',0)&65535)+self.take()
            self.events.append(event)
            return result
    return Recorder


a.output.parent.mkdir(parents=True, exist_ok=True)
counts = Counter()
with a.output.open('wb') as out:
    base = bytes(stations.Suite(stations.SEEDS[0],0).machine.template)
    out.write(b'STATION2'+u32(len(base))+base)
    def emit(routine, function, memory, args, cfg, context, ds):
        env = recorder(context)(memory.copy(), ds, cfg)
        result = function(stations.spec.State(env.memory, ds), env, *args)
        final = env.take()
        out.write(b'C'+string(routine)+u32(ds)+u32(cfg.get('entry_di',0x2468))+values(args)+patches(memory,changes(base,memory)))
        out.write(u32(len(env.events)))
        for event in env.events: out.write(event)
        out.write(final)
        defined = isinstance(result, (int,dict))
        reply = result if isinstance(result,dict) else {'AX':result or 0}
        out.write(u32(2 if cfg.get('key_cycle') else defined)+u32(reply.get('AX',0)&65535)+u32(reply.get('DX',0)&65535))
        counts[routine] += 1
    for seed in stations.SEEDS:
        suite = stations.Suite(seed, a.count)
        for address in (0xE60,0x121C,0x11D6,0x6235,0x6274,0x34BC,0x3598,0xB206,0xB003,
                        0x2F65,0x3681,0x25DE,0x2C5C,0x312F,0x1C1C,0x1085B,0x12659,0x11677,0xA456):
            routine = stations.FUNCTIONS[address]
            for memory,args,cfg in suite.cases(address):
                emit(routine,getattr(stations.spec,routine),memory,args,cfg,stations.Context,suite.machine.dgroup)
            print(f'Export {seed:X} {routine}: {counts[routine]}',flush=True)
        class GameContext(stations.Context):
            def call(self,name,*args,registers=None):
                if name in ('pilot','gun','navigation','engineering','communications','crew'):
                    self.trace.append((name,args,()));stations.spec.State(self.memory,self.ds).station=self.next('stations',-1)
                    return {'AX':0}
                if name=='direction_sector':
                    self.trace.append((name,args,()));return {'AX':((args[0]//64)+(args[1]//64)*4)&65535}
                return super().call(name,*args,registers=registers)
        for requested in range(-1,21):
            for selected in range(5):
                memory,s=suite.fixture();s.requested_station=requested;s.pbyte(1,selected)
                for i in range(12):s.pbyte(0xA0+i,1+(i+selected)%5)
                emit('dispatch_ship_stations',stations.spec.dispatch_ship_stations,memory,(),{'stations':[13,14,15,17,-1]},GameContext,suite.machine.dgroup)
        for mode in range(-8,9):
            for crew in range(6):
                memory,s=suite.fixture();s.station=15;s.pbyte(0xA5,crew);s.object_count=6
                for i in range(6):s.set('world_ids',i,i);s.set('object_x',i*32,i);s.set('object_y',i*16,i)
                emit('redraw_communications',stations.spec.redraw_communications,memory,(mode,),{},GameContext,suite.machine.dgroup)
        suite = timing.Suite(seed,a.count)
        for address,kind in ((0x28D8,'station_cases'),(0xAD01,'station_cases'),(0x4707,'modal_cases')):
            routine = timing.FUNCTIONS[address][0]
            for memory,args,cfg in getattr(suite,kind)(address):
                function=timing.spec.station_key_cycle if cfg.get('key_cycle') else getattr(timing.spec,routine)
                emit(routine,function,memory,args,cfg,timing.Context,suite.machine.dgroup)
            print(f'Export {seed:X} {routine}: {counts[routine]}',flush=True)
    out.write(b'E')
for routine,count in sorted(counts.items()): print(f'{routine}: {count}')
print(f'Exported {sum(counts.values())} independent cases to {a.output}',flush=True)
