use super::*;

pub const NAMES_MINIMAP_H: u32 = 32;
pub const CLASSIC: [Rect; 4] = [Rect::new(0, 0, 320, 54), Rect::new(192, 54, 128, 146), Rect::new(0, 166, 192, 34), Rect::new(0, 54, 192, 112)];

impl Rect {
    pub fn contains(self, at: (i32, i32)) -> bool {
        i64::from(at.0) >= i64::from(self.x) && i64::from(at.1) >= i64::from(self.y)
            && i64::from(at.0) < i64::from(self.x) + i64::from(self.w) && i64::from(at.1) < i64::from(self.y) + i64::from(self.h)
    }
}
impl UiState {
    pub(super) fn log_oldest(&self) -> i32 {
        let (width, height) = self.components.log_size.unwrap_or((192, u32::MAX));
        self.components.log.rows(width.saturating_sub(4) as usize).len()
            .saturating_sub(super::combat_log::visible_lines(self, height as usize).saturating_sub(self.components.log_header_extra)) as i32
    }
    pub fn host_kind(&self, id: ComponentId) -> HostKind { self.hosts.iter().find(|(i, _)| *i == id).map_or(HostKind::Docked, |(_, h)| *h) }
    pub fn set_host_kind(&mut self, id: ComponentId, kind: HostKind) {
        if let Some((_, h)) = self.hosts.iter_mut().find(|(i, _)| *i == id) { *h = kind; } else { self.hosts.push((id, kind)); }
        if self.capture == Some(id) { self.capture = None; }
    }
    pub fn apply(&mut self, intent: Intent) -> bool {
        match intent {
            Intent::PanTo(_) | Intent::Pan(..) | Intent::Zoom(_) | Intent::Follow(_) if self.fixed_screen => {},
            Intent::Hover(t) => { self.hover = t; self.hint = None } Intent::Hint(h) => self.hint = h, Intent::Focus(u) => self.focus = Some(u),
            Intent::BuyPrompt(b) => self.buy = b.map(|i| (self.decision.as_ref().map(|d| d.id.clone()).unwrap_or_default(), i)),
            Intent::PanTo((r, c)) => { self.camera = (r as f32, c as f32); self.follow = None; }
            Intent::Pan(r, c) => { self.camera.0 = (self.camera.0 + r as f32).clamp(-32768.0, 32767.0); self.camera.1 = (self.camera.1 + c as f32).clamp(-32768.0, 32767.0); self.follow = None; }
            Intent::Zoom(z) => if z.is_finite() { self.zoom = z.clamp(0.25, 2.0); },
            Intent::Follow(u) => self.follow = u,
            Intent::OpenMenu { target, at } => { self.menu = Some((target, at)); self.keyboard.menu_row = None } Intent::CloseMenu => { self.menu = None; self.menu_host = None; },
            Intent::LogScroll(n) => self.log_scroll = self.log_scroll.saturating_add(n).clamp(0, self.log_oldest()),
            Intent::LogResize(n) => { self.log_lines = n.clamp(2, 20); self.log_expanded = false; }
            Intent::LogExpand => self.log_expanded = !self.log_expanded,
            Intent::Layout(p) => self.preset = p,
            _ => return false,
        }
        true
    }
}

