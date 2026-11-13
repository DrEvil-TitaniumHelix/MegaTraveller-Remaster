use std::io;
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes { crc ^= b as u32; for _ in 0..8 { crc = (crc >> 1) ^ (0xEDB88320 & 0u32.wrapping_sub(crc & 1)); } }
    !crc
}
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len(); out.extend_from_slice(kind); out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&out[start..]).to_be_bytes());
}
pub fn rgb(width: u32, height: u32, pixels: &[u32]) -> io::Result<Vec<u8>> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 || width as usize * height as usize != pixels.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "PNG: invalid RGB dimensions"));
    }
    let mut raw = Vec::with_capacity(pixels.len() * 3 + height as usize);
    for row in pixels.chunks(width as usize) {
        raw.push(0);
        for &rgb in row { raw.extend_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]); }
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        z.push(u8::from((i + 1) * 65535 >= raw.len()));
        let len = block.len() as u16; z.extend_from_slice(&len.to_le_bytes()); z.extend_from_slice(&(!len).to_le_bytes()); z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw { a = (a + byte as u32) % 65521; b = (b + a) % 65521; }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::from(width.to_be_bytes()); header.extend_from_slice(&height.to_be_bytes()); header.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header); chunk(&mut out, b"IDAT", &z); chunk(&mut out, b"IEND", &[]); Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_crc_stored_blocks_and_scanlines_decode() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
        for (w, h) in [(1, 1), (320, 200)] {
            let pixels: Vec<_> = (0..w * h).map(|i| 0x123400 | (i & 255)).collect();
            let png = rgb(w, h, &pixels).unwrap(); assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            let mut at = 8; let mut types = Vec::new();
            while at < png.len() {
                let n = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
                let kind = &png[at + 4..at + 8]; let data = &png[at + 8..at + 8 + n];
                assert_eq!(crc32(&png[at + 4..at + 8 + n]), u32::from_be_bytes(png[at + 8 + n..at + 12 + n].try_into().unwrap()));
                types.push(kind);
                if kind == b"IHDR" { assert_eq!(&data[..4], &w.to_be_bytes()); assert_eq!(&data[4..8], &h.to_be_bytes()); assert_eq!(&data[8..], &[8, 2, 0, 0, 0]); }
                if kind == b"IDAT" {
                    assert_eq!(&data[..2], &[0x78, 1]); let mut pos = 2; let mut raw = Vec::new();
                    loop {
                        let final_block = data[pos]; assert!(final_block <= 1);
                        let len = u16::from_le_bytes(data[pos + 1..pos + 3].try_into().unwrap());
                        assert_eq!(len, !u16::from_le_bytes(data[pos + 3..pos + 5].try_into().unwrap()));
                        raw.extend_from_slice(&data[pos + 5..pos + 5 + len as usize]); pos += 5 + len as usize;
                        if final_block == 1 { break; }
                    }
                    assert_eq!(pos + 4, data.len()); assert_eq!(raw.len(), (w * 3 + 1) as usize * h as usize);
                    let a = (1 + raw.iter().map(|&v| v as u64).sum::<u64>()) % 65521;
                    let b = (raw.len() as u64 + raw.iter().enumerate().map(|(i, &v)| (raw.len() - i) as u64 * v as u64).sum::<u64>()) % 65521;
                    assert_eq!(u32::from_be_bytes(data[pos..].try_into().unwrap()), ((b << 16) | a) as u32);
                    for (y, row) in raw.chunks(w as usize * 3 + 1).enumerate() {
                        assert_eq!(row[0], 0);
                        for (x, p) in row[1..].chunks(3).enumerate() { assert_eq!(p, &[0x12, 0x34, ((y * w as usize + x) & 255) as u8]); }
                    }
                }
                at += n + 12;
            }
            assert_eq!(types, [b"IHDR", b"IDAT", b"IEND"]);
        }
        assert!(rgb(0, 1, &[]).is_err()); assert!(rgb(2, 1, &[0]).is_err()); assert!(rgb(u32::MAX, 1, &[]).is_err());
    }
}
