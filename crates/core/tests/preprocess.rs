use ditscover_core::config::Config;
use ditscover_core::discover;
use ditscover_core::preprocess::{Fill, Norm, Step, Trend, apply};
use ditscover_core::series::Series;
use serde_json::json;
use std::sync::atomic::AtomicBool;

const NAN: f64 = f64::NAN;

fn run(x: &[f64], steps: &[Step]) -> Series {
    apply(Series::new(x.to_vec()), steps).unwrap()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn missing_values() {
    let x = [1.0, NAN, 3.0, NAN, NAN, 6.0];
    assert!(close(&run(&x, &[Step::Nan(Fill::Interpolate)]).values, &[1., 2., 3., 4., 5., 6.]));
    assert!(close(&run(&x, &[Step::Nan(Fill::Ffill)]).values, &[1., 1., 3., 3., 3., 6.]));
    let d = run(&x, &[Step::Nan(Fill::Drop)]);
    assert!(close(&d.values, &[1., 3., 6.]));
    assert_eq!(d.origin, [0, 2, 5]);
    let lead = [NAN, 2.0, 3.0, NAN];
    assert!(close(&run(&lead, &[Step::Nan(Fill::Interpolate)]).values, &[2., 2., 3., 3.]));
    assert!(close(&run(&lead, &[Step::Nan(Fill::Ffill)]).values, &[2., 2., 3., 3.]));
}

#[test]
fn detrend_smooth_normalize() {
    let lin = run(&[1.0, 3.0, 5.0, 7.0], &[Step::Detrend(Trend::Linear)]);
    assert!(close(&lin.values, &[0.0; 4]));
    let diff = run(&[1.0, 4.0, 9.0], &[Step::Detrend(Trend::Diff)]);
    assert!(close(&diff.values, &[3.0, 5.0]));
    assert_eq!(diff.origin, [1, 2]);
    let sm = run(&[0.0, 3.0, 6.0, 9.0, 12.0], &[Step::Smooth(3)]);
    assert!(close(&sm.values, &[1.5, 3.0, 6.0, 9.0, 10.5]));
    let z = run(&[1.0, 2.0, 3.0, 4.0], &[Step::Normalize(Norm::Zscore)]);
    let mean = z.values.iter().sum::<f64>() / 4.0;
    let var = z.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / 4.0;
    assert!(mean.abs() < 1e-12 && (var - 1.0).abs() < 1e-12);
    let mm = run(&[2.0, 4.0, 6.0], &[Step::Normalize(Norm::Minmax)]);
    assert!(close(&mm.values, &[0.0, 0.5, 1.0]));
}

#[test]
fn paa_tracks_original_positions() {
    let p = run(&[1., 2., 3., 4., 5., 6., 7.], &[Step::Paa(2)]);
    assert!(close(&p.values, &[1.5, 3.5, 5.5]));
    assert_eq!((p.origin.clone(), p.scale), (vec![0, 2, 4], 2));
    assert_eq!(p.to_original(1, 2), (2, 4));
    let x: Vec<f64> = (0..40).map(|i| if i == 5 { NAN } else { i as f64 }).collect();
    let c = run(&x, &[Step::Nan(Fill::Drop), Step::Paa(4)]);
    assert_eq!(c.origin[..3], [0, 4, 9]);
    assert_eq!(c.scale, 4);
}

#[test]
fn steps_compose_in_declared_order() {
    let x = [1.0, NAN, 3.0, 5.0, 7.0, 9.0];
    let a = run(&x, &[Step::Nan(Fill::Interpolate), Step::Normalize(Norm::Minmax)]);
    assert!(close(&a.values, &[0.0, 0.125, 0.25, 0.5, 0.75, 1.0]));
}

#[test]
fn invalid_steps_fail_before_running() {
    let x = vec![1.0; 10];
    let e = apply(Series::new(x.clone()), &[Step::Smooth(20)]).unwrap_err().to_string();
    assert!(e.starts_with("preprocess[0].smooth") && e.contains("10"), "{e}");
    let e = apply(Series::new(x), &[Step::Nan(Fill::Drop), Step::Paa(11)]).unwrap_err().to_string();
    assert!(e.starts_with("preprocess[1].paa"), "{e}");
    let o = [("min_len", json!(5)), ("max_len", json!(8)), ("preprocess", json!([{"paa": 30}]))];
    let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
    let e = discover(vec![0.5; 40], &cfg, &AtomicBool::new(false)).unwrap_err().to_string();
    assert!(e.starts_with("series"), "{e}");
}
