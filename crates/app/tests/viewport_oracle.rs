#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use megatraveller::panels::{assets::Assets, viewport::Viewport, Component, Surface, UiState};
use mt1::{ground::{render, state::{Ground, Mem}}, ui::{Scene, ScreenKind}};
use mt_engine::{io::{Dir, Scripted}, present::fb::Fb, sim::tick::Counter};
use std::{path::{Path, PathBuf}, process::Command, sync::Arc};

fn ground(folder: &Path, files: &Path, segments: &[usize]) -> Ground {
    let memory = std::fs::read(folder.join("memory.bin")).unwrap();
    let ds: usize = std::fs::read_to_string(folder.join("ds.txt")).unwrap().parse().unwrap();
    let mut g = Ground::from_ds(memory[ds * 16..ds * 16 + 65536].to_vec(), memory[0x90000..0x90000 + Mem::PARTY_SIZE].to_vec(), Box::new(Scripted::new([])), Box::new(Counter::at(100)), Box::new(Dir(files.to_owned())));
    for &seg in segments { if seg != Mem::DS as usize && seg != 0x9000 { g.mem.segs.insert(seg as u16, memory[seg * 16..(seg * 16 + 65536).min(memory.len())].to_vec()); } }
    assert!(g.fb.restore(&std::fs::read(folder.join("start.bin")).unwrap())); g
}
#[test]
fn viewport_matches_independent_efate_oracle() {
    let Some((oracle, files)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."); let out = root.join("target/component-fixtures/efate");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py")).args(["--seconds", "120", "--mib", "1024", "--", "python", "-B"]).arg(root.join("tools/m4a_dump_render.py")).arg(oracle).arg(&files).arg(&out).output().unwrap();
    assert!(result.status.success(), "export: {}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let files = PathBuf::from(files); let assets = Arc::new(Assets::load(&mut Dir(files.clone()))); let renderer = Viewport::new(assets);
    let segments: Vec<usize> = std::fs::read_to_string(out.join("segments.txt")).unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
    let mut count = 0; let mut failures = Vec::new();
    for name in std::fs::read_to_string(out.join("manifest.txt")).unwrap().lines().filter(|n| n.starts_with("efate-") || n.starts_with("overlays-") || n.starts_with("walkers-")) {
        let folder = out.join(name); let mut g = ground(&folder, &files, &segments);
        render::redraw(&mut g);
        let scene = Scene::capture(&g, ScreenKind::Ground, None); let ui = UiState { camera: Viewport::oracle_camera(&scene), ..UiState::default() };
        let before = (g.mem.ds.clone(), g.mem.segs.clone(), g.fb.planes.clone()); let mut surface = Surface::new(192, 112); renderer.draw(&scene, &ui, &mut surface);
        assert_eq!(before, (g.mem.ds.clone(), g.mem.segs.clone(), g.fb.planes.clone()));
        let mut expected = Fb::new(); assert!(expected.restore(&std::fs::read(folder.join("end.bin")).unwrap()));
        let page = (g.mem.u16(0x52F4) & 1) as usize;
        let differences: Vec<_> = (0..112).flat_map(|y| (0..192).map(move |x| (x, y))).filter(|&(x, y)| surface.get(x, y) != Some(expected.get(page, y + 54, x))).collect();
        if !differences.is_empty() { std::fs::write(folder.join("viewport.bin"), &surface.indexed).unwrap(); failures.push(format!("{name}: {} pixels; first {:?}", differences.len(), &differences[..differences.len().min(8)])); }
        count += 1;
    }
    assert!(count >= 44, "fixture coverage dropped: {count}"); assert!(failures.is_empty(), "{}", failures.join("\n")); eprintln!("Viewport oracle: {count} complete map rectangles PASS");
    for leader in 0..5 { for phase in 0..=2 {
        let mut g = ground(&out.join("efate-0-0-0-0"), &files, &segments);
        g.mem.set_combat(1); g.mem.set_leader(leader); g.mem.set_individual(0); g.mem.set_orders(0); g.mem.set_phase(phase);
        g.mem.set_x(6); g.mem.set_y(10); g.mem.set_heading(3);
        for i in 0..5 { let w = Mem::walker(i); g.mem.write(w, 1, 1); g.mem.write(w.add(8), (2 + i * 2) as u32, 2); g.mem.write(w.add(10), (2 + i * 3) as u32, 2); g.mem.write(w.add(1), i as u32, 1); g.mem.write(w.add(20), (i % 3) as u32, 2); }
        render::redraw(&mut g); let scene = Scene::capture(&g, ScreenKind::Ground, None);
        let ui = UiState { camera: Viewport::oracle_camera(&scene), ..UiState::default() }; let mut surface = Surface::new(192, 112); renderer.draw(&scene, &ui, &mut surface);
        let page = (g.mem.u16(0x52F4) & 1) as usize;
        let differences = (0..112).flat_map(|y| (0..192).map(move |x| (x, y))).filter(|&(x, y)| surface.get(x, y) != Some(g.fb.get(page, y + 54, x))).count();
        assert_eq!(differences, 0, "broken party leader={leader} phase={phase}");
    } }
}

