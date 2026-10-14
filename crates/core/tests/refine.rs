use ditscover_core::config::Config;
use ditscover_core::fitness::members;
use ditscover_core::motif::{Ctx, Scratch, Segment};
use ditscover_core::{ops, synth};
use proptest::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let o: Vec<(String, Value)> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Config::build(None, None, &o).unwrap()
}

fn planted() -> (Vec<f64>, Vec<Segment>) {
    let (x, truth) = synth::planted(3000, &[60], 6, 0.0, 0.0, 11);
    (x, truth[0].iter().map(|&(start, len)| Segment { start, len }).collect())
}

fn assert_recovered(got: &[Segment], truth: &[Segment]) {
    assert_eq!(got.len(), truth.len());
    for (g, t) in got.iter().zip(truth) {
        let ok = g.start.abs_diff(t.start) <= 2 && g.end().abs_diff(t.end()) <= 2;
        assert!(ok, "{g:?} vs planted {t:?} (all: {got:?})");
    }
}

#[test]
fn shifted_occurrences_recover_planted_boundaries() {
    let (x, truth) = planted();
    let c = cfg(&[("min_len", json!(45)), ("max_len", json!(75))]);
    let ctx = Ctx::new(&x, &c);
    let shifts = [9i64, -9, 6, -6, 3, 0];
    let moved: Vec<Segment> = truth
        .iter()
        .zip(shifts)
        .map(|(t, d)| Segment { start: (t.start as i64 + d) as u32, len: t.len })
        .collect();
    assert_recovered(&ops::refine(&moved, &ctx, &mut Scratch::new(75)), &truth);
}

#[test]
fn resized_occurrences_recover_planted_boundaries() {
    let (x, truth) = planted();
    let c = cfg(&[("min_len", json!(45)), ("max_len", json!(75)), ("max_stretch", json!(1.5))]);
    let ctx = Ctx::new(&x, &c);
    let lens = [51u32, 69, 60, 55, 66, 60];
    let resized: Vec<Segment> = truth
        .iter()
        .zip(lens)
        .map(|(t, len)| Segment { start: t.start + 30 - len / 2, len })
        .collect();
    assert_recovered(&ops::refine(&resized, &ctx, &mut Scratch::new(75)), &truth);
}

fn walk(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut v = 0.0;
    (0..n)
        .map(|_| {
            v += rng.random_range(-1.0..1.0);
            v
        })
        .collect()
}

fn spread(segs: &[Segment], ctx: &Ctx) -> f64 {
    let d = members(segs, ctx, &mut Scratch::new(ctx.cfg.motif.max_len as usize));
    d.iter().map(|v| v * v).sum::<f64>() / d.len() as f64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn refinement_never_worsens_the_member_distance(
        seed in any::<u64>(),
        k in 2usize..8,
        kind in prop::sample::select(vec!["zeuclid", "euclid", "dtw"]),
    ) {
        let x = walk(2000, seed);
        let c = cfg(&[("min_len", json!(20)), ("max_len", json!(40)), ("kind", json!(kind))]);
        let ctx = Ctx::new(&x, &c);
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 3);
        let segs: Vec<Segment> = (0..k as u32)
            .map(|i| Segment { start: i * 250 + rng.random_range(0..150), len: rng.random_range(25..35) })
            .collect();
        let refined = ops::refine(&segs, &ctx, &mut Scratch::new(40));
        prop_assert!(spread(&refined, &ctx) <= spread(&segs, &ctx) + 1e-12);
    }
}
