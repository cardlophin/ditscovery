use ditscover_core::config::Config;
use ditscover_core::metrics::GroundTruth;
use ditscover_core::motif::{Ctx, Individual, Segment};
use ditscover_core::result::{RunResult, to_motif, top_k};
use ditscover_core::series::Series;
use ditscover_core::{discover, discover_dated, synth};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

fn small_run(dir: Option<&std::path::Path>) -> RunResult {
    let (mut x, _) = synth::planted(2000, &[30], 5, 0.2, 0.1, 4);
    x[7] = f64::NAN;
    let mut o = vec![
        ("min_len", json!(20)),
        ("max_len", json!(40)),
        ("population", json!(20)),
        ("max_generations", json!(5)),
        ("preprocess", json!([{"nan": "interpolate"}])),
    ];
    if let Some(d) = dir {
        o.push(("dir", json!(d)));
    }
    let o: Vec<(String, Value)> = o.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let cfg = Config::build(None, None, &o).unwrap();
    discover(x, &cfg, &AtomicBool::new(false)).unwrap()
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("ditscover-result-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn same(a: &RunResult, b: &RunResult) {
    assert_eq!(a.motifs, b.motifs);
    assert_eq!(a.front, b.front);
    assert_eq!(a.history, b.history);
    assert_eq!((a.stop, a.generations, a.elapsed_s), (b.stop, b.generations, b.elapsed_s));
    assert_eq!((a.dropped_events, &a.version), (b.dropped_events, &b.version));
    assert_eq!(a.config, b.config);
    assert_eq!(a.series.len(), b.series.len());
    assert!(a.series.iter().zip(&b.series).all(|(x, y)| x == y || (x.is_nan() && y.is_nan())));
}

#[test]
fn save_and_load_round_trip() {
    let res = small_run(None);
    let dir = tmp("roundtrip");
    res.save(&dir).unwrap();
    for f in ["results.json", "history.jsonl", "config.yaml", "series.csv"] {
        assert!(dir.join(f).exists(), "missing {f}");
    }
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("results.json")).unwrap()).unwrap();
    assert!(json.get("history").is_none() && json.get("series").is_none());
    assert!(json["motifs"][0]["aligned"].is_array() && json["front"][0]["aligned"].is_null());
    let csv = std::fs::read_to_string(dir.join("series.csv")).unwrap();
    assert!(csv.starts_with("value\n") && csv.contains("NaN"));
    let loaded = RunResult::load(&dir).unwrap();
    assert!(loaded.series[7].is_nan());
    same(&res, &loaded);
}

#[test]
fn output_dir_writes_the_result() {
    let dir = tmp("output-dir");
    let res = small_run(Some(&dir));
    same(&res, &RunResult::load(&dir).unwrap());
}

