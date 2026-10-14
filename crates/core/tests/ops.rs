use ditscover_core::config::Config;
use ditscover_core::motif::{Ctx, Scratch, Segment};
use ditscover_core::{ops, synth};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let o: Vec<(String, Value)> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Config::build(None, None, &o).unwrap()
}

fn hits(found: &Segment, truth: &[Segment]) -> bool {
    truth.iter().any(|t| {
        let inter = found.end().min(t.end()).saturating_sub(found.start.max(t.start)) as f64;
        inter / ((found.len + t.len) as f64 - inter) > 0.5
    })
}

fn planted(noise: f64) -> (Vec<f64>, Vec<Segment>) {
    let (x, truth) = synth::planted(4000, &[60], 6, 0.0, noise, 21);
    (x, truth[0].iter().map(|&(start, len)| Segment { start, len }).collect())
}

#[test]
fn nn_seeds_start_on_planted_occurrences() {
    let (x, truth) = planted(0.02);
    let c = cfg(&[("min_len", json!(45)), ("max_len", json!(75))]);
    let ctx = Ctx::new(&x, &c);
    let seeds = ops::nn_seeds(&ctx, 8);
    assert!(!seeds.is_empty());
    let on = seeds.iter().take(4).filter(|g| hits(g, &truth)).count();
    assert!(on >= 3, "{seeds:?} vs {truth:?}");
}

#[test]
fn gap_cut_keeps_the_planted_count() {
    let (x, truth) = planted(0.02);
    let c = cfg(&[("min_len", json!(45)), ("max_len", json!(75)), ("max_occ", json!(15))]);
    let ctx = Ctx::new(&x, &c);
    let mut rng = ChaCha8Rng::seed_from_u64(0);
    let set = ops::grow(truth[2], &ctx, &mut Scratch::new(75), &mut rng).unwrap();
    assert_eq!(set.len(), truth.len(), "{set:?}");
    assert!(set.iter().all(|g| hits(g, &truth)), "{set:?}");
}
