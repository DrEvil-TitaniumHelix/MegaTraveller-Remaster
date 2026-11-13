mod oracle_env;
use mt1::space::stations::{raster, Host, Reply};
use mt_engine::present::fb::Fb;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    process::Command,
};
type R<T> = Result<T, String>;
fn bytes(r: &mut impl Read, n: usize) -> R<Vec<u8>> {
    let mut v = vec![0; n];
    r.read_exact(&mut v).map_err(|e| e.to_string())?;
    Ok(v)
}
fn number(r: &mut impl Read) -> R<usize> {
    let b = bytes(r, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}
struct Memory {
    ds: i32,
    bytes: Vec<u8>,
}
impl Host for Memory {
    fn ds(&self) -> i32 {
        self.ds
    }
    fn read(&self, off: i32, seg: i32, width: usize) -> u32 {
        (0..width.min(4)).fold(0, |v, i| {
            v | ((self
                .bytes
                .get(seg as u16 as usize * 16 + (off + i as i32) as u16 as usize)
                .copied()
                .unwrap_or(0) as u32)
                << (8 * i))
        })
    }
    fn write(&mut self, _: i32, _: i32, _: u32, _: usize) {}
    fn call(&mut self, name: &str, _: &[i32]) -> R<Reply> {
        Err(format!("unexpected raster service {name}"))
    }
}
#[test]
fn full_station_frames() -> R<()> {
    let Some(_oracle) = oracle_env::oracle() else { return Ok(()) };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reuse = std::env::var_os("MT1_STATIONS_FRAMES");
    let path = reuse
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/m6-stations/frames.bin"));
    if reuse.is_none() {
        let status = Command::new("python")
            .arg("-B")
            .arg(root.join("tools/m6_stations_frames.py"))
            .arg(&path)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("station frame export failed".into());
        }
    }
    let mut input = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    if bytes(&mut input, 8)? != b"SFRAME01" {
        return Err("frame magic".into());
    }
    let mut h = Memory { ds: 0, bytes: Vec::new() };
    let mut fb = Fb::new();
    let mut frames = 0;
    let mut calls = BTreeMap::new();
    loop {
        match bytes(&mut input, 1)?[0] {
            b'S' => {
                h.ds = number(&mut input)? as i32;
                let n = number(&mut input)?;
                h.bytes = bytes(&mut input, n)?;
                fb.planes.copy_from_slice(&bytes(&mut input, 4 * 65536)?);
            }
            b'D' => {
                let address = number(&mut input)? as u32;
                let args = (0..number(&mut input)?)
                    .map(|_| number(&mut input).map(|n| n as i32))
                    .collect::<R<Vec<_>>>()?;
                for _ in 0..number(&mut input)? {
                    let at = number(&mut input)?;
                    let len = number(&mut input)?;
                    let b = bytes(&mut input, len)?;
                    h.bytes.get_mut(at..at + len).ok_or("memory patch bounds")?.copy_from_slice(&b)
                }
                raster::draw(&mut fb, &h, address, &args).map_err(|e| format!("frame {frames} {address:X} {args:?}: {e}"))?;
                *calls.entry(address).or_insert(0usize) += 1;
            }
            b'F' => {
                let n = number(&mut input)?;
                let label = String::from_utf8(bytes(&mut input, n)?).map_err(|e| e.to_string())?;
                let expected = bytes(&mut input, 4 * 65536)?;
                if let Some(i) = fb.planes.iter().zip(&expected).position(|(a, b)| a != b) {
                    return Err(format!(
                        "frame {frames} {label}: plane {}, byte {:04X}: got {:02X}, expected {:02X}",
                        i / 65536,
                        i % 65536,
                        fb.planes[i],
                        expected[i]
                    ));
                }
                frames += 1;
                eprintln!("PASS frame {frames}: {label}");
            }
            b'E' => break,
            _ => return Err("unknown frame record".into()),
        }
    }
    if frames != 154 {
        return Err(format!("expected 154 S11–S20 frames, got {frames}"));
    }
    eprintln!(
        "PASS S11–S20: {frames} full-plane frames, {} raster calls",
        calls.values().sum::<usize>()
    );
    Ok(())
}

#[test]
fn zz_skip_report() { oracle_env::report() }
