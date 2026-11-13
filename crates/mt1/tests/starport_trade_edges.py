"""Run the read-only trade.py routines with their own scripted test context.

Only external menus/keys are scripted; neither tested algorithm is copied here.
"""
from pathlib import Path
import sys

oracle = Path(sys.argv[1])
sys.path[:0] = [str(oracle / 'tools/spec'), str(oracle / 'tools')]
import trade as spec
from test_trade import Context, Machine, Memory

machine = Machine()

class CaptureMemory(Memory):
    damage = None

    def put_byte(self, pointer, value):
        if pointer == (0xC6, 0x5000):
            self.damage = value
        super().put_byte(pointer, value)

def context(config):
    memory = machine.template.copy()
    mem = CaptureMemory(data=memory)
    mem.write((0, 0x5000), bytes(65536))
    mem.write((0, spec.SS_SEG), bytes(65536))
    mem.put_pointer(spec.ds(0x6D65), (0, 0x5000))
    return Context(mem, config)

if sys.argv[2] == 'repair':
    for base, tech, damage, money in [
        (100, 0, 100, 10001), (255, 0, 128, 99999),
        (255, 0, 129, 99999), (200, 5, 100, 49201),
        (255, 6, 255, 99999), (255, 255, 255, 99999),
        (200, 5, 100, 0xFFFF8000), (200, 5, 100, 0xFFFFFFFF),
        (200, 5, 100, 30000001),
    ]:
        c = context({'choose_payer': [-2, -1], 'shop_menu': [1, 0],
                     'read_key': [ord('A'), ord('P')]})
        m = c.mem
        m.put_byte(spec.ds(0x1FF7), base)
        m.put_byte(spec.ds(0x6DEE), tech)
        m.put_byte(spec.party(c, 0xC6), damage)
        m.put_dword(spec.account(c, -2), money)
        spec.repair_ship(c)
        cost = base // 5 * tech + base
        print(cost, damage, money, m.damage, m.byte(spec.party(c, 0xC6)),
              m.dword(spec.account(c, -2)))
elif sys.argv[2] == 'supplies':
    c = context({'choose_payer': [-2, -1], 'shop_menu': [2, 0, 0]})
    m = c.mem
    m.put_dword(spec.account(c, -2), 99999)
    m.put_byte(spec.ds(0x1D6B), 0)
    m.put_byte(spec.ds(0x1D6C), 6)
    for i, price in enumerate([0, 10, 20, 30, 0, 40]):
        pointer = (0x1000 + i * 64, 0x6000)
        m.text(pointer, f'Item{i}'.encode())
        m.put_pointer(spec.ds(0x1CDB + i * 4), pointer)
        m.put_dword(spec.ds(0x1D9D + i * 4), price)
        m.put_byte(spec.ds(0x1D7D + i), i + 1)
        previous = (0x2000 + i * 64, 0x6000)
        m.text(previous, f'Previous{i}'.encode())
        m.put_pointer(spec.advance(spec.local(0x14E), i * 4), previous)
    spec.ship_supplies(c)
    menus = [entry for entry in c.trace if entry[0] == 'shop_menu']
    _, args, extra = menus[1]
    assert args[4:6] == (6, 6)
    for (_, name), limit, price in zip(extra[1], extra[2], extra[3]):
        print(name.hex(), limit, price)
elif sys.argv[2] == 'cancel':
    for routine in (spec.refuel, spec.repair_ship):
        c = context({'choose_payer': [-2, -1], 'shop_menu': [1, 0], 'read_key': [27]})
        m = c.mem
        m.put_byte(spec.party(c, 0xC6), 10)
        m.put_dword(spec.account(c, -2), 99999)
        m.put_dword(spec.account(c, -4), 0x12345678)
        routine(c)
        assert m.dword(spec.account(c, -4)) == (99999 if routine == spec.refuel else 0x12345678)
        assert m.dword(spec.account(c, -2)) == 99999
        print(routine.__name__, spec.account(c, -4), hex(m.dword(spec.account(c, -4))))
    c.mem.put_pointer(spec.ds(0x6D65), (0, 0x9000))
    assert spec.account(c, -4) == (0xFDFE, 0x9000)
    print('launcher account(-4): 9000:FDFE, not DS:6B63')
else:
    raise ValueError(sys.argv[2])
