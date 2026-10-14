use ditscover_core::config::Config;
use ditscover_core::fitness::objectives;
use ditscover_core::motif::{Ctx, Scratch, Segment, align, evaluate, jaccard};
use serde_json::json;

fn seg(start: u32, len: u32) -> Segment {
    Segment { start, len }
}

fn cfg(kind: &str) -> Config {
    let o = [("min_len", json!(3)), ("max_len", json!(8)), ("kind", json!(kind))];
    Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

const X: [f64; 16] = [0., 2., 4., 6., 9., 9., 9., 9., 9., 9., 0., 1., 2., 3., 4., 5.];

#[test]
fn prototype_is_mean_of_resampled_occurrences() {
    let c = cfg("euclid");
    let ctx = Ctx::new(&X, &c);
    let (proto, aligned) = align(&[seg(0, 4), seg(10, 6)], &ctx);
    assert!(close(&proto, &[0.0, 1.375, 2.75, 4.125, 5.5]), "{proto:?}");
    assert!(close(&aligned[0], &[0.0, 1.5, 3.0, 4.5, 6.0]));
    assert!(close(&aligned[1], &[0.0, 1.25, 2.5, 3.75, 5.0]));
}

#[test]
fn raw_and_normalised_objectives() {
    let o = [("min_len", json!(3)), ("max_len", json!(8)), ("kind", json!("euclid"))];
    let c = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
    let ctx = Ctx::new(&X, &c);
    let raw = evaluate(&[seg(0, 4), seg(10, 6)], &ctx, &mut Scratch::new(8));
    // Two members: each one's leave-one-out prototype is the other occurrence.
    let d = (0.46875f64 * 4.0 / 5.0).sqrt();
    assert_eq!(raw.support, 2);
    assert!((raw.cohesion - d).abs() < 1e-9, "{}", raw.cohesion);
    assert!((raw.extent - d).abs() < 1e-9, "{}", raw.extent);
    assert_eq!((raw.mean_len, raw.stretch), (5.0, 1.5));
    let obj = objectives(&raw, &ctx, 2.0 * d);
    assert!(close(&obj, &[1.0, 1.0 / 3.0, 1.0 / 3.0]), "{obj:?}");
}

#[test]
fn aligned_rows_are_znormalised_for_zeuclid() {
    let c = cfg("zeuclid");
    let ctx = Ctx::new(&X, &c);
    let (proto, aligned) = align(&[seg(0, 4), seg(10, 6)], &ctx);
    assert_eq!(proto.len(), 5);
    for row in &aligned {
        let mean = row.iter().sum::<f64>() / 5.0;
        let var = row.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / 5.0;
        assert!(mean.abs() < 1e-9 && (var - 1.0).abs() < 1e-9);
    }
    let raw = evaluate(&[seg(0, 4), seg(10, 6)], &ctx, &mut Scratch::new(8));
    assert!(raw.cohesion < 1e-9, "linear ramps share the same z-normalised shape");
}

#[test]
fn coverage_jaccard() {
    let a = [seg(0, 10), seg(20, 10)];
    assert!((jaccard(&a, &[seg(5, 10)]) - 0.2).abs() < 1e-12);
    assert_eq!(jaccard(&a, &a), 1.0);
    assert_eq!(jaccard(&a, &[seg(40, 5)]), 0.0);
}

#[test]
fn scratch_reuse_does_not_change_results() {
    let c = cfg("dtw");
    let ctx = Ctx::new(&X, &c);
    let m = [seg(0, 4), seg(10, 6)];
    let fresh = evaluate(&m, &ctx, &mut Scratch::new(8));
    let mut s = Scratch::new(8);
    evaluate(&[seg(2, 8), seg(11, 5)], &ctx, &mut s);
    assert_eq!(evaluate(&m, &ctx, &mut s), fresh);
}