fn gt_config(gt: &std::path::Path, dir: &std::path::Path) -> Config {
    let o: Vec<(String, Value)> = [
        ("min_len", json!(20)),
        ("max_len", json!(40)),
        ("population", json!(20)),
        ("max_generations", json!(5)),
        ("ground_truth", json!(gt)),
        ("dir", json!(dir)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    Config::build(None, None, &o).unwrap()
}

#[test]
fn ground_truth_adds_metrics_that_survive_reload() {
    let (x, sets) = synth::planted(2000, &[30], 5, 0.2, 0.1, 4);
    let dir = tmp("metrics");
    std::fs::create_dir_all(&dir).unwrap();
    let gt = dir.join("gt.json");
    GroundTruth { name: "p".into(), series: x.clone(), sets }.save(&gt).unwrap();
    let res = discover(x, &gt_config(&gt, &dir.join("run")), &AtomicBool::new(false)).unwrap();
    let m = res.metrics.clone().expect("metrics");
    assert_eq!(m.tp + m.fn_, 5);
    assert_eq!(RunResult::load(&dir.join("run")).unwrap().metrics, res.metrics);
}

#[test]
fn missing_ground_truth_fails_before_running() {
    let (x, _) = synth::planted(2000, &[30], 5, 0.2, 0.1, 4);
    let dir = tmp("missing-gt");
    let cfg = gt_config(&dir.join("nope.json"), &dir.join("run"));
    let e = discover(x, &cfg, &AtomicBool::new(false)).unwrap_err().to_string();
    assert!(e.contains("nope.json"), "{e}");
    assert!(!dir.join("run").exists());
}

#[test]
fn results_without_metrics_still_load() {
    let res = small_run(None);
    assert!(res.metrics.is_none());
    let dir = tmp("old-format");
    res.save(&dir).unwrap();
    let path = dir.join("results.json");
    let mut json: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    json.as_object_mut().unwrap().remove("metrics");
    std::fs::write(&path, json.to_string()).unwrap();
    assert!(RunResult::load(&dir).unwrap().metrics.is_none());
}

#[test]
fn timestamps_are_the_first_column_of_series_csv() {
    let mut res = small_run(None);
    res.timestamps = Some((0..res.series.len()).map(|i| format!("t{i}")).collect());
    let dir = tmp("dated");
    res.save(&dir).unwrap();
    let csv = std::fs::read_to_string(dir.join("series.csv")).unwrap();
    assert!(csv.starts_with("timestamp,value\nt0,"), "{}", &csv[..40]);
    let back = RunResult::load(&dir).unwrap();
    assert_eq!(back.timestamps, res.timestamps);
    assert!(back.series[7].is_nan());
    let plain = small_run(None);
    plain.save(&tmp("undated")).unwrap();
    assert!(RunResult::load(&tmp_keep("undated")).unwrap().timestamps.is_none());
}

#[test]
fn timestamps_must_match_the_series() {
    let (x, _) = synth::planted(2000, &[30], 5, 0.2, 0.1, 4);
    let o: Vec<(String, Value)> =
        vec![("min_len".into(), json!(20)), ("max_len".into(), json!(40))];
    let cfg = Config::build(None, None, &o).unwrap();
    let ts = Some(vec!["2024-01-01".to_string()]);
    let e = discover_dated(x, ts, &cfg, &AtomicBool::new(false)).unwrap_err().to_string();
    assert!(e.contains("timestamps"), "{e}");
}

fn tmp_keep(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join("ditscover-result-tests").join(name)
}

fn ind(start: u32, rank: u32, obj: [f64; 3]) -> Individual {
    let segs = vec![Segment { start, len: 10 }, Segment { start: start + 20, len: 10 }];
    let mut i = Individual::new(segs);
    (i.rank, i.obj) = (rank, obj);
    i
}

#[test]
fn top_k_orders_by_rank_then_ideal_distance() {
    let pop = [
        ind(0, 1, [0.0, 0.0, 0.0]),
        ind(100, 0, [0.5, 0.5, 0.5]),
        ind(200, 0, [0.1, 0.1, 0.1]),
        ind(300, 0, [0.1, 0.1, 0.1]),
    ];
    assert_eq!(top_k(&pop, 3, 0.3), [2, 3, 1]);
    assert_eq!(top_k(&pop, 10, 0.3), [2, 3, 1, 0]);
}

#[test]
fn top_k_skips_overlapping_motifs() {
    let pop =
        [ind(0, 0, [0.1, 0.1, 0.1]), ind(2, 0, [0.2, 0.2, 0.2]), ind(100, 0, [0.3, 0.3, 0.3])];
    assert_eq!(top_k(&pop, 5, 0.3), [0, 2]);
    assert_eq!(top_k(&pop, 5, 1.0), [0, 1, 2]);
}

#[test]
fn motifs_report_original_indices() {
    let cfg =
        Config::build(None, None, &[("min_len".into(), json!(2)), ("max_len".into(), json!(4))])
            .unwrap();
    let series = Series {
        values: (0..12).map(|v| (v as f64).sin()).collect(),
        origin: (0..12).map(|i| i * 4).collect(),
        scale: 4,
    };
    let ctx = Ctx::new(&series.values, &cfg);
    let mut i = Individual::new(vec![Segment { start: 1, len: 2 }, Segment { start: 6, len: 3 }]);
    i.raw.support = 2;
    let m = to_motif(&i, &ctx, &series, true);
    assert_eq!(m.occurrences, [(4, 8), (24, 12)]);
    assert_eq!(m.support, 2);
    let aligned = m.aligned.unwrap();
    assert_eq!(aligned.len(), 2);
    assert!(aligned.iter().all(|r| r.len() == m.prototype.len()));
    assert!(to_motif(&i, &ctx, &series, false).aligned.is_none());
}
