use super::*;
use std::fs;

#[test]
#[ignore = "requires a Windows desktop; hidden window, run in its own test process"]
fn native_bridge_saved_report_notes_status_and_preview() {
    let root = std::env::temp_dir().join(format!("mt-native-bridge-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let previous = std::env::var_os("MT_REPO"); std::env::set_var("MT_REPO", &root);
    let mut win = Win::with_settings(3, false, false, root.join("look.toml")).unwrap();
    let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
    win.panel_assets(&mut mt_engine::io::Dir(dir.clone()));
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let prepared = crate::scenario::load(&source.join("scenarios/efate_quiet.toml"), &dir).unwrap();
    let scene = Scene::capture(&prepared.game, mt1::ui::ScreenKind::Ground, None);
    win.app.scene = Some(scene.clone()); win.app.plus_scene = Some(scene);
    win.app.rgb = prepared.game.fb.rgb(0, &prepared.game.dac);
    *win.app.ui.frame = Frame::from_rgb(&win.app.rgb);
    win.app.plus_action(A::Open);
    win.app.plus_text("Bridge UI round trip (fake fixer)"); win.app.plus_action(A::Submit);
    let report = win.app.ui.plus.report.clone().unwrap();
    assert!(report.directory.join("bundle.json").exists());
    assert!(report.directory.join("screen.png").exists());
    let out = source.join("target/bridge-proof"); fs::create_dir_all(&out).unwrap();
    let preview = |app: &mut App, name: &str| {
        app.render().unwrap(); let size = render_size(app.window.as_ref().unwrap().inner_size());
        fs::write(out.join(name), megatraveller::plus::png::rgb(size.width, size.height, &app.output).unwrap()).unwrap();
    };
    preview(&mut win.app, "dev-saved.png");
    win.app.plus_event(UiEvent::Up(Button::Left), (15, 232)); assert!(win.app.ui.plus.bridge.editing_note);
    win.app.plus_text("More details"); win.app.plus_key(Some(KeyCode::Enter), None, ElementState::Pressed, false, true);
    let settle = |app: &mut App| {
        let deadline = Instant::now() + Duration::from_secs(2);
        while app.ui.plus.bridge.busy() {
            app.bridge_poll(); assert!(Instant::now() < deadline); std::thread::sleep(Duration::from_millis(5));
        }
    };
    settle(&mut win.app); assert!(!win.app.ui.plus.bridge.editing_note);
    let note = fs::read_to_string(report.directory.join("notes.jsonl")).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(note.trim()).unwrap()["text"], "More details");
    let before = fs::read(report.directory.join("bundle.json")).unwrap();
    for (state, filename) in [("received", "dev-received.png"), ("fixed", "dev-fixed.png")] {
        let value = serde_json::json!({"id":report.id,"state":state,"at":123,"lane":"fake-fixer","commit":"abc1234","message":"Round trip verified"});
        let tmp = report.directory.join("status.tmp"); fs::write(&tmp, value.to_string()).unwrap(); fs::rename(&tmp, report.directory.join("status.json")).unwrap();
        let deadline = Instant::now() + Duration::from_secs(7);
        while !win.app.ui.plus.bridge.remote.as_ref().is_some_and(|s| s.state == state) {
            win.pump_once(Duration::from_millis(16)); assert!(Instant::now() < deadline, "5-second host poll");
        }
        preview(&mut win.app, filename);
    }
    assert!(win.app.ui.plus.bridge.fixed());
    assert!(win.app.input.events.is_empty());
    assert_eq!(before, fs::read(report.directory.join("bundle.json")).unwrap(), "notes never rewrite the saved report");
    win.app.plus_action(A::Close); drop(win);
    match previous { Some(value) => std::env::set_var("MT_REPO", value), None => std::env::remove_var("MT_REPO") }
    fs::remove_dir_all(root).unwrap();
}
