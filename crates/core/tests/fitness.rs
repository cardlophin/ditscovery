use ditscover_core::config::Config;
use ditscover_core::fitness::{members, objectives};
use ditscover_core::index::nearest_outsider;
use ditscover_core::motif::{Ctx, Raw, Scratch, Segment, dist_to_proto, evaluate, prototype};
use ditscover_core::synth;
use proptest::prelude::*;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{Value, json};

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let o: Vec<(String, Value)> = pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    Config::build(None, None, &o).unwrap()
}

fn seg(start: u32, len: u32) -> Segment {
    Segment { start, len }
}

fn planted() -> (Vec<f64>, Vec<Segment>) {
    let (x, truth) = synth::planted(3000, &[50], 6, 0.0, 0.05, 4);
    (x, truth[0].iter().map(|&(s, l)| seg(s, l)).collect())
}

fn base() -> Config {
    cfg(&[("min_len", json!(40)), ("max_len", json!(60)), ("max_stretch", json!(1.2))])
}

fn eval(segs: &[Segment], ctx: &Ctx) -> (Raw, [f64; 3]) {
    let mut s = Scratch::new(ctx.cfg.motif.max_len as usize);
    let mut sorted = segs.to_vec();
    sorted.sort();
    let raw = evaluate(&sorted, ctx, &mut s);
    let obj = objectives(&raw, ctx, 1.0);
    (raw, obj)
}

// A background window of the motif length that touches no occurrence.
fn junk(truth: &[Segment], n: u32) -> Segment {
    (0..n - 50)
        .step_by(37)
        .map(|st| seg(st, 50))
        .find(|g| truth.iter().all(|h| g.end() + 5 < h.start || h.end() + 5 < g.start))
        .unwrap()
}

#[test]
fn true_set_outranks_set_plus_junk() {
    let (x, truth) = planted();
    let c = base();
    let ctx = Ctx::new(&x, &c);
    let (raw, _) = eval(&truth, &ctx);
    assert!(raw.margin > 0.0, "planted set is closed: {raw:?}");
    let mut more = truth.clone();
    more.push(junk(&truth, 3000));
    let (j, _) = eval(&more, &ctx);
    assert!(j.extent > raw.extent && j.margin < raw.margin, "{j:?} vs {raw:?}");
    assert!(j.quality < raw.quality, "{j:?} vs {raw:?}");
}

#[test]
fn missing_occurrence_lowers_the_margin() {
    let (x, truth) = planted();
    let c = base();
    let ctx = Ctx::new(&x, &c);
    let (full, _) = eval(&truth, &ctx);
    for i in 0..truth.len() {
        let mut less = truth.clone();
        less.remove(i);
        let (raw, _) = eval(&less, &ctx);
        assert!(raw.margin < full.margin, "without {i}: {} >= {}", raw.margin, full.margin);
    }
}

#[test]
fn tiling_has_no_quality() {
    let (x, truth) = planted();
    let c = base();
    let ctx = Ctx::new(&x, &c);
    let (t, _) = eval(&truth, &ctx);
    let tiling: Vec<Segment> = (0..20).map(|i| seg(i * 150, 60)).collect();
    let (raw, o) = eval(&tiling, &ctx);
    assert!(raw.margin <= 0.0 && raw.quality == 0.0 && t.quality > 0.0, "{raw:?}");
    assert!(o[1] > 0.0 && raw.extent > t.extent, "{o:?}");
}

#[test]
fn two_occurrences_are_not_halved() {
    let (x, truth) = planted();
    let c = cfg(&[("min_len", json!(40)), ("max_len", json!(60)), ("kind", json!("euclid"))]);
    let ctx = Ctx::new(&x, &c);
    let pair = [truth[0], truth[3]];
    let mut s = Scratch::new(60);
    let l = prototype(&pair[..1], &ctx, &mut s);
    let direct = dist_to_proto(&pair[1], l, &ctx, &mut s);
    let (raw, _) = eval(&pair, &ctx);
    assert!((raw.extent - direct).abs() < 1e-9, "{} vs {direct}", raw.extent);
    assert!((raw.cohesion - direct).abs() < 1e-9);
}

#[test]
fn flat_and_noise_windows_cannot_form_motifs() {
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let mut x: Vec<f64> = (0..3000).map(|_| rng.random_range(-1.0..1.0)).collect();
    x[1000..1400].iter_mut().for_each(|v| *v *= 0.01);
    let strict = cfg(&[("min_len", json!(40)), ("max_len", json!(60)), ("min_std", json!(0.05))]);
    let ctx = Ctx::new(&x, &strict);
    assert!(ctx.is_flat(&seg(1100, 50)));
    assert!(!Ctx::new(&x, &base()).is_flat(&seg(1100, 50)));
    let noise: Vec<Segment> = (0..6).map(|i| seg(100 + i * 400, 50)).collect();
    let (raw, _) = eval(&noise, &ctx);
    assert!(raw.margin <= 0.0 && raw.quality == 0.0, "{raw:?}");
}

#[test]
fn objectives_are_normalised() {
    let (x, _) = planted();
    let c = base();
    let ctx = Ctx::new(&x, &c);
    let mut rng = ChaCha8Rng::seed_from_u64(9);
    for _ in 0..50 {
        let k = rng.random_range(2..10u32);
        let segs: Vec<Segment> = (0..k)
            .map(|i| seg(i * 300 + rng.random_range(0..200), rng.random_range(40..=60)))
            .collect();
        let (raw, o) = eval(&segs, &ctx);
        assert!(o.iter().all(|v| (0.0..=1.0).contains(v)), "{o:?} {raw:?}");
        assert!(raw.quality >= 0.0 && raw.coverage > 0.0 && raw.coverage <= 1.0);
    }
}

