use ditscover_core::config::{Column, Config, Kind, TopK, validate_len};
use ditscover_core::preprocess::{Fill, Step};
use serde_json::{Value, json};

fn ov(pairs: &[(&str, Value)]) -> Vec<(String, Value)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

fn base() -> Vec<(String, Value)> {
    ov(&[("min_len", json!(10)), ("max_len", json!(20))])
}

fn err(yaml: Option<&str>, pairs: &[(&str, Value)]) -> String {
    Config::build(yaml, None, &ov(pairs)).unwrap_err().to_string()
}

#[test]
fn defaults() {
    let c = Config::default();
    assert_eq!(c.seed, 0);
    assert_eq!(c.data.path, None);
    assert_eq!(c.data.column, Column::Index(0));
    assert_eq!(c.data.delimiter, ',');
    assert!(c.data.header);
    assert!(c.preprocess.is_empty());
    assert_eq!((c.motif.min_len, c.motif.max_len), (0, 0));
    assert_eq!(c.motif.max_stretch, 1.5);
    assert_eq!((c.motif.min_occ, c.motif.max_occ, c.motif.min_gap), (2, 20, 0));
    assert_eq!(c.motif.min_std, 1e-6);
    assert_eq!(c.distance.kind, Kind::Zeuclid);
    assert_eq!(c.distance.band, 0.1);
    assert_eq!(c.ga.population, 200);
    assert_eq!(
        [c.ga.crossover, c.ga.shift, c.ga.stretch, c.ga.add, c.ga.remove],
        [0.9, 0.3, 0.2, 0.2, 0.1]
    );
    assert_eq!([c.ga.sigma_shift, c.ga.sigma_stretch], [0.1, 0.1]);
    assert_eq!(c.ga.scan_step, 0.125);
    assert_eq!([c.ga.init_greedy, c.ga.niche_jaccard], [0.5, 0.5]);
    assert_eq!(c.stop.max_generations, 300);
    assert_eq!(c.stop.time_budget_s, None);
    assert_eq!(c.stop.stagnation_window, Some(50));
    assert_eq!(c.stop.stagnation_delta, 1e-3);
    assert_eq!(c.output.dir, None);
    assert_eq!((c.output.top_k, c.output.snapshot_every), (TopK::N(5), 10));
    assert_eq!(c.log.level, "info");
    assert_eq!((c.log.log_every, c.log.channel_capacity), (10, 1024));
    assert!(c.log.history);
}

#[test]
fn motif_quality_defaults() {
    let c = Config::default();
    let f = &c.fitness;
    assert_eq!((f.extent_q, f.null_pairs, f.cid, f.loo), (0.9, 256, false, true));
    let g = &c.ga;
    assert_eq!([g.refine, g.refine_radius, g.seed_nn], [0.3, 0.1, 0.4]);
    assert!(g.guarded_add);
    assert_eq!(c.stop.min_generations, 100);
    let o = &c.output;
    assert_eq!((o.dedupe_overlap, o.dedupe_shape), (0.5, 0.3));
    assert!(o.labels);
}

#[test]
fn motif_quality_overrides_and_validation() {
    let mut o = base();
    o.extend(ov(&[
        ("refine", json!(0)),
        ("top_k", json!("auto")),
        ("kind", json!("zdtw")),
        ("loo", json!(false)),
        ("guarded_add", json!(false)),
    ]));
    let c = Config::build(None, None, &o).unwrap();
    assert_eq!(c.ga.refine, 0.0);
    assert_eq!(c.output.top_k, TopK::Auto);
    assert_eq!(c.distance.kind, Kind::Zdtw);
    assert!(!c.fitness.loo && !c.ga.guarded_add);
    assert_eq!(Config::build(Some(&c.to_yaml()), None, &[]).unwrap(), c);
    for removed in ["model", "rank_by", "dedupe_jaccard", "min_std_rel", "archive", "split"] {
        let e = err(None, &[("min_len", json!(10)), ("max_len", json!(20)), (removed, json!(0))]);
        assert!(e.contains("unknown parameter"), "{removed}: {e}");
    }
    let cases = [
        ("extent_q", json!(0.0), "fitness.extent_q"),
        ("null_pairs", json!(8), "fitness.null_pairs"),
        ("refine", json!(1.5), "ga.refine"),
        ("refine_radius", json!(0.0), "ga.refine_radius"),
        ("seed_nn", json!(1.1), "ga.seed_nn"),
        ("dedupe_overlap", json!(1.5), "output.dedupe_overlap"),
        ("dedupe_shape", json!(-0.5), "output.dedupe_shape"),
        ("top_k", json!("many"), "output.top_k"),
    ];
    for (k, v, path) in cases {
        let e = err(None, &[("min_len", json!(10)), ("max_len", json!(20)), (k, v)]);
        assert!(e.starts_with(path) || e.contains(k), "{k}: {e}");
    }
    let e = err(None, &[("min_len", json!(10)), ("max_len", json!(20)), ("refine", json!(1.5))]);
    assert!(e.starts_with("ga.refine"), "{e}");
}

#[test]
fn unknown_yaml_key_is_named() {
    let e = err(Some("motif: {min_len: 5, max_len: 9, foo: 1}"), &[]);
    assert!(e.contains("foo"), "{e}");
}

