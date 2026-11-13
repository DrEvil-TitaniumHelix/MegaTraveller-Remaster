use super::*;

pub(super) struct ComponentWindow {
    pub component: ComponentId,
    pub window: Rc<Window>,
    pub surface: Surface<Rc<Window>, Rc<Window>>,
    pub cursor: Option<(f64, f64)>,
    pub visible: bool,
}
pub(super) struct GripDrag { component: ComponentId, source: WindowId, start: (f64, f64), current: (f64, f64) }

fn same_click_target(last: ComponentId, old: (i32, i32), id: ComponentId, at: (i32, i32), screen: Option<mt1::ui::ScreenKind>) -> bool {
    let portrait_host = |id| id == "Party strip" || (id == "Picture" && screen == Some(mt1::ui::ScreenKind::Record));
    if portrait_host(last) && portrait_host(id) {
        let slot = panels::picture::portrait(old);
        return slot.is_some() && slot == panels::picture::portrait(at);
    }
    last == id && old.0.abs_diff(at.0) <= 4 && old.1.abs_diff(at.1) <= 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt1::ui::ScreenKind;
    #[test]
    fn double_click_stays_on_one_portrait_across_record_entry() {
        for screen in [ScreenKind::Ground, ScreenKind::Record] {
            let id = if screen == ScreenKind::Record { "Picture" } else { "Party strip" };
            assert!(same_click_target("Party strip", (46, 20), id, (85, 20), Some(screen)));
            assert!(!same_click_target("Party strip", (45, 20), id, (46, 20), Some(screen)));
            assert!(!same_click_target("Party strip", (206, 20), id, (206, 20), Some(screen)));
        }
        assert!(!same_click_target("Party strip", (46, 20), "Picture", (46, 20), Some(ScreenKind::Cg)));
        assert!(!same_click_target("Viewport", (i32::MIN, 0), "Viewport", (i32::MAX, 0), Some(ScreenKind::Ground)));
    }
}

