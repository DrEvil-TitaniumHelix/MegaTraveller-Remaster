use super::{assets::{Assets, Bitmap}, controls, Component, Rect, Surface, UiEvent, UiState, Button};
use mt1::ui::{Intent, Preset, Scene, Target, Unit, Verb};
use mt1::ground::orders::{PATH_BLOCKED, PATH_LATER, PATH_RUN};
use mt1::ground::fire::PATH_DANGER;
use mt_engine::io::{mouse, Event, MouseEvent};
use std::{collections::HashMap, sync::Arc};

pub const SCREEN_PIXELS: (u32, u32) = (mt1::ground::round::SCREEN_TILES.1 as u32 * 16, mt1::ground::round::SCREEN_TILES.0 as u32 * 16);
pub fn screen_rect(size: (u32, u32)) -> Rect {
    let (sw, sh) = (u64::from(SCREEN_PIXELS.0), u64::from(SCREEN_PIXELS.1));
    let (w, h) = if u64::from(size.0) * sh <= u64::from(size.1) * sw {
        (size.0, (u64::from(size.0) * sh / sw) as u32)
    } else { ((u64::from(size.1) * sw / sh) as u32, size.1) };
    Rect::new(((size.0 - w) / 2) as i32, ((size.1 - h) / 2) as i32, w, h)
}

