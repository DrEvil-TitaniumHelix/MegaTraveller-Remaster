"""Read-only native 1D8DA check used by mt1's ending unit test. No oracle writes.

Execute original relocated instructions, including its hex-byte helper and CRT
copy/date routines. Supply DOS/BIOS/timer inputs, observe drawing calls, and
answer three non-Esc keys before Esc at the code panel.
"""
from pathlib import Path
import struct
import sys

sys.dont_write_bytecode = True
root = Path(sys.argv[1])
sys.path.insert(0, str(root / 'tools'))
import unicorn as uc
from asm_equiv.engine import Machine, BASE, DGROUP, REGS


class Ending(Machine):
    def __init__(self, case):
        self.case = case
        self.calls = []
        self.keys = iter([32, 32, 28, 1])
        super().__init__(root / 'work/mt1/T.exe')
        self.cpu.hook_add(uc.UC_HOOK_CODE, self.call)

    def _interrupt(self, cpu, number, user):
        assert number == 0x21
        ah = self.reg('AX') >> 8
        if ah == 0x30:
            cpu.reg_write(REGS['AX'], self.case['major'] | self.case['minor'] << 8)
        elif ah == 0x2A:
            cpu.reg_write(REGS['CX'], self.case['year'])
            cpu.reg_write(REGS['DX'], self.case['month'] << 8 | self.case['day'])
        else:
            raise AssertionError(('unexpected DOS service', ah))

    def ret(self, result=0):
        sp = self.reg('SP')
        ip, cs = struct.unpack('<HH', self.cpu.mem_read(self.reg('SS') * 16 + sp, 4))
        for key, value in dict(AX=result, DX=0, CS=cs, IP=ip, SP=sp + 4).items():
            self.cpu.reg_write(REGS[key], value)

    def call(self, cpu, address, size, user):
        at = address - BASE * 16
        stack = self.reg('SS') * 16 + self.reg('SP') + 4
        if at == 0x1DAF7:
            self.ret(self.case['timer'])
        elif at == 0x5488:
            self.calls.append(['message', *struct.unpack('<HH', cpu.mem_read(stack, 4))])
            self.ret()
        elif at == 0xD31A:
            key = next(self.keys)
            self.calls.append(['key', key])
            self.ret(key)
        elif at == 0x10562:
            off, seg, reset, color = struct.unpack('<HHHH', cpu.mem_read(stack, 8))
            text = bytes(cpu.mem_read(seg * 16 + off, 128)).split(b'\0')[0].decode('latin1')
            self.calls.append(['say', text, reset, color])
            self.ret()
        elif at in (0xF50B, 0xEA9D):
            self.ret()


cases = []
for i, timer in enumerate([0, 1, 0x1234, 0xABCD, 0xFFFF, 0xFF00, 0x00FF, 0x5678]):
    case = dict(major=3 if i < 4 else 6, minor=30 if i < 4 else 22, timer=timer,
                bios='010190' if i < 4 else '122599', model=0xFC if i < 4 else 0xFF,
                year=1990 if i < 4 else 2026, month=10 if i < 4 else 11, day=4 if i < 4 else 11)
    machine = Ending(case)
    memory = bytearray(machine.template)
    bios = case['bios']
    memory[0xFFFF5:0xFFFFD] = f'{bios[:2]}/{bios[2:4]}/{bios[4:]}'.encode()
    memory[0xFFFFE] = case['model']
    machine.run(memory, 0x1D8DA, [190, 191], far=True, segment=0x1B4B, limit=50000)
    calls = machine.calls
    assert calls[:4] == [['message', 190, 0], ['key', 32], ['message', 191, 0], ['key', 32]], calls
    assert calls[4] == ['say', 'Here is your special winning code number!:', 1, 6], calls
    assert calls[5][0] == 'say' and len(calls[5][1]) == 28 and calls[5][2:] == [0, 4], calls
    assert calls[6:] == [['say', 'Record it and press ESC to return to DOS', 0, 6], ['key', 28], ['key', 1]], calls
    case['code'] = calls[5][1]
    cases.append(case)
for case in cases:
    print('\t'.join(str(case[k]) for k in ('major', 'minor', 'timer', 'bios', 'model', 'year', 'month', 'day', 'code')))