impl App {
    pub(super) fn show_scene(&mut self, e: panels::replay::Entry) {
        let s = e.scene;
        let follow_up = self.ui.walk.borrow_mut().observe(&s);
        self.ui.frame = e.frame;
        panels::controls::observe(&s, self.scene.as_ref(), &mut self.ui);
        if self.scene.as_ref().is_some_and(|old| old.screen != s.screen) {
            self.ui.capture = None; self.capture_button = None; self.capture_rect = None; self.grip_drag = None;
            self.ui.dock_preview = None; self.ui.grip_hover = None; self.mouse_buttons = 0;
            self.ui.component_input.drag = None; self.ui.components.pointer = None;
        }
        self.ui.replay = e.fx.then_some((s.squad.fx.seq, 0.0));
        match (&s.squad.menu, self.ui.menu) {
            (Some(m), None) => self.ui.menu = Some((m.target, self.mouse_pos.map_or((0, 0), |(x, y)| (x as i32, y as i32)))),
            (None, Some(_)) => { self.ui.menu = None; self.ui.menu_host = None; }
            _ => {}
        }
        let combat = panels::combat_bar::observe(&s, &mut self.ui);
        self.scene = Some(s);
        if !follow_up.is_empty() { self.apply_intents(follow_up); }
        if !combat.is_empty() { self.apply_intents(combat); }
        self.redraw_all();
    }
    pub(super) fn replay_tick(&mut self) {
        if !self.replay.active() { return; }
        let (show, progress) = self.replay.tick(Instant::now());
        for e in show { self.show_scene(e); }
        if progress.map(|p| (p.0, (p.1 * 20.0) as i32)) != self.ui.replay.map(|p| (p.0, (p.1 * 20.0) as i32)) { self.ui.replay = progress; self.redraw_all(); }
        if let Some(f) = self.replay.take_held() { *self.ui.frame = f; self.redraw_all(); }
    }
    pub(super) fn skip_replay(&mut self) -> bool {
        if !self.replay.active() { return false; }
        if let Some(e) = self.replay.flush() { self.show_scene(e); }
        self.ui.replay = None; if let Some(f) = self.replay.take_held() { *self.ui.frame = f; } self.redraw_all(); true
    }
    pub(super) fn request_tone(&mut self, tone: megatraveller::tone::Tone) {
        if self.faithful || self.look.menu_sound == megatraveller::look::MenuSound::Off { return }
        if self.scene.as_ref().is_some_and(|s| s.context.silent) {
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) { super::sound_log("menu tone skipped: the game's own Sound is off (DS:6D0B, Game > Sound)"); }
            return
        }
        self.tones.push(tone);
    }
    pub(super) fn redraw_all(&mut self) {
        self.dirty = true;
        if let Some(w) = &self.window { w.request_redraw(); }
        for c in &self.components { c.window.request_redraw(); }
    }
    pub(super) fn window_by_id(&self, id: WindowId) -> Option<&Rc<Window>> {
        self.window.as_ref().filter(|w| w.id() == id).or_else(|| self.components.iter().find(|c| c.window.id() == id).map(|c| &c.window))
    }
    pub(super) fn metrics(&self, id: WindowId) -> Option<Metrics> {
        let w = self.window_by_id(id)?; let s = render_size(w.inner_size());
        if let Some(c) = self.components.iter().find(|c| c.window.id() == id) {
            Some(Metrics::component([s.width, s.height], self.layout.placement(c.component)?.size.preferred, &self.look))
        } else { Some(Metrics::for_ui([s.width, s.height], &self.look, self.scene.as_ref(), &self.ui)) }
    }
    fn cursor(&self, id: WindowId) -> Option<(f64, f64)> {
        if self.window.as_ref().is_some_and(|w| w.id() == id) { self.mouse_window_pos }
        else { self.components.iter().find(|c| c.window.id() == id)?.cursor }
    }
    fn desktop(&self, id: WindowId, at: (f64, f64)) -> Option<(f64, f64)> {
        let p = self.window_by_id(id)?.inner_position().ok()?;
        Some((p.x as f64 + at.0, p.y as f64 + at.1))
    }
    pub(super) fn prepare_layout(&mut self) {
        for c in &self.components {
            if let Some(m) = self.metrics(c.window.id()) { self.layout.set_host(c.component, Host::Window { id: c.window.id().into(), size: m.logical }); }
        }
        let (Some(w), Some(scene)) = (&self.window, &self.scene) else { return; };
        let s = render_size(w.inner_size());
        let metrics = Metrics::for_ui([s.width, s.height], &self.look, Some(scene), &self.ui);
        let mut ui = self.ui.clone(); if metrics.classic { ui.preset = mt1::ui::Preset::Classic; }
        let mut scene = scene.clone(); scene.squad.encounter |= ui.debug_initiative;
        self.layout.arrange_for(&scene, &ui, metrics.logical);
        panels::controls::prepare(&self.layout, &mut self.ui);
    }
    fn dispatch_component(&mut self, id: ComponentId, event: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let Some(mut scene) = self.plus_scene_for(id) else { return Vec::new(); };
        scene.squad.encounter |= self.ui.debug_initiative;
        panels::controls::dispatch(&self.layout, id, &scene, &mut self.ui, event, at)
    }
    pub(super) fn edge_pan(&mut self) {
        if self.grip_drag.is_some() { return; }
        if let (Some(id), Some(Button::Left), Some(src)) = (self.ui.capture, self.capture_button, self.pointer_window) {
            if id == panels::cargo::ID {
                self.repeat += 1;
                if self.repeat >= 8 && self.repeat % 2 == 0 { if let Some((hit, at)) = self.target(src, true) { if hit == id {
                    let intents = self.dispatch_component(id, &UiEvent::Down(Button::Left), at); self.apply_component_intents(id, intents);
                } } }
                return;
            }
        }
        self.repeat = 0;
        let Some(scene) = &self.scene else { return; };
        let intents = panels::controls::edge_tick(&self.layout, scene, &mut self.ui);
        if !intents.is_empty() { self.apply_intents(intents); }
    }
    pub(super) fn sync_hosts(&mut self, events: &ActiveEventLoop) -> Result<(), String> {
        if self.closed { return Ok(()); }
        let plus_open = self.ui.plus.open;
        self.components.retain(|c| if c.component == panels::plus::ID { plus_open } else { self.ui.host_kind(c.component) == HostKind::Window });
        for id in self.layout.ids() {
            let plus = id == panels::plus::ID;
            if (if plus { !plus_open } else { self.ui.host_kind(id) != HostKind::Window }) || self.components.iter().any(|c| c.component == id) { continue; }
            let Some(p) = self.layout.placement(id) else { continue; };
            if !p.undockable && !plus { continue; }
            let main = self.window.as_ref().map(|w| render_size(w.inner_size())).unwrap_or(PhysicalSize::new(960, 720));
            let s = if plus { 2.0 } else { pipeline::scale::picture_rect(main.width as usize, main.height as usize, &self.look).width as f64 / 320.0 };
            let a = if self.look.shape == Shape::Classic { 1.2 } else { 1.0 };
            let saved = self.look.layout.components.entry(id.into()).or_default();
            let size = saved.window_size.unwrap_or([(p.size.preferred.0 as f64 * s).round().max(1.0) as u32, (p.size.preferred.1 as f64 * s * a).round().max(1.0) as u32]);
            let [w, h] = clamp_window_size(size);
            saved.window_pos = panels::settings::visible_position(saved.window_pos, events.available_monitors().map(|m| { let p = m.position(); let s = m.size(); ([p.x, p.y], [s.width, s.height]) }));
            let mut attrs = Window::default_attributes().with_title(window_title(Some(p.title))).with_resizable(true).with_visible(false)
                .with_inner_size(PhysicalSize::new(w.max(1), h.max(1))).with_max_inner_size(PhysicalSize::new(MAX_WINDOW_SIDE, MAX_WINDOW_SIDE));
            if let Some([x, y]) = saved.window_pos { attrs = attrs.with_position(PhysicalPosition::new(x, y)); }
            let window = Rc::new(events.create_window(attrs).map_err(|e| format!("component window {id}: {e}"))?);
            pointer_intake::attach(&window)?;
            let context = Context::new(window.clone()).map_err(|e| format!("component context: {e}"))?;
            let surface = Surface::new(&context, window.clone()).map_err(|e| format!("component surface: {e}"))?;
            window.set_ime_allowed(true);
            let m = Metrics::component([w.max(1), h.max(1)], p.size.preferred, &self.look);
            self.layout.set_host(id, Host::Window { id: window.id().into(), size: m.logical });
            self.components.push(ComponentWindow { component: id, window, surface, cursor: None, visible: false });
            self.geometry_dirty = true; self.redraw_all();
        }
        self.prepare_layout();
        let scene = self.scene.as_ref().map(|s| { let mut s = s.clone(); s.squad.encounter |= self.ui.debug_initiative; s });
        for c in &mut self.components {
            let visible = if c.component == panels::plus::ID { plus_open } else { scene.as_ref().is_some_and(|s| self.layout.visible(c.component, s)) };
            if c.visible != visible {
                #[cfg(not(test))] c.window.set_visible(visible);
                c.visible = visible; if visible { c.window.request_redraw(); }
            }
        }
        Ok(())
    }
    pub(super) fn component_pixels(&mut self, index: usize) -> Result<Option<(PhysicalSize<u32>, Vec<u32>)>, String> {
        self.prepare_layout();
        let c = &self.components[index];
        let Some(mut scene) = self.plus_scene_for(c.component) else { return Ok(None); };
        let size = render_size(c.window.inner_size());
        if size.width == 0 || size.height == 0 { return Ok(None); }
        let Some(metrics) = self.metrics(c.window.id()) else { return Ok(None); };
        scene.squad.encounter |= self.ui.debug_initiative;
        let surfaces = self.layout.draw_host(&scene, &self.ui, Some(c.window.id().into())).into_iter().map(|(_, r, s)| (r, s));
        let output = if c.component == panels::plus::ID { panels::plus::compose(surfaces, metrics, &self.render_look(c.component)) } else { panels::render::compose(surfaces, metrics, &self.look) };
        Ok((!output.is_empty()).then_some((size, output)))
    }
    pub(super) fn render_component(&mut self, index: usize) -> Result<(), String> {
        let Some((size, output)) = self.component_pixels(index)? else { return Ok(()); };
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return Ok(()); };
        let c = &mut self.components[index];
        c.surface.resize(w, h).map_err(|e| format!("component resize: {e}"))?;
        let mut buffer = c.surface.buffer_mut().map_err(|e| format!("component buffer: {e}"))?;
        if buffer.len() != output.len() { c.window.request_redraw(); return Ok(()); }
        if let Some(dir) = std::env::var_os("MT_FRAME_DUMP") {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0); let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(png) = megatraveller::plus::png::rgb(size.width, size.height, &output) { let d = PathBuf::from(dir); let tag = c.component.replace(' ', "_");
                let _ = std::fs::write(d.join(format!("{tag}_{n:04}.png")), &png); let _ = std::fs::write(d.join(format!("{tag}_latest.png")), &png); }
        }
        buffer.copy_from_slice(&output); c.window.pre_present_notify(); buffer.present().map_err(|e| format!("component present: {e}"))
    }
    fn target(&self, source: WindowId, capture: bool) -> Option<(ComponentId, (i32, i32))> {
        let mut scene = self.plus_scene_for(if self.is_plus_window(source) { panels::plus::ID } else { "" })?; scene.squad.encounter |= self.ui.debug_initiative;
        let mut host = source; let mut at = self.cursor(source)?;
        if capture {
            if let Some(id) = self.ui.capture {
                host = self.layout.host_window(id).map(WindowId::from).or_else(|| self.window.as_ref().map(|w| w.id()))?;
                if host != source { let global = self.desktop(source, at)?; let p = self.window_by_id(host)?.inner_position().ok()?; at = (global.0 - p.x as f64, global.1 - p.y as f64); }
            }
        }
        let m = self.metrics(host)?; let pixel = m.unwarp(at, &self.look, capture)?;
        let hit = if capture { self.ui.capture } else { None }.or_else(|| {
            let window = self.components.iter().any(|c| c.window.id() == host).then_some(host.into());
            self.layout.hit_mapped(&scene, window, pixel, |r| { let p = m.rect(r); Rect::new(p.x as i32, p.y as i32, p.width as u32, p.height as u32) }).map(|(id, _)| id)
        })?;
        let r = match self.layout.host(hit)? { Host::Dock(r) => r, Host::Window { size, .. } => Rect::new(0, 0, size.0, size.1) };
        let r = if capture && self.ui.capture == Some(hit) { self.capture_rect.unwrap_or(r) } else { r };
        Some((hit, m.local(pixel, r, &self.look)))
    }
    pub(super) fn product_pointer(&self) -> Option<(i16, i16)> {
        let source = self.pointer_window?;
        let (id, at) = self.target(source, self.ui.capture.is_some())?;
        if id == "Viewport" && !self.layout.is_classic() {
            return panels::viewport::Viewport::oracle_pointer(self.scene.as_ref()?, &self.ui, self.layout.host(id)?.size(), at);
        }
        if id == "Combat log" && !self.layout.is_classic() { return None; }
        let Origin::Classic1990(r) = self.layout.placement(id)?.origin else { return None; };
        if at.0 < 0 || at.1 < 0 || at.0 as u32 >= r.w || at.1 as u32 >= r.h { return None; }
        Some(((r.x + at.0) as i16, (r.y + at.1) as i16))
    }
    pub(super) fn reposition(&mut self, col: i16, row: i16) {
        self.prepare_layout();
        let Some(scene) = self.scene.as_ref() else { return; };
        let Some((id, origin)) = self.layout.ids().into_iter().find_map(|id| {
            let Origin::Classic1990(r) = self.layout.placement(id)?.origin else { return None; };
            (r.contains((col as i32, row as i32)) && self.layout.visible(id, scene)).then_some((id, r))
        }) else { return; };
        let Some(host) = self.layout.host(id) else { return; };
        let (window, r) = match host {
            Host::Dock(r) => { let Some(w) = &self.window else { return; }; (w.id(), r) },
            Host::Window { id, size } => (WindowId::from(id), Rect::new(0, 0, size.0, size.1)),
        };
        let Some(metrics) = self.metrics(window) else { return; };
        let local = if id == "Viewport" && !self.layout.is_classic() {
            let Some(at) = panels::viewport::Viewport::local_pointer(scene, &self.ui, host.size(), col, row) else { return; }; at
        } else { (col as i32 - origin.x, row as i32 - origin.y) };
        let Some(at) = metrics.project(r, local, &self.look) else { return; };
        if let Some(c) = self.components.iter_mut().find(|c| c.window.id() == window) { c.cursor = Some(at); } else { self.mouse_window_pos = Some(at); }
        self.pointer_window = Some(window); self.mouse_pos = self.product_pointer();
        if let Some(w) = self.window_by_id(window) { let _ = w.set_cursor_position(PhysicalPosition::new(at.0, at.1)); }
    }
    fn apply_component_intents(&mut self, id: ComponentId, mut intents: Vec<Intent>) {
        if id != "Viewport" && intents.iter().any(|i| matches!(i, Intent::Select(_) | Intent::Verb { .. } | Intent::MenuPick(_) | Intent::EndTurn | Intent::Continue | Intent::Raw(_))) { self.ui.walk.borrow_mut().reset(); }
        for intent in &mut intents { if let Intent::OpenMenu { at, .. } = intent {
            match self.layout.host(id) {
                Some(Host::Dock(r)) => { self.ui.menu_host = None; at.0 = at.0.saturating_add(r.x); at.1 = at.1.saturating_add(r.y); }
                Some(Host::Window { .. }) => self.ui.menu_host = Some(id), _ => {},
            }
        } }
        if matches!(id, "Commands" | "Choices" | "Combat controls" | "Text input" | "Continue" | "Flight") {
            let events: Vec<_> = intents.iter().filter_map(|i| if let Intent::Raw(e) = i { Some(*e) } else { None }).collect();
            if let Some(t) = megatraveller::tone::for_route(self.ui.decision.as_ref(), &events) { self.request_tone(t); }
            self.decision.queue(&mut self.ui, &events);
            intents.retain(|i| !matches!(i, Intent::Raw(_)));
        }
        self.apply_intents(intents);
    }
    pub(super) fn apply_intents(&mut self, mut intents: Vec<Intent>) {
        if intents.iter().any(|i| matches!(i, Intent::Verb { verb: mt1::ui::Verb::Move, .. })) { self.ui.hover = None; }
        if let Some(events) = megatraveller::mouse::decision::released_route(&intents) {
            let buy = intents.contains(&Intent::BuyPrompt(None));
            if let Some(t) = megatraveller::tone::for_route(self.ui.decision.as_ref(), &events) { self.request_tone(if buy && t == megatraveller::tone::Tone::Choice { megatraveller::tone::Tone::Done } else { t }); }
            self.decision.queue(&mut self.ui, &events); intents.retain(|i| !matches!(i, Intent::Raw(_)));
        }
        if intents.iter().any(|i| matches!(i, Intent::MenuPick(_))) { self.request_tone(megatraveller::tone::Tone::Choice); }
        if intents.contains(&Intent::BuyPrompt(None)) && !intents.iter().any(|i| matches!(i, Intent::Raw(_))) { self.request_tone(megatraveller::tone::Tone::Back); }
        megatraveller::mouse::walk::retarget(&mut self.intents, &intents);
        for intent in intents {
            match intent {
                Intent::Plus(action) => self.plus_action(action),
                Intent::Raw(IoEvent::Mouse(mut e)) => { e.buttons = self.mouse_buttons; self.input.push(IoEvent::Mouse(e)); }
                Intent::Raw(e) => self.input.push(e),
                Intent::Undock(id) => self.change(Action::Host(id, HostKind::Window)),
                Intent::Dock(id) => self.change(Action::Host(id, HostKind::Docked)),
                Intent::Hide(id) => self.change(Action::Host(id, HostKind::Hidden)),
                Intent::ResetLayout => self.change(Action::ResetLayout),
                Intent::Layout(p) => self.change(Action::Layout(p.into())),
                Intent::LogResize(n) => self.change(Action::LogLines(n)),
                _ => {
                    let forward = matches!(intent, Intent::Hover(t) if t != self.ui.hover) || matches!(intent, Intent::OpenMenu { .. } | Intent::CloseMenu);
                    if (!self.ui.apply(intent) || forward) && self.intents.len() < MAX_INPUT_EVENTS { self.intents.push_back(intent); }
                }
            }
        }
        self.redraw_all();
    }
    pub(super) fn route_keys(&mut self, keys: VecDeque<Key>) {
        if self.faithful || self.scene.is_none() { self.input.keys(keys, false); return; }
        self.prepare_layout();
        for key in keys {
            if megatraveller::mouse::decision::modal(&self.ui) {
                if panels::choices::ENABLED && self.ui.decision.as_ref().is_some_and(|d| matches!(d.kind, mt_engine::play::Kind::Menu | mt_engine::play::Kind::YesNo | mt_engine::play::Kind::List | mt_engine::play::Kind::Ack) && panels::choices::handles_key(d, key)) {
                    let intents = self.dispatch_component("Choices", &UiEvent::Key(key), (0, 0));
                    self.apply_component_intents("Choices", intents);
                } else { self.input.push(IoEvent::Key(key)); }
                continue;
            }
            if matches!(key, 13 | 27 | 32) && self.skip_replay() && self.ui.menu.is_none() && key != 32
                && !self.scene.as_ref().is_some_and(|s| megatraveller::mouse::walk::cancel_after_replay(UiEvent::Key(key), s, &self.ui, &self.intents)) { continue; }
            let Some(scene) = self.scene.as_ref() else { continue; };
            if let Some(intents) = panels::keyboard::route(scene, &mut self.ui, key) {
                self.apply_intents(intents); self.redraw_all(); continue;
            }
            let focused_float = self.ui.menu.and_then(|_| self.layout.ids().into_iter().rev().find(|id| self.layout.placement(id).is_some_and(|p| matches!(p.dock, Dock::Float(_))) && self.layout.visible(id, scene)));
            if let Some(id) = focused_float {
                let intents = self.dispatch_component(id, &UiEvent::Key(key), (0, 0));
                if !intents.is_empty() { self.apply_component_intents(id, intents); continue; }
            }
            let Some(scene) = self.scene.as_ref() else { continue; };
            if key == 27 && self.ui.menu.is_none() {
                let queued = megatraveller::mouse::walk::drop_moves(&mut self.intents);
                let cancel = self.ui.walk.borrow_mut().cancel(scene, (0, 0));
                if let Some(intents) = cancel { self.apply_intents(intents); continue; }
                if queued { continue; }
            }
            if key != 27 { self.ui.walk.borrow_mut().reset(); }
            if matches!(key, 13 | 27 | 32 | 0x13b..=0x144) { self.input.push(IoEvent::Key(key)); continue; }
            if key == 76 && scene.screen == mt1::ui::ScreenKind::Ground && !self.layout.is_classic() { self.apply_intents(vec![Intent::LogExpand]); continue; }
            if matches!(key, 91 | 93) && scene.screen == mt1::ui::ScreenKind::Ground && !self.layout.is_classic() { self.apply_intents(vec![Intent::LogScroll(if key == 91 { 1 } else { -1 })]); continue; }
            if key == mt_engine::io::keys::HOME && scene.screen == mt1::ui::ScreenKind::Ground && !self.layout.is_classic() {
                let intents = self.dispatch_component("Viewport", &UiEvent::Key(key), (0, 0));
                self.apply_component_intents("Viewport", intents); continue;
            }
            let hit = self.pointer_window.and_then(|id| self.target(id, false)).or_else(|| self.layout.ids().into_iter().find(|id| self.layout.placement(id).is_some_and(|p| p.dock == Dock::Fill) && self.layout.visible(id, scene)).map(|id| (id, (0, 0))));
            let intents = hit.map_or_else(Vec::new, |(id, at)| self.dispatch_component(id, &UiEvent::Key(key), at));
            if intents.is_empty() { self.input.push(IoEvent::Key(key)); } else if let Some((id, _)) = hit { self.apply_component_intents(id, intents); }
        }
    }
    pub(super) fn pointer_event(&mut self, source: WindowId, event: UiEvent) {
        self.prepare_layout();
        if matches!(event, UiEvent::Down(Button::Right) | UiEvent::Up(Button::Right))
            && self.scene.as_ref().is_some_and(|scene| self.target(source, self.ui.capture.is_some())
                .is_none_or(|(id, at)| !megatraveller::mouse::decision::pointer_allowed(scene, id, &event, at))) { return; }
        if self.grip_drag.is_some() {
            if let Some(at) = self.cursor(source).and_then(|at| self.desktop(source, at)) {
                if let Some(drag) = &mut self.grip_drag { drag.current = at; }
                let on_main = self.window.as_ref().and_then(|w| w.inner_position().ok().map(|p| (w.inner_size(), p))).is_some_and(|(s, p)| at.0 >= p.x as f64 && at.1 >= p.y as f64 && at.0 < p.x as f64 + s.width as f64 && at.1 < p.y as f64 + s.height as f64);
                self.ui.dock_preview = self.grip_drag.as_ref().filter(|_| on_main).map(|d| d.component);
                self.redraw_all();
            }
            return;
        }
        let skip = event == UiEvent::Up(Button::Left) && self.skip_replay();
        let old = self.ui.grip_hover;
        let target = self.target(source, self.ui.capture.is_some());
        if std::env::var_os("MT_POINTER_TRACE").is_some() { eprintln!("pointer_event: {event:?} target={target:?} modal={} skip={skip} capture={:?}", megatraveller::mouse::decision::modal(&self.ui), self.ui.capture); }
        if let Some((id, at)) = target {
            if megatraveller::mouse::decision::modal(&self.ui) && !matches!(id, "Choices" | "Text input" | "Continue" | panels::plus::ID | panels::cargo::ID | panels::buy::ID) {
                let ack = self.ui.decision.as_ref().is_some_and(|d| d.kind == mt_engine::play::Kind::Ack);
                let (id, at) = if ack && !matches!(id, "Picture" | "Side panel" | "Party strip" | "Combat log") { ("Picture", (160, 100)) } else { (id, at) };
                if matches!(id, "Picture" | "Side panel" | "Party strip" | "Combat log") && event == UiEvent::Up(Button::Left) && !skip {
                    let intents = self.dispatch_component(id, &event, at);
                    if !intents.is_empty() { self.apply_component_intents("Choices", intents); }
                }
                self.ui.capture = None; self.capture_button = None; self.capture_rect = None; self.ui.component_input.drag = None; return;
            }
            self.ui.grip_hover = self.layout.placement(id).filter(|p| p.undockable).map(|_| id);
            if let UiEvent::Down(button) = event {
                if self.ui.capture.is_none() {
                    self.ui.capture = Some(id); self.capture_button = Some(button);
                    self.capture_rect = self.layout.host(id).map(|h| match h { Host::Dock(r) => r, Host::Window { size, .. } => Rect::new(0, 0, size.0, size.1) });
                }
            }
            let dragging = self.ui.component_input.drag.is_some_and(|d| d.button == Button::Middle);
            let mut intents = if skip { Vec::new() } else { self.dispatch_component(id, &event, at) };
            if event == UiEvent::Move && !dragging { intents.retain(|i| !matches!(i, Intent::Pan(..) | Intent::Follow(None))); }
            if !intents.is_empty() { self.apply_component_intents(id, intents); }
            else if matches!(id, "Choices" | "Commands" | "Text input" | "Flight") && matches!(event, UiEvent::Up(_) | UiEvent::Wheel(_)) { self.redraw_all(); }
        } else {
            self.ui.grip_hover = None; self.ui.components.pointer = None;
            if matches!(event, UiEvent::Up(_)) { if let Some(id) = self.ui.capture {
                let intents = if skip { Vec::new() } else { self.dispatch_component(id, &event, (-1, -1)) }; self.apply_component_intents(id, intents);
                self.ui.components.pointer = None;
            } }
        }
        if matches!(event, UiEvent::Up(b) if Some(b) == self.capture_button) { self.ui.capture = None; self.capture_button = None; self.capture_rect = None; }
        if old != self.ui.grip_hover { self.redraw_all(); }
    }
    pub(super) fn product_button(&mut self, source: WindowId, state: ElementState, button: MouseButton) {
        let b = match button { MouseButton::Left => Button::Left, MouseButton::Right => Button::Right, MouseButton::Middle => Button::Middle, _ => return };
        let mask = match b { Button::Left => mouse::LEFT_DOWN, Button::Right => mouse::RIGHT_DOWN, Button::Middle => 4 };
        if state == ElementState::Pressed { self.mouse_buttons |= mask; } else { self.mouse_buttons &= !mask; }
        self.prepare_layout();
        if b == Button::Left && state == ElementState::Released && self.grip_drag.is_some() { self.drop_grip(); return; }
        if b == Button::Left && state == ElementState::Pressed {
            if let Some((id, at)) = self.target(source, false) {
                if let (Some(p), Some(host)) = (self.layout.placement(id), self.layout.host(id)) {
                    if p.undockable && panels::grip_rect(p.dock, host).contains(at) {
                        if let Some(start) = self.cursor(source).and_then(|at| self.desktop(source, at)) {
                            self.grip_drag = Some(GripDrag { component: id, source, start, current: start }); return;
                        }
                    }
                }
            }
        }
        let event = if state == ElementState::Pressed { UiEvent::Down(b) } else { UiEvent::Up(b) };
        self.pointer_event(source, event);
        if state == ElementState::Pressed {
            if let Some((id, at)) = self.target(source, false) {
                if self.last_click.is_some_and(|(last, button, time, old)| button == b && time.elapsed() < Duration::from_millis(400)
                    && same_click_target(last, old, id, at, self.scene.as_ref().map(|s| s.screen))) {
                    let intents = self.dispatch_component(id, &UiEvent::DoubleClick(b), at); self.apply_component_intents(id, intents);
                    self.last_click = None;
                } else { self.last_click = Some((id, b, Instant::now(), at)); }
            }
        }
        self.mouse_pos = self.pointer();
    }
    fn drop_grip(&mut self) {
        let Some(drag) = self.grip_drag.take() else { return; };
        self.ui.dock_preview = None; self.redraw_all();
        if (drag.current.0 - drag.start.0).abs() + (drag.current.1 - drag.start.1).abs() < 6.0 { return; }
        let on_main = self.window.as_ref().and_then(|w| w.inner_position().ok().map(|p| (w.inner_size(), p))).is_some_and(|(s, p)| {
            drag.current.0 >= p.x as f64 && drag.current.1 >= p.y as f64 && drag.current.0 < p.x as f64 + s.width as f64 && drag.current.1 < p.y as f64 + s.height as f64
        });
        if on_main { self.change(Action::Host(drag.component, HostKind::Docked)); }
        else {
            let pos = [drag.current.0 as i32, drag.current.1 as i32];
            self.look.layout.components.entry(drag.component.into()).or_default().window_pos = Some(pos);
            if let Some(c) = self.components.iter().find(|c| c.window.id() == drag.source) { c.window.set_outer_position(PhysicalPosition::new(pos[0], pos[1])); }
            self.change(Action::Host(drag.component, HostKind::Window));
        }
        self.ui.capture = None; self.capture_button = None; self.capture_rect = None;
    }
    pub(super) fn wheel(&mut self, source: WindowId, delta: MouseScrollDelta) {
        let n = match delta { MouseScrollDelta::LineDelta(_, y) => y as f64, MouseScrollDelta::PixelDelta(p) => p.y / 24.0 };
        if n.is_finite() && n != 0.0 { self.pointer_event(source, UiEvent::Wheel(if n.abs() < 1.0 { n.signum() as i32 } else { n as i32 })); }
    }
    pub(super) fn component_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(i) = self.components.iter().position(|c| c.window.id() == id) else { return; };
        let component = self.components[i].component;
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed if component == panels::plus::ID => { self.plus_action(mt1::ui::scene::PlusAction::Close); self.components.remove(i); }
            WindowEvent::CloseRequested | WindowEvent::Destroyed => { self.change(Action::Host(component, HostKind::Docked)); self.components.remove(i); }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    let bounded = render_size(size);
                    if size != bounded { let _ = self.components[i].window.request_inner_size(bounded); }
                    self.look.layout.components.entry(component.into()).or_default().window_size = Some([bounded.width, bounded.height]); self.geometry_dirty = true;
                }
                self.prepare_layout(); self.redraw_all();
            }
            WindowEvent::Moved(p) => { self.look.layout.components.entry(component.into()).or_default().window_pos = Some([p.x, p.y]); self.geometry_dirty = true; }
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Focused(false) => self.modifiers = ModifiersState::empty(),
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                let code = native_key(event.physical_key, &event.logical_key, false);
                if self.plus_key(code, event.text.as_deref(), event.state, event.repeat, component == panels::plus::ID) { return; }
                if self.text_key(code, event.text.as_deref(), event.state) { return; }
                if self.scenario_shortcut(code, event.state, event.repeat) { return; }
                let mut keys = VecDeque::new();
                let text = event.text_with_all_modifiers().or(event.text.as_deref());
                if event.state == ElementState::Pressed && self.modifiers.shift_key() && code == Some(KeyCode::Tab) { keys.push_back(mt_engine::io::keys::ext(mt_engine::io::scan::TAB)); self.route_keys(keys); return }
                if event.state == ElementState::Pressed {
                    let unmodified = event.key_without_modifiers();
                    match key_press(&mut keys, code, key_text(text, &unmodified, self.modifiers, false), &self.look, self.modifiers.control_key(), false, event.repeat) {
                        Ok(Some(a)) => self.change(a),
                        Ok(None) if event.physical_key == PhysicalKey::Code(KeyCode::Numpad7) || (self.modifiers.control_key() && code == Some(KeyCode::Home)) => self.input.keys(keys, false),
                        Ok(None) => self.route_keys(keys), Err(e) => self.error = Some(e),
                    }
                } else { match enqueue(&mut keys, code, text) { Ok(()) => self.input.keys(keys, true), Err(e) => self.error = Some(e) } }
            }
            WindowEvent::Ime(Ime::Commit(text)) => { if (component == panels::plus::ID && self.plus_text(&text)) || self.text_commit(&text) { return; } let mut keys = VecDeque::new(); match enqueue(&mut keys, None, Some(&text)) { Ok(()) => self.route_keys(keys), Err(e) => self.error = Some(e) } }
            WindowEvent::CursorMoved { position, .. } => { self.components[i].cursor = Some((position.x, position.y)); self.pointer_window = Some(id); self.pointer_event(id, UiEvent::Move); }
            WindowEvent::CursorLeft { .. } => { self.ui.components.pointer = None; if self.ui.capture.is_none() && self.grip_drag.is_none() { self.components[i].cursor = None; self.ui.grip_hover = None; self.redraw_all(); } }
            WindowEvent::MouseInput { state, button, .. } => self.product_button(id, state, button),
            WindowEvent::MouseWheel { delta, .. } => self.wheel(id, delta),
            WindowEvent::RedrawRequested => if let Err(e) = self.render_component(i) { self.error = Some(e); events.exit(); },
            _ => {},
        }
    }
}
