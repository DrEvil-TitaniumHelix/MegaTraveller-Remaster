use mt1::ground::{render, state::{Ground, Mem}};
use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};

#[test]
fn message_width_beyond_i32() {
    for (count, suffix, row, col) in [(8_421_505, &b""[..], 178, 128), (8_421_504, &b" B"[..], 187, 256)] {
        let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE],
            Box::new(Scripted::new([])), Box::new(Counter::at(100)), Box::new(Overlay::new(None)));
        for c in b"A B" { g.mem.set_u8(0x5128 + *c as u16, 255) }
        let mut text = vec![b'A'; count]; text.extend_from_slice(suffix);
        render::message(&mut g, &text);
        assert_eq!((g.mem.u16(0x7139), g.mem.u16(0x713B)), (row, col), "{count} A bytes, suffix {suffix:?}");
        assert_eq!(g.mem.message_deadline(), 280);
        assert_eq!(g.mem.u8(0x713F) & 1, 0);
    }
}
