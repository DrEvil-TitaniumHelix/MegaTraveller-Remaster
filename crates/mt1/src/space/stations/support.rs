use super::*;
fn string(h: &dyn Host, off: i32, seg: i32) -> Result<Vec<u8>> {
    let mut text = Vec::new();
    for i in 0..4096 {
        let b = h.read((off + i) & 65535, seg, 1) as u8;
        if b == 0 {
            return Ok(text);
        }
        text.push(b)
    }
    Err("S station: unterminated string".into())
}
fn format(h: &dyn Host, fmt: &[u8], args: &[i32]) -> Result<Vec<u8>> {
    let (mut out, mut pos, mut argument) = (Vec::new(), 0, 0);
    while pos < fmt.len() {
        if fmt[pos] != b'%' {
            out.push(fmt[pos]);
            pos += 1;
            continue;
        }
        let start = pos;
        pos += 1;
        let mut left = false;
        while fmt.get(pos).is_some_and(|b| b"-+0 #".contains(b)) {
            left |= fmt[pos] == b'-';
            pos += 1
        }
        let mut width = 0usize;
        while fmt.get(pos).is_some_and(u8::is_ascii_digit) {
            width = width.saturating_mul(10).saturating_add((fmt[pos] - b'0') as usize);
            pos += 1
        }
        if width > 65535 {
            return Err("S station: excessive format width".into());
        }
        let kind = fmt.get(pos).copied().unwrap_or(0);
        if !b"cdsu%".contains(&kind) {
            out.extend_from_slice(&fmt[start..pos]);
            continue;
        }
        pos += 1;
        if kind == b'%' {
            out.push(b'%');
            continue;
        }
        let v = *args.get(argument).ok_or("S station: missing format argument")?;
        argument += 1;
        let text = match kind {
            b's' => {
                let seg = *args.get(argument).ok_or("S station: missing far string segment")?;
                argument += 1;
                string(h, v, seg)?
            }
            b'c' => vec![v as u8],
            b'd' => (v as i16).to_string().into_bytes(),
            _ => (v as u16).to_string().into_bytes(),
        };
        let pad = width.saturating_sub(text.len());
        if !left {
            out.resize(out.len() + pad, b' ')
        }
        out.extend_from_slice(&text);
        if left {
            out.resize(out.len() + pad, b' ')
        }
    }
    if argument != args.len() {
        return Err("S station: unused format arguments".into());
    }
    Ok(out)
}
pub(super) fn invoke(h: &mut dyn Host, name: &str, a: &[i32]) -> Option<Result<Reply>> {
    let need = match name {
        "struct_copy" => 5,
        "setmem" => 4,
        "strchr" => 3,
        "sprintf" => 4,
        _ => return None,
    };
    if a.len() < need {
        return Some(Err(format!("S station {name}: missing arguments")));
    }
    Some((|| match name {
        "struct_copy" => {
            for i in 0..a[4] as u16 as i32 {
                let b = h.read((a[0] + i) & 65535, a[1], 1);
                h.write((a[2] + i) & 65535, a[3], b, 1)
            }
            Ok(Reply::int(a[2]))
        }
        "setmem" => {
            for i in 0..a[2] as u16 as i32 {
                h.write((a[0] + i) & 65535, a[1], a[3] as u8 as u32, 1)
            }
            Ok(Reply::default())
        }
        "strchr" => {
            let mut s = string(h, a[0], a[1])?;
            s.push(0);
            Ok(s.iter()
                .position(|&b| b == a[2] as u8)
                .map(|i| Reply {
                    ax: (a[0] + i as i32) as u16,
                    dx: a[1] as u16,
                })
                .unwrap_or_default())
        }
        "sprintf" => {
            let fmt = string(h, a[2], a[3])?;
            let text = format(h, &fmt, &a[4..])?;
            for (i, &b) in text.iter().chain(std::iter::once(&0)).enumerate() {
                h.write((a[0] + i as i32) & 65535, a[1], b as u32, 1)
            }
            Ok(Reply::int(text.len() as i32))
        }
        _ => Err("S station: unknown memory service".into()),
    })())
}
