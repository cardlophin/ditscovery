use ditscover_core::config::Config;
use ditscover_core::result::RunResult;
use ditscover_core::{discover, synth};
use serde_json::json;
use std::sync::atomic::AtomicBool;

fn run(threads: usize, cfg: &Config, x: &[f64]) -> RunResult {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let mut r = pool.install(|| discover(x.to_vec(), cfg, &AtomicBool::new(false))).unwrap();
    r.elapsed_s = 0.0;
    r.history.iter_mut().for_each(|h| h.elapsed_s = 0.0);
    r
}

#[test]
fn same_seed_same_result_regardless_of_threads() {
    let (x, _) = synth::planted(3000, &[30, 45], 6, 0.2, 0.1, 9);
    let o = [
        ("min_len", json!(20)),
        ("max_len", json!(60)),
        ("max_stretch", json!(2.0)),
        ("population", json!(40)),
        ("max_generations", json!(25)),
        ("snapshot_every", json!(4)),
        ("seed", json!(42)),
    ];
    let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
    let one = run(1, &cfg, &x);
    let four = run(4, &cfg, &x);
    assert!(!one.motifs.is_empty());
    assert_eq!(one, four);
    assert_eq!(one.history.len() as u32, one.generations + 1);
    for h in &one.history {
        assert_eq!(h.front.is_some(), h.generation % 4 == 0, "gen {}", h.generation);
    }
}
