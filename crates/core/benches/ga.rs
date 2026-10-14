use criterion::{Criterion, criterion_group, criterion_main};
use ditscover_core::config::Config;
use ditscover_core::distance::Distance;
use ditscover_core::motif::{Ctx, Scratch, Segment, evaluate};
use ditscover_core::{discover, synth};
use serde_json::{Value, json};
use std::hint::black_box;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

fn cfg(pairs: &[(&str, Value)]) -> Config {
    let mut o = vec![
        ("min_len", json!(40)),
        ("max_len", json!(135)),
        ("max_stretch", json!(2.0)),
        ("stagnation_window", Value::Null),
    ];
    o.extend(pairs.iter().cloned());
    let o: Vec<(String, Value)> = o.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    Config::build(None, None, &o).unwrap()
}

fn ga(c: &mut Criterion) {
    let (x, _) = synth::planted(10_000, &[60, 80, 100], 8, 0.3, 0.1, 0);
    let run = |cfg: &Config| discover(x.clone(), cfg, &AtomicBool::new(false)).unwrap();
    let mut g = c.benchmark_group("ga");
    g.sample_size(10).measurement_time(Duration::from_secs(20));
    let full = cfg(&[]);
    g.bench_function("run_10k", |b| b.iter(|| run(&full)));
    let short = cfg(&[("max_generations", json!(60))]);
    for threads in [1, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        g.bench_function(format!("threads_{threads}"), |b| b.iter(|| pool.install(|| run(&short))));
    }
    let dir = std::env::temp_dir().join("ditscover-bench");
    let on =
        cfg(&[("max_generations", json!(60)), ("snapshot_every", json!(1)), ("dir", json!(dir))]);
    let off = cfg(&[("max_generations", json!(60)), ("history", json!(false))]);
    g.bench_function("log_on", |b| b.iter(|| run(&on)));
    g.bench_function("log_off", |b| b.iter(|| run(&off)));
    g.finish();
}

// SC-005: library defaults (stagnation, min_generations, all v2 components) at 5k and 20k points.
fn v2(c: &mut Criterion) {
    let mut g = c.benchmark_group("ga");
    g.sample_size(10).measurement_time(Duration::from_secs(30));
    for (name, n) in [("v2_defaults_5k", 5_000), ("v2_defaults_20k", 20_000)] {
        let (x, _) = synth::planted(n, &[60, 80, 100], 8, 0.3, 0.1, 0);
        let o = [("min_len", json!(40)), ("max_len", json!(135)), ("max_stretch", json!(2.0))];
        let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
        g.bench_function(name, |b| b.iter(|| discover(x.clone(), &cfg, &AtomicBool::new(false))));
    }
    g.finish();
}

fn distance(c: &mut Criterion) {
    let (x, truth) = synth::planted(10_000, &[100], 20, 0.3, 0.1, 1);
    let segs: Vec<Segment> = truth[0].iter().map(|&(start, len)| Segment { start, len }).collect();
    let mut g = c.benchmark_group("distance");
    for (name, kind) in [("zeuclid", "zeuclid"), ("dtw", "dtw")] {
        let c = cfg(&[("kind", json!(kind))]);
        let ctx = Ctx::new(&x, &c);
        let mut s = Scratch::new(135);
        g.bench_function(name, |b| b.iter(|| evaluate(black_box(&segs), &ctx, &mut s)));
    }
    let (a, b) = (&x[..100], &x[100..200]);
    let mut row = vec![0.0; 101];
    g.bench_function("dtw_band_0.1", |bn| {
        bn.iter(|| Distance::Dtw { band: 0.1 }.dist(black_box(a), black_box(b), &mut row))
    });
    g.finish();
}

criterion_group!(benches, ga, v2, distance);
criterion_main!(benches);
