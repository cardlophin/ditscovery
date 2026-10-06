use ditscover_core::config::Config;
use ditscover_core::{discover, metrics, synth};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let o: Vec<_> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Config::build(None, None, &o).unwrap()
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    (v[(v.len() - 1) / 2] + v[v.len() / 2]) / 2.0
}

#[test]
#[cfg_attr(debug_assertions, ignore = "SC-001 runs in --release")]
fn planted_motifs_are_recovered() {
    let (mut recalls, mut precisions) = (Vec::new(), Vec::new());
    for seed in 0..10 {
        let (x, planted) = synth::planted(10_000, &[60, 80, 100], 8, 0.3, 0.1, seed);
        let c = cfg(&[
            ("min_len", json!(40)),
            ("max_len", json!(135)),
            ("max_stretch", json!(2.0)),
            ("seed", json!(seed)),
        ]);
        let res = discover(x, &c, &AtomicBool::new(false)).unwrap();
        let found: Vec<(u32, u32)> =
            res.motifs.iter().take(5).flat_map(|m| m.occurrences.clone()).collect();
        let truth: Vec<(u32, u32)> = planted.into_iter().flatten().collect();
        let (r, p) = metrics::match_occurrences(&found, &truth);
        eprintln!(
            "seed {seed}: recall {r:.3} precision {p:.3} stop {:?} gens {}",
            res.stop, res.generations
        );
        recalls.push(r);
        precisions.push(p);
    }
    let (r, p) = (median(recalls), median(precisions));
    assert!(r >= 0.9 && p >= 0.8, "median recall {r:.3}, precision {p:.3}");
}

#[test]
fn white_noise_returns_motifs_without_failing() {
    let (x, _) = synth::planted(2000, &[], 0, 0.0, 1.0, 3);
    let c = cfg(&[
        ("min_len", json!(20)),
        ("max_len", json!(40)),
        ("population", json!(20)),
        ("max_generations", json!(5)),
    ]);
    let res = discover(x, &c, &AtomicBool::new(false)).unwrap();
    assert!(!res.motifs.is_empty());
}
