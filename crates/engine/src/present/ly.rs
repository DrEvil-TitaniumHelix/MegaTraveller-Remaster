use super::{fb::Fb, prim::Raster};
use crate::data::ly::{Entry, Rec};

pub const OPS: [u8; 4] = [0, 8, 16, 24];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pen { pub colour: u8, pub op: u8, pub ops: [u8; 4] }
impl Pen { pub fn new(colour: u8) -> Pen { Pen { colour, op: 0, ops: OPS } } }

pub fn entry(fb: &mut Fb, r: Raster, page: usize, entries: &[Entry], i: usize, y: i32, x: i32, pen: &mut Pen) { walk(fb, r, page, entries, i, y as i64, x as i64, pen) }
pub fn layout(fb: &mut Fb, r: Raster, page: usize, entries: &[Entry], i: usize, y: i32, x: i32, pen: &mut Pen) {
    let capture = super::textlog::layout(fb, entries, page, i, y, x, pen.colour);
    pen.op = 0; entry(fb, r, page, entries, i, y, x, pen);
    super::textlog::finish(fb, capture, pen.colour);
}
pub fn draw(fb: &mut Fb, r: Raster, page: usize, entries: &[Entry], i: usize, y: i32, x: i32) { layout(fb, r, page, entries, i, y, x, &mut Pen::new(0)) }

pub(super) fn cast(r: Raster, v: i64) -> i32 { if r != Raster::Clip { v as i32 } else { v.clamp(i32::MIN as i64, i32::MAX as i64) as i32 } }
fn walk(fb: &mut Fb, r: Raster, page: usize, entries: &[Entry], i: usize, y: i64, x: i64, pen: &mut Pen) {
    let Some(e) = entries.get(i) else { return };
    let mut stack: Vec<(&[Rec], usize, i64, i64)> = vec![(e, 0, y, x)];
    loop {
        let Some(top) = stack.last_mut() else { break };
        let (recs, y, x) = (top.0, top.2, top.3);
        let Some(k) = recs.get(top.1) else { stack.pop(); continue };
        top.1 += 1;
        match k.op {
            0 => { stack.pop(); }
            1..=4 => {
                if k.colour != 16 { pen.colour = k.colour }
                let (y1, x1) = (cast(r, y + k.y1 as i64), cast(r, x + k.x1 as i64));
                match k.op {
                    1 => fb.rect(r, page, y1, x1, cast(r, y + k.y2 as i64), cast(r, x + k.x2 as i64), pen.colour, pen.op),
                    2 => fb.fill(r, page, y1, x1, k.y2 as i32 - k.y1 as i32 + 1, k.x2 as i32 - k.x1 as i32 + 1, pen.colour, pen.op),
                    3 => fb.line(r, page, y1, x1, cast(r, y + k.y2 as i64), cast(r, x + k.x2 as i64), pen.colour, pen.op),
                    _ => fb.pixel(r, page, y1, x1, pen.colour, pen.op),
                }
            }
            5 => if let Some(&op) = pen.ops.get(k.colour as usize) { pen.op = op },
            6 => if let Some(e) = entries.get(k.colour as usize) { if stack.len() <= entries.len() { stack.push((e, 0, y + k.y1 as i64, x + k.x1 as i64)) } else { return } },
            _ => {}
        }
    }
}

