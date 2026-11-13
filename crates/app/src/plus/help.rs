use mt1::ui::{Scene, ScreenKind};
pub const KNOWLEDGE: &str = include_str!("../../../../docs/KNOWLEDGE.md");
pub const GROUND_KEYS: &str = "Keyboard shortcuts\n\
E: End turn. Carry out standing orders, then finish.\n\
O: Carry out orders; keep the turn.\n\
Tab / Shift+Tab: Next / previous thug in sight on this screen.\n\
F / Enter: Fire at the target.\n\
1-5: Select a Traveller. R: Reload. L: Light, the first carried torch or lamp on or off.\n\
M: Menu on the square cursor or target; otherwise the selected portrait.\n\
Menu: press an item's letter, Enter for the highlighted action, Esc to close.\n\
X: Choose a square. Arrows move the cursor. Enter walks there. M opens its menu. X / Esc cancels.\n\
Arrows / keypad: Walk, eight ways. PgUp NE, End SW, PgDn SE; keypad 7 / Ctrl+Home NW.\n\
F1-F5: Character records. Esc: Back.\n\
=: Swap hand. Q: Semi / full auto.\n\
T: Choose grenade; E / C / R chooses its type. Then Tab, M, T at a thug, or X, arrows, M, T at a square.\n\
H: Hold here. Shift+H: Hold to interrupt. Ctrl+H: Hold fire. + / -: Tactical points.\n\
Round start: Space / O chooses us first; N chooses them first.\n\
Without a thug in sight on this screen, Tab / Shift+Tab selects the next / previous Traveller.\n\
P: Break / regroup. O when grouped: Marching order.\n\
C: Converse. I: Take / use items. V: Vehicle.\n\
G: Game menu. Starport list: Save.\n\
Menus / lists: Arrows, Enter, Esc.\n\
[ / ]: Older / newer log line. Shift+L: Long log.\n\
Home: Follow. ?: Help. F12: Plus.\n\
F11: Fullscreen. Ctrl+S: Pixel shape. Ctrl+Shift+L: Testing scenarios.\n\
Hide and the portrait's Use / Give entries are unfinished. Use I for items or P in a character record to give.";
pub fn section(start: &str, end: &str) -> &'static str {
    KNOWLEDGE.split_once(start).map_or("", |(_, rest)| rest.split_once(end).map_or(rest, |(s, _)| s))
}
pub fn cards(scene: &Scene) -> Vec<&'static str> {
    let starport = matches!(scene.screen, ScreenKind::Ground | ScreenKind::Picture) && matches!(scene.context.event, 9 | 10);
    let name = match scene.screen {
        ScreenKind::Picture => "Picture", ScreenKind::Startup => "Startup", ScreenKind::Cg => "Cg",
        ScreenKind::Ground => "Ground", ScreenKind::Intro => "Intro", ScreenKind::Record => "Record", ScreenKind::Space => "Space",
    };
    let mut rows: Vec<_> = section("## H.", "## 1.").lines().filter_map(|line| {
        let cols: Vec<_> = line.split('|').map(str::trim).collect();
        (cols.len() >= 5 && (cols[1].starts_with(&format!("`{name}`")) || (starport && cols[1].contains("starport panel")))).then(|| (cols[1], cols[3]))
    }).collect();
    let preferred = if starport { "starport" } else if scene.screen == ScreenKind::Ground {
        if scene.squad.active && scene.squad.encounter { "squad fight" }
        else if scene.combat && !scene.squad.active { "1990 real-time" }
        else { "grouped" }
    } else if scene.context.station.is_some() { "stations" } else { "flight" };
    rows.sort_by_key(|(title, _)| !title.contains(preferred));
    rows.into_iter().map(|(_, text)| text).collect()
}
pub fn clicks(scene: &Scene) -> &'static str {
    match scene.screen {
        ScreenKind::Ground if matches!(scene.context.event, 9 | 10) => "Click an office to enter. Click a list row to select it. On the cargo page click [-] / [+] (hold to repeat), then Done, or Back to leave. On the refuel bill click Buy one unit, All or Pay.",
        ScreenKind::Ground if scene.squad.active && scene.squad.encounter => "Click a Traveller to select; click a map square to move; click an enemy to fire. Right-click a target for its available actions. Use the combat buttons to reload, give orders and end the turn.",
        ScreenKind::Ground if scene.combat && !scene.squad.active => "Use the command buttons for the current ground actions and Traveller records. Click Party to break or regroup; Orders pauses the classic fight. Click menu rows to choose and Back to leave.",
        ScreenKind::Ground => "Click a map square to walk there. Click a Traveller to select, an item to take, or a person to converse. Right-click a target for its available actions. Use the command buttons for Game, Party, Items, Vehicle and Traveller records. Use Next / Previous for more commands.",
        ScreenKind::Record => "Click character actions, equipment or inventory entries. Click another Traveller to view that record; click Close to return.",
        ScreenKind::Startup => "Click Start game to continue or Quit to leave.",
        ScreenKind::Cg => "Click a menu row to choose it; click Yes / No or Back when offered. Click a name field to focus it, then OK or Cancel.",
        ScreenKind::Intro => "Click Continue for the next page.",
        ScreenKind::Picture => "Click a menu row to choose. In a shop click an item, then Yes to buy one or Cancel; click Exit or Back to leave. Use Previous / Next for more rows.",
        ScreenKind::Space if scene.context.station.is_some() => "Click a station command. Click Back to return to flight; use menu rows for station choices.",
        ScreenKind::Space => "Click the flight controls for thrust, turning, stopping, landing, jumping and crew stations. Click a menu row to choose and Back to return.",
    }
}
pub fn text(scene: &Scene) -> String { text_with_decision(scene, None) }
pub fn text_with_decision(scene: &Scene, decision: Option<&mt_engine::play::Decision>) -> String {
    let available = decision.filter(|d| d.kind != mt_engine::play::Kind::CombatTurn).map(|d| {
        let mut labels: Vec<_> = d.options.iter().filter(|o| o.enabled).map(|o| o.label.as_str()).collect();
        if let Some(list) = &d.list {
            labels.extend(list.rows.iter().filter(|r| r.enabled).map(|r| r.label.as_str()));
            if !list.increase.is_empty() { labels.push("+"); } if !list.decrease.is_empty() { labels.push("-"); } labels.push("OK");
        }
        format!("\n\nAvailable now: {}", labels.join(", "))
    }).unwrap_or_default();
    let cards = cards(scene).join("\n\n");
    format!("{GROUND_KEYS}\n\nCurrent screen: {:?}{}\n\n{cards}\n\nGuides\n\n1. {}", scene.screen, available, section("## 1. ", "## J."))
}
pub fn lines(text: &str, width: usize) -> Vec<String> {
    let text = text.replace(['`', '*', '#'], "").replace(['—', '–'], "-").replace('→', "->")
        .replace(['’', '‘'], "'").replace(['“', '”'], "\"").replace('×', "x").replace('≤', "<=");
    let mut out = Vec::new();
    for para in text.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            for chunk in word.chars().collect::<Vec<_>>().chunks(width.max(1)) {
                if !line.is_empty() && line.chars().count() + 1 + chunk.len() > width { out.push(std::mem::take(&mut line)); }
                if !line.is_empty() { line.push(' '); }
                line.extend(chunk.iter().map(|c| if c.is_ascii() { *c } else { '?' }));
            }
        }
        out.push(line);
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn efate_quiet_broken_and_shop_keys_precede_context_and_guides() {
        use mt_engine::play::{Decision, Kind, Option as Opt};
        let mut d = Decision::new(Kind::Ground, "t", "6923");
        d.options.push(Opt::new("cmd:game", "Game", vec![]));
        let mut disabled = Opt::new("no", "Unavailable action", vec![]); disabled.enabled = false; d.options.push(disabled);
        for kind in [ScreenKind::Ground, ScreenKind::Picture, ScreenKind::Record, ScreenKind::Space, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Startup] {
            let mut s = Scene::program(kind);
            for encounter in [false, true] {
                s.squad.active = true; s.squad.encounter = encounter;
                let t = text_with_decision(&s, Some(&d));
                assert!(t.starts_with(GROUND_KEYS));
                assert!(t.find("Keyboard shortcuts").unwrap() < t.find("Available now: Game").unwrap());
                assert!(t.find("Keyboard shortcuts").unwrap() < t.find("Guides").unwrap());
                assert!(!t.contains("Unavailable action"));
            }
        }
    }
    #[test]
    fn every_screen_has_embedded_help_and_context_precedes_guides() {
        for k in [ScreenKind::Picture, ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Ground, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Space] {
            let s = Scene::program(k); assert!(!cards(&s).is_empty(), "{k:?}");
            let t = text(&s); assert!(t.find(cards(&s)[0]).unwrap() < t.find("Guides").unwrap());
        }
        let mut s = Scene::program(ScreenKind::Ground);
        assert!(cards(&s)[0].contains("8-way"));
        s.combat = true; assert!(cards(&s)[0].contains("Break"));
        s.squad.active = true; s.squad.encounter = true; assert!(cards(&s)[0].contains("Round start"));
        s.context.event = 9; assert!(cards(&s)[0].contains("second invokes"));
        s.screen = ScreenKind::Picture; assert!(cards(&s)[0].contains("second invokes"));
        s.screen = ScreenKind::Space; s.context.station = Some(4); assert!(cards(&s)[0].contains("assign"));
        assert!(lines("**a** → longword", 4).iter().all(|l| l.len() <= 4));
    }
}
