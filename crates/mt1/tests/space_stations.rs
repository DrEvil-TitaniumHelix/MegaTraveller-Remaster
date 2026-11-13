mod oracle_env;
use mt1::space::stations::{self, Host, Reply};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::{BufReader, Read},
    path::PathBuf,
    process::Command,
};
type R<T> = Result<T, String>;
fn read(r: &mut impl Read, n: usize) -> R<Vec<u8>> {
    let mut b = vec![0; n];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}
fn number(r: &mut impl Read) -> R<u32> {
    let b = read(r, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}
fn text(r: &mut impl Read) -> R<String> {
    let n = number(r)?;
    String::from_utf8(read(r, n as usize)?).map_err(|e| e.to_string())
}
fn values(r: &mut impl Read) -> R<Vec<i32>> {
    (0..number(r)?).map(|_| number(r).map(|n| n as i32)).collect()
}
fn patches(r: &mut impl Read) -> R<Vec<(usize, u8)>> {
    (0..number(r)?).map(|_| Ok((number(r)? as usize, read(r, 1)?[0]))).collect()
}
struct Event {
    name: String,
    args: Vec<i32>,
    before: Vec<(usize, u8)>,
    reply: Reply,
    after: Vec<(usize, u8)>,
}
struct Script {
    memory: Vec<u8>,
    ds: i32,
    di: u16,
    written: BTreeSet<usize>,
    events: VecDeque<Event>,
    position: usize,
}
impl Script {
    fn check(&mut self, expected: &[(usize, u8)], where_: &str) -> R<()> {
        let wanted: BTreeSet<_> = expected.iter().map(|x| x.0).collect();
        if wanted != self.written {
            return Err(format!(
                "{where_}: write footprint missing {:?}, extra {:?}",
                wanted.difference(&self.written).take(8).collect::<Vec<_>>(),
                self.written.difference(&wanted).take(8).collect::<Vec<_>>()
            ));
        }
        for &(i, v) in expected {
            if self.memory.get(i) != Some(&v) {
                return Err(format!("{where_}: memory {i:05X}: expected {v:02X}, got {:?}", self.memory.get(i)));
            }
        }
        self.written.clear();
        Ok(())
    }
}
impl Host for Script {
    fn ds(&self) -> i32 {
        self.ds
    }
    fn entry_di(&self) -> R<u16> {
        Ok(self.di)
    }
    fn read(&self, off: i32, seg: i32, width: usize) -> u32 {
        (0..width.min(4)).fold(0, |v, i| {
            v | ((self
                .memory
                .get((seg as u16 as usize) * 16 + (off + i as i32) as u16 as usize)
                .copied()
                .unwrap_or(0) as u32)
                << (i * 8))
        })
    }
    fn write(&mut self, off: i32, seg: i32, value: u32, width: usize) {
        for i in 0..width.min(4) {
            let at = (seg as u16 as usize) * 16 + (off + i as i32) as u16 as usize;
            if let Some(b) = self.memory.get_mut(at) {
                *b = (value >> (i * 8)) as u8;
                self.written.insert(at);
            }
        }
    }
    fn call(&mut self, name: &str, args: &[i32]) -> R<Reply> {
        self.position += 1;
        let event = self
            .events
            .pop_front()
            .ok_or_else(|| format!("extra call {}: {name} {args:?}", self.position))?;
        let mut actual: Vec<_> = args.iter().map(|v| v & 65535).collect();
        if name == "draw_character" {
            if let Some(n) = actual.first_mut() {
                *n &= 255
            }
        }
        if name == "shift_string" {
            if let Some(n) = actual.get_mut(2) {
                *n &= 255
            }
        }
        if event.name != name || event.args != actual {
            return Err(format!(
                "call {}: expected {} {:?}, got {name} {actual:?}",
                self.position, event.name, event.args
            ));
        }
        self.check(&event.before, &format!("before {name} #{}", self.position))?;
        if matches!(
            name,
            "struct_copy"
                | "setmem"
                | "sprintf"
                | "strchr"
                | "remap_numeric_glyphs"
                | "is_double_program"
                | "matching_program_is_loaded"
                | "repair_interval"
                | "healing_interval"
        ) {
            let got = stations::invoke(self, name, args, 0xF000).ok_or("missing native leaf service")??;
            self.check(&event.after, &format!("native leaf {name}"))?;
            if got != event.reply {
                return Err(format!("native leaf {name}: {got:?} != {:?}", event.reply));
            }
            return Ok(got);
        }
        for (i, v) in event.after {
            let b = self.memory.get_mut(i).ok_or("service write outside memory")?;
            *b = v;
        }
        Ok(event.reply)
    }
}
#[test]
fn oracle_station_contracts() -> R<()> {
    let Some(_oracle) = oracle_env::oracle() else { return Ok(()) };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reuse = std::env::var_os("MT1_STATIONS_FIXTURE");
    let output = reuse
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/m6-stations/spec.bin"));
    if reuse.is_none() {
        let status = Command::new("python")
            .args(["-B"])
            .arg(root.join("tools/m6_stations_spec.py"))
            .arg(&output)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("station fixture export failed".into());
        }
    }
    let mut input = BufReader::new(File::open(output).map_err(|e| e.to_string())?);
    if read(&mut input, 8)? != b"STATION2" {
        return Err("fixture magic".into());
    }
    let size = number(&mut input)? as usize;
    let base = read(&mut input, size)?;
    let mut counts = BTreeMap::new();
    while read(&mut input, 1)? == b"C" {
        let routine = text(&mut input)?;
        let ds = number(&mut input)? as i32;
        let di = number(&mut input)? as u16;
        let args = values(&mut input)?;
        let mut memory = base.clone();
        for (i, v) in patches(&mut input)? {
            memory[i] = v
        }
        let mut events = VecDeque::new();
        for _ in 0..number(&mut input)? {
            events.push_back(Event {
                name: text(&mut input)?,
                args: values(&mut input)?,
                before: patches(&mut input)?,
                reply: Reply {
                    ax: number(&mut input)? as u16,
                    dx: number(&mut input)? as u16,
                },
                after: patches(&mut input)?,
            })
        }
        let final_writes = patches(&mut input)?;
        let defined = number(&mut input)?;
        let wanted = Reply {
            ax: number(&mut input)? as u16,
            dx: number(&mut input)? as u16,
        };
        if routine == "translate_station_key" { continue; }
        let count = counts.entry(routine.clone()).or_insert(0);
        *count += 1;
        let mut host = Script {
            memory,
            ds,
            di,
            written: BTreeSet::new(),
            events,
            position: 0,
        };
        let result = stations::invoke(&mut host, &routine, &args, 0xFEFE).ok_or_else(|| format!("unimplemented {routine}"))?;
        let got = if defined == 2 {
            match result {
                Err(e) if e.contains("stable unmapped-key spin") => Reply::default(),
                other => return Err(format!("{routine} case {count}: expected stable spin, got {other:?}")),
            }
        } else {
            result.map_err(|e| format!("{routine} case {count}: {e}"))?
        };
        if !host.events.is_empty() {
            return Err(format!("{routine} case {count}: {} missing calls", host.events.len()));
        }
        host.check(&final_writes, &format!("{routine} case {count} return"))?;
        if defined == 1 && got != wanted {
            return Err(format!("{routine} case {count}: return {got:?} expected {wanted:?}"));
        }
    }
    for (name, count) in &counts {
        eprintln!("PASS {name}: {count}")
    }
    eprintln!("PASS stations: {} cases", counts.values().sum::<i32>());
    Ok(())
}

#[test]
fn zz_skip_report() { oracle_env::report() }
