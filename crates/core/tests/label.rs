use ditscover_core::label::{Cand, labels, resolve, schedule};
use ditscover_core::motif::Segment;
use proptest::prelude::*;

fn seg(start: u32, len: u32) -> Segment {
    Segment { start, len }
}

fn clash(a: &Segment, b: &Segment, gap: u32) -> bool {
    a.start < b.end() + gap && b.start < a.end() + gap
}

fn brute(items: &[(Segment, f64)], gap: u32) -> f64 {
    let n = items.len();
    (0u32..1 << n)
        .filter(|mask| {
            (0..n).all(|i| {
                mask & (1 << i) == 0
                    || (i + 1..n)
                        .all(|j| mask & (1 << j) == 0 || !clash(&items[i].0, &items[j].0, gap))
            })
        })
        .map(|mask| (0..n).filter(|i| mask & (1 << i) != 0).map(|i| items[i].1).sum::<f64>())
        .fold(0.0, f64::max)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn schedule_matches_brute_force(
        raw in prop::collection::vec((0u32..200, 5u32..40, 0.01f64..5.0), 0..12),
        gap in 0u32..4,
    ) {
        let items: Vec<(Segment, f64)> = raw.iter().map(|&(s, l, w)| (seg(s, l), w)).collect();
        let picked = schedule(&items, gap);
        for (a, &i) in picked.iter().enumerate() {
            for &j in &picked[a + 1..] {
                prop_assert!(!clash(&items[i].0, &items[j].0, gap));
            }
        }
        let total: f64 = picked.iter().map(|&i| items[i].1).sum();
        prop_assert!((total - brute(&items, gap)).abs() < 1e-9, "{total} vs {}", brute(&items, gap));
    }
}

fn cand(occ: &[(u32, u32)], w: f64) -> Cand {
    Cand { occ: occ.iter().map(|&(s, l)| seg(s, l)).collect(), weights: vec![w; occ.len()] }
}

#[test]
fn starved_motifs_are_replaced_by_the_next_candidate() {
    let a = cand(&[(0, 10), (100, 10), (200, 10)], 1.0);
    let b = cand(&[(2, 10), (102, 10), (300, 10)], 0.5);
    let c = cand(&[(500, 10), (600, 10)], 0.2);
    let out = resolve(&[a, b, c], 2, 2, 0, |_, _| true);
    assert_eq!(out.iter().map(|o| o.0).collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(out[0].1, vec![seg(0, 10), seg(100, 10), seg(200, 10)]);
    let all: Vec<Segment> = out.iter().flat_map(|o| o.1.clone()).collect();
    for (i, x) in all.iter().enumerate() {
        assert!(all[i + 1..].iter().all(|y| !clash(x, y, 0)));
    }
}

#[test]
fn fewer_motifs_when_candidates_run_out() {
    let a = cand(&[(0, 10), (100, 10)], 1.0);
    let b = cand(&[(1, 10), (101, 10)], 0.9);
    let out = resolve(&[a, b], 3, 2, 0, |_, _| true);
    assert_eq!(out.len(), 1);
}

#[test]
fn trimmed_motifs_the_check_rejects_are_replaced() {
    let a = cand(&[(0, 10), (100, 10), (200, 10)], 1.0);
    let b = cand(&[(2, 10), (300, 10), (400, 10)], 0.5);
    let c = cand(&[(500, 10), (600, 10)], 0.2);
    let kept = resolve(&[a, b, c], 2, 2, 0, |_, _| true);
    assert_eq!(kept.iter().map(|o| o.0).collect::<Vec<_>>(), vec![0, 1]);
    let strict = resolve(
        &[
            cand(&[(0, 10), (100, 10), (200, 10)], 1.0),
            cand(&[(2, 10), (300, 10), (400, 10)], 0.5),
            cand(&[(500, 10), (600, 10)], 0.2),
        ],
        2,
        2,
        0,
        |c, _| c != 1,
    );
    assert_eq!(strict.iter().map(|o| o.0).collect::<Vec<_>>(), vec![0, 2]);
}

#[test]
fn labels_follow_the_occurrences() {
    let motifs = vec![vec![(2, 3), (10, 2)], vec![(6, 2)]];
    let l = labels(14, &motifs);
    assert_eq!(l, vec![-1, -1, 0, 0, 0, -1, 1, 1, -1, -1, 0, 0, -1, -1]);
}
