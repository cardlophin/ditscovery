use ditscover_core::config::Config;
use ditscover_core::index::nearest_outsider;
use ditscover_core::motif::{Ctx, Scratch, Segment, dist_to_proto, prototype};
use ditscover_core::synth;
use proptest::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let o: Vec<(String, Value)> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Config::build(None, None, &o).unwrap()
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

fn clash(taken: &[Segment], g: &Segment, gap: u32) -> bool {
    taken.iter().any(|h| g.start < h.end() + gap && h.start < g.end() + gap)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn nearest_outsider_matches_brute_force(
        seed in any::<u64>(),
        len in 16u32..48,
        kind in prop::sample::select(vec!["zeuclid", "euclid"]),
        n_taken in 1usize..5,
        gap in 0u32..4,
    ) {
        let x = walk(800, seed);
        let c = cfg(&[
            ("min_len", json!(len)), ("max_len", json!(len)), ("max_stretch", json!(1.0)),
            ("kind", json!(kind)), ("min_gap", json!(gap)),
        ]);
        let ctx = Ctx::new(&x, &c);
        let mut s = Scratch::new(len as usize);
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 1);
        let mut taken: Vec<Segment> = (0..n_taken)
            .map(|_| Segment { start: rng.random_range(0..800 - len), len })
            .collect();
        taken.sort();
        let l = prototype(&taken[..1], &ctx, &mut s);
        let got = nearest_outsider(&ctx, l, &taken, &mut s);
        let (mut strided, mut all) = (f64::INFINITY, f64::INFINITY);
        let stride = (len / 8).max(1);
        for start in 0..=800 - len {
            let g = Segment { start, len };
            if clash(&taken, &g, gap) {
                continue;
            }
            let d = dist_to_proto(&g, l, &ctx, &mut s);
            all = all.min(d);
            if start % stride == 0 {
                strided = strided.min(d);
            }
        }
        match got {
            None => prop_assert!(all.is_infinite()),
            Some((d, g)) => {
                prop_assert!(!clash(&taken, &g, gap), "returned an excluded window {g:?}");
                prop_assert!((dist_to_proto(&g, l, &ctx, &mut s) - d).abs() < 1e-9);
                prop_assert!(d <= strided + 1e-9, "{d} > strided brute force {strided}");
                prop_assert!(d >= all - 1e-9, "{d} < exhaustive {all}");
            }
        }
    }
}

#[test]
fn index_is_deterministic_and_small() {
    let (x, _) = synth::planted(20_000, &[60, 80, 100], 8, 0.3, 0.1, 0);
    let c = cfg(&[("min_len", json!(40)), ("max_len", json!(135)), ("max_stretch", json!(2.0))]);
    let a = Ctx::new(&x, &c);
    let b = Ctx::new(&x, &c);
    assert!(a.index == b.index);
    assert!(a.index.bytes() <= 2_000_000, "{} bytes", a.index.bytes());
}

#[test]
fn flat_windows_are_not_indexed() {
    let mut x = walk(600, 3);
    x[100..300].fill(1.0);
    let c = cfg(&[("min_len", json!(20)), ("max_len", json!(30))]);
    let ctx = Ctx::new(&x, &c);
    for lev in &ctx.index.levels {
        for &st in &lev.starts {
            assert!(!ctx.is_flat(&Segment { start: st, len: lev.len }), "{st}+{}", lev.len);
        }
    }
}
