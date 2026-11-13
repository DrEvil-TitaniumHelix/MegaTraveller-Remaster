use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex};
use mt_engine::io::audio::Speaker;

fn main() {
    let host = cpal::default_host();
    println!("host {:?}", host.id());
    for (i, d) in host.output_devices().into_iter().flatten().enumerate() {
        let name = d.description().map(|d| d.name().to_string()).unwrap_or_else(|e| format!("? {e}")); let cfg = d.default_output_config().map(|c| format!("{:?} {} Hz {} ch", c.sample_format(), c.sample_rate(), c.channels())).unwrap_or_else(|e| e.to_string());
        println!("output {i}: {name}: {cfg}");
    }
    let Some(device) = host.default_output_device() else { println!("NO default output device"); return };
    let config = match device.default_output_config() { Ok(c) => c, Err(e) => { println!("default config: {e}"); return } };
    if let Some(secs) = std::env::args().skip_while(|a| a != "--listen").nth(1).and_then(|s| s.parse::<u64>().ok()) {
        let rec = Arc::new(Mutex::new(Vec::<f32>::new())); let r = rec.clone(); let ch = config.channels() as usize; let hz = config.sample_rate() as usize;
        let cap = device.build_input_stream(config.config(), move |data: &[f32], _| { if let Ok(mut r) = r.lock() { r.extend(data.chunks(ch).map(|f| f[0])) } }, |_| {}, None).expect("loopback");
        cap.play().expect("loopback play"); std::thread::sleep(std::time::Duration::from_secs(secs));
        let v = rec.lock().unwrap(); let win = hz / 10;
        let loud: Vec<String> = v.chunks(win).enumerate().filter_map(|(i, c)| { let p = c.iter().fold(0f32, |m, x| m.max(x.abs())); (p > 0.005).then(|| format!("{:.1}s:{:.2}", i as f32 / 10.0, p)) }).collect();
        println!("listened {} s: {} frames, loud 100 ms windows (t:peak): {}", secs, v.len(), if loud.is_empty() { "NONE".to_string() } else { loud.join(" ") }); return
    }
    println!("default: {} {:?} {} Hz {} ch buffer {:?}", device.description().map(|d| d.name().to_string()).unwrap_or_default(), config.sample_format(), config.sample_rate(), config.channels(), config.buffer_size());
    let speaker = Arc::new(Mutex::new(Speaker::new(config.sample_rate())));
    let (calls, nonzero, peak) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    let channels = (config.channels() as usize).max(1);
    let (s, c1, n1, p1) = (speaker.clone(), calls.clone(), nonzero.clone(), peak.clone());
    let mut mono = Vec::new();
    let out = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(config.config(), move |out: &mut [f32], _| {
            mono.resize(out.len() / channels, 0.0);
            if let Ok(mut s) = s.lock() { s.fill(&mut mono) }
            c1.fetch_add(1, Ordering::Relaxed);
            for &v in &mono { if v != 0.0 { n1.fetch_add(1, Ordering::Relaxed); } p1.fetch_max((v.abs() * 10000.0) as u64, Ordering::Relaxed); }
            for (frame, &v) in out.chunks_mut(channels).zip(&mono) { frame.fill(v) }
        }, |e| println!("output error: {e}"), None),
        f => { println!("default format is {f}, not f32: the game's typed build path handles it, this probe only measures f32"); return }
    };
    let out = match out { Ok(s) => s, Err(e) => { println!("build_output_stream: {e}"); return } };
    let rec = Arc::new(Mutex::new((0u64, 0f64, 0f32))); let r = rec.clone();
    let cap = device.build_input_stream(config.config(), move |data: &[f32], _| {
        if let Ok(mut r) = r.lock() { for &v in data { r.0 += 1; r.1 += (v as f64) * (v as f64); r.2 = r.2.max(v.abs()); } }
    }, |e| println!("loopback error: {e}"), None);
    let cap = match cap { Ok(s) => Some(s), Err(e) => { println!("no loopback capture ({e}); callback counts only"); None } };
    if let Some(c) = &cap { if let Err(e) = c.play() { println!("loopback play: {e}") } }
    if let Err(e) = out.play() { println!("play: {e}"); return }
    std::thread::sleep(std::time::Duration::from_millis(300));
    let tone = std::env::args().any(|a| a == "--tone");
    if let Ok(mut s) = speaker.lock() {
        if tone { s.push_tone(megatraveller::tone::HZ, &megatraveller::tone::render(megatraveller::look::MenuSound::On, megatraveller::tone::Tone::Choice)); }
        else {
            let levels: Vec<u8> = (0..880).map(|i| if i % 2 == 0 { 255 } else { 0 }).collect(); s.push(880, levels, 0);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(if tone { 600 } else { 1400 }));
    let r = rec.lock().map(|r| *r).unwrap_or_default();
    let rms = if r.0 > 0 { (r.1 / r.0 as f64).sqrt() } else { 0.0 };
    println!("callbacks {} nonzero samples {} output peak {:.3} | loopback frames {} rms {:.4} peak {:.3}", calls.load(Ordering::Relaxed), nonzero.load(Ordering::Relaxed), peak.load(Ordering::Relaxed) as f64 / 10000.0, r.0 / channels as u64, rms, r.2);
    println!("VERDICT: {}", if calls.load(Ordering::Relaxed) == 0 { "NO CALLBACKS (stream never ran)" } else if nonzero.load(Ordering::Relaxed) == 0 { "CALLBACKS BUT SILENT SAMPLES" } else if cap.is_some() && r.2 < 0.001 { "SAMPLES WRITTEN, LOOPBACK HEARS NOTHING (device muted / wrong endpoint)" } else { "sound reaches the device" });
}
