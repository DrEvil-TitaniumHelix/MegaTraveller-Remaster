import sys,os;sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from m1_dump_common import main
main({'W.DAT':lambda df,d:df.world_dec(d),'TRAV.DAT':lambda df,d:df.dict_dec(d),'TRAV.MSG':lambda df,d:df.msg_dec(d),
      'TRAV.ITM':lambda df,d:df.itm_dec(d),'CHARS':lambda df,d:df.chars_dec(d),'SAV':lambda df,d:df.sav_dec(d)})
