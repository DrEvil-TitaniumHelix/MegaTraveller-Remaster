import sys,os,json
def setup(oracle_tools,gamefile):
    sys.path.insert(0,oracle_tools)
    import data_formats
    data_formats.D=os.path.dirname(os.path.abspath(gamefile))+os.sep
    return data_formats
def canon(v):
    if isinstance(v,dict): return {str(k):canon(x) for k,x in v.items()}
    if isinstance(v,(list,tuple)): return [canon(x) for x in v]
    if isinstance(v,(bytes,bytearray)): return bytes(v).hex()
    return v
def dump(v): return json.dumps(canon(v),sort_keys=True,separators=(',',':'))
def main(table):
    """table: format name -> function(data_formats module, bytes) -> value."""
    oracle,fmt,path=sys.argv[1:4]
    df=setup(oracle,path)
    print(dump(table[fmt](df,open(path,'rb').read())))