#[test]
fn viewport_matches_session_01_reference_redraws() {
    let (Some(lab), Some(references)) = (std::env::var_os("MT1_LAB"), std::env::var_os("MT1_LOCKSTEP_REF")) else { eprintln!("SKIP: MT1_LAB/MT1_LOCKSTEP_REF unset"); return };
    let Some(files) = oracle_env::gamefiles() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."); let out = root.join("target/component-fixtures/session01");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py")).args(["--seconds", "120", "--mib", "1024", "--", "node"]).arg(root.join("tools/m4c0_viewport_frames.mjs")).arg(lab).arg(PathBuf::from(references).join("01-boot-to-efate.keys")).arg(&out).output().unwrap();
    assert!(result.status.success(), "export: {}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let frames: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    assert!(frames.len() >= 11, "session 01 lost redraw coverage");
    let files = PathBuf::from(files); let renderer = Viewport::new(Arc::new(Assets::load(&mut Dir(files.clone()))));
    let mut failures = Vec::new();
    for meta in &frames {
        let id = meta["id"].as_str().unwrap(); let folder = out.join(id); let memory = std::fs::read(folder.join("memory.bin")).unwrap();
        let ds = meta["ds"].as_u64().unwrap() as usize * 16;
        let word = |at| u16::from_le_bytes([memory[ds + at], memory[ds + at + 1]]) as usize;
        let party = word(0x6D65) + word(0x6D67) * 16;
        let mut g = Ground::from_ds(memory[ds..ds + 65536].to_vec(), memory[party..party + Mem::PARTY_SIZE].to_vec(), Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(files.clone())));
        let map_seg = word(0xBE); g.mem.segs.insert(map_seg as u16, memory[map_seg * 16..(map_seg * 16 + 65536).min(memory.len())].to_vec());
        let scene = Scene::capture(&g, ScreenKind::Ground, None); let ui = UiState { camera: Viewport::oracle_camera(&scene), ..UiState::default() };
        let mut surface = Surface::new(192, 112); renderer.draw(&scene, &ui, &mut surface);
        let expected = std::fs::read(folder.join("map.bin")).unwrap();
        let differences: Vec<_> = surface.indexed.iter().zip(&expected).enumerate().filter(|(_, (a,b))| a != b).map(|(i, _)| (i % 192, i / 192)).collect();
        if !differences.is_empty() { std::fs::write(folder.join("viewport.bin"), &surface.indexed).unwrap(); failures.push(format!("{id}: {} pixels; first {:?}; view {:?} phase {} moving {}", differences.len(), &differences[..differences.len().min(8)], scene.view, scene.art.phase, scene.art.moving)); }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n")); eprintln!("Session 01 viewport: {} reference frames PASS (all pixels)", frames.len());
}

#[test]
fn zz_skip_report() { oracle_env::report() }
