"""Oracle JSON. Reads originals and the oracle without writing bytecode or dumps."""
import sys

sys.dont_write_bytecode = True
from m1_dump_common import main


def exepack(_df, data):
    from unexepack import unpack
    return unpack(data)


if __name__ == '__main__':
    main({
        'STP': lambda df, data: df.stp_dec(data),
        'VMF': lambda df, data: df.vmf_dec(data),
        'EXEPACK': exepack,
    })