#[cfg(test)]
mod t {
    use super::*;
    fn rec(op: u8, colour: u8, y1: i16, x1: i16, y2: i16, x2: i16) -> Rec { Rec { op, colour, y1, x1, y2, x2 } }
    #[test]
    fn rules() {
        let e: Vec<Entry> = vec![
            vec![rec(2, 3, 0, 0, 1, 1), rec(5, 3, 0, 0, 0, 0), rec(4, 16, 0, 0, 0, 0), rec(6, 1, 5, 5, 0, 0), rec(4, 16, 9, 9, 0, 0), rec(0, 0, 0, 0, 0, 0), rec(2, 7, 0, 0, 50, 50)],
            vec![rec(3, 5, 0, 0, 0, 2), rec(99, 0, 0, 0, 0, 0), rec(4, 16, 1, 0, 0, 0)],
        ];
        let mut fb = Fb::new();
        let mut pen = Pen { colour: 9, op: 16, ops: OPS };
        layout(&mut fb, Raster::Clip, 0, &e, 0, 10, 10, &mut pen);
        assert_eq!((pen.colour, pen.op), (5, 24));
        assert_eq!((fb.get(0, 10, 11), fb.get(0, 11, 10), fb.get(0, 11, 11)), (3, 3, 3));
        assert_eq!(fb.get(0, 10, 10), 0);
        assert_eq!((fb.get(0, 15, 15), fb.get(0, 15, 17), fb.get(0, 15, 18)), (5, 5, 0));
        assert_eq!(fb.get(0, 16, 15), 5);
        assert_eq!(fb.get(0, 19, 19), 5);
        assert_eq!(fb.get(0, 30, 30), 0);
        entry(&mut fb, Raster::Clip, 0, &e, 1, 40, 40, &mut pen); assert_eq!(fb.get(0, 40, 40), 5);
        entry(&mut fb, Raster::Clip, 0, &e, 1, 40, 40, &mut pen); assert_eq!(fb.get(0, 40, 40), 0);
        let n = fb.planes.iter().filter(|&&b| b != 0).count();
        draw(&mut fb, Raster::Clip, 0, &e, 7, 0, 0);
        assert_eq!(fb.planes.iter().filter(|&&b| b != 0).count(), n);
        let mut p = Pen { colour: 1, op: 0, ops: [0, 0, 0, 8] };
        layout(&mut fb, Raster::Clip, 1, &vec![vec![rec(5, 3, 0, 0, 0, 0), rec(2, 6, 0, 0, 0, 0)]], 0, 0, 0, &mut p);
        assert_eq!((p.op, fb.get(1, 0, 0)), (8, 0));
    }
    #[test]
    fn extremes() {
        let mut fb = Fb::new();
        let huge = vec![vec![rec(0, 0, 1, 1, 0, 0), rec(4, 7, 1, 1, 1, 1)], vec![rec(4, 7, 0, 0, 1, 1)], vec![rec(3, 7, -5, -5, 5, 5)]];
        draw(&mut fb, Raster::Clip, 0, &huge, 0, i32::MAX, i32::MAX);
        draw(&mut fb, Raster::Clip, 0, &huge, 1, i32::MAX, i32::MIN);
        draw(&mut fb, Raster::Clip, 0, &huge, 2, i32::MIN, i32::MAX);
        assert!(fb.planes.iter().all(|&b| b == 0));
        let cyc = vec![vec![rec(4, 7, 0, 0, 0, 0), rec(6, 0, 1, 1, 0, 0)]];
        draw(&mut fb, Raster::Clip, 0, &cyc, 0, 0, 0);
        assert_eq!((fb.get(0, 0, 0), fb.get(0, 1, 1), fb.get(0, 2, 2)), (7, 7, 0));
        let chain: Vec<Entry> = (0..255u8).map(|i| if i == 254 { vec![rec(4, 9, 0, 0, 0, 0)] } else { vec![rec(6, i + 1, 0, 1, 0, 0)] }).collect();
        draw(&mut fb, Raster::Clip, 0, &chain, 0, 3, 5); assert_eq!(fb.get(0, 3, 259), 9);
        let tab = vec![vec![rec(5, 4, 0, 0, 0, 0), rec(6, 9, 0, 0, 0, 0), rec(4, 16, 3, 3, 0, 0)]];
        let mut p = Pen { colour: 2, op: 24, ops: OPS }; layout(&mut fb, Raster::Wrap, 1, &tab, 0, 0, 0, &mut p);
        assert_eq!((p.op, fb.get(1, 3, 3)), (0, 2));
        let mut fb = Fb::new();
        draw(&mut fb, Raster::Wrap, 0, &vec![vec![rec(4, 7, 1, 0, 0, 0)]], 0, i32::MAX, 0); assert_eq!(fb.get(0, 0, 0), 7);
        draw(&mut fb, Raster::Clip, 1, &vec![vec![rec(4, 7, 1, 0, 0, 0)]], 0, i32::MAX, 0); assert!(fb.plane(0)[8192..16384].iter().all(|&b| b == 0));
    }
}
