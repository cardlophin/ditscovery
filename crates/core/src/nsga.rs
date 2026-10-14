use crate::motif::{Individual, jaccard};
use rand::Rng;
use std::cmp::Ordering;
use std::collections::HashSet;

fn dominates(a: &[f64; 3], b: &[f64; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| x <= y) && a.iter().zip(b).any(|(x, y)| x < y)
}

pub fn fronts(objs: &[[f64; 3]]) -> Vec<Vec<usize>> {
    let n = objs.len();
    let mut count = vec![0usize; n];
    let mut beats: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            if dominates(&objs[i], &objs[j]) {
                beats[i].push(j);
                count[j] += 1;
            } else if dominates(&objs[j], &objs[i]) {
                beats[j].push(i);
                count[i] += 1;
            }
        }
    }
    let mut current: Vec<usize> = (0..n).filter(|&i| count[i] == 0).collect();
    let mut out = Vec::new();
    while !current.is_empty() {
        let mut next = Vec::new();
        for &i in &current {
            for &j in &beats[i] {
                count[j] -= 1;
                if count[j] == 0 {
                    next.push(j);
                }
            }
        }
        next.sort_unstable();
        out.push(std::mem::replace(&mut current, next));
    }
    out
}

pub fn crowding(objs: &[[f64; 3]], front: &[usize]) -> Vec<f64> {
    let m = front.len();
    if m <= 2 {
        return vec![f64::INFINITY; m];
    }
    let mut d = vec![0.0; m];
    let mut idx: Vec<usize> = (0..m).collect();
    for k in [0, 1, 2] {
        let v = |i: usize| objs[front[i]][k];
        idx.sort_by(|&a, &b| v(a).total_cmp(&v(b)).then(a.cmp(&b)));
        let span = v(idx[m - 1]) - v(idx[0]);
        if span <= 0.0 {
            continue;
        }
        d[idx[0]] = f64::INFINITY;
        d[idx[m - 1]] = f64::INFINITY;
        for w in 1..m - 1 {
            d[idx[w]] += (v(idx[w + 1]) - v(idx[w - 1])) / span;
        }
    }
    d
}

fn area2d(points: &mut [[f64; 3]], r: f64) -> f64 {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let (mut area, mut y) = (0.0, r);
    for p in points.iter() {
        if p[1] < y {
            area += (r - p[0]) * (y - p[1]);
            y = p[1];
        }
    }
    area
}

// Exact 3D hypervolume (minimisation) w.r.t. the cube corner (r, r, r), by slicing along f3.
pub fn hv3d(points: &[[f64; 3]], r: f64) -> f64 {
    let mut pts: Vec<[f64; 3]> =
        points.iter().filter(|p| p.iter().all(|&v| v < r)).copied().collect();
    pts.sort_by(|a, b| a[2].total_cmp(&b[2]));
    let mut slice = Vec::with_capacity(pts.len());
    let mut vol = 0.0;
    for i in 0..pts.len() {
        slice.push(pts[i]);
        let dz = pts.get(i + 1).map_or(r, |p| p[2]) - pts[i][2];
        if dz > 0.0 {
            vol += area2d(&mut slice, r) * dz;
        }
    }
    vol
}

pub fn better(a: &Individual, b: &Individual) -> Ordering {
    a.rank.cmp(&b.rank).then(b.crowd.total_cmp(&a.crowd))
}

pub fn tournament(pop: &[Individual], rng: &mut impl Rng) -> usize {
    let (i, j) = (rng.random_range(0..pop.len()), rng.random_range(0..pop.len()));
    if better(&pop[j], &pop[i]) == Ordering::Less { j } else { i }
}