#[test]
fn margin_is_zero_without_outsiders() {
    let x: Vec<f64> = (0..100).map(|i| (i as f64 * 0.37).sin() + (i as f64 * 0.11).cos()).collect();
    let c = cfg(&[("min_len", json!(40)), ("max_len", json!(40)), ("max_stretch", json!(1.0))]);
    let ctx = Ctx::new(&x, &c);
    let (raw, _) = eval(&[seg(10, 40), seg(55, 40)], &ctx);
    assert_eq!(raw.margin, 0.0);
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

fn clash(a: &Segment, b: &Segment) -> bool {
    a.start < b.end() && b.start < a.end()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // Lengths <= 15 give an exhaustive (stride 1) outsider search, so the bound is exact.
    #[test]
    fn far_additions_never_improve_extent_or_margin(
        seed in any::<u64>(),
        len in 10u32..16,
        k in 2usize..9,
        kind in prop::sample::select(vec!["zeuclid", "euclid"]),
    ) {
        let x = walk(1200, seed);
        let c = cfg(&[
            ("min_len", json!(len)), ("max_len", json!(len)), ("max_stretch", json!(1.0)),
            ("kind", json!(kind)), ("max_occ", json!(20)),
        ]);
        let ctx = Ctx::new(&x, &c);
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 7);
        let mut set: Vec<Segment> = Vec::new();
        while set.len() < k {
            let g = seg(rng.random_range(0..1200 - len), len);
            if set.iter().all(|h| !clash(h, &g)) {
                set.push(g);
            }
        }
        set.sort();
        let (raw, _) = eval(&set, &ctx);
        let mut s = Scratch::new(len as usize);
        let l = prototype(&set, &ctx, &mut s);
        let outsider = nearest_outsider(&ctx, l, &set, &mut s).map(|o| o.1);
        let need = raw.extent * (k as f64 + 1.0) / k as f64;
        let far = (0..1200 - len).map(|st| seg(st, len)).find(|g| {
            set.iter().all(|h| !clash(h, g))
                && outsider.is_none_or(|o| !clash(&o, g))
                && dist_to_proto(g, l, &ctx, &mut s) > need
        });
        if let Some(w) = far {
            let mut more = set.clone();
            more.push(w);
            let (after, _) = eval(&more, &ctx);
            prop_assert!(after.extent >= raw.extent - 1e-12, "extent improved {raw:?} -> {after:?}");
            let gap = |r: &Raw| r.outsider - r.extent;
            prop_assert!(gap(&after) <= gap(&raw) + 1e-12, "margin improved {raw:?} -> {after:?}");
        }
    }
}

#[test]
fn loo_member_distances_match_brute_force() {
    let (x, truth) = planted();
    for kind in ["zeuclid", "euclid"] {
        let c = cfg(&[("min_len", json!(40)), ("max_len", json!(60)), ("kind", json!(kind))]);
        let ctx = Ctx::new(&x, &c);
        let mut s = Scratch::new(60);
        let set = &truth[..4];
        let got = members(set, &ctx, &mut s);
        for (i, g) in set.iter().enumerate() {
            let others: Vec<Segment> =
                set.iter().enumerate().filter(|p| p.0 != i).map(|p| *p.1).collect();
            let l = prototype(&others, &ctx, &mut s);
            let want = dist_to_proto(g, l, &ctx, &mut s);
            assert!((got[i] - want).abs() < 1e-9, "{kind} member {i}: {} vs {want}", got[i]);
        }
    }
}

#[test]
fn dtw_kinds_use_a_medoid_and_two_members_are_symmetric() {
    let (x, truth) = planted();
    for kind in ["dtw", "zdtw"] {
        let c = cfg(&[("min_len", json!(40)), ("max_len", json!(60)), ("kind", json!(kind))]);
        let ctx = Ctx::new(&x, &c);
        let mut s = Scratch::new(60);
        let two = members(&truth[..2], &ctx, &mut s);
        assert!((two[0] - two[1]).abs() < 1e-12, "{kind}: {two:?}");
        let many = members(&truth, &ctx, &mut s);
        assert_eq!(many.len(), truth.len());
        assert!(many.iter().all(|d| d.is_finite() && *d >= 0.0));
    }
}

#[test]
fn zdtw_ignores_level_and_amplitude() {
    let t: Vec<f64> = (0..60).map(|i| (i as f64 / 6.0).sin()).collect();
    let mut x = walk(2000, 3).iter().map(|v| v * 0.05).collect::<Vec<_>>();
    for (k, st) in [100usize, 600, 1100, 1600].into_iter().enumerate() {
        let (lvl, amp) = (k as f64 * 3.0, 1.0 + k as f64);
        x[st..st + 60].iter_mut().zip(&t).for_each(|(v, s)| *v = lvl + amp * s);
    }
    let set: Vec<Segment> = [100, 600, 1100, 1600].map(|st| seg(st, 60)).to_vec();
    let ext = |kind: &str| {
        let c = cfg(&[("min_len", json!(50)), ("max_len", json!(70)), ("kind", json!(kind))]);
        eval(&set, &Ctx::new(&x, &c)).0.extent
    };
    assert!(ext("zdtw") < 1e-9, "{}", ext("zdtw"));
    assert!(ext("dtw") > 0.5, "{}", ext("dtw"));
}
