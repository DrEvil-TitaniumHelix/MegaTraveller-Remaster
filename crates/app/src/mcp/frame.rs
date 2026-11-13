pub(super) fn png(rgb: &[u32]) -> Result<Vec<u8>, String> {
    if rgb.len() != 320 * 200 { return Err("no complete 320x200 frame has been presented".into()) }
    fn crc(bytes: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in bytes { c ^= b as u32; for _ in 0..8 { c = (c >> 1) ^ (0xEDB88320 & 0u32.wrapping_sub(c & 1)); } } !c
    }
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes()); let start = out.len();
        out.extend_from_slice(tag); out.extend_from_slice(bytes); let sum = crc(&out[start..]); out.extend_from_slice(&sum.to_be_bytes());
    }
    let mut raw = Vec::with_capacity(200 * 961);
    for row in rgb.chunks_exact(320) { raw.push(0); for &p in row { raw.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]); } }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        z.push(u8::from((i + 1) * 65535 >= raw.len())); let n = block.len() as u16;
        z.extend_from_slice(&n.to_le_bytes()); z.extend_from_slice(&(!n).to_le_bytes()); z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32); for v in raw { a = (a + v as u32) % 65521; b = (b + a) % 65521; }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec(); let mut head = vec![];
    head.extend_from_slice(&320u32.to_be_bytes()); head.extend_from_slice(&200u32.to_be_bytes()); head.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &head); chunk(&mut out, b"IDAT", &z); chunk(&mut out, b"IEND", &[]); Ok(out)
}
pub(super) fn base64(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for b in bytes.chunks(3) {
        let v = ((b[0] as u32) << 16) | ((b.get(1).copied().unwrap_or(0) as u32) << 8) | b.get(2).copied().unwrap_or(0) as u32;
        for i in 0..4 { out.push(if i > b.len() { '=' } else { CHARS[((v >> (18 - i * 6)) & 63) as usize] as char }); }
    } out
}