// NSGA-II survival with clearing: motifs overlapping a better one by more than `niche`
// (coverage Jaccard) and exact genotype duplicates are pushed behind every distinct motif.
pub fn survive(pool: Vec<Individual>, n: usize, niche: f64) -> Vec<Individual> {
    let mut seen = HashSet::new();
    let (mut uniq, mut dups): (Vec<Individual>, Vec<Individual>) =
        pool.into_iter().partition(|i| seen.insert(i.segs.clone()));
    let objs: Vec<[f64; 3]> = uniq.iter().map(|i| i.obj).collect();
    let fr = fronts(&objs);
    for (r, f) in fr.iter().enumerate() {
        for (&i, c) in f.iter().zip(crowding(&objs, f)) {
            (uniq[i].rank, uniq[i].crowd) = (r as u32, c);
        }
    }
    let mut order: Vec<usize> = (0..uniq.len()).collect();
    order.sort_by(|&a, &b| better(&uniq[a], &uniq[b]).then(a.cmp(&b)));
    let (mut winners, mut cleared) = (Vec::new(), Vec::new());
    for i in order {
        if winners.iter().any(|&w: &usize| jaccard(&uniq[i].segs, &uniq[w].segs) > niche) {
            cleared.push(i);
        } else {
            winners.push(i);
        }
    }
    let last = fr.len() as u32;
    cleared.iter().for_each(|&i| uniq[i].rank = last);
    dups.iter_mut().for_each(|d| d.rank = last + 1);
    let mut slots: Vec<Option<Individual>> = uniq.into_iter().map(Some).collect();
    winners.into_iter().chain(cleared).filter_map(|i| slots[i].take()).chain(dups).take(n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::motif::Segment;

    fn ind(segs: &[(u32, u32)], obj: [f64; 3]) -> Individual {
        let mut i =
            Individual::new(segs.iter().map(|&(start, len)| Segment { start, len }).collect());
        i.obj = obj;
        i
    }

    #[test]
    fn non_dominated_fronts() {
        let pts = [[0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 1.0]];
        assert_eq!(fronts(&pts), vec![vec![0], vec![2, 3], vec![1]]);
    }

    #[test]
    fn crowding_extremes_are_infinite() {
        let pts = [[0.0, 1.0, 0.5], [0.5, 0.5, 0.5], [1.0, 0.0, 0.5]];
        let c = crowding(&pts, &[0, 1, 2]);
        assert!(c[0].is_infinite() && c[2].is_infinite());
        assert!((c[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn hv3d_matches_grid_count() {
        let pts =
            [[0.0, 3.0, 2.0], [1.0, 1.0, 3.0], [2.0, 0.0, 1.0], [3.0, 2.0, 0.0], [4.0, 4.0, 4.0]];
        let mut cells = 0;
        for a in 0..5 {
            for b in 0..5 {
                for c in 0..5 {
                    let cell = [a as f64, b as f64, c as f64];
                    if pts.iter().any(|p| (0..3).all(|i| p[i] <= cell[i])) {
                        cells += 1;
                    }
                }
            }
        }
        assert_eq!(hv3d(&pts, 5.0), cells as f64);
        assert_eq!(hv3d(&[], 5.0), 0.0);
    }

    #[test]
    fn clearing_relegates_overlapping_motifs() {
        let a = ind(&[(0, 10), (20, 10)], [0.0, 0.5, 0.5]);
        let b = ind(&[(1, 10), (21, 10)], [0.5, 0.0, 0.5]);
        let c = ind(&[(50, 10), (70, 10)], [1.0, 1.0, 1.0]);
        let kept = survive(vec![a.clone(), b.clone(), c.clone()], 2, 0.5);
        assert_eq!(
            kept.iter().map(|i| i.segs.clone()).collect::<Vec<_>>(),
            [a.segs.clone(), c.segs.clone()]
        );
        let all = survive(vec![a, b.clone(), c], 3, 0.5);
        assert_eq!(all[2].segs, b.segs);
        assert!(all[2].rank > all[1].rank);
    }

    #[test]
    fn exact_duplicates_go_last() {
        let a = ind(&[(0, 10), (20, 10)], [0.0, 0.0, 0.0]);
        let c = ind(&[(50, 10), (70, 10)], [1.0, 1.0, 1.0]);
        let kept = survive(vec![a.clone(), a.clone(), c.clone()], 3, 1.0);
        assert_eq!(kept[1].segs, c.segs);
        assert_eq!(kept[2].segs, a.segs);
    }
}
