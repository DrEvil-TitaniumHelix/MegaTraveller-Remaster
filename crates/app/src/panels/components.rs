use super::{always, assets::Assets, combat_log::CombatLog, context_menu::{self, ContextMenu}, initiative::Initiative, minimap::Minimap, party_strip::PartyStrip, side_panel::SidePanel, viewport::Viewport, Anchor, Dock, Layout, Origin, Picture, Placement, Rect, Size, FRAME};
use mt1::ui::ScreenKind;
use std::sync::Arc;
pub const PARTY_RECT: Rect = Rect::new(0, 0, 320, 54);
pub const SIDE_RECT: Rect = Rect::new(192, 54, 128, 146);
pub const LOG_RECT: Rect = Rect::new(0, 166, 192, 34);
pub const VIEW_RECT: Rect = Rect::new(0, 54, 192, 112);
pub const MINIMAP_ID: &str = "Minimap"; pub const MINIMAP_H: u32 = 64; pub const MINIMAP_MIN_H: u32 = 48;
pub static MINIMAP_BELOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn register(layout: &mut Layout, assets: Arc<Assets>) {
    super::textbox::register(layout, assets.clone());
    layout.register(Box::new(super::tooltip::Tooltip::new(assets.clone())), super::tooltip::Tooltip::placement());
    layout.register(Box::new(super::combat_bar::CombatBar::new(assets.clone())), super::combat_bar::CombatBar::placement());
    layout.register(Box::new(Picture::full()), Placement { screens: &[ScreenKind::Picture, ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Space], undockable: false, ..Picture::placement("Picture", FRAME) });
    layout.register(Box::new(PartyStrip::new(assets.clone())), Placement { dock: Dock::Top, order: 0, screens: &[ScreenKind::Ground], size: Size { preferred: (320, PARTY_RECT.h + super::party_strip::NAMES_H), min: (320, PARTY_RECT.h) }, ..Picture::placement("Party strip", PARTY_RECT) });
    layout.register(Box::new(Initiative::new(assets.clone())), Placement { dock: Dock::Top, size: Size { preferred: (320, 24), min: (64, 24) }, order: 5, screens: &[ScreenKind::Ground], when: always, origin: Origin::New, optional: true, undockable: true, title: "Initiative order" });
    layout.register(Box::new(Minimap::new(assets.clone())), Placement { dock: Dock::Right, size: Size { preferred: (128, MINIMAP_H), min: (64, MINIMAP_MIN_H) }, order: 8, screens: &[ScreenKind::Ground], when: always, origin: Origin::New, optional: true, undockable: true, title: MINIMAP_ID });
    layout.register(Box::new(SidePanel::new(assets.clone())), Placement { dock: Dock::Right, order: 10, screens: &[ScreenKind::Ground], ..Picture::placement("Side panel", SIDE_RECT) });
    layout.register(Box::new(CombatLog::new(assets.clone())), Placement { dock: Dock::Bottom, order: 20, screens: &[ScreenKind::Ground], size: Size { preferred: (192, 36), min: (64, 27) }, ..Picture::placement("Combat log", LOG_RECT) });
    layout.register(Box::new(Viewport::new(assets.clone())), Placement { dock: Dock::Fill, order: 30, screens: &[ScreenKind::Ground], undockable: false, ..Picture::placement("Viewport", VIEW_RECT) });
    layout.register(Box::new(super::plus::Plus::new(assets.clone())), super::plus::Plus::placement());
    layout.register(Box::new(super::toolbar::Toolbar::new(assets.clone())), super::toolbar::Toolbar::placement());
    layout.register(Box::new(super::choices::Choices::new(assets.clone())), super::choices::Choices::placement());
    layout.register(Box::new(super::cargo::Cargo::new(assets.clone())), super::cargo::Cargo::placement());
    layout.register(Box::new(super::buy::Buy::new(assets.clone())), super::buy::Buy::placement());
    layout.register(Box::new(ContextMenu::new(assets.clone())), Placement { dock: Dock::Float(Anchor::Pointer), size: Size { preferred: (context_menu::WIDTH, context_menu::HEIGHT), min: (context_menu::WIDTH, context_menu::HEIGHT) }, order: 90, screens: &[ScreenKind::Ground], when: |s| s.squad.menu.is_some(), origin: Origin::New, optional: false, undockable: false, title: "Menu" });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn minimap_has_its_slot_above_the_side_panel_at_every_window_size() {
        use super::super::{render::Metrics, Host, HostKind, UiState};
        for size in [[640, 652], [885, 663], [1210, 1221], [1920, 1080]] {
            let look = crate::look::Look::default(); let scene = mt1::ui::Scene::program(ScreenKind::Ground);
            let m = Metrics::main(size, &look, Some(&scene)); let mut l = Layout::default(); register(&mut l, Arc::default());
            let mut ui = UiState::default(); l.arrange_for(&scene, &ui, m.logical);
            let rect = |l: &Layout, id| match l.host(id) { Some(Host::Dock(r)) => r, h => panic!("{size:?}: {id} is {h:?}") };
            let (map, side, bar) = (rect(&l, MINIMAP_ID), rect(&l, "Side panel"), rect(&l, super::super::combat_bar::ID));
            assert!(l.visible(MINIMAP_ID, &scene), "{size:?}: shown");
            assert!(map.h <= MINIMAP_H && map.h >= 8 && map.w == 128, "{size:?}: within its slot, the column's width: {map:?}");
            if crate::look::Look::default().scale != crate::look::ScaleMode::Integer { assert!(map.h >= MINIMAP_MIN_H, "{size:?}: Fill scale keeps the min: {map:?}"); }
            assert_eq!((map.x, map.w, map.y + map.h as i32), (side.x, side.w, side.y), "{size:?}: directly above the side panel: {map:?} {side:?}");
            if l.visible(super::super::combat_bar::ID, &scene) { assert!(map.y >= bar.y + bar.h as i32, "{size:?}: below the combat bar: {map:?} {bar:?}"); }
            assert!(side.h >= SIDE_RECT.h, "{size:?}: the side panel keeps at least its 1990 size: {side:?}");
            let strip = rect(&l, "Party strip").h;
            assert!(strip == PARTY_RECT.h || (strip == PARTY_RECT.h + super::super::party_strip::NAMES_H && map.h >= super::super::layout::NAMES_MINIMAP_H), "{size:?}: strip {strip}, minimap {map:?}");
            if size == [1210, 1221] { assert_eq!(strip, PARTY_RECT.h + super::super::party_strip::NAMES_H, "a 1210 x 1221 window shows the names row") }
            ui.set_host_kind(MINIMAP_ID, HostKind::Hidden); l.arrange_for(&scene, &ui, m.logical);
            assert!(!l.visible(MINIMAP_ID, &scene), "{size:?}: hidden by the user");
            assert_eq!(rect(&l, "Side panel"), side, "{size:?}: hidden, the side panel keeps its place");
            ui.set_host_kind(MINIMAP_ID, HostKind::Docked); l.arrange_for(&scene, &ui, m.logical); assert_eq!(rect(&l, MINIMAP_ID), map, "{size:?}: shown again in the same slot");
        }
    }
    #[test]
    fn home_follows_in_modern_and_is_raw_in_classic() {
        use super::super::{controls, UiEvent, UiState};
        use mt_engine::io::{keys, Event};
        let mut l = Layout::default(); register(&mut l, Arc::default());
        let scene = mt1::ui::Scene::program(ScreenKind::Ground);
        let mut ui = UiState::default(); l.arrange_for(&scene, &ui, (500, 400));
        assert!(!l.is_classic());
        assert_eq!(controls::dispatch(&l, "Viewport", &scene, &mut ui, &UiEvent::Key(keys::HOME), (40, 40)),
            [mt1::ui::Intent::Follow(Some(controls::selected(&scene)))]);
        ui.preset = mt1::ui::Preset::Classic; l.arrange_for(&scene, &ui, (500, 400));
        assert!(l.is_classic());
        assert_eq!(controls::dispatch(&l, "Viewport", &scene, &mut ui, &UiEvent::Key(keys::HOME), (40, 40)),
            [mt1::ui::Intent::Raw(Event::Key(keys::HOME))]);
    }
    #[test]
    fn catalogue_covers_all_programs_and_uses_placement_gates() {
        let mut l = Layout::default(); register(&mut l, Arc::default());
        assert_eq!(l.ids().len(), 17);
        assert!(["Commands", "Choices", "Combat controls", "Text input", "Continue", "Cargo"].into_iter().all(|id| l.ids().contains(&id))); assert!(!l.ids().contains(&"Flight")); assert!(!l.placement("Viewport").unwrap().undockable);
        for screen in [ScreenKind::Picture, ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Space] {
            let s = mt1::ui::Scene::program(screen); assert_eq!(l.draw(&s, &super::super::UiState::default()).len(), 1);
        }
        assert_eq!(l.placement("Initiative order").unwrap().origin, Origin::New);
        assert!((l.placement("Initiative order").unwrap().when)(&mt1::ui::Scene::program(ScreenKind::Ground)), "F10 r2: the row is reserved outside a fight too (drawn blank)");
    }
}