#[test]
fn validation_errors_carry_key_path() {
    let e = err(None, &[]);
    assert!(e.starts_with("motif.min_len"), "{e}");
    let e = err(None, &[("min_len", json!(12)), ("max_len", json!(5))]);
    assert!(e.starts_with("motif.max_len") && e.contains("12"), "{e}");
    let cases = [
        ("population", json!(7), "ga.population"),
        ("crossover", json!(1.5), "ga.crossover"),
        ("band", json!(0.0), "distance.band"),
        ("max_stretch", json!(0.5), "motif.max_stretch"),
        ("min_occ", json!(1), "motif.min_occ"),
        ("top_k", json!(0), "output.top_k"),
        ("level", json!("loud"), "log.level"),
        ("preprocess", json!([{"smooth": 1}]), "preprocess[0].smooth"),
    ];
    for (k, v, path) in cases {
        let e = err(None, &[("min_len", json!(10)), ("max_len", json!(20)), (k, v)]);
        assert!(e.starts_with(path), "{k}: {e}");
    }
}

#[test]
fn set_leaf_by_unique_name() {
    let mut o = base();
    o.extend(ov(&[
        ("dir", json!("out")),
        ("column", json!("IPG")),
        ("kind", json!("dtw")),
        ("preprocess", json!([{"nan": "drop"}, {"paa": 2}])),
        ("time_budget_s", json!(5.0)),
    ]));
    let c = Config::build(None, None, &o).unwrap();
    assert_eq!((c.motif.min_len, c.motif.max_len), (10, 20));
    assert_eq!(c.output.dir.as_deref(), Some(std::path::Path::new("out")));
    assert_eq!(c.data.column, Column::Name("IPG".into()));
    assert_eq!(c.distance.kind, Kind::Dtw);
    assert_eq!(c.preprocess, vec![Step::Nan(Fill::Drop), Step::Paa(2)]);
    assert_eq!(c.stop.time_budget_s, Some(5.0));
    let e = err(None, &[("nope", json!(1))]);
    assert!(e.contains("nope"), "{e}");
    let e = err(None, &[("population", json!("many"))]);
    assert!(e.contains("population"), "{e}");
}

#[test]
fn precedence_defaults_yaml_overrides() {
    let yaml = "motif: {min_len: 5, max_len: 9}\nga: {population: 100, scan_step: 0.25}\n";
    let c = Config::build(Some(yaml), None, &ov(&[("population", json!(300))])).unwrap();
    assert_eq!(c.ga.population, 300);
    assert_eq!(c.ga.scan_step, 0.25);
    assert_eq!(c.ga.crossover, 0.9);
    assert_eq!(c.motif.min_len, 5);
}

#[test]
fn base_dict_and_yaml_roundtrip() {
    let c =
        Config::build(None, Some(json!({"seed": 7, "motif": {"min_len": 4, "max_len": 8}})), &[])
            .unwrap();
    assert_eq!(c.seed, 7);
    let again = Config::build(Some(&c.to_yaml()), None, &[]).unwrap();
    assert_eq!(again, c);
    let yaml = "motif: {min_len: 4, max_len: 8}\npreprocess:\n  - nan: drop\n  - smooth: 3\n";
    let c = Config::build(Some(yaml), None, &[]).unwrap();
    assert_eq!(c.preprocess, vec![Step::Nan(Fill::Drop), Step::Smooth(3)]);
    assert_eq!(Config::build(Some(&c.to_yaml()), None, &[]).unwrap(), c);
    let e = Config::build(None, Some(json!({"motif": {"oops": 1}})), &[]).unwrap_err();
    assert!(e.to_string().contains("oops"));
}

#[test]
fn market_data_fields_and_validation() {
    let d = Config::default().data;
    assert_eq!((d.ticker, d.start, d.end, d.period), (None, None, None, None));
    assert_eq!((d.interval.as_str(), d.field.as_str(), d.refresh), ("1d", "Close", false));
    assert_eq!(d.time_column, None);
    let ok = |pairs: &[(&str, Value)]| {
        let mut all = base();
        all.extend(ov(pairs));
        Config::build(None, None, &all)
    };
    let c = ok(&[("ticker", json!("AAPL")), ("interval", json!("1wk")), ("period", json!("5y"))]);
    let c = c.unwrap();
    assert_eq!((c.data.ticker.as_deref(), c.data.interval.as_str()), (Some("AAPL"), "1wk"));
    for (pairs, key) in [
        (vec![("ticker", json!("A")), ("path", json!("x.csv"))], "data.ticker"),
        (
            vec![("ticker", json!("A")), ("period", json!("1y")), ("start", json!("2020-01-01"))],
            "data.period",
        ),
        (vec![("ticker", json!("A")), ("interval", json!("2d"))], "data.interval"),
        (vec![("ticker", json!("A")), ("start", json!("2020-13-01"))], "data.start"),
        (
            vec![
                ("ticker", json!("A")),
                ("start", json!("2020-01-01")),
                ("end", json!("01/02/2020")),
            ],
            "data.end",
        ),
        (vec![("ticker", json!("A")), ("period", json!("7y"))], "data.period"),
    ] {
        let e = ok(&pairs).unwrap_err().to_string();
        assert!(e.contains(key), "{key}: {e}");
    }
}

#[test]
fn ticker_without_values_points_to_python() {
    let mut o = base();
    o.push(("ticker".into(), json!("AAPL")));
    let c = Config::build(None, None, &o).unwrap();
    let e = ditscover_core::load_data(&c).unwrap_err().to_string();
    assert!(e.contains("ditscover[finance]"), "{e}");
}

#[test]
fn series_length_check() {
    let c = Config::build(None, None, &base()).unwrap();
    let e = validate_len(19, &c).unwrap_err().to_string();
    assert!(e.contains("19") && e.contains("min_len"), "{e}");
    assert!(validate_len(20, &c).is_ok());
}
