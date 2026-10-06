use ditscover_core::metrics::{Average, GroundTruth, OffTarget, Prom, prom};

type Sets = Vec<Vec<(u32, u32)>>;

fn micro(found: &Sets, gt: &Sets) -> Prom {
    prom(found, gt, Average::Micro, OffTarget::Ignore)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-12, "{a} != {b}");
}

fn prf(p: &Prom, v: (f64, f64, f64)) {
    close(p.precision, v.0);
    close(p.recall, v.1);
    close(p.f1, v.2);
}

#[test]
fn identity_scores_one() {
    let gt = vec![vec![(0, 10), (50, 10)], vec![(100, 20), (200, 20)]];
    let p = micro(&gt, &gt);
    prf(&p, (1.0, 1.0, 1.0));
    assert_eq!((p.tp, p.fp, p.fn_), (4, 0, 0));
    assert_eq!(p.sets.iter().map(|s| s.found).collect::<Vec<_>>(), [Some(0), Some(1)]);
}

#[test]
fn half_overlap_does_not_match() {
    let gt = vec![vec![(0, 10)]];
    prf(&micro(&vec![vec![(0, 5)]], &gt), (0.0, 0.0, 0.0));
    prf(&micro(&vec![vec![(0, 6)]], &gt), (1.0, 1.0, 1.0));
}

#[test]
fn off_target_policy() {
    let gt = vec![vec![(0, 10), (50, 10)]];
    let found = vec![vec![(0, 10), (50, 10)], vec![(100, 10), (150, 10)]];
    let ignore = prom(&found, &gt, Average::Micro, OffTarget::Ignore);
    prf(&ignore, (1.0, 1.0, 1.0));
    let penalize = prom(&found, &gt, Average::Micro, OffTarget::Penalize);
    prf(&penalize, (0.5, 1.0, 2.0 / 3.0));
    assert_eq!((penalize.tp, penalize.fp), (2, 2));
}

#[test]
fn micro_and_macro_averaging() {
    let gt = vec![vec![(0, 10), (20, 10), (40, 10), (60, 10)], vec![(100, 10), (120, 10)]];
    let found = vec![vec![(0, 10), (20, 10), (40, 10), (60, 10)], vec![(100, 10), (140, 10)]];
    let m = micro(&found, &gt);
    prf(&m, (5.0 / 6.0, 5.0 / 6.0, 5.0 / 6.0));
    assert_eq!((m.tp, m.fp, m.fn_), (5, 1, 1));
    prf(&prom(&found, &gt, Average::Macro, OffTarget::Ignore), (0.75, 0.75, 0.75));
}

#[test]
fn empty_inputs_score_zero() {
    let gt = vec![vec![(0, 10)]];
    let none: Sets = Vec::new();
    for avg in [Average::Micro, Average::Macro] {
        for off in [OffTarget::Ignore, OffTarget::Penalize] {
            prf(&prom(&none, &gt, avg, off), (0.0, 0.0, 0.0));
            prf(&prom(&gt, &none, avg, off), (0.0, 0.0, 0.0));
            prf(&prom(&none, &none, avg, off), (0.0, 0.0, 0.0));
        }
    }
    let p = micro(&none, &gt);
    assert_eq!((p.tp, p.fp, p.fn_), (0, 0, 1));
    assert_eq!(p.sets[0].found, None);
}

#[test]
fn variable_length_ground_truth() {
    let gt = vec![vec![(0, 10), (30, 20), (70, 15)]];
    prf(&micro(&vec![vec![(1, 10), (31, 19), (70, 14)]], &gt), (1.0, 1.0, 1.0));
}

#[test]
fn overlapping_found_occurrences_match_once() {
    let gt = vec![vec![(0, 10), (50, 10)]];
    let p = micro(&vec![vec![(0, 10), (1, 10), (50, 10)]], &gt);
    assert_eq!((p.tp, p.fp, p.fn_), (2, 1, 0));
    prf(&p, (2.0 / 3.0, 1.0, 0.8));
}

#[test]
fn per_set_breakdown_and_assignment() {
    let gt = vec![vec![(0, 10), (20, 10)], vec![(100, 10), (120, 10), (140, 10)]];
    let found = vec![vec![(100, 10), (120, 10)], vec![(0, 10), (20, 10), (300, 10)]];
    let p = micro(&found, &gt);
    let s = &p.sets;
    assert_eq!((s[0].gt, s[0].found, s[0].tp, s[0].fp, s[0].fn_), (0, Some(1), 2, 1, 0));
    assert_eq!((s[1].gt, s[1].found, s[1].tp, s[1].fp, s[1].fn_), (1, Some(0), 2, 0, 1));
    close(s[0].precision, 2.0 / 3.0);
    close(s[1].recall, 2.0 / 3.0);
}

#[test]
fn ties_prefer_the_set_with_fewer_false_positives() {
    let gt = vec![vec![(0, 10), (20, 10)]];
    let found = vec![vec![(0, 10), (300, 10), (320, 10)], vec![(20, 10)]];
    let p = micro(&found, &gt);
    assert_eq!(p.sets[0].found, Some(1));
    prf(&p, (1.0, 0.5, 2.0 / 3.0));
}

#[test]
fn ground_truth_file_round_trip_and_validation() {
    let dir = std::env::temp_dir().join("ditscover-metrics-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("gt.json");
    let gt = GroundTruth {
        name: "t".into(),
        series: vec![0.0, f64::NAN, 2.0, 3.0, 4.0, 5.0],
        sets: vec![vec![(0, 2), (3, 2)]],
    };
    gt.save(&path).unwrap();
    let back = GroundTruth::load(&path).unwrap();
    assert!(back.series[1].is_nan());
    assert_eq!(back.sets, gt.sets);
    for (sets, key) in [
        (vec![vec![(5, 2)]], "sets[0][0]"),
        (vec![vec![(0, 3), (2, 2)]], "sets[0][1]"),
        (vec![vec![(0, 0)]], "sets[0][0]"),
    ] {
        std::fs::write(
            &path,
            serde_json::json!({"series": [0.0, 1.0, 2.0, 3.0, 4.0, 5.0], "sets": sets}).to_string(),
        )
        .unwrap();
        let e = GroundTruth::load(&path).unwrap_err().to_string();
        assert!(e.contains(key), "{e}");
    }
}