impl Layout {
    pub fn pictures() -> Self {
        let mut out = Self::default();
        out.register(Box::new(Picture::full()), Placement { screens: &[ScreenKind::Picture, ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Space], undockable: false, ..Picture::placement("Picture", FRAME) });
        for (i, (id, dock)) in [("Party strip", Dock::Top), ("Side panel", Dock::Right), ("Combat log", Dock::Bottom), ("Viewport", Dock::Fill)].into_iter().enumerate() {
            out.register(Box::new(Picture::new(CLASSIC[i])), Placement { dock, order: (i * 10) as u8, screens: &[ScreenKind::Ground], undockable: dock != Dock::Fill, ..Picture::placement(id, CLASSIC[i]) });
        }
        out
    }
    pub fn reset_sizes(&mut self) { for e in &mut self.entries { e.placement.size = e.initial_size; } }
    pub fn set_size(&mut self, id: ComponentId, size: [u32; 2]) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.placement.title == id) {
            let [w, h] = crate::look::clamp_window_size(size);
            e.placement.size.preferred = (w.max(e.placement.size.min.0), h.max(e.placement.size.min.1));
        }
    }
    pub fn set_kind(&mut self, ui: &mut UiState, id: ComponentId, kind: HostKind) -> bool {
        let Some(p) = self.placement(id) else { return false; };
        if kind == HostKind::Window && !p.undockable || kind == HostKind::Hidden && !p.optional { return false; }
        ui.set_host_kind(id, kind);
        if kind != HostKind::Window { self.set_host(id, Host::Dock(FRAME)); }
        true
    }
    pub fn arrange_for(&mut self, scene: &Scene, ui: &UiState, main: (u32, u32)) {
        self.classic = ui.preset == Preset::Classic;
        self.log_header_extra = 0;
        let main = (main.0.min(crate::look::MAX_WINDOW_SIDE), main.1.min(crate::look::MAX_WINDOW_SIDE));
        for e in &mut self.entries {
            let reserve_controls = ui.fixed_screen && ui.preset == Preset::Modern && scene.screen == ScreenKind::Ground
                && e.placement.title == super::combat_bar::ID;
            e.suppressed = ((!Self::shown(e, scene) || !e.component.visible(ui)) && !reserve_controls) || ui.host_kind(e.placement.title) == HostKind::Hidden
                || (e.placement.title == "Continue" && ((ui.preset == Preset::Modern && scene.screen == ScreenKind::Ground)
                    || (super::choices::beside_picture(scene, ui) && main.0 >= super::choices::logical(scene, ui).0 && main.1 >= super::choices::logical(scene, ui).1)))
                || (ui.preset == Preset::Classic && matches!(e.host, Host::Dock(_)) && e.placement.origin == Origin::New && !matches!(e.placement.title, "Commands" | "Flight") && !matches!(e.placement.dock, Dock::Float(_)));
        }
        if ui.preset == Preset::Classic {
            for e in &mut self.entries {
                if let Host::Dock(_) = e.host { if let Origin::Classic1990(r) = e.placement.origin { e.host = Host::Dock(r); } }
            }
            for e in self.entries.iter_mut().filter(|e| matches!(e.placement.title, "Commands" | "Flight")) {
                e.host = Host::Dock(Rect::new(0, main.1.saturating_sub(super::toolbar::HEIGHT) as i32, main.0, super::toolbar::HEIGHT.min(main.1)));
            }
            self.place_floats(scene, ui, main); return;
        }
        if super::choices::beside_picture(scene, ui) && main.0 >= super::choices::logical(scene, ui).0 && main.1 >= super::choices::logical(scene, ui).1 {
            self.place_floats(scene, ui, main);
            for e in &mut self.entries {
                match e.placement.title {
                    "Picture" => e.host = Host::Dock(FRAME),
                    "Choices" => e.host = Host::Dock(super::choices::dock(scene, ui, main)),
                    _ => {}
                }
            }
            return;
        }
        loop {
            let mut free = Rect::new(0, 0, main.0, main.1);
            if let Some(e) = self.entries.iter_mut().find(|e| !e.suppressed && e.placement.title == "Commands" && matches!(e.host, Host::Dock(_))) {
                let h = e.placement.size.preferred.1.min(free.h); free.h -= h;
                e.host = Host::Dock(Rect::new(0, free.h as i32, free.w, h));
            }
            let slot = |e: &Entry| e.placement.title == super::components::MINIMAP_ID && scene.screen == ScreenKind::Ground && ui.host_kind(e.placement.title) == HostKind::Hidden && matches!(e.host, Host::Dock(_));
            let below = super::components::MINIMAP_BELOW.load(std::sync::atomic::Ordering::Relaxed);
            let mut order: Vec<_> = self.entries.iter().enumerate().filter(|(_, e)| (!e.suppressed || slot(e)) && e.placement.title != "Commands" && matches!(e.host, Host::Dock(_)) && !matches!(e.placement.dock, Dock::Fill | Dock::Float(_))).map(|(i, _)| i).collect();
            order.sort_by_key(|&i| { let p = self.entries[i].placement; (match p.dock { Dock::Top => 0, Dock::Right => 1, Dock::Left => 2, Dock::Bottom => 3, _ => 4 }, p.origin != Origin::New && p.dock == Dock::Right && !(below && p.title == "Side panel"), if below && p.title == super::components::MINIMAP_ID { 99 } else { p.order }) });
            let right_width = order.iter().filter_map(|&i| { let p = self.entries[i].placement; (p.dock == Dock::Right).then_some(p.size.preferred.0.max(p.size.min.0)) }).max().unwrap_or(0);
            let mut right: Option<Rect> = None;
            let tops: u32 = order.iter().map(|&i| self.entries[i].placement).filter(|p| p.dock == Dock::Top).map(|p| p.size.preferred.1.max(p.size.min.1)).sum();
            let names_room = free.h.saturating_sub(tops) >= super::components::SIDE_RECT.h + NAMES_MINIMAP_H;
            for i in order {
                let p = self.entries[i].placement;
                let (w, mut h) = (p.size.preferred.0.max(p.size.min.0).min(free.w), p.size.preferred.1.max(p.size.min.1).min(free.h));
                if p.dock == Dock::Bottom && matches!(p.origin, Origin::Classic1990(r) if r == CLASSIC[2]) {
                    h = if ui.log_expanded { main.1 / 2 } else { (u32::from(ui.log_lines.clamp(2, 20)) + 1) * 9 }.min(free.h);
                    if ui.fixed_screen {
                        let map_h = free.h.saturating_sub(27).max(7).min(112).min(free.h);
                        h = h.min(free.h - map_h);
                    }
                }
                let r = match p.dock {
                    Dock::Top => { if p.title == "Party strip" && !names_room { h = h.min(p.size.min.1) } let r = Rect::new(free.x, free.y, free.w, h); free.y += h as i32; free.h -= h; r }
                    Dock::Bottom => { free.h -= h; Rect::new(free.x, free.y + free.h as i32, free.w, h) }
                    Dock::Left => { let r = Rect::new(free.x, free.y, w, free.h); free.x += w as i32; free.w -= w; r }
                    Dock::Right => {
                        let column = right.get_or_insert_with(|| { let w = right_width.min(free.w); free.w -= w; Rect::new(free.x + free.w as i32, free.y, w, free.h) });
                        let side_h = super::components::SIDE_RECT.h;
                        let h = if p.title == super::components::MINIMAP_ID {
                            let leftover = column.h.saturating_sub(side_h);
                            if leftover < 8 { self.entries[i].suppressed = true; continue }
                            leftover.min(p.size.preferred.1)
                        } else if p.origin == Origin::New { p.size.preferred.1.max(p.size.min.1).min(column.h) } else if below && p.title == "Side panel" { p.size.preferred.1.min(column.h) } else { column.h };
                        let r = Rect::new(column.x, column.y, column.w, h); column.y += h as i32; column.h -= h;
                        if self.entries[i].suppressed { continue }
                        r
                    }
                    _ => continue,
                };
                self.entries[i].host = Host::Dock(r);
            }
            let clipped_required_edge = self.entries.iter().any(|e| {
                if e.suppressed || e.placement.optional || matches!(e.placement.dock, Dock::Fill | Dock::Float(_)) { return false; }
                let Host::Dock(r) = e.host else { return false; };
                let log = e.placement.dock == Dock::Bottom && e.placement.origin == Origin::Classic1990(CLASSIC[2]);
                r.w < e.placement.size.min.0 || (!log && r.h < e.placement.size.min.1)
            });
            if free.w < 192 || free.h < 112 || clipped_required_edge {
                if let Some(i) = self.entries.iter().enumerate().filter(|(_, e)| !e.suppressed && e.placement.optional && e.placement.title != super::components::MINIMAP_ID && matches!(e.host, Host::Dock(_)) && !matches!(e.placement.dock, Dock::Float(_))).max_by_key(|(_, e)| e.placement.order).map(|(i, _)| i) {
                    self.entries[i].suppressed = true; continue;
                }
            }
            for e in &mut self.entries { if !e.suppressed && matches!(e.host, Host::Dock(_)) && e.placement.dock == Dock::Fill { e.host = Host::Dock(free); } }
            break;
        }
        self.place_floats(scene, ui, main);
        let width = self.host("Combat log").map_or(192, |h| h.size().0) as usize;
        self.log_header_extra = super::combat_log::header(scene, ui, width).len().saturating_sub(1);
    }
    fn place_floats(&mut self, scene: &Scene, ui: &UiState, main: (u32, u32)) {
        let parent = ui.menu_host.and_then(|id| self.host(id)).and_then(|h| match h { Host::Window { id, size } => Some((id, size)), _ => None });
        let tooltip = super::tooltip::geometry(self, scene, ui);
        let keyboard_menu = self.host("Viewport").and_then(|h| match h { Host::Dock(r) => Some((r.x + 4, r.y + 4)), _ => None }).unwrap_or((0, 0));
        for e in &mut self.entries {
            if !matches!(e.host, Host::Dock(_)) { continue; }
            if let Dock::Float(anchor) = e.placement.dock {
                if e.placement.title == super::tooltip::ID {
                    e.suppressed |= tooltip.is_none();
                    if let Some((rect, parent)) = tooltip {
                        e.overlay_window = parent;
                        e.host = Host::Dock(rect);
                    }
                    continue;
                }
                let parent = if anchor == Anchor::Pointer { parent } else { None };
                e.overlay_window = parent.map(|(id, _)| id);
                let main = parent.map_or(main, |(_, size)| size);
                let (w, mut h) = if e.placement.title == "Menu" { super::context_menu::size(scene, main) }
                    else { (e.placement.size.preferred.0.min(main.0), e.placement.size.preferred.1.min(main.1)) };
                let choice = ui.decision.as_ref().filter(|_| e.placement.title == "Choices");
                if let Some(d) = choice { h = h.min(super::choices::height(d)); }
                let at = if choice.is_some_and(|d| d.kind == mt_engine::play::Kind::Ack) { (((main.0 - w) / 2) as i32, main.1.saturating_sub(h + 10) as i32) }
                    else { match anchor { Anchor::Centre => (((main.0 - w) / 2) as i32, ((main.1 - h) / 2) as i32), Anchor::Pointer => ui.menu.map_or((0, 0), |(_, at)| if at == (0, 0) && parent.is_none() { keyboard_menu } else { at }) } };
                e.host = Host::Dock(Rect::new(at.0.clamp(0, (main.0 - w) as i32), at.1.clamp(0, (main.1 - h) as i32), w, h));
            }
        }
    }
    pub fn visible(&self, id: ComponentId, scene: &Scene) -> bool { self.entries.iter().any(|e| e.placement.title == id && !e.suppressed && Self::shown(e, scene)) }
    pub fn hit(&self, scene: &Scene, window: Option<u64>, at: (i32, i32)) -> Option<(ComponentId, (i32, i32))> {
        self.hit_mapped(scene, window, at, |r| r)
    }
    pub fn hit_mapped(&self, scene: &Scene, window: Option<u64>, at: (i32, i32), map: impl Fn(Rect) -> Rect) -> Option<(ComponentId, (i32, i32))> {
        self.entries.iter().enumerate().filter(|(_, e)| !e.suppressed && Self::shown(e, scene) && e.placement.title != super::tooltip::ID).filter_map(|(i, e)| {
            let r = if let Some(parent) = e.overlay_window {
                if window != Some(parent) { return None; }
                let Host::Dock(r) = e.host else { return None; }; r
            } else { match (window, e.host) { (None, Host::Dock(r)) => r, (Some(id), Host::Window { id: other, size }) if id == other => Rect::new(0, 0, size.0, size.1), _ => return None } };
            let r = map(r);
            r.contains(at).then_some(((matches!(e.placement.dock, Dock::Float(_)), e.placement.order, i), (e.placement.title, (at.0 - r.x, at.1 - r.y))))
        }).max_by_key(|(z, _)| *z).map(|(_, hit)| hit)
    }
    pub fn route(&self, scene: &Scene, ui: &mut UiState, window: Option<u64>, event: UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let hit = ui.capture.and_then(|id| self.host(id).map(|host| (id, match host { Host::Dock(r) => (at.0.saturating_sub(r.x), at.1.saturating_sub(r.y)), Host::Window { .. } => at }))).or_else(|| self.hit(scene, window, at));
        let Some((id, local)) = hit else { return Vec::new(); };
        if matches!(event, UiEvent::Down(_)) { ui.capture = Some(id); }
        let out = self.input(id, scene, ui, &event, local);
        if matches!(event, UiEvent::Up(_)) { ui.capture = None; }
        out
    }
}
