use ditscover_core::bench::{BenchConfig, DatasetSpec, K, Split, TsmdName};
use serde_json::{Value, json};

fn example() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../specs/002-benchmark-metrics/contracts");
    std::fs::read_to_string(format!("{path}/bench.example.yaml")).unwrap()
}

fn build(yaml: &str, overrides: &[(&str, Value)]) -> Result<BenchConfig, String> {
    let o: Vec<(String, Value)> =
        overrides.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    BenchConfig::build(Some(yaml), None, &o).map_err(|e| e.to_string())
}

const MINIMAL: &str = "datasets: [{synthetic: {name: s}}]\nmethods: [{method: ditscover}]\n";

#[test]
fn example_parses() {
    let c = build(&example(), &[]).unwrap();
    assert_eq!(c.datasets.len(), 4);
    assert_eq!(
        c.methods.iter().map(|m| m.label.as_deref().unwrap()).collect::<Vec<_>>(),
        ["ditscover", "locomotif", "stomp"]
    );
    assert_eq!(c.k, K::Oracle);
    let DatasetSpec::Yfinance(y) = &c.datasets[3] else { panic!("yfinance") };
    assert_eq!((y.k, y.min_len, y.max_len), (5, 8, 26));
    let DatasetSpec::Tsmd(t) = &c.datasets[1] else { panic!("tsmd") };
    assert_eq!(t.name, TsmdName::ECG5000);
    assert_eq!(c.scaling.unwrap().lengths, [1000, 2000, 5000, 10000, 20000]);
}

#[test]
fn defaults_are_filled() {
    let c = build(MINIMAL, &[]).unwrap();
    assert_eq!((c.seeds.clone(), c.timeout_s, c.jobs, c.threads), (vec![0, 1, 2], 600.0, 1, None));
    let DatasetSpec::Synthetic(s) = &c.datasets[0] else { panic!("synthetic") };
    assert_eq!((s.n_series, s.n, s.lens.clone(), s.occ), (20, 5000, vec![40, 60, 80], 6));
    assert!(c.scaling.is_none());
    let yaml = c.to_yaml();
    assert_eq!(build(&yaml, &[]).unwrap(), c);
}

#[test]
fn errors_carry_key_path() {
    let ds = |d: &str| format!("datasets: [{d}]\nmethods: [{{method: ditscover}}]\n");
    let cases = [
        (format!("{MINIMAL}foo: 1\n"), "foo"),
        ("datasets: []\nmethods: [{method: ditscover}]\n".into(), "datasets"),
        ("datasets: [{synthetic: {name: s}}]\nmethods: []\n".into(), "methods"),
        (
            "datasets: [{synthetic: {name: s}}]\nmethods: [{method: ditscover}, {method: ditscover}]\n"
                .into(),
            "methods[1].label",
        ),
        (ds("{synthetic: {name: s}}, {synthetic: {name: s}}"), "datasets[1].name"),
        (ds("{synthetic: {}}"), "datasets[0].name"),
        (ds("{tsmd: {name: ECG9}}"), "ECG9"),
        (ds("{synthetic: {name: s, bogus: 1}}"), "bogus"),
        (format!("{MINIMAL}scaling: {{lengths: [2000, 1000]}}\n"), "scaling.lengths"),
        (format!("{MINIMAL}scaling: {{lengths: [1000], methods: [x]}}\n"), "scaling.methods"),
        (ds("{yfinance: {name: y, tickers: [A], period: 1y}}"), "datasets[0].min_len"),
        (ds("{yfinance: {name: y, tickers: [A], period: 1y, min_len: 5, max_len: 9, k: 0}}"), "datasets[0].k"),
        (format!("{MINIMAL}k: 0\n"), "k"),
        (format!("{MINIMAL}timeout_s: 0\n"), "timeout_s"),
    ];
    for (yaml, key) in cases {
        let e = build(&yaml, &[]).unwrap_err();
        assert!(e.contains(key), "{key}: {e}");
    }
}

#[test]
fn tuning_fields_parse_and_validate() {
    let yaml = "datasets: [{synthetic: {name: s, split: validation}}, {synthetic: {name: s}}, \
                {tsmd: {name: ECG5000, split: validation, n_series: 30}}, \
                {tsmd: {name: ECG5000, n_series: 30}}]\n\
                methods: [{method: locomotif, grid: {rho: [0.5, 0.6], highpass: [false, true]}}]\n\
                tune: {n_series: 4}\nmax_runs: 500\nstats: false\nsave_sets: false\n";
    let c = build(yaml, &[]).unwrap();
    let t = c.tune.clone().unwrap();
    assert_eq!((t.n_series, t.metric.as_str()), (4, "f1"));
    assert_eq!((c.max_runs, c.stats, c.save_sets), (500, false, false));
    assert_eq!(c.methods[0].grid["rho"], json!([0.5, 0.6]));
    let DatasetSpec::Synthetic(s) = &c.datasets[0] else { panic!("synthetic") };
    assert_eq!(s.split, Split::Validation);
    let DatasetSpec::Synthetic(s) = &c.datasets[1] else { panic!("synthetic") };
    assert_eq!(s.split, Split::Test);
    assert_eq!(build(&c.to_yaml(), &[]).unwrap(), c);
    let d = build(MINIMAL, &[]).unwrap();
    assert_eq!((d.tune, d.max_runs, d.stats, d.save_sets), (None, 20_000, true, true));
    assert!(d.methods[0].grid.is_empty());
}

#[test]
fn tuning_errors_carry_key_path() {
    let ds = |d: &str| format!("datasets: [{d}]\nmethods: [{{method: ditscover}}]\n");
    let grid = "datasets: [{synthetic: {name: s}}]\nmethods: [{method: stomp, grid: {max_matches: [10]}}]\n";
    let cases = [
        (ds("{synthetic: {name: s}}, {synthetic: {name: s, split: test}}"), "datasets[1].name"),
        (ds("{tsmd: {name: Fungi}}, {tsmd: {name: Fungi, split: test}}"), "datasets[1].name"),
        (grid.into(), "methods[0].grid"),
        (format!("{grid}tune: {{n_series: 2}}\nmax_runs: 0\n"), "max_runs"),
        (format!("{grid}tune: {{n_series: 0}}\n"), "tune.n_series"),
        (format!("{grid}tune: {{n_series: 2, metric: auc}}\n"), "tune.metric"),
        (
            "datasets: [{synthetic: {name: s}}]\nmethods: [{method: stomp, grid: {max_matches: []}}]\ntune: {n_series: 2}\n"
                .into(),
            "methods[0].grid.max_matches",
        ),
    ];
    for (yaml, key) in cases {
        let e = build(&yaml, &[]).unwrap_err();
        assert!(e.contains(key), "{key}: {e}");
    }
}

#[test]
fn bare_override_sets_every_matching_leaf() {
    let c = build(
        &example(),
        &[("n_series", json!(2)), ("seeds", json!([0])), ("scaling", Value::Null), ("k", json!(3))],
    )
    .unwrap();
    for d in &c.datasets {
        match d {
            DatasetSpec::Synthetic(s) => assert_eq!(s.n_series, 2),
            DatasetSpec::Tsmd(t) => assert_eq!(t.n_series, 2),
            DatasetSpec::Yfinance(y) => assert_eq!(y.k, 3),
            DatasetSpec::Files(_) => {}
        }
    }
    assert_eq!((c.seeds, c.k, c.scaling), (vec![0], K::Fixed(3), None));
    assert!(build(&example(), &[("nope", json!(1))]).unwrap_err().contains("nope"));
}
