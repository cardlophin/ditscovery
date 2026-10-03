use ditscover_core::config::Config;
use ditscover_core::motif::{Ctx, Scratch, Segment};
use ditscover_core::ops;
use proptest::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::json;

const N: usize = 3000;

fn series(seed: u64, flat: bool) -> Vec<f64> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut v = 0.0;
    let mut x: Vec<f64> = (0..N)
        .map(|_| {
            v += rng.random_range(-1.0..1.0);
            v
        })
        .collect();
    if flat {
        x[500..1200].fill(3.0);
    }
    x
}

fn config(min_len: u32, span: u32, rho: f64, min_occ: u32, span_occ: u32, gap: u32) -> Config {
    let o = [
        ("min_len", json!(min_len)),
        ("max_len", json!(min_len + span)),
        ("max_stretch", json!(rho)),
        ("min_occ", json!(min_occ)),
        ("max_occ", json!(min_occ + span_occ)),
        ("min_gap", json!(gap)),
        ("scan_step", json!(0.5)),
    ];
    Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap()
}

fn check(segs: &[Segment], ctx: &Ctx, cfg: &Config) -> Result<(), TestCaseError> {
    let m = &cfg.motif;
    let k = segs.len() as u32;
    prop_assert!(k >= m.min_occ && k <= m.max_occ, "k = {k}");
    for s in segs {
        prop_assert!(s.len >= m.min_len && s.len <= m.max_len, "len {s:?}");
        prop_assert!(s.end() as usize <= N, "out of series {s:?}");
        prop_assert!(
            ctx.stats.mean_std(s.start as usize, s.len as usize).1 >= m.min_std,
            "flat {s:?}"
        );
    }
    for w in segs.windows(2) {
        prop_assert!(w[0].end() + m.min_gap <= w[1].start, "overlap {w:?}");
    }
    let (lo, hi) = segs.iter().fold((u32::MAX, 0), |(a, b), s| (a.min(s.len), b.max(s.len)));
    prop_assert!(hi as f64 / lo as f64 <= m.max_stretch + 1e-9, "stretch {hi}/{lo}");
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn operators_keep_invariants(
        seed in any::<u64>(),
        min_len in 5u32..30,
        span in 0u32..40,
        rho in 1.0f64..2.5,
        min_occ in 2u32..4,
        span_occ in 0u32..10,
        gap in 0u32..5,
        flat in any::<bool>(),
    ) {
        let x = series(seed, flat);
        let cfg = config(min_len, span, rho, min_occ, span_occ, gap);
        let ctx = Ctx::new(&x, &cfg);
        let mut s = Scratch::new(cfg.motif.max_len as usize);
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0x5eed);
        let a = ops::init(true, &ctx, &mut s, &mut rng).expect("seeded init");
        check(&a, &ctx, &cfg)?;
        let b = ops::init(false, &ctx, &mut s, &mut rng).expect("random init");
        check(&b, &ctx, &cfg)?;
        let (c, d) = ops::crossover(&a, &b, &ctx, &mut s, &mut rng);
        check(&c, &ctx, &cfg)?;
        check(&d, &ctx, &cfg)?;
        let ga = &cfg.ga;
        check(&ops::shift(&a, ga, &ctx, &mut s, &mut rng), &ctx, &cfg)?;
        check(&ops::stretch(&a, ga, &ctx, &mut s, &mut rng), &ctx, &cfg)?;
        check(&ops::add(&a, &ctx, &mut s, &mut rng), &ctx, &cfg)?;
        check(&ops::remove(&a, &ctx, &mut s, &mut rng), &ctx, &cfg)?;
        let (e, f) = ops::vary(&a, &b, ga, &ctx, &mut s, &mut rng);
        check(&e, &ctx, &cfg)?;
        check(&f, &ctx, &cfg)?;
        let garbage: Vec<Segment> = (0..rng.random_range(0..30))
            .map(|_| Segment { start: rng.random_range(0..N as u32 + 50), len: rng.random_range(0..200) })
            .collect();
        if let Some(r) = ops::repair(garbage, &a, &ctx, &mut s, &mut rng) {
            check(&r, &ctx, &cfg)?;
        }
    }
}
