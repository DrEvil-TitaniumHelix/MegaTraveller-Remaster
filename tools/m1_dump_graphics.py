import os,sys
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import m1_dump_common as C
C.main({'pic':lambda df,d:df.pic_dec(d),'ly':lambda df,d:df.ly_dec(d),'pal':lambda df,d:df.pal_dec(d),'piccont':lambda df,d:df.piccont_dec(d)})
