use crate::look::MenuSound;
use mt_engine::{io::Event, play::{Decision, Kind}};

pub const HZ: u32 = 16000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone { Choice, Back, Done }
pub fn render(sound: MenuSound, tone: Tone) -> Vec<u8> {
    let note = |hz: f32, ms: u32, decay: f32| -> Vec<u8> {
        let n = HZ * ms / 1000; (0..n).map(|i| {
            let t = i as f32 / HZ as f32; let attack = (t / 0.004).min(1.0); let env = attack * (-t * decay).exp();
            (120.0 * env * (1.0 + (t * hz * std::f32::consts::TAU).sin())).round().clamp(0.0, 240.0) as u8
        }).collect()
    };
    match (sound, tone) {
        (MenuSound::Off, _) => Vec::new(),
        (_, Tone::Choice) => [note(1047.0, 70, 12.0), note(1397.0, 80, 14.0)].concat(),
        (_, Tone::Back) => [note(880.0, 70, 12.0), note(659.0, 80, 14.0)].concat(),
        (_, Tone::Done) => note(1319.0, 150, 12.0),
    }
}
pub fn for_route(d: Option<&Decision>, events: &[Event]) -> Option<Tone> {
    let d = d?;
    if matches!(d.kind, Kind::Text | Kind::Number) { return None }
    let keys: Vec<u16> = events.iter().filter_map(|e| if let Event::Key(k) = e { Some(*k) } else { None }).collect();
    if keys.contains(&mt_engine::io::keys::ESC) { return Some(Tone::Back) }
    let option = |k: u16| d.options.iter().find(|o| o.enabled && o.input.iter().filter(|e| matches!(e, Event::Key(_))).count() == 1 && o.input.contains(&Event::Key(k)));
    if d.kind == Kind::Ack && keys.iter().any(|&k| k == mt_engine::io::keys::ENTER || k == mt_engine::io::keys::SPACE || option(k).is_some()) { return Some(Tone::Done) }
    if keys.iter().any(|&k| option(k).is_some_and(|o| o.id == "pay")) { return Some(Tone::Done) }
    keys.iter().any(|&k| k == mt_engine::io::keys::ENTER || option(k).is_some()).then_some(Tone::Choice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::keys, play::Option as Opt};
    #[test]
    fn a_choice_blips_back_blips_down_a_done_act_dings_and_off_is_silent() {
        let mut d = Decision::new(Kind::Menu, "t", "AF79");
        d.options = vec![Opt::new("r", "Restore", vec![Event::Key(0x13b)]), Opt::new("b", "Back", vec![Event::Key(keys::ESC)]), Opt::new("pay", "Pay", vec![Event::Key(80)])];
        assert_eq!(for_route(Some(&d), &[Event::Key(0x13b), Event::KeyUp(0x13b)]), Some(Tone::Choice), "the F1 row");
        assert_eq!(for_route(Some(&d), &[Event::Key(keys::DOWN), Event::KeyUp(keys::DOWN), Event::Key(keys::ENTER), Event::KeyUp(keys::ENTER)]), Some(Tone::Choice), "a list row route ends in Enter");
        assert_eq!(for_route(Some(&d), &[Event::Key(keys::DOWN), Event::KeyUp(keys::DOWN)]), None, "moving the cursor is no choice");
        assert_eq!(for_route(Some(&d), &[Event::Key(keys::ESC), Event::KeyUp(keys::ESC)]), Some(Tone::Back), "Back is the lower blip");
        assert_eq!(for_route(Some(&d), &[Event::Key(80), Event::KeyUp(80)]), Some(Tone::Done), "Pay is a completed act");
        let mut ack = Decision::new(Kind::Ack, "t", "1"); ack.options = vec![Opt::new("continue", "Continue", vec![Event::Key(keys::SPACE)])];
        assert_eq!(for_route(Some(&ack), &[Event::Key(keys::SPACE)]), Some(Tone::Done), "a prose continue (the save's done message too)");
        assert_eq!(for_route(None, &[Event::Key(keys::ENTER)]), None, "no decision, no tone");
        let mut text = Decision::new(Kind::Text, "t", "1"); text.options = d.options.clone(); assert_eq!(for_route(Some(&text), &[Event::Key(keys::ENTER)]), None);
        for t in [Tone::Choice, Tone::Back, Tone::Done] {
            assert!(render(MenuSound::Off, t).is_empty(), "Sound off = silence");
            let s = render(MenuSound::On, t); let ms = s.len() as u32 * 1000 / HZ;
            assert!((120..=180).contains(&ms), "{t:?}: {ms} ms"); assert!(s.iter().any(|&x| x >= 220), "full level, as loud as a 1990 effect"); assert!(s[..3].iter().all(|&x| x <= 16), "no click at the start");
        }
        assert_ne!(render(MenuSound::On, Tone::Choice), render(MenuSound::On, Tone::Done), "two tones: blip and ding");
        assert_ne!(render(MenuSound::On, Tone::Choice), render(MenuSound::On, Tone::Back));
        for (old, want) in [("a", MenuSound::On), ("b", MenuSound::On), ("on", MenuSound::On), ("off", MenuSound::Off)] {
            #[derive(serde::Deserialize)] struct L { menu_sound: MenuSound }
            assert_eq!(toml::from_str::<L>(&format!("menu_sound = \"{old}\"")).unwrap().menu_sound, want, "old look.toml {old}");
        }
    }
}
