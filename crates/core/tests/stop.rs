use ditscover_core::config::Config;
use ditscover_core::result::{RunResult, StopReason};
use ditscover_core::{discover, synth};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn run(pairs: &[(&str, Value)], cancel: &AtomicBool) -> RunResult {
    let (x, _) = synth::planted(3000, &[30], 6, 0.2, 0.1, 8);
    let mut o = vec![
        ("min_len", json!(20)),
        ("max_len", json!(40)),
        ("population", json!(40)),
        ("max_generations", json!(1_000_000_000u32)),
        ("stagnation_window", Value::Null),
    ];
    o.extend(pairs.iter().cloned());
    let o: Vec<(String, Value)> = o.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    discover(x, &Config::build(None, None, &o).unwrap(), cancel).unwrap()
}

#[test]
fn time_budget_stops_on_time() {
    let t0 = Instant::now();
    let res = run(&[("time_budget_s", json!(0.5))], &AtomicBool::new(false));
    assert_eq!(res.stop, StopReason::TimeBudget);
    assert!(t0.elapsed() < Duration::from_millis(1500), "{:?}", t0.elapsed());
    assert!(!res.motifs.is_empty());
}

#[test]
fn hypervolume_stagnation() {
    let res = run(
        &[("stagnation_window", json!(3)), ("stagnation_delta", json!(1e9))],
        &AtomicBool::new(false),
    );
    assert_eq!((res.stop, res.generations), (StopReason::Stagnation, 3));
}

#[test]
fn cancelled_before_first_generation_is_empty() {
    let res = run(&[], &AtomicBool::new(true));
    assert_eq!((res.stop, res.generations), (StopReason::Cancelled, 0));
    assert!(res.motifs.is_empty() && res.front.is_empty());
}

#[test]
fn cancelled_mid_run_returns_last_generation() {
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        flag.store(true, Ordering::Relaxed);
    });
    let t0 = Instant::now();
    let res = run(&[], &cancel);
    stopper.join().unwrap();
    assert_eq!(res.stop, StopReason::Cancelled);
    assert!(t0.elapsed() < Duration::from_millis(1300));
    assert!(res.generations > 0 && !res.motifs.is_empty());
    assert_eq!(res.history.len() as u32, res.generations + 1);
}

#[test]
fn saturated_log_channel_never_stops_the_run() {
    let dir = std::env::temp_dir().join("ditscover-stop-tests");
    let pairs = [
        ("max_generations", json!(200)),
        ("channel_capacity", json!(1)),
        ("snapshot_every", json!(1)),
        ("dir", json!(dir)),
    ];
    let res = run(&pairs, &AtomicBool::new(false));
    assert_eq!((res.stop, res.generations), (StopReason::MaxGenerations, 200));
    let delivered = res.history.len() as u64;
    assert!(delivered + res.dropped_events >= 201, "{delivered} + {}", res.dropped_events);
    assert!(delivered + res.dropped_events <= 203);
}
