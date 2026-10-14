use ditscover_core::config::Config;
use ditscover_core::metrics::GroundTruth;
use ditscover_core::motif::{Ctx, Individual, Segment};
use ditscover_core::result::{RunResult, rank, shape_distance, to_motif};
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
    assert!(csv.starts_with("value,label\n") && csv.contains("NaN"));
    let loaded = RunResult::load(&dir).unwrap();
    assert!(loaded.series[7].is_nan());
    same(&res, &loaded);
    assert_eq!(loaded.labels, res.labels);
    let labels = res.labels.as_ref().expect("labels on by default");
    assert_eq!(labels.len(), res.series.len());
    for (m, motif) in res.motifs.iter().enumerate() {
        for &(s, n) in &motif.occurrences {
            assert!(labels[s as usize..(s + n) as usize].iter().all(|&l| l == m as i32));
        }
    }
}

#[test]
fn reported_occurrences_never_overlap() {
    let res = small_run(None);
    let mut occ: Vec<(u32, u32)> = res.motifs.iter().flat_map(|m| m.occurrences.clone()).collect();
    occ.sort();
    assert!(occ.windows(2).all(|w| w[0].0 + w[0].1 <= w[1].0), "{occ:?}");
}

fn graded(start: u32, margin: f64, coverage: f64) -> Individual {
    let segs = vec![Segment { start, len: 40 }, Segment { start: start + 500, len: 40 }];
    let mut i = Individual::new(segs);
    (i.raw.margin, i.raw.coverage) = (margin, coverage);
    i.raw.quality = coverage * margin.max(0.0);
    i
}

#[test]
fn quality_ranking_puts_closed_sets_first_and_auto_keeps_only_them() {
    let x: Vec<f64> =
        (0..3000).map(|i| ((i as f64) * 0.37).sin() + ((i as f64) * 0.05).cos()).collect();
    let o = [("min_len", json!(30)), ("max_len", json!(50)), ("dedupe_shape", json!(0.0))];
    let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
    let ctx = Ctx::new(&x, &cfg);
    let pool =
        [graded(0, -0.2, 0.9), graded(100, 0.3, 0.1), graded(200, 0.0, 0.5), graded(300, 0.5, 0.1)];
    assert_eq!(rank(&pool, &ctx), [3, 1, 0, 2]);
    let auto = [
        ("min_len", json!(30)),
        ("max_len", json!(50)),
        ("dedupe_shape", json!(0.0)),
        ("top_k", json!("auto")),
    ];
    let cfg = Config::build(None, None, &auto.map(|(k, v)| (k.to_string(), v))).unwrap();
    assert_eq!(rank(&pool, &Ctx::new(&x, &cfg)), [3, 1]);
}

#[test]
fn shape_dedupe_tolerates_small_lags() {
    let t: Vec<f64> =
        (0..64).map(|i| (i as f64 / 64.0 * 4.0 * std::f64::consts::PI).sin()).collect();
    let lagged: Vec<f64> =
        (0..64).map(|i| ((i + 6) as f64 / 64.0 * 4.0 * std::f64::consts::PI).sin()).collect();
    let other: Vec<f64> = (0..64).map(|i| 1.0 - (i as f64 / 32.0 - 1.0).abs()).collect();
    assert!(shape_distance(&t, &lagged) < 0.15, "{}", shape_distance(&t, &lagged));
    assert!(shape_distance(&t, &other) > 0.5, "{}", shape_distance(&t, &other));
    assert_eq!(shape_distance(&t, &t), 0.0);
}

#[test]
fn containment_and_shape_duplicates_are_dropped() {
    let (x, truth) = synth::planted(6000, &[60, 61], 8, 0.0, 0.02, 3);
    let o = [("min_len", json!(45)), ("max_len", json!(75))];
    let cfg = Config::build(None, None, &o.map(|(k, v)| (k.to_string(), v))).unwrap();
    let ctx = Ctx::new(&x, &cfg);
    let set = |occ: &[(u32, u32)], shift: u32, q: f64| {
        let mut i = Individual::new(
            occ.iter().map(|&(s, l)| Segment { start: s + shift, len: l }).collect(),
        );
        (i.raw.margin, i.raw.coverage, i.raw.quality) = (0.5, q / 0.5, q);
        i
    };
    let a = set(&truth[0][..4], 0, 0.9);
    let contained = set(&truth[0][..3], 2, 0.8);
    let same_shape = set(&truth[0][4..], 4, 0.7);
    let other = set(&truth[1][..4], 0, 0.6);
    assert_eq!(rank(&[a, contained, same_shape, other], &ctx), [0, 3]);
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
    assert!(csv.starts_with("timestamp,value,label\nt0,"), "{}", &csv[..40]);
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
