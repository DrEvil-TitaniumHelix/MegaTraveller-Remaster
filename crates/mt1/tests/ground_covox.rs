mod oracle_env;
use mt1::ground::{state::Mem,trace::Trace,walk::covox_state};
use std::{collections::BTreeMap,io::Read,path::PathBuf,process::Command};

#[test]
fn covox_state_matches_oracle() {
    let Some((oracle,files))=oracle_env::both() else {return};
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out=root.join("target/m4a-fixtures/covox");
    let result=Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds","90","--mib","1536","--","python","-B"])
        .arg(root.join("tools/m4a_dump_covox.py")).arg(oracle).arg(files).arg(&out).output().unwrap();
    assert!(result.status.success(),"{}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    let mut f=std::io::BufReader::new(std::fs::File::open(out.join("cases.bin")).unwrap());
    fn bytes(f:&mut impl Read,n:usize)->Vec<u8> {let mut b=vec![0;n];f.read_exact(&mut b).unwrap();b}
    fn word(f:&mut impl Read)->u16 {u16::from_le_bytes(bytes(f,2).try_into().unwrap())}
    fn number(f:&mut impl Read)->usize {u32::from_le_bytes(bytes(f,4).try_into().unwrap()) as usize}
    let count=number(&mut f);assert_eq!(count,139);let mut port_writes=0usize;
    for _ in 0..count {
        let size=number(&mut f);let name=String::from_utf8(bytes(&mut f,size)).unwrap();
        let mut mem=Mem {ds:bytes(&mut f,65536),segs:BTreeMap::new(),trace: Trace::default(), bios_ticks: [0; 4] };
        let size=number(&mut f);let data=bytes(&mut f,size);
        let (rate,device,diff,abort,ret)=(word(&mut f),word(&mut f),word(&mut f),word(&mut f),word(&mut f));
        let mut refresh=word(&mut f) as u8;let expected_refresh=word(&mut f) as u8;
        let expected=bytes(&mut f,65536);
        let writes=number(&mut f);let expected_ports:Vec<(u16,u8)>=(0..writes).map(|_|{let p=word(&mut f);(p,bytes(&mut f,1)[0])}).collect();
        let mut ports=Vec::new();
        assert_eq!(covox_state(&mut mem,&data,rate,device,diff!=0,abort!=0,&mut refresh,&mut ports),ret,"{name} AX");
        assert_eq!(refresh,expected_refresh,"{name} speaker refresh input");
        assert_eq!(mem.ds.iter().zip(&expected).position(|(a,b)|a!=b),None,"{name} DS difference");
        assert_eq!(ports.iter().zip(&expected_ports).position(|(a,b)|a!=b).or((ports.len()!=expected_ports.len()).then_some(ports.len().min(expected_ports.len()))),None,"{name} port writes differ ({} vs {} writes)",ports.len(),expected_ports.len());
        port_writes+=writes;
    }
    assert_eq!(f.read(&mut [0]).unwrap(),0);
    println!("PASS Covox state: {count} cases, nineteen VMFs, full DS + AX + {port_writes} port writes 0 differ");
}

#[test]
fn zz_skip_report() { oracle_env::report() }
