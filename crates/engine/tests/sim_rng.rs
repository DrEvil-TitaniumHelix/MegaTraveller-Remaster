mod common_m3;
use mt_engine::sim::rng::Rng;

#[test]
fn rng_stream_matches_msrand() {
    let Some(dir) = common_m3::fixtures() else { return };
    let text = std::fs::read_to_string(dir.join("rng.txt")).unwrap();
    let mut n = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let v = common_m3::ints(line);
        let (seed, state, want) = (v[0] as u16, v[1] as u32, &v[2..]);
        assert_eq!(want.len(), 10000, "seed {seed}");
        let mut r = Rng::new(seed);
        for (i, &w) in want.iter().enumerate() { assert_eq!(r.next() as u64, w, "seed {seed} value {i}") }
        assert_eq!(r.state, state, "seed {seed} state");
        assert_eq!(r.take_trace().len(), 10000);
        let mut f = Rng::new(0); f.force = Some(seed); f.srand(seed.wrapping_add(1)); assert_eq!(f.next() as u64, want[0]);
        println!("PASS seed {seed}: 10000 values + state"); n += 1;
    }
    assert_eq!(n, 8, "8 seeds expected");
}
