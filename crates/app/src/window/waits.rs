use super::*;
use megatraveller::panels::textbox;

impl App {
    pub(super) fn text_key(&mut self, code: Option<KeyCode>, text: Option<&str>, state: ElementState) -> bool {
        if self.faithful || !textbox::active(&self.ui) || self.modifiers.control_key() || code == Some(KeyCode::F11) { return false }
        if state == ElementState::Released { return true }
        let mut keys = VecDeque::new();
        match enqueue(&mut keys, code, text) {
            Ok(()) => { for key in keys { self.text_event(UiEvent::Key(key)); } },
            Err(e) => self.error = Some(e),
        }
        true
    }
    pub(super) fn text_commit(&mut self, text: &str) -> bool { self.text_key(None, Some(text), ElementState::Pressed) }
    fn text_event(&mut self, event: UiEvent) {
        self.prepare_layout();
        let fallback = Scene::program(mt1::ui::ScreenKind::Picture);
        let scene = self.scene.as_ref().unwrap_or(&fallback);
        let intents = panels::controls::dispatch(&self.layout, textbox::ID, scene, &mut self.ui, &event, (0, 0));
        self.wait_intents(intents);
        self.redraw_all();
    }
    pub(super) fn wait_button(&mut self, state: ElementState, button: MouseButton) {
        let Some(button) = (match button { MouseButton::Left => Some(Button::Left), MouseButton::Right => Some(Button::Right), MouseButton::Middle => Some(Button::Middle), _ => None }) else { return };
        let at = self.mouse_window_pos.and_then(|(x, y)| picture_to_game(x, y, self.picture(), &self.look)).map(|(x, y)| (i32::from(x), i32::from(y)));
        let event = if state == ElementState::Pressed { UiEvent::Down(button) } else { UiEvent::Up(button) };
        let intents = textbox::before_scene(&mut self.layout, &mut self.ui, event, at);
        self.wait_intents(intents);
        self.redraw_all();
    }
    fn wait_intents(&mut self, mut intents: Vec<Intent>) {
        let events: Vec<_> = intents.iter().filter_map(|i| if let Intent::Raw(e) = i { Some(*e) } else { None }).collect();
        self.decision.queue(&mut self.ui, &events);
        intents.retain(|i| !matches!(i, Intent::Raw(_)));
        self.apply_intents(intents);
    }
}