pub struct Viewport { pub assets: Arc<Assets> }
#[derive(Default, Debug)]
pub struct GroundCamera { last: Option<CameraFrame> }
#[derive(Debug)]
struct CameraFrame {
    origin: (i32, i32), focus: (i16, i16), clock: Option<u32>, map: MapKey,
}
#[derive(Debug, PartialEq, Eq)]
struct MapKey { world: Option<u8>, area: Option<u8>, level: i8, rows: u16, cols: u16, tileset: u8, interior: u8 }
impl GroundCamera {
    pub fn observe(scene: &Scene, previous: Option<&Scene>, ui: &UiState) {
        if previous.is_none() || matches!(scene.screen, mt1::ui::ScreenKind::Startup | mt1::ui::ScreenKind::Cg
            | mt1::ui::ScreenKind::Intro | mt1::ui::ScreenKind::Space) {
            ui.ground_camera.lock().unwrap().last = None;
        }
        if scene.screen == mt1::ui::ScreenKind::Ground { Camera::fixed_origin(scene, Some(ui)); }
    }
    fn origin(&mut self, scene: &Scene) -> (i32, i32) {
        let selected = if scene.squad.active { scene.squad.selected.min(4) } else { scene.leader.clamp(0, 4) as u8 };
        let w = &scene.walkers[selected as usize];
        let focus = if scene.combat && w.active() { (w.x(), w.y()) } else { scene.pos };
        let map = MapKey { world: scene.party.0.get(6).copied(), area: scene.party.0.get(7).copied(), level: scene.level,
            rows: scene.map.rows, cols: scene.map.cols, tileset: scene.map.tileset, interior: scene.map.interior };
        let reset = self.last.as_ref().is_none_or(|p| {
            p.map != map
                || p.focus.0.abs_diff(focus.0) > 2 || p.focus.1.abs_diff(focus.1) > 2
                || p.clock.zip(scene.context.clock).is_some_and(|(old, now)| now < old)
        });
        let previous = if reset { (i32::from(scene.view.0).div_euclid(2), i32::from(scene.view.1).div_euclid(2)) }
            else { self.last.as_ref().unwrap().origin };
        let axis = |old: i32, half: i16, extent: i32, bound: u16, row: bool| {
            let max = (i32::from(bound) - extent).max(0);
            let lo = ((i32::from(half) + 1).div_euclid(2) - (extent - 3)).clamp(0, max);
            let hi = (i32::from(half).div_euclid(2) - 2).clamp(lo, max);
            let mut all_lo = lo; let mut all_hi = hi;
            if scene.combat { for w in scene.walkers.iter().filter(|w| w.active()) {
                let p = i32::from(if row { w.x() } else { w.y() });
                all_lo = all_lo.max((p + 1).div_euclid(2) + 1 - extent);
                all_hi = all_hi.min(p.div_euclid(2));
            } }
            if all_lo <= all_hi { old.clamp(all_lo, all_hi) } else { old.clamp(lo, hi) }
        };
        let origin = (axis(previous.0, focus.0, 7, scene.map.rows, true), axis(previous.1, focus.1, 12, scene.map.cols, false));
        self.last = Some(CameraFrame { origin, focus, clock: scene.context.clock, map });
        (origin.0 * 16, origin.1 * 16)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Camera { pub top: f32, pub left: f32, pub zoom: f32 }
impl Camera {
    fn fixed_origin(scene: &Scene, ui: Option<&UiState>) -> (i32, i32) {
        if let Some(ui) = ui.filter(|u| u.fixed_screen && u.preset == Preset::Modern) {
            let (top, left) = ui.ground_camera.lock().unwrap().origin(scene);
            return (top, left + i32::from(scene.shake) * 8);
        }
        (i32::from(scene.view.0) * 8, i32::from(scene.view.1) * 8)
    }
    pub fn new(scene: &Scene, ui: &UiState, size: (u32, u32)) -> Self {
        if ui.fixed_screen {
            let r = screen_rect(size);
            let zoom = (r.w as f32 / SCREEN_PIXELS.0 as f32).min(r.h as f32 / SCREEN_PIXELS.1 as f32).max(f32::EPSILON);
            let (top, left) = Self::fixed_origin(scene, Some(ui));
            return Self { top: top as f32 - r.y as f32 / zoom, left: left as f32 - r.x as f32 / zoom, zoom };
        }
        let (row, col) = controls::camera(scene, ui); let zoom = controls::zoom(ui.zoom);
        Self { top: (row * 16.0 - size.1 as f32 / (2.0 * zoom)).floor(), left: (col * 16.0 - size.0 as f32 / (2.0 * zoom)).floor(), zoom }
    }
    pub fn world(self, at: (i32, i32)) -> (f32, f32) { (self.top + at.1 as f32 / self.zoom, self.left + at.0 as f32 / self.zoom) }
    fn outline(self, out: &mut Surface, world: (i32, i32), size: i32, c: u8) {
        let (x0, y0) = (((world.1 as f32 - self.left) * self.zoom).round() as i32, ((world.0 as f32 - self.top) * self.zoom).round() as i32);
        let (x1, y1) = (x0 + (size as f32 * self.zoom).round() as i32 - 1, y0 + (size as f32 * self.zoom).round() as i32 - 1);
        for x in x0..=x1 { out.put(x, y0, c); out.put(x, y1, c); }
        for y in y0..=y1 { out.put(x0, y, c); out.put(x1, y, c); }
    }
    fn screen(self, world: (i32, i32)) -> (i32, i32) { (((world.1 as f32 - self.left) * self.zoom).round() as i32, ((world.0 as f32 - self.top) * self.zoom).round() as i32) }
    fn plot(self, out: &mut Surface, world: (i32, i32), c: u8) { let (x, y) = self.screen(world); let n = self.zoom.ceil().max(1.0) as i32; for dy in 0..n { for dx in 0..n { out.put(x + dx, y + dy, c) } } }
    fn line(self, out: &mut Surface, a: (i32, i32), b: (i32, i32), c: u8, dotted: bool) {
        let (x0, y0) = self.screen(a); let (x1, y1) = self.screen(b);
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs()); let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        for i in 0..4096 { if !dotted || i % 2 == 0 { out.put(x, y, c) } if x == x1 && y == y1 { break } let e2 = 2 * err; if e2 >= dy { err += dy; x += sx } if e2 <= dx { err += dx; y += sy } }
    }
    fn bracket(self, out: &mut Surface, world: (i32, i32), c: u8) {
        let (x0, y0) = self.screen(world); let (x1, y1) = self.screen((world.0 + 16, world.1 + 16)); let (x1, y1) = (x1 - 1, y1 - 1); let n = ((x1 - x0) / 4).max(2);
        for k in 0..=n { out.put(x0 + k, y0, c); out.put(x1 - k, y0, c); out.put(x0 + k, y1, c); out.put(x1 - k, y1, c); out.put(x0, y0 + k, c); out.put(x0, y1 - k, c); out.put(x1, y0 + k, c); out.put(x1, y1 - k, c) }
    }
    fn marker(self, out: &mut Surface, p: (f32, f32), c: u8) { let (x, y) = self.screen(((p.0 * 16.0) as i32, (p.1 * 16.0) as i32)); for dy in -1..=1 { for dx in -1..=1 { out.put(x + dx, y + dy, c) } } }
    fn draw(self, out: &mut Surface, bitmap: &Bitmap, src: Rect, world: (i32, i32), transparent: Option<u8>) {
        let (x, y) = ((world.1 as f32 - self.left) * self.zoom, (world.0 as f32 - self.top) * self.zoom);
        let (x0, y0) = (x.ceil().max(0.0) as i32, y.ceil().max(0.0) as i32);
        let (x1, y1) = ((x + src.w as f32 * self.zoom).ceil().min(out.w as f32) as i32, (y + src.h as f32 * self.zoom).ceil().min(out.h as f32) as i32);
        for dy in y0..y1 { for dx in x0..x1 {
            let sx = src.x.saturating_add(((dx as f32 - x) / self.zoom).floor() as i32);
            let sy = src.y.saturating_add(((dy as f32 - y) / self.zoom).floor() as i32);
            if let Some(c) = bitmap.get(sx, sy) { if Some(c) != transparent { out.put(dx, dy, c); } }
        } }
    }
}
impl Viewport {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn oracle_camera(scene: &Scene) -> (f32, f32) { (scene.view.0 as f32 * 0.5 + 3.5, scene.view.1 as f32 * 0.5 + 6.0) }
    pub fn fixed_world(scene: &Scene, ui: Option<&UiState>, size: (u32, u32), at: (i32, i32)) -> Option<(i32, i32)> {
        let r = screen_rect(size);
        if !r.contains(at) { return None; }
        let (top, left) = Camera::fixed_origin(scene, ui);
        Some((top + ((at.1 - r.y) as u64 * u64::from(SCREEN_PIXELS.1) / u64::from(r.h)) as i32,
            left + ((at.0 - r.x) as u64 * u64::from(SCREEN_PIXELS.0) / u64::from(r.w)) as i32))
    }
    pub fn oracle_pointer(scene: &Scene, ui: &UiState, size: (u32, u32), at: (i32, i32)) -> Option<(i16, i16)> {
        if ui.fixed_screen {
            let (row, col) = Self::fixed_world(scene, Some(ui), size, at)?;
            let (row, col) = (row - scene.view.0 as i32 * 8, col - scene.view.1 as i32 * 8);
            return ((0..192).contains(&col) && (0..112).contains(&row)).then_some((col as i16, row as i16 + 54));
        }
        if at.0 < 0 || at.1 < 0 || at.0 as u32 >= size.0 || at.1 as u32 >= size.1 { return None; }
        let world = Camera::new(scene, ui, size).world(at);
        let col = world.1.floor() as i64 - scene.view.1 as i64 * 8;
        let row = world.0.floor() as i64 - scene.view.0 as i64 * 8;
        ((0..192).contains(&col) && (0..112).contains(&row)).then(|| (col as i16, row as i16 + 54))
    }
    pub fn local_pointer(scene: &Scene, ui: &UiState, size: (u32, u32), col: i16, row: i16) -> Option<(i32, i32)> {
        if ui.fixed_screen {
            if !(0..192).contains(&col) || !(54..166).contains(&row) { return None; }
            let r = screen_rect(size);
            let (top, left) = Camera::fixed_origin(scene, Some(ui));
            let (row, col) = (i32::from(row) - 54 + i32::from(scene.view.0) * 8 - top,
                i32::from(col) + i32::from(scene.view.1) * 8 - left);
            if !(0..192).contains(&col) || !(0..112).contains(&row) { return None; }
            let at = (r.x + (col as u64 * u64::from(r.w)).div_ceil(192) as i32,
                r.y + (row as u64 * u64::from(r.h)).div_ceil(112) as i32);
            return r.contains(at).then_some(at);
        }
        let c = Camera::new(scene, ui, size);
        let x = ((scene.view.1 as f32 * 8.0 + col as f32 - c.left) * c.zoom).ceil() as i32;
        let y = ((scene.view.0 as f32 * 8.0 + row as f32 - 54.0 - c.top) * c.zoom).ceil() as i32;
        (x >= 0 && y >= 0 && (x as u32) < size.0 && (y as u32) < size.1).then_some((x, y))
    }
    fn tile(&self, scene: &Scene, bytes: [u8; 3], objects: bool, pen: &mut u8) -> Bitmap {
        let [f, tile, flags] = bytes; let mut out = Surface::new(16, 16);
        let sheet = if f & 1 != 0 { self.assets.terrain.get(scene.map.tileset as usize) } else { self.assets.objects.get(4) };
        if let Some(sheet) = sheet { sheet.draw(&mut out, Rect::new((tile as i32 % 20) * 16, (tile as i32 / 20) * 16, 16, 16), (0, 0), None); }
        if flags & 128 != 0 { if let Some(b) = self.assets.objects.get(3) { b.draw(&mut out, Rect::new(0, 0, 16, 16), (0, 0), Some(1)); } }
        if flags & 64 != 0 { if let Some(b) = self.assets.cpic(scene, 1) { b.draw(&mut out, Rect::new(0, 0, 16, 16), (0, 0), Some(1)); } }
        let mut object = if objects { f >> 1 } else { 0 }; let mut visited = [false; 128];
        while object != 0 && object < 128 && !visited[object as usize] {
            visited[object as usize] = true;
            let Some(o) = scene.objects.get(object as usize) else { break };
            self.assets.ground(&mut out, o.glyph() as usize, (0, 0), pen); object = o.next();
        }
        Bitmap { w: 16, h: 16, pixels: out.indexed }
    }
    pub fn render(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        if ui.fixed_screen {
            let r = screen_rect((out.w as u32, out.h as u32));
            if r.w == 0 || r.h == 0 { return; }
            let mut screen = Surface::new(SCREEN_PIXELS.0, SCREEN_PIXELS.1);
            let camera = Camera::new(scene, ui, SCREEN_PIXELS);
            self.render_map(scene, ui, &mut screen, camera);
            if (r.w, r.h) == SCREEN_PIXELS { out.draw(&screen, r.x, r.y); }
            else { for y in 0..r.h { for x in 0..r.w {
                out.put(r.x + x as i32, r.y + y as i32, screen.indexed[(y * SCREEN_PIXELS.1 / r.h * SCREEN_PIXELS.0 + x * SCREEN_PIXELS.0 / r.w) as usize]);
            } } }
            return;
        }
        let camera = Camera::new(scene, ui, (out.w as u32, out.h as u32));
        self.render_map(scene, ui, out, camera);
    }
    fn render_map(&self, scene: &Scene, ui: &UiState, out: &mut Surface, camera: Camera) {
        let rows = (camera.top / 16.0).floor() as i32..((camera.top + out.h as f32 / camera.zoom) / 16.0).ceil() as i32;
        let cols = (camera.left / 16.0).floor() as i32..((camera.left + out.w as f32 / camera.zoom) / 16.0).ceil() as i32;
        let mut npcs = Vec::new(); let mut pen = scene.art.pen;
        let mut cache = HashMap::new();
        let travellers: Vec<(i32, i32)> = (0..5u8).filter_map(|i| controls::position(scene, Unit::Traveller(i)).map(|p| ((p.0 - 0.5) as i32, (p.1 - 0.5) as i32))).collect();
        let near = |r: i32, c: i32| travellers.iter().any(|t| (t.0 - r).abs().max((t.1 - c).abs()) <= 2);
        {
            for r in rows.start.max(0)..rows.end.min(scene.map.rows as i32) { for c in cols.start.max(0)..cols.end.min(scene.map.cols as i32) {
                let Some(bytes) = scene.map.cell(r, c) else { continue };
                let objects = r as f32 * 16.0 >= camera.top && c as f32 * 16.0 >= camera.left;
                let level = if !scene.art.dark { 0u8 } else if near(r, c) { 1 } else { 2 };
                let (tile, final_pen) = cache.entry((bytes, objects, pen, level)).or_insert_with(|| { let mut tile = self.tile(scene, bytes, objects, &mut pen); if level > 0 { for p in &mut tile.pixels { *p = if level == 1 { if *p >= 8 { *p - 8 } else { *p } } else if *p == 0 { 0 } else { 8 } } } (tile, pen) });
                pen = *final_pen;
                camera.draw(out, tile, Rect::new(0, 0, 16, 16), (r * 16, c * 16), None);
                if scene.art.dark && !near(r, c) { continue }
                if let Some(i) = (bytes[2] & 63).checked_sub(1) { if let Some(n) = scene.npcs.get(i as usize) { npcs.push((r, c, i + 1, n)); } }
            } }
            let walk = ui.replay.filter(|&(seq, _)| seq == scene.squad.fx.seq).and_then(|(_, t)| { let f = &scene.squad.fx; match f.unit { Some(Unit::Npc(k)) if f.walk.len() > 1 => Some((k, f.walk[((t * 1.6).clamp(0.0, 1.0) * (f.walk.len() - 1) as f32) as usize])), _ => None } });
            for (r, c, k, n) in npcs {
                if ui.fixed_screen && ui.preset == Preset::Classic && (n.x() < scene.view.0 || n.y() < scene.view.1
                    || i32::from(n.x()) > i32::from(scene.view.0) + 2 * (mt1::ground::round::SCREEN_TILES.0 - 1)
                    || i32::from(n.y()) > i32::from(scene.view.1) + 2 * (mt1::ground::round::SCREEN_TILES.1 - 1)) { continue; }
                let at = match walk { Some((wk, cell)) if wk == k => (cell.0 as i32 * 16, cell.1 as i32 * 16), _ => (r * 16 + (n.x() as i32 & 1) * 8, c * 16 + (n.y() as i32 & 1) * 8) };
                if let Some(b) = self.assets.cpic(scene, n.sprite()) {
                    let sx = (n.facing() as i32 & 7) * 16 + (n.animation() as i8 as i32 + 1) * 128;
                    camera.draw(out, b, Rect::new(sx, 0, 16, 16), at, Some(1));
                }
            }
        }
        let mut leader_phase = scene.art.phase;
        if leader_phase != 0 {
            let moving = if !scene.combat { scene.art.moving } else {
                let leader = scene.leader.clamp(0, 4) as usize;
                if scene.walkers[leader + 1..].iter().all(|w| !w.active()) { scene.art.moving }
                else {
                    let previous = scene.walkers[..leader].iter().rev().find(|w| w.active()).map(|w| (w.x(), w.y())).unwrap_or(scene.art.last_pos);
                    let w = &scene.walkers[leader]; let pos = if scene.art.individual == 0 { scene.pos } else { (w.x(), w.y()) };
                    previous != pos && !(scene.art.npc_mode == 0 && scene.art.individual != 0)
                }
            };
            if moving { leader_phase ^= 3; }
        }
        for i in 0..if scene.combat { 5 } else { 1 } {
            let w = &scene.walkers[i]; if scene.combat && !w.active() { continue }
            let leader = i as i16 == scene.leader;
            let (pos, facing) = if scene.combat && (!leader || scene.art.individual != 0) { ((w.x(), w.y()), w.facing() as i16) } else { (scene.pos, scene.heading) };
            let phase = if scene.combat && !leader { w.phase() } else { leader_phase };
            let slot = if scene.combat { 2 } else { u8::from(scene.encounter_mode != 0) };
            let animation = if scene.combat && i as i16 > scene.leader { scene.art.phase } else { leader_phase };
            let sx = facing as i32 * 16 + if phase == 0 { 0 } else { (if scene.encounter_mode != 0 { animation as i32 - 1 } else { phase as i32 }) * 128 };
            if let Some(b) = self.assets.cpic(scene, slot) { camera.draw(out, b, Rect::new(sx, 0, 16, 16), (pos.0 as i32 * 8, pos.1 as i32 * 8), Some(1)); }
            if scene.combat {
                let mut marker = Surface::new(16, 16); let mut colour = i as u8 + 1;
                self.assets.ground(&mut marker, 49, (5, 6), &mut colour);
                camera.draw(out, &Bitmap { w: 16, h: 16, pixels: marker.indexed }, Rect::new(0, 0, 16, 16), (pos.0 as i32 * 8, pos.1 as i32 * 8), Some(0));
            }
        }
        if scene.squad.active { self.overlays(scene, ui, out, camera) }
    }
    fn overlays(&self, scene: &Scene, ui: &UiState, out: &mut Surface, camera: Camera) {
        if let Some(p) = controls::position(scene, Unit::Traveller(scene.squad.selected)) { camera.outline(out, (((p.0 - 0.5) * 16.0) as i32, ((p.1 - 0.5) * 16.0) as i32), 16, scene.squad.selected + 1); }
        let hovered = match ui.hover.map(|t| match t { Target::Portrait(i) => Target::Unit(Unit::Traveller(i)), t => t }) { Some(Target::Cell(c)) => Some(c), Some(Target::Unit(u)) => controls::position(scene, u).map(|p| ((p.0 - 0.5) as i16, (p.1 - 0.5) as i16)), _ => None };
        if let Some((r, c)) = hovered { camera.outline(out, (r as i32 * 16, c as i32 * 16), 16, 15); }
        for &((r, c), class) in &scene.squad.path {
            let colour = match class { PATH_BLOCKED => 12, PATH_LATER => 8, PATH_RUN => 14, PATH_DANGER => 4, _ => 15 };
            camera.outline(out, (r as i32 * 16 + 6, c as i32 * 16 + 6), 4, colour);
        }
        for &(r, c) in &scene.squad.markers { camera.outline(out, (r as i32 * 16 + 3, c as i32 * 16 + 3), 10, 14) }
        let centre = |cell: (i16, i16)| (cell.0 as i32 * 16 + 8, cell.1 as i32 * 16 + 8);
        let sel = scene.squad.selected.min(4) as usize;
        if scene.combat { let w = &scene.walkers[sel]; if w.active() && w.target() > 0 { if let Some(p) = controls::position(scene, Unit::Npc(w.target() as u8)) { camera.line(out, centre((w.x() >> 1, w.y() >> 1)), ((p.0 * 16.0) as i32, (p.1 * 16.0) as i32), sel as u8 + 1, true); } } }
        if let Some((from, to, colour)) = crate::mouse::pointer::line(scene, ui) { camera.line(out, centre(from), centre(to), colour, false); }
        crate::mouse::walk::label(scene, ui, out, camera, &self.assets);
        if let Some(s) = scene.squad.sight {
            let colour = if s.los { 14 } else { 12 };
            camera.bracket(out, (s.to.0 as i32 * 16, s.to.1 as i32 * 16), colour);
            if s.cover { let (r, c) = (s.to.0 as i32 * 16, s.to.1 as i32 * 16); for dy in 0..5 { for dx in 0..5 { camera.plot(out, (r + 1 + dy, c + 11 + dx), if dy == 0 || dx == 0 || dx == 4 { 0 } else { 11 }); } } }
        }
        for (i, d) in scene.squad.destinations.iter().enumerate() { if let Some((r, c)) = *d { let (r, c) = (r as i32 * 16, c as i32 * 16); for dy in 0..8 { camera.plot(out, (r + 2 + dy, c + 3), i as u8 + 1); } for dy in 0..3 { for dx in 1..6 { camera.plot(out, (r + 2 + dy, c + 3 + dx), i as u8 + 1); } } } }
        if let Some((_, t)) = ui.replay.filter(|&(seq, _)| seq == scene.squad.fx.seq) {
            let fx = &scene.squad.fx;
            for &(from, to, kind) in &fx.shots { match kind { 0 => camera.outline(out, (to.0 as i32 * 16 + 4, to.1 as i32 * 16 + 4), 8, 15), 0xAA => camera.line(out, centre(from), centre(to), 10, false), 0xAB => camera.line(out, centre(from), centre(to), 14, false), _ => camera.line(out, centre(from), centre(to), 15, true) } }
            for &(r, c) in &fx.blasts { camera.outline(out, (r as i32 * 16 - 2, c as i32 * 16 - 2), 20, 14); camera.outline(out, (r as i32 * 16 + 4, c as i32 * 16 + 4), 8, 12) }
            if t < 0.85 { for &(_, (r, c), dmg) in &fx.hits { let text = if dmg == 255 { "KO".to_string() } else { dmg.to_string() }; let (x, y) = camera.screen((r as i32 * 16 + 2, c as i32 * 16 + 4)); self.assets.text(out, &text, (x, y), 12) } }
        }
        if matches!(scene.squad.acting, Some(Unit::Npc(_))) { for k in 0..2 { for x in 0..out.w as i32 { out.put(x, k, 4); out.put(x, out.h as i32 - 1 - k, 4) } for y in 0..out.h as i32 { out.put(k, y, 4); out.put(out.w as i32 - 1 - k, y, 4) } } }
        if camera.zoom < 1.0 {
            for (i, n) in scene.npcs.iter().enumerate() { if n.present() { if let Some(p) = controls::position(scene, Unit::Npc(i as u8 + 1)) { camera.marker(out, p, super::minimap::attitude(n.attitude())) } } }
            for i in 0..5u8 { if (scene.combat || i == sel as u8) && scene.party.mask() & (1 << i) != 0 { if let Some(p) = controls::position(scene, Unit::Traveller(i)) { camera.marker(out, p, i + 1) } } }
            if let Some(p) = controls::position(scene, Unit::Traveller(sel as u8)) { let r = mt1::ground::round::ENCOUNTER_RADIUS; camera.outline(out, (((p.0 - 0.5) as i32 - r) * 16, ((p.1 - 0.5) as i32 - r) * 16), (2 * r + 1) * 16, 6) }
        }
    }
    fn target(&self, scene: &Scene, cell: (i16, i16)) -> Target {
        for (i, w) in scene.walkers.iter().enumerate() { if scene.combat && w.active() && (w.x() / 2, w.y() / 2) == cell { return Target::Unit(Unit::Traveller(i as u8)) } }
        if !scene.combat && (scene.pos.0 / 2, scene.pos.1 / 2) == cell { return Target::Unit(controls::selected(scene)) }
        if let Some(b) = scene.map.cell(cell.0 as i32, cell.1 as i32) {
            if b[2] & 63 != 0 { return Target::Unit(Unit::Npc(b[2] & 63)) }
            if b[0] >> 1 != 0 { return Target::Object(b[0] >> 1) }
        }
        Target::Cell(cell)
    }
}
impl Component for Viewport {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) { self.render(scene, ui, out); }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let hit = crate::mouse::walk::cell(scene, ui, at);
        let clicked = scene.squad.active && ui.walk.borrow_mut().gesture(e, at, hit);
        if ui.fixed_screen {
            if matches!(e, UiEvent::Wheel(_) | UiEvent::Key(90 | 122 | mt_engine::io::keys::HOME) | UiEvent::Down(Button::Middle) | UiEvent::Up(Button::Middle)) { return Vec::new(); }
            if !matches!(e, UiEvent::Key(_)) && !screen_rect(ui.component_input.size).contains(at) { return vec![Intent::Hover(None)]; }
        }
        let target = hit.map(|c| self.target(scene, c));
        match *e {
            UiEvent::Wheel(_) => Vec::new(),
            UiEvent::Key(90 | 122) => vec![Intent::Zoom(if controls::zoom(ui.zoom) >= 2.0 { 0.25 } else { controls::next_zoom(ui.zoom, 1) })],
            UiEvent::Key(mt_engine::io::keys::HOME) => vec![Intent::Follow(Some(controls::selected(scene)))],
            UiEvent::Key(27) if scene.squad.active && ui.menu.is_none() => ui.walk.borrow_mut().cancel(scene, at).unwrap_or_else(|| vec![Intent::Raw(Event::Key(27))]),
            UiEvent::Key(k) => vec![Intent::Raw(Event::Key(k))],
            UiEvent::Move => {
                if ui.info.get().is_some_and(|(t, _)| Some(t) != target) { ui.info.set(None) }
                vec![Intent::Hover(target)]
            }
            UiEvent::Up(Button::Left) if scene.squad.active => {
                if ui.menu.is_some() { return vec![Intent::CloseMenu] }
                if !clicked { return Vec::new() }
                ui.walk.borrow_mut().reset();
                let Some(target) = target else { return Vec::new() };
                match target {
                    Target::Unit(Unit::Traveller(i)) => vec![Intent::Select(i)],
                    Target::Unit(Unit::Npc(k)) => {
                        if crate::mouse::pointer::hostile(scene, k) { vec![Intent::Verb { verb: Verb::Fire, target }] } else if !scene.combat { vec![Intent::Verb { verb: Verb::Converse, target }] } else { vec![Intent::OpenMenu { target, at }] }
                    }
                    Target::Object(_) => vec![Intent::Verb { verb: Verb::Take, target }],
                    Target::Cell(c) => ui.walk.borrow_mut().move_to(scene, c),
                    Target::Portrait(i) => vec![Intent::Select(i)],
                }
            }
            UiEvent::Up(Button::Right) if scene.squad.active => {
                if ui.menu.is_some() { ui.walk.borrow_mut().reset(); return vec![Intent::CloseMenu] }
                let grenade = scene.walkers[scene.squad.selected.min(4) as usize].0[mt1::ground::state::W_BUILTIN as usize] != 0;
                let Some(target) = target.filter(|t| grenade || !matches!(t, Target::Cell(_))) else { return Vec::new() };
                ui.info.set(Some((target, 1 + scene.squad.preview.len().min(7) as u8)));
                vec![Intent::Hover(Some(target)), Intent::OpenMenu { target, at }]
            }
            UiEvent::Up(Button::Left | Button::Right) => {
                let Some((col, row)) = Self::oracle_pointer(scene, ui, ui.component_input.size, at) else { return Vec::new(); };
                vec![Intent::Raw(Event::Mouse(MouseEvent { event: if *e == UiEvent::Up(Button::Left) { mouse::LEFT_RELEASE } else { mouse::RIGHT_RELEASE }, buttons: 0, col, row }))]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::io::keys;
    fn scene() -> Scene {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); let m = Arc::make_mut(&mut s.map);
        m.rows = 12; m.cols = 20; m.stride = 20; m.cells = vec![0; 12 * 20 * 3];
        for cell in m.cells.chunks_mut(3) { cell[0] = 1; }
        s
    }
    fn scroll_scene() -> Scene {
        let mut s = scene(); s.view = (20, 20); s.pos = (26, 32);
        let m = Arc::make_mut(&mut s.map); m.rows = 40; m.cols = 60; m.stride = 60;
        m.cells = [1, 0, 0].repeat(40 * 60); s
    }
    fn origin(s: &Scene, ui: &UiState) -> (i32, i32) {
        let c = Camera::new(s, ui, SCREEN_PIXELS); ((c.top / 16.0) as i32, (c.left / 16.0) as i32)
    }
    fn modern() -> UiState { UiState { fixed_screen: true, ..Default::default() } }
    fn walker(s: &mut Scene, i: usize, at: (i16, i16)) {
        s.walkers[i].0[0] = 1;
        s.walkers[i].0[8..10].copy_from_slice(&(at.0 * 2).to_le_bytes());
        s.walkers[i].0[10..12].copy_from_slice(&(at.1 * 2).to_le_bytes());
    }
    #[test]
    fn dark_room_is_drawn_dim_never_black() {
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let g = crate::scenario::load(&root.join("scenarios/efate_dark.toml"), std::path::Path::new(&files)).unwrap().game;
        let mut s = Scene::capture(&g, mt1::ui::ScreenKind::Ground, None); s.squad.active = true; assert!(s.art.dark, "no light on: dark");
        let v = Viewport::new(Arc::new(Assets::load(&mut mt_engine::io::Dir(std::path::PathBuf::from(&files)))));
        let ui = UiState { fixed_screen: true, ..UiState::default() }; let size = (384, 224);
        let draw = |s: &Scene| { let mut out = Surface::new(size.0, size.1); v.render(s, &ui, &mut out); out };
        let dark = draw(&s); let mut lit = s.clone(); lit.art.dark = false; let lit = draw(&lit);
        let count = |o: &Surface, f: &dyn Fn(u8) -> bool| o.indexed.iter().filter(|&&c| f(c)).count();
        let total = (size.0 * size.1) as usize;
        assert!(count(&dark, &|c| c != 0) > total / 10, "never black: {} lit pixels of {total}", count(&dark, &|c| c != 0));
        assert!(count(&dark, &|c| c >= 9) * 3 < count(&lit, &|c| c >= 9), "dim: bright pixels {} vs lit {}", count(&dark, &|c| c >= 9), count(&lit, &|c| c >= 9));
        assert!(count(&dark, &|c| c == 8) > count(&lit, &|c| c == 8), "the far squares turn dark grey");
    }
    #[test]
    fn modern_camera_slides_one_square_at_each_two_square_margin() {
        for (before, after, expected) in [((12, 16), (11, 16), (9, 10)), ((14, 16), (15, 16), (11, 10)),
            ((13, 12), (13, 11), (10, 9)), ((13, 19), (13, 20), (10, 11))] {
            let mut s = scroll_scene(); let ui = modern();
            s.pos = (before.0 * 2, before.1 * 2); assert_eq!(origin(&s, &ui), (10, 10));
            s.pos = (after.0 * 2, after.1 * 2); assert_eq!(origin(&s, &ui), expected);
            s.view = (0, 0); assert_eq!(origin(&s, &ui), expected);
            assert_eq!(origin(&s, &ui.clone()), expected, "render copies share camera memory");
        }
    }
    #[test]
    fn modern_camera_half_step_never_leaves_a_fractional_origin_or_jitters() {
        for (start, half, end, expected) in [((24, 32), (23, 32), (22, 32), (9, 10)),
            ((28, 32), (29, 32), (30, 32), (11, 10)), ((26, 24), (26, 23), (26, 22), (10, 9)),
            ((26, 38), (26, 39), (26, 40), (10, 11))] {
            let mut s = scroll_scene(); let ui = modern(); s.pos = start; assert_eq!(origin(&s, &ui), (10, 10));
            s.pos = half; assert_eq!(origin(&s, &ui), expected);
            let c = Camera::new(&s, &ui, SCREEN_PIXELS); assert_eq!((c.top % 16.0, c.left % 16.0), (0.0, 0.0));
            s.pos = end; assert_eq!(origin(&s, &ui), expected);
            s.pos = half; assert_eq!(origin(&s, &ui), expected);
            s.pos = start; assert_eq!(origin(&s, &ui), expected, "the interior is a dead zone, not a centring spring");
        }
    }
    #[test]
    fn modern_camera_stays_inside_margin_and_clamps_at_map_bounds() {
        let mut s = scroll_scene(); let ui = modern(); assert_eq!(origin(&s, &ui), (10, 10));
        for r in [13, 12, 13, 14, 13] { for c in [16, 15, 14, 13, 12, 13, 14, 15, 16, 17, 18, 19, 18, 17, 16] {
            s.pos = (r * 2, c * 2); assert_eq!(origin(&s, &ui), (10, 10));
        } }
        for (at, expected) in [((0, 0), (0, 0)), ((0, 59), (0, 48)), ((39, 0), (33, 0)), ((39, 59), (33, 48))] {
            s.pos = (at.0 * 2, at.1 * 2); assert_eq!(origin(&s, &ui), expected);
        }
        let m = Arc::make_mut(&mut s.map); m.rows = 3; m.cols = 4;
        s.pos = (2, 2); assert_eq!(origin(&s, &ui), (0, 0));
    }
    #[test]
    fn modern_camera_snaps_on_teleport_map_entry_and_clock_restore() {
        let mut s = scroll_scene(); s.context.clock = Some(100); let ui = modern();
        assert_eq!(origin(&s, &ui), (10, 10));
        s.pos = (60, 80); s.view = (54, 70); assert_eq!(origin(&s, &ui), (27, 35));
        s.pos = (26, 32); s.view = (20, 20); assert_eq!(origin(&s, &ui), (10, 10));
        s.view = (22, 22); s.context.clock = Some(50); assert_eq!(origin(&s, &ui), (11, 11));
        s.view = (20, 20); s.level = 1; assert_eq!(origin(&s, &ui), (10, 10));
        s.view = (22, 22); s.party.0 = vec![0; 8]; s.party.0[6] = 2; s.party.0[7] = 1;
        assert_eq!(origin(&s, &ui), (11, 11), "new map with the same dimensions");
        s.view = (20, 20); let m = Arc::make_mut(&mut s.map); m.cells[1] = 2; m.cells[4] = 3;
        assert_eq!(origin(&s, &ui), (11, 11), "doors or NPCs changing map bytes do not reset the camera");
        s.view = (20, 20); GroundCamera::observe(&s, None, &ui); assert_eq!(origin(&s, &ui), (10, 10));
        s.view = (22, 22); GroundCamera::observe(&Scene::program(mt1::ui::ScreenKind::Space), Some(&s), &ui);
        assert_eq!(origin(&s, &ui), (11, 11));
    }
    #[test]
    fn modern_camera_keeps_its_dead_zone_across_inventory_and_service_pictures() {
        let mut s = scroll_scene(); let ui = modern(); s.pos = (24, 32); assert_eq!(origin(&s, &ui), (10, 10));
        s.pos.0 -= 2; assert_eq!(origin(&s, &ui), (9, 10));
        s.pos.0 += 2; assert_eq!(origin(&s, &ui), (9, 10));
        for screen in [mt1::ui::ScreenKind::Record, mt1::ui::ScreenKind::Picture] {
            let mut overlay = s.clone(); overlay.screen = screen;
            GroundCamera::observe(&overlay, Some(&s), &ui); GroundCamera::observe(&s, Some(&overlay), &ui);
            assert_eq!(origin(&s, &ui), (9, 10), "a menu must not recenter a stationary party");
        }
    }
    #[test]
    fn modern_fight_follows_selection_and_keeps_companions_when_they_fit() {
        let mut s = scroll_scene(); s.combat = true; s.squad.active = true; s.squad.selected = 2;
        walker(&mut s, 0, (12, 10)); walker(&mut s, 2, (13, 19));
        let ui = modern(); assert_eq!(origin(&s, &ui), (10, 10));
        walker(&mut s, 2, (13, 20)); assert_eq!(origin(&s, &ui), (10, 11), "selection margin has priority over the far companion");
        walker(&mut s, 0, (12, 11)); assert_eq!(origin(&s, &ui), (10, 11));
        s.squad.selected = 0; assert_eq!(origin(&s, &ui), (10, 9), "both now fit with the new selection's margin");
        for w in s.walkers.iter().filter(|w| w.active()) {
            let (r, c) = origin(&s, &ui); assert!((r..r + 7).contains(&(i32::from(w.x()) / 2)) && (c..c + 12).contains(&(i32::from(w.y()) / 2)));
        }
    }
    #[test]
    fn classic_camera_keeps_even_half_cell_pages_and_ignores_modern_memory() {
        let mut s = scroll_scene(); let ui = modern(); origin(&s, &ui);
        let classic = UiState { preset: Preset::Classic, ..ui.clone() };
        for page in [(20, 20), (21, 23), (0, 0), (66, 96)] {
            s.view = page; let c = Camera::new(&s, &classic, SCREEN_PIXELS);
            assert_eq!((c.top, c.left, c.zoom), (page.0 as f32 * 8.0, page.1 as f32 * 8.0, 1.0));
            assert_eq!(Viewport::fixed_world(&s, None, SCREEN_PIXELS, (0, 0)), Some((i32::from(page.0) * 8, i32::from(page.1) * 8)));
        }
    }
    #[test]
    fn scrolled_draw_walk_and_pointer_agree_at_every_pixel_and_cell_boundary() {
        let mut s = scroll_scene(); s.pos = (26, 38); let mut ui = modern(); origin(&s, &ui); s.pos.1 += 2;
        for (i, cell) in Arc::make_mut(&mut s.map).cells.chunks_exact_mut(3).enumerate() { cell[1] = (i % 15 + 1) as u8; }
        let mut assets = Assets::default();
        assets.terrain.push(Bitmap { w: 320, h: 16, pixels: (0..320 * 16).map(|i| (i % 320 / 16) as u8).collect() });
        let v = Viewport::new(Arc::new(assets));
        for size in [(192, 112), (382, 167), (197, 83), (384, 224)] {
            ui.component_input.size = size; let mut out = Surface::new(size.0, size.1); v.render(&s, &ui, &mut out);
            let (top, left) = origin(&s, &ui); let rect = screen_rect(size);
            for y in 0..size.1 as i32 { for x in 0..size.0 as i32 {
                let expected = rect.contains((x, y)).then(|| (top + (y - rect.y) * 112 / rect.h as i32 / 16,
                    left + (x - rect.x) * 192 / rect.w as i32 / 16));
                let cell = crate::mouse::walk::cell(&s, &ui, (x, y));
                assert_eq!(cell, expected.map(|(r, c)| (r as i16, c as i16)));
                assert_eq!(crate::mouse::pointer::map_cell(&s, &ui, size, (x, y), false).map(|c| c.row_col()), cell);
                assert_eq!(out.get(x, y), Some(expected.map_or(0, |(r, c)| s.map.cell(r, c).unwrap()[1])));
                if let Some((col, row)) = Viewport::oracle_pointer(&s, &ui, size, (x, y)) {
                    let p = Viewport::local_pointer(&s, &ui, size, col, row).unwrap();
                    assert_eq!(crate::mouse::walk::cell(&s, &ui, p), cell);
                }
            } }
        }
    }
    #[test]
    fn hover_and_click_read_the_walk_cell_and_off_the_map_is_nothing() {
        let mut s = scene(); s.squad.active = true; let v = Viewport::new(Arc::default());
        let mut ui = UiState { camera: (2.0, 3.0), ..UiState::default() }; ui.component_input.size = (640, 400);
        let (mut on, mut off) = (0, 0);
        for y in (0..400).step_by(7) { for x in (0..640).step_by(7) {
            let cell = crate::mouse::walk::cell(&s, &ui, (x, y)); if cell.is_some() { on += 1 } else { off += 1 }
            assert_eq!(v.input(&s, &ui, &UiEvent::Move, (x, y)), [Intent::Hover(cell.map(|c| v.target(&s, c)))]);
        } }
        assert!(on > 0 && off > 0, "{on} {off}");
    }
    #[test]
    fn scrolled_overlays_npc_clicks_and_darkness_use_the_same_camera() {
        use mt1::ground::{orders::Sight, round::Fx, rules::Band};
        let mut s = scroll_scene(); s.pos = (26, 38); s.squad.active = true; s.combat = true;
        walker(&mut s, 0, (13, 19)); let ui = modern(); assert_eq!(origin(&s, &ui), (10, 10));
        s.pos.1 += 2; walker(&mut s, 0, (13, 20)); assert_eq!(origin(&s, &ui), (10, 11));
        let target = (13, 22); Arc::make_mut(&mut s.map).cells[(13 * 60 + 22) * 3 + 2] = 1;
        s.npcs = vec![mt1::ui::scene::Npc([0; 46])]; s.npcs[0].0[32] = 26; s.npcs[0].0[34] = 44; s.npcs[0].0[8] = 2;
        s.art.slots = vec![0; 64];
        let mut assets = Assets::default(); assets.people = vec![Bitmap { w: 384, h: 16, pixels: vec![9; 384 * 16] }; 5];
        let v = Viewport::new(Arc::new(assets)); let at = (11 * 16 + 8, 3 * 16 + 8);
        let mut out = Surface::new(192, 112); v.render(&s, &ui, &mut out); assert_eq!(out.get(at.0, at.1), Some(9));
        v.input(&s, &ui, &UiEvent::Down(Button::Left), at);
        assert_eq!(v.input(&s, &ui, &UiEvent::Up(Button::Left), at), [Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(1)) }]);
        for (attitude, hit, hostile) in [(0, 0, true), (1, 0, false), (2, 0, false), (3, 1, true)] {
            let mut n = s.clone(); n.npcs[0].0[13] = attitude; n.npcs[0].0[0x26] = hit; assert_eq!(crate::mouse::pointer::hostile(&n, 1), hostile);
            v.input(&n, &ui, &UiEvent::Down(Button::Left), at);
            assert_eq!(v.input(&n, &ui, &UiEvent::Up(Button::Left), at) == [Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(1)) }], hostile);
        }
        assert_eq!(v.input(&s, &ui, &UiEvent::Up(Button::Right), at), [Intent::Hover(Some(Target::Unit(Unit::Npc(1)))), Intent::OpenMenu { target: Target::Unit(Unit::Npc(1)), at }]);
        assert_eq!(ui.info.get().map(|i| i.0), Some(Target::Unit(Unit::Npc(1))));
        assert_eq!(Viewport::oracle_pointer(&s, &ui, SCREEN_PIXELS, at), None, "1990 input must not wrap an off-page click");
        let classic = UiState { preset: Preset::Classic, ..ui.clone() };
        let mut old = Surface::new(192, 112); v.render(&s, &classic, &mut old); assert_ne!(old.get(at.0, at.1), Some(9));
        s.squad.path = vec![((14, 21), PATH_RUN)]; s.squad.destinations[0] = Some((12, 21));
        s.squad.sight = Some(Sight { from: (13, 20), to: target, los: false, cover: true, band: Band::Short });
        s.squad.fx = Fx { seq: 3, shots: vec![((13, 20), target, 0xAA)], ..Default::default() };
        for hover in [Target::Cell((12, 20)), Target::Unit(Unit::Npc(1))] {
            let ui = UiState { hover: Some(hover), replay: Some((3, 0.1)), ..ui.clone() };
            let mut rebased = s.clone(); rebased.view = (20, 22);
            let mut expected = Surface::new(192, 112); v.render(&rebased, &UiState { preset: Preset::Classic, ..ui.clone() }, &mut expected);
            v.render(&s, &ui, &mut out); assert!(out == expected, "cursor, predictor, green/red lines, brackets and replay share the origin");
        }
        s.art.dark = true; s.squad.fx = Fx::default();
        let mut hidden = s.clone(); Arc::make_mut(&mut hidden.map).cells[(13 * 60 + 22) * 3 + 2] = 0;
        v.render(&s, &ui, &mut out); let mut without = Surface::new(192, 112); v.render(&hidden, &ui, &mut without);
        assert!(out != without, "Dark rooms 1 = A: an NPC within two squares of a Traveller is drawn in the dim room"); assert!(out.indexed.iter().any(|&c| c != 0), "dim, never black");
        s.squad.active = false;
        for button in [Button::Left, Button::Right] { assert!(v.input(&s, &ui, &UiEvent::Up(button), at).is_empty()); }
    }
    #[test]
    #[ignore = "headless PNG proof; needs MT1_GAMEFILES"]
    fn headless_scroll_proof() {
        use mt1::ground::{orders, state::Mem, squad};
        use mt_engine::io::{Dir, Input};
        use std::{fs, path::PathBuf};
        struct Idle;
        impl Input for Idle {
            fn next_key(&mut self) -> u16 { panic!("unexpected modal wait in scroll proof") }
            fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { clock.wait_tick(); Event::Idle }
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let files = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let outdir = root.join("local/scroll"); fs::create_dir_all(&outdir).unwrap();
        let assets = Arc::new(Assets::load(&mut Dir(files.clone()))); let viewport = Viewport::new(assets.clone());
        let load = |name: &str| {
            let mut g = crate::scenario::load(&root.join(format!("scenarios/{name}.toml")), &files).unwrap().game;
            crate::scenario::attach_scene(&mut g); squad::set_classic(&mut g, false); g.input = Box::new(Idle); g
        };
        let mut manifest = String::from("frame\tscenario\tparty half-cells\tsim page half-cells\tcamera cells\n");
        let mut frame = |name: &str, scenario: &str, s: &Scene, ui: &UiState| {
            let mut surface = Surface::new(192, 112); viewport.render(s, ui, &mut surface);
            let look = crate::look::Look { shape: crate::look::Shape::Square, ..Default::default() };
            let metrics = super::super::render::Metrics::component([768, 448], SCREEN_PIXELS, &look);
            let pixels = super::super::render::compose([(Rect::new(0, 0, 192, 112), surface)], metrics, &look);
            fs::write(outdir.join(format!("{name}.png")), crate::plus::png::rgb(768, 448, &pixels).unwrap()).unwrap();
            manifest.push_str(&format!("{name}.png\t{scenario}\t{:?}\t{:?}\t{:?}\n", s.pos, s.view, origin(s, ui)));
        };
        for (edge, margin, delta) in [("north", (2, 6), (-1, 0)), ("south", (4, 6), (1, 0)),
            ("west", (3, 2), (0, -1)), ("east", (3, 9), (0, 1))] {
            let mut g = load("efate_quiet");
            let mut candidates: Vec<(i16, i16)> = (5..g.mem.map_blocks_high() as i16 * 12 - 5)
                .flat_map(|r| (10..g.mem.map_stride() - 12).map(move |c| (r, c))).collect();
            candidates.sort_by_key(|&(r, c)| (r - 32).abs() + (c - 9).abs());
            let start = candidates.into_iter().find(|&p| [(0, 1), (0, -1), (1, 0), (-1, 0)].into_iter().all(|d| {
                let to = (p.0 + d.0, p.1 + d.1);
                orders::find_path(&g, 0, p, to).is_some_and(|path| path == [to])
                    && orders::find_path(&g, 0, to, p).is_some_and(|path| path == [p])
            })).expect("clear cross on Efate");
            mt1::ground::walk::set_player_pos(&mut g, start.0, start.1);
            mt1::ground::render::redraw(&mut g);
            g.mem.set_view_x((start.0 - margin.0) * 2); g.mem.set_view_y((start.1 - margin.1) * 2);
            let step = (start.0 + delta.0, start.1 + delta.1);
            let before = Scene::capture(&g, mt1::ui::ScreenKind::Ground, None); let ui = modern();
            let old = origin(&before, &ui); frame(&format!("explore-{edge}-before"), "efate_quiet", &before, &ui);
            orders::intent(&mut g, Intent::Verb { verb: Verb::Move, target: Target::Cell(step) });
            let after = Scene::capture(&g, mt1::ui::ScreenKind::Ground, Some(&before.map));
            assert_eq!(after.pos, (step.0 * 2, step.1 * 2), "{edge}: {:?}", after.squad.log);
            assert_eq!(origin(&after, &ui), (old.0 + i32::from(delta.0), old.1 + i32::from(delta.1)), "{edge}");
            frame(&format!("explore-{edge}-after"), "efate_quiet", &after, &ui);
        }
        let fight = || {
            let mut g = load("efate_broken");
            let (who, from, step, delta, page) = (0..5u8).find_map(|who| {
                let from = mt1::ground::round::traveller_cell(&g, who);
                [((2, 6), (-1, 0)), ((4, 6), (1, 0)), ((3, 2), (0, -1)), ((3, 9), (0, 1))].into_iter().find_map(|(margin, delta)| {
                    let to = (from.0 + delta.0, from.1 + delta.1);
                    let page = ((from.0 - margin.0) * 2, (from.1 - margin.1).max(0) * 2);
                    if delta.1 != 0 && from.1 < margin.1 { return None; }
                    orders::find_path(&g, who, from, to).filter(|p| p == &[to]).map(|_| (who, from, to, delta, page))
                })
            }).expect("a one-square combat step");
            orders::select(&mut g, who); g.mem.set_view_x(page.0); g.mem.set_view_y(page.1);
            (g, who, from, step, delta)
        };
        let (mut g, who, _, step, delta) = fight();
        let before = Scene::capture(&g, mt1::ui::ScreenKind::Ground, None); let ui = modern();
        let old = origin(&before, &ui); frame("fight-before", "efate_broken", &before, &ui);
        orders::intent(&mut g, Intent::Verb { verb: Verb::Move, target: Target::Cell(step) });
        let after = Scene::capture(&g, mt1::ui::ScreenKind::Ground, Some(&before.map));
        assert_eq!((after.walkers[who as usize].x(), after.walkers[who as usize].y()), (step.0 * 2, step.1 * 2), "{:?}", after.squad.log);
        assert_eq!(origin(&after, &ui), (old.0 + i32::from(delta.0), old.1 + i32::from(delta.1))); frame("fight-after", "efate_broken", &after, &ui);
        let (mut g, _, _, step, delta) = fight(); let n = Mem::npc(14);
        let npc = (step.0 + 2 * delta.0, step.1 + 2 * delta.1);
        let old_npc = (g.mem.read_i(n.add(32), 2), g.mem.read_i(n.add(34), 2));
        mt1::ground::walk::release_cell(&mut g, old_npc.0 as i16, old_npc.1 as i16);
        g.mem.write(n.add(32), (npc.0 * 2) as u32, 2); g.mem.write(n.add(34), (npc.1 * 2) as u32, 2);
        mt1::ground::walk::set_cell_flags(&mut g, npc.0 * 2, npc.1 * 2, 14);
        let before = Scene::capture(&g, mt1::ui::ScreenKind::Ground, None); let mut ui = modern();
        frame("npc-before", "efate_broken", &before, &ui);
        orders::intent(&mut g, Intent::Verb { verb: Verb::Move, target: Target::Cell(step) });
        let after = Scene::capture(&g, mt1::ui::ScreenKind::Ground, Some(&before.map));
        assert_eq!(after.pos, (step.0 * 2, step.1 * 2));
        assert!(!mt1::ground::round::on_screen(&g, npc), "the NPC remains beyond the old page");
        assert_eq!(after.map.cell(i32::from(npc.0), i32::from(npc.1)).unwrap()[2] & 63, 14);
        let mut rendered = Surface::new(192, 112); viewport.render(&after, &ui, &mut rendered);
        let mut absent = after.clone(); Arc::make_mut(&mut absent.map).cells[(npc.0 as usize * after.map.cols as usize + npc.1 as usize) * 3 + 2] &= !63;
        let mut without = Surface::new(192, 112); viewport.render(&absent, &ui, &mut without);
        assert!(rendered.indexed != without.indexed, "the NPC must actually draw before contact");
        ui.hover = Some(Target::Unit(Unit::Npc(14))); frame("npc-visible-before-bump", "efate_broken", &after, &ui);
        frame("npc-classic-hidden", "efate_broken", &after, &UiState { preset: Preset::Classic, ..ui.clone() });
        *ui.frame = crate::pipeline::Frame::from_rgb(&g.fb.rgb(0, &g.dac));
        let mut layout = super::super::Layout::default(); super::super::components::register(&mut layout, assets);
        let pixels = super::super::render::main(&mut layout, &after, &ui, &crate::look::Look::default(), [1080, 1120]);
        fs::write(outdir.join("npc-modern-full.png"), crate::plus::png::rgb(1080, 1120, &pixels).unwrap()).unwrap();
        drop(frame); fs::write(outdir.join("frames.tsv"), manifest).unwrap();
    }
    #[test]
    fn map_uses_the_largest_proportional_fit() {
        for (size, expected) in [
            ((382, 167), Rect::new(48, 0, 286, 167)),
            ((512, 228), Rect::new(61, 0, 390, 228)),
            ((384, 224), Rect::new(0, 0, 384, 224)),
            ((222, 167), Rect::new(0, 19, 222, 129)),
            ((192, 112), Rect::new(0, 0, 192, 112)),
            ((197, 83), Rect::new(27, 0, 142, 83)),
        ] {
            assert_eq!(screen_rect(size), expected, "{size:?}: only aspect-ratio remainder may be blank");
        }
    }
    #[test]
    fn overlays_enlarge_with_the_same_map_pixels() {
        use mt1::ground::{orders::Sight, round::Fx, rules::Band};
        let mut s = scene(); s.view = (2, 4); s.squad.active = true; s.combat = true;
        s.walkers[0].0[0] = 1; s.walkers[0].0[8] = 6; s.walkers[0].0[10] = 8;
        s.squad.path = vec![((5, 6), PATH_RUN)];
        s.squad.sight = Some(Sight { from: (3, 4), to: (4, 8), los: false, cover: true, band: Band::Medium });
        s.squad.fx = Fx { seq: 3, shots: vec![((3, 4), (2, 10), 0xAA)], ..Fx::default() };
        let ui = UiState { fixed_screen: true, hover: Some(Target::Cell((2, 3))), replay: Some((3, 0.1)), ..UiState::default() };
        let v = Viewport::new(Arc::default());
        let mut native = Surface::new(192, 112); v.draw(&s, &ui, &mut native);
        for (at, colour, what) in [((16, 16), 15, "X cursor"), ((70, 70), 14, "predictor"),
            ((96, 48), 12, "target box"), ((136, 24), 10, "fight line")] {
            assert_eq!(native.get(at.0, at.1), Some(colour), "fixture {what}");
        }
        for size in [(382, 167), (512, 228), (384, 224), (222, 167)] {
            let mut out = Surface::new(size.0, size.1); v.draw(&s, &ui, &mut out);
            let r = screen_rect(size);
            assert!(r.w > 192 && r.h > 112, "the overlays must enlarge, not stay in a native-size island");
            for y in 0..size.1 { for x in 0..size.0 {
                let expected = if r.contains((x as i32, y as i32)) {
                    native.get(((x - r.x as u32) * 192 / r.w) as i32, ((y - r.y as u32) * 112 / r.h) as i32).unwrap()
                } else { 0 };
                assert_eq!(out.get(x as i32, y as i32), Some(expected), "{size:?} at {x},{y}");
            } }
        }
    }
    #[test]
    fn fixed_screen_is_84_cells_at_every_host_size_and_ignores_camera_controls() {
        let mut s = scene(); s.view = (2, 4);
        let mut assets = Assets::default(); assets.terrain.push(Bitmap { w: 16, h: 16, pixels: vec![5; 256] });
        let v = Viewport::new(Arc::new(assets));
        for size in [(192, 112), (1531, 1221), (982, 806), (320, 200), (197, 83), (12, 7)] {
            let mut ui = UiState { fixed_screen: true, preset: Preset::Classic, camera: (-900.0, 600.0), zoom: 2.0, follow: Some(Unit::Traveller(0)), ..UiState::default() };
            ui.component_input.size = size;
            let mut out = Surface::new(size.0, size.1); v.draw(&s, &ui, &mut out);
            let r = screen_rect(size);
            let mut cells = std::collections::BTreeSet::new();
            for y in 0..size.1 as i32 { for x in 0..size.0 as i32 {
                assert_eq!(out.get(x, y), Some(if r.contains((x, y)) { 5 } else { 0 }));
                if let Some(cell) = crate::mouse::walk::cell(&s, &ui, (x, y)) { cells.insert(cell); }
            } }
            assert_eq!(cells.len(), 84, "{size:?}");
            assert_eq!(cells.first(), Some(&(1, 2))); assert_eq!(cells.last(), Some(&(7, 13)));
            let before = out;
            for intent in [Intent::Zoom(0.25), Intent::Pan(1, 1), Intent::PanTo((9, 9)), Intent::Follow(None)] { assert!(ui.apply(intent)); }
            let mut after = Surface::new(size.0, size.1); v.draw(&s, &ui, &mut after); assert_eq!(before, after);
            assert!(v.input(&s, &ui, &UiEvent::Wheel(1), (r.x, r.y)).is_empty());
            assert!(!v.input(&s, &ui, &UiEvent::Move, (r.x, r.y)).iter().any(|i| matches!(i, Intent::Pan(..) | Intent::PanTo(_) | Intent::Follow(_))));
        }
    }
    #[test]
    fn fixed_screen_hostile_one_column_past_the_screen_draws_no_pixel() {
        let mut s = scene(); s.view = (2, 4); s.art.slots = vec![0; 64];
        s.npcs = vec![mt1::ui::scene::Npc([0; 46])];
        s.npcs[0].0[32..34].copy_from_slice(&6i16.to_le_bytes());
        s.npcs[0].0[34..36].copy_from_slice(&28i16.to_le_bytes());
        Arc::make_mut(&mut s.map).cells[(3 * 20 + 14) * 3 + 2] = 1;
        let mut assets = Assets::default(); assets.people = vec![Bitmap { w: 384, h: 16, pixels: vec![12; 384 * 16] }; 5];
        let v = Viewport::new(Arc::new(assets));
        for size in [(192, 112), (1531, 1221), (982, 806)] {
            let ui = UiState { fixed_screen: true, preset: Preset::Classic, ..UiState::default() };
            let mut with = Surface::new(size.0, size.1); v.draw(&s, &ui, &mut with);
            let mut absent = s.clone(); Arc::make_mut(&mut absent.map).cells[(3 * 20 + 14) * 3 + 2] = 0;
            let mut without = Surface::new(size.0, size.1); v.draw(&absent, &ui, &mut without);
            assert_eq!(with, without, "{size:?}");
            let mut visible = s.clone(); Arc::make_mut(&mut visible.map).cells[(3 * 20 + 13) * 3 + 2] = 1;
            visible.npcs[0].0[34..36].copy_from_slice(&26i16.to_le_bytes());
            let mut inside = Surface::new(size.0, size.1); v.draw(&visible, &ui, &mut inside);
            assert_ne!(inside, without, "the fixture must draw a hostile on the last visible column");
        }
    }
    #[test]
    fn fixed_screen_layout_keeps_all_rows_above_even_an_expanded_log() {
        let s = scene();
        for size in [[1531, 1221], [982, 806], [960, 720], [640, 480], [639, 479], [320, 200]] {
            for expanded in [false, true] {
                let metrics = super::super::render::Metrics::main(size, &crate::look::Look::default(), Some(&s));
                let mut ui = UiState { fixed_screen: true, log_expanded: expanded, log_lines: 20, ..UiState::default() };
                if metrics.classic { ui.preset = mt1::ui::Preset::Classic; }
                let mut layout = super::super::Layout::default();
                super::super::components::register(&mut layout, Arc::default()); layout.arrange_for(&s, &ui, metrics.logical);
                let super::super::Host::Dock(map) = layout.host("Viewport").unwrap() else { panic!() };
                let super::super::Host::Dock(log) = layout.host("Combat log").unwrap() else { panic!() };
                assert!(map.y + map.h as i32 <= log.y, "{size:?}, expanded={expanded}: {map:?}, {log:?}");
                ui.component_input.size = (map.w, map.h);
                let r = screen_rect((map.w, map.h)); assert!(r.w >= 12 && r.h >= 7);
                let mut cells = std::collections::BTreeSet::new();
                for row in 0..7 { for col in 0..12 {
                    cells.insert(crate::mouse::walk::cell(&s, &ui, (r.x + col * r.w as i32 / 12, r.y + row * r.h as i32 / 7)).unwrap());
                } }
                assert_eq!(cells.len(), 84, "{size:?}, expanded={expanded}");
            }
        }
    }
    #[test]
    fn off_map_pointer_never_converts_an_out_of_range_oracle_row() {
        let s = scene();
        let ui = UiState { camera: (2048.5, 6.0), ..UiState::default() };
        assert_eq!(Viewport::oracle_pointer(&s, &ui, (192, 112), (30, 40)), None);
        for row in [f32::MIN, -32768.0, 2048.5, 32767.0, f32::MAX, f32::NAN] {
            let ui = UiState { camera: (row, row), ..UiState::default() };
            for at in [(i32::MIN, i32::MIN), (-1, 0), (0, 0), (191, 111), (i32::MAX, i32::MAX)] {
                if let Some((col, row)) = Viewport::oracle_pointer(&s, &ui, (192, 112), at) {
                    assert!((0..192).contains(&col) && (54..166).contains(&row));
                }
            }
        }
    }
    #[test]
    fn wrapped_tileset_draws_nothing_instead_of_panicking() {
        let mut s = scene(); Arc::make_mut(&mut s.map).tileset = 0u8.wrapping_sub(b'1');
        let mut assets = Assets::default(); assets.terrain.push(Bitmap { w: 16, h: 16, pixels: vec![5; 256] });
        let v = Viewport::new(Arc::new(assets)); let ui = UiState { camera: Viewport::oracle_camera(&s), ..UiState::default() };
        let mut out = Surface::new(192, 112); v.draw(&s, &ui, &mut out);
        assert!(out.indexed.iter().all(|&c| c == 0));
        Arc::make_mut(&mut s.map).tileset = 0; v.draw(&s, &ui, &mut out); assert!(out.indexed.iter().all(|&c| c == 5));
    }
    #[test]
    fn zoom_follow_pan_and_raw_coordinates() {
        let s = scene(); let v = Viewport::new(Arc::default()); let mut ui = UiState { camera: Viewport::oracle_camera(&s), ..UiState::default() };
        assert!(v.input(&s, &ui, &UiEvent::Wheel(-1), (30, 40)).is_empty(), "no wheel zoom");
        assert_eq!(v.input(&s, &ui, &UiEvent::Key(keys::HOME), (0, 0)), [Intent::Follow(Some(controls::selected(&s)))]);
        assert_eq!(v.input(&s, &ui, &UiEvent::Up(Button::Left), (30, 40)), [Intent::Raw(Event::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: 30, row: 94 }))]);
        controls::begin(&mut ui, &UiEvent::Down(Button::Middle), (40, 40), (192, 112));
        assert!(!v.input(&s, &ui, &UiEvent::Move, (56, 56)).iter().any(|i| matches!(i, Intent::Pan(..) | Intent::PanTo(_) | Intent::Follow(_))), "no drag pan");
        controls::end(&mut ui, &UiEvent::Up(Button::Middle));
        assert!(!v.input(&s, &ui, &UiEvent::Move, (0, 50)).iter().any(|i| matches!(i, Intent::Pan(..) | Intent::Follow(_))), "no edge pan");
        ui.camera = (30.0, 30.0); assert!(v.input(&s, &ui, &UiEvent::Up(Button::Left), (50, 50)).is_empty());
        ui.camera = (4.0, 6.5); assert!(v.input(&s, &ui, &UiEvent::Up(Button::Left), (-1, 10)).is_empty());
    }
    #[test]
    fn world_left_click_by_context_exploration_door_and_offscreen_refusal() {
        let mut s = scene(); s.squad.active = true; s.combat = false; s.pos = (6, 8); s.context.event = -1;
        let m = Arc::make_mut(&mut s.map); m.interior = b'I'; for c in m.cells.chunks_mut(3) { c[1] = 5; } m.cells[(2 * 20 + 9) * 3 + 2] = 17; m.cells[(4 * 20 + 9) * 3..(4 * 20 + 9) * 3 + 3].copy_from_slice(&[1, 236, 0]); m.cells[(2 * 20 + 7) * 3 + 2] = 14;
        s.npcs = (0..32).map(|_| mt1::ui::scene::Npc([0; 46])).collect(); s.npcs[16].0[13] = 2;
        let v = Viewport::new(Arc::default()); let mut ui = UiState { camera: (6.0, 10.0), ..UiState::default() }; ui.component_input.size = (320, 192);
        let at = |r: i32, c: i32| (c * 16 + 8, r * 16 + 8);
        let click = |s: &Scene, p: (i32, i32)| { v.input(s, &ui, &UiEvent::Down(Button::Left), p); v.input(s, &ui, &UiEvent::Up(Button::Left), p) };
        assert_eq!(click(&s, at(2, 9)), [Intent::Verb { verb: Verb::Converse, target: Target::Unit(Unit::Npc(17)) }]);
        assert!(crate::mouse::walk::door(&s, (4, 9)));
        assert_eq!(click(&s, at(4, 9)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((3, 8)) }], "walk to the nearest square beside the door, then bump it");
        assert!(click(&s, (at(2, 7).0, 192 + 8)).is_empty(), "below the viewport: no shot, no walk");
        assert!(click(&s, (-8, at(2, 7).1)).is_empty(), "left of the viewport");
    }
    #[test]
    fn squad_mode_clicks_are_select_verb_and_menu_intents() {
        let mut s = scene(); s.squad.active = true; s.combat = true;
        s.walkers[2].0[0] = 1; s.walkers[2].0[8..10].copy_from_slice(&6i16.to_le_bytes()); s.walkers[2].0[10..12].copy_from_slice(&8i16.to_le_bytes());
        let m = Arc::make_mut(&mut s.map); m.cells[(2 * 20 + 7) * 3 + 2] = 14; m.cells[(2 * 20 + 9) * 3 + 2] = 17; m.cells[(5 * 20 + 2) * 3] = 1 | (3 << 1);
        s.npcs = (0..32).map(|_| mt1::ui::scene::Npc([0; 46])).collect(); s.npcs[16].0[13] = 2;
        let v = Viewport::new(Arc::default()); let mut ui = UiState { camera: (6.0, 10.0), ..UiState::default() };
        ui.component_input.size = (320, 192);
        let at = |r: i32, c: i32| (c * 16 + 8, r * 16 + 8);
        let click = |s: &Scene, r, c| { v.input(s, &ui, &UiEvent::Down(Button::Left), at(r, c)); v.input(s, &ui, &UiEvent::Up(Button::Left), at(r, c)) };
        assert_eq!(click(&s, 3, 4), [Intent::Select(2)]);
        assert_eq!(click(&s, 2, 7), [Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }]);
        assert_eq!(click(&s, 2, 9), [Intent::OpenMenu { target: Target::Unit(Unit::Npc(17)), at: at(2, 9) }], "a neutral while broken: no safe default");
        assert_eq!(click(&s, 5, 2), [Intent::Verb { verb: Verb::Take, target: Target::Object(3) }]);
        assert_eq!(click(&s, 8, 8), [Intent::Verb { verb: Verb::Move, target: Target::Cell((8, 8)) }]);
        assert!(v.input(&s, &ui, &UiEvent::Up(Button::Right), at(8, 8)).is_empty(), "right on empty ground = nothing"); assert_eq!(ui.info.get(), None);
        assert_eq!(v.input(&s, &ui, &UiEvent::Up(Button::Right), at(2, 7)), [Intent::Hover(Some(Target::Unit(Unit::Npc(14)))), Intent::OpenMenu { target: Target::Unit(Unit::Npc(14)), at: at(2, 7) }], "right = the information and the menu of applicable actions, never an action itself");
        assert_eq!(ui.info.get(), Some((Target::Unit(Unit::Npc(14)), 1)));
        v.input(&s, &ui, &UiEvent::Move, at(8, 8)); assert_eq!(ui.info.get(), None, "the pin follows the pointer off its target");
        let open = UiState { menu: Some((Target::Cell((0, 0)), (0, 0))), ..ui.clone() };
        assert_eq!(v.input(&s, &open, &UiEvent::Up(Button::Left), at(8, 8)), [Intent::CloseMenu]);
        s.squad.active = false; assert!(matches!(click(&s, 3, 4)[..], [Intent::Raw(_)]), "Classic 1990 combat: the 1990 pointer");
        s.squad.active = true; s.squad.path = vec![((8, 8), 0)]; s.squad.selected = 2; let mut out = Surface::new(320, 192); v.draw(&s, &ui, &mut out);
        let (x, y) = at(8, 8); assert_eq!(out.get(x - 2, y - 2), Some(15), "the path dot at the hovered square's centre");
        let (x, y) = at(3, 4); assert_eq!(out.get(x - 8, y - 8), Some(3), "Trav3's ring in its colour");
        let ui = UiState { hover: Some(Target::Portrait(2)), ..ui.clone() }; s.squad.selected = 0; s.squad.path.clear(); let mut out = Surface::new(320, 192); v.draw(&s, &ui, &mut out);
        assert_eq!(out.get(x - 8, y - 8), Some(15), "hovering Trav3's card frames Trav3's figure on the map");
    }
    #[test]
    fn sight_bracket_tracer_replay_and_overview_markers() {
        use mt1::ground::{orders::Sight, round::Fx, rules::Band};
        let mut s = scene(); s.squad.active = true; s.combat = true; s.squad.selected = 0;
        s.walkers[0].0[0] = 1; s.walkers[0].0[8..10].copy_from_slice(&6i16.to_le_bytes()); s.walkers[0].0[10..12].copy_from_slice(&8i16.to_le_bytes());
        s.npcs = (0..32).map(|_| mt1::ui::scene::Npc([0; 46])).collect(); s.npcs[13].0[32] = 8; s.npcs[13].0[34] = 16; Arc::make_mut(&mut s.map).cells[(4 * 20 + 8) * 3 + 2] = 14;
        let v = Viewport::new(Arc::default()); let ui = UiState { camera: (6.0, 10.0), ..UiState::default() };
        let at = |r: i32, c: i32| ((c as f32 * 16.0 - (10.0 * 16.0 - 96.0)) as i32, (r as f32 * 16.0 - (6.0 * 16.0 - 56.0)) as i32);
        s.squad.sight = Some(Sight { from: (3, 4), to: (4, 8), los: false, cover: true, band: Band::Medium });
        let mut out = Surface::new(192, 112); v.draw(&s, &ui, &mut out);
        let (x, y) = at(4, 8); assert_eq!(out.get(x, y), Some(12), "the red bracket corner without a line of sight"); assert_eq!(out.get(x + 12, y + 2), Some(11), "the cover icon");
        s.squad.sight = Some(Sight { from: (3, 4), to: (4, 8), los: true, cover: false, band: Band::Medium }); let mut out = Surface::new(192, 112); v.draw(&s, &ui, &mut out);
        assert_eq!((out.get(x, y), out.get(x + 12, y + 2)), (Some(14), Some(0)));
        s.squad.sight = None; s.squad.fx = Fx { seq: 3, unit: Some(Unit::Traveller(0)), shots: vec![((3, 4), (4, 8), 0x78)], hits: vec![(Unit::Npc(14), (4, 8), 4)], ..Fx::default() };
        let mut out = Surface::new(192, 112); v.draw(&s, &UiState { replay: Some((3, 0.1)), ..ui.clone() }, &mut out);
        let (x0, y0) = at(3, 4); let (x1, y1) = at(4, 8); let on_line = (0..=16).filter(|&i| out.get(x0 + 8 + (x1 - x0) * i / 16, y0 + 8 + (y1 - y0) * i / 16) == Some(15)).count();
        assert!(on_line >= 6, "a dotted tracer along the line: {on_line}"); assert!(out.text.visible.iter().any(|r| r.bytes == b"4"), "the hit digit");
        let mut out = Surface::new(192, 112); v.draw(&s, &UiState { replay: Some((2, 0.1)), ..ui.clone() }, &mut out);
        assert!(out.text.visible.is_empty() && (0..=16).all(|i| out.get(x0 + 8 + (x1 - x0) * i / 16, y0 + 8 + (y1 - y0) * i / 16) != Some(15)), "another seq: nothing replayed");
        let mut assets = Assets::default(); assets.people = vec![Bitmap { w: 128 * 3, h: 16, pixels: vec![5; 128 * 3 * 16] }; 2]; let v = Viewport::new(Arc::new(assets));
        s.squad.fx = Fx { seq: 4, unit: Some(Unit::Npc(14)), walk: vec![(4, 6), (4, 7), (4, 8)], ..Fx::default() }; s.art.slots = vec![0; 64];
        let mut out = Surface::new(192, 112); v.draw(&s, &UiState { replay: Some((4, 0.0)), ..ui.clone() }, &mut out);
        let (x, y) = at(4, 6); assert_eq!(out.get(x + 4, y + 4), Some(5), "at its start square at t = 0");
        let mut out = Surface::new(192, 112); v.draw(&s, &UiState { replay: None, ..ui.clone() }, &mut out);
        let (x, y) = at(4, 8); assert_eq!(out.get(x + 4, y + 4), Some(5), "no replay: at its record square");
        s.squad.acting = Some(Unit::Npc(14)); let mut out = Surface::new(192, 112); v.draw(&s, &ui, &mut out); assert_eq!((out.get(0, 0), out.get(191, 111)), (Some(4), Some(4)));
        let mut out = Surface::new(1024, 1024); v.draw(&s, &UiState { zoom: 0.25, ..ui.clone() }, &mut out);
        let c = Camera::new(&s, &UiState { zoom: 0.25, ..ui.clone() }, (1024, 1024)); let (x, y) = c.screen((4 * 16 + 8, 8 * 16 + 8)); assert_eq!(out.get(x, y), Some(12), "the hostile's red marker");
        let (x, y) = c.screen(((3 - 29) * 16, (4 - 29) * 16)); assert_eq!(out.get(x, y), Some(6), "the ENCOUNTER_RADIUS ring's corner");
    }
    #[test]
    fn draw_is_pure_and_zoom_changes_world_coverage() {
        let s = scene(); let before = s.clone(); let v = Viewport::new(Arc::default());
        for z in [0.25, 0.5, 1.0, 2.0, f32::NAN, f32::INFINITY] {
            let ui = UiState { zoom: z, camera: Viewport::oracle_camera(&s), ..UiState::default() };
            let mut out = Surface::new(192, 112); v.draw(&s, &ui, &mut out);
            let c = Camera::new(&s, &ui, (192, 112)); assert!(c.top.is_finite() && c.left.is_finite());
        }
        assert_eq!(s, before);
    }
}
