use crate::config::{Error, cfg_err};
use serde::{Deserialize, Deserializer, Serialize};
use std::path::Path;

type Sets = [Vec<(u32, u32)>];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Average {
    #[default]
    Micro,
    Macro,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OffTarget {
    #[default]
    Ignore,
    Penalize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetScore {
    pub gt: usize,
    pub found: Option<usize>,
    pub tp: u32,
    pub fp: u32,
    #[serde(rename = "fn")]
    pub fn_: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prom {
    pub tp: u32,
    pub fp: u32,
    #[serde(rename = "fn")]
    pub fn_: u32,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub sets: Vec<SetScore>,
    pub average: Average,
    pub off_target: OffTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundTruth {
    #[serde(default)]
    pub name: String,
    #[serde(deserialize_with = "nan_as_null")]
    pub series: Vec<f64>,
    pub sets: Vec<Vec<(u32, u32)>>,
}

// JSON has no NaN: serde_json writes it as null, so null reads back as NaN.
fn nan_as_null<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
    let v: Vec<Option<f64>> = Deserialize::deserialize(d)?;
    Ok(v.into_iter().map(|x| x.unwrap_or(f64::NAN)).collect())
}

impl GroundTruth {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let io = |source| Error::Io { path: path.to_path_buf(), source };
        let text = std::fs::read_to_string(path).map_err(io)?;
        let gt: Self = serde_json::from_str(&text)
            .map_err(|e| Error::Data(format!("{}: {e}", path.display())))?;
        gt.validate().map_err(|e| Error::Data(format!("{}: {e}", path.display())))?;
        Ok(gt)
    }

    pub fn save(&self, path: &Path) -> Result<(), Error> {
        let text = serde_json::to_string(self).expect("ground truth serialises");
        std::fs::write(path, text).map_err(|source| Error::Io { path: path.to_path_buf(), source })
    }

    pub fn validate(&self) -> Result<(), Error> {
        let n = self.series.len() as u64;
        let mut all = Vec::new();
        for (i, set) in self.sets.iter().enumerate() {
            for (j, &(s, l)) in set.iter().enumerate() {
                let key = format!("sets[{i}][{j}]");
                if l == 0 || s as u64 + l as u64 > n {
                    return Err(cfg_err(key, format!("({s}, {l}) outside series of length {n}")));
                }
                all.push((s, l, key));
            }
        }
        all.sort();
        match all.windows(2).find(|w| w[0].0 + w[0].1 > w[1].0) {
            Some(w) => Err(cfg_err(&w[1].2, format!("overlaps {}", w[0].2))),
            None => Ok(()),
        }
    }
}

pub fn iou(a: (u32, u32), b: (u32, u32)) -> f64 {
    let inter = (a.0 + a.1).min(b.0 + b.1).saturating_sub(a.0.max(b.0)) as f64;
    inter / ((a.1 + b.1) as f64 - inter)
}

pub fn match_occurrences(found: &[(u32, u32)], planted: &[(u32, u32)]) -> (f64, f64) {
    let mut pairs: Vec<(f64, usize, usize)> = found
        .iter()
        .enumerate()
        .flat_map(|(i, &f)| planted.iter().enumerate().map(move |(j, &p)| (iou(f, p), i, j)))
        .filter(|&(v, _, _)| v >= 0.5)
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    let (mut uf, mut up) = (vec![false; found.len()], vec![false; planted.len()]);
    let mut tp = 0;
    for (_, i, j) in pairs {
        if !uf[i] && !up[j] {
            (uf[i], up[j]) = (true, true);
            tp += 1;
        }
    }
    let ratio = |d: usize| if d == 0 { 0.0 } else { tp as f64 / d as f64 };
    (ratio(planted.len()), ratio(found.len()))
}

// M[i][j]: occurrences of GT set i whose best match (overlap > 0.5, first on ties) is in set j.
fn contingency(found: &Sets, gt: &Sets) -> Vec<Vec<u32>> {
    let mut m = vec![vec![0u32; found.len()]; gt.len()];
    for (i, set) in gt.iter().enumerate() {
        for &b in set {
            let mut best = (None, 0.0);
            for (j, f) in found.iter().enumerate() {
                for &a in f {
                    let ov = iou(a, b);
                    if ov > 0.5 && ov > best.1 {
                        best = (Some(j), ov);
                    }
                }
            }
            if let Some(j) = best.0 {
                m[i][j] += 1;
            }
        }
    }
    m
}

// Port of scipy's rectangular LSAP (Crouse 2016) so that ties resolve exactly as in the
// reference PROM; minimises, `f64::INFINITY` marks forbidden pairs. Pairs sorted by row.
pub fn lsap(cost: &[Vec<f64>]) -> Vec<(usize, usize)> {
    let (nr, nc) = (cost.len(), cost.first().map_or(0, Vec::len));
    if nr == 0 || nc == 0 {
        return Vec::new();
    }
    let transpose = nc < nr;
    let c = |i: usize, j: usize| if transpose { cost[j][i] } else { cost[i][j] };
    let (nr, nc) = if transpose { (nc, nr) } else { (nr, nc) };
    let mut s = Lsap::new(nr, nc);
    for row in 0..nr {
        let sink = s.augmenting_path(row, &c);
        s.update(row, sink);
    }
    let mut pairs: Vec<(usize, usize)> =
        (0..nr).map(|i| if transpose { (s.col4row[i], i) } else { (i, s.col4row[i]) }).collect();
    pairs.sort();
    pairs
}

struct Lsap {
    u: Vec<f64>,
    v: Vec<f64>,
    spc: Vec<f64>,
    path: Vec<usize>,
    col4row: Vec<usize>,
    row4col: Vec<Option<usize>>,
    sr: Vec<bool>,
    sc: Vec<bool>,
    min_val: f64,
}

impl Lsap {
    fn new(nr: usize, nc: usize) -> Self {
        Self {
            u: vec![0.0; nr],
            v: vec![0.0; nc],
            spc: vec![f64::INFINITY; nc],
            path: vec![0; nc],
            col4row: vec![usize::MAX; nr],
            row4col: vec![None; nc],
            sr: vec![false; nr],
            sc: vec![false; nc],
            min_val: 0.0,
        }
    }

    fn augmenting_path(&mut self, row: usize, c: &impl Fn(usize, usize) -> f64) -> usize {
        let nc = self.v.len();
        let mut remaining: Vec<usize> = (0..nc).rev().collect();
        self.sr.fill(false);
        self.sc.fill(false);
        self.spc.fill(f64::INFINITY);
        let (mut i, mut min_val) = (row, 0.0);
        loop {
            self.sr[i] = true;
            let (mut index, mut lowest) = (0, f64::INFINITY);
            for (it, &j) in remaining.iter().enumerate() {
                let r = min_val + c(i, j) - self.u[i] - self.v[j];
                if r < self.spc[j] {
                    (self.path[j], self.spc[j]) = (i, r);
                }
                if self.spc[j] < lowest || (self.spc[j] == lowest && self.row4col[j].is_none()) {
                    (lowest, index) = (self.spc[j], it);
                }
            }
            assert!(lowest.is_finite(), "infeasible assignment");
            min_val = lowest;
            let j = remaining.swap_remove(index);
            self.sc[j] = true;
            match self.row4col[j] {
                None => {
                    self.min_val = min_val;
                    return j;
                }
                Some(r) => i = r,
            }
        }
    }

    fn update(&mut self, row: usize, sink: usize) {
        let m = self.min_val;
        self.u[row] += m;
        for i in (0..self.u.len()).filter(|&i| self.sr[i] && i != row) {
            self.u[i] += m - self.spc[self.col4row[i]];
        }
        for j in (0..self.v.len()).filter(|&j| self.sc[j]) {
            self.v[j] -= m - self.spc[j];
        }
        let mut j = sink;
        loop {
            let i = self.path[j];
            self.row4col[j] = Some(i);
            std::mem::swap(&mut self.col4row[i], &mut j);
            if i == row {
                break;
            }
        }
    }
}

// Optimal GT→found assignment; with more found than GT sets, rows that could take several sets
// with the same TP get the one with fewest false positives (reference `break_ties`).
fn assign(m: &[Vec<u32>], sizes: &[u32]) -> Vec<(usize, usize)> {
    let (g, d) = (m.len(), sizes.len());
    let neg: Vec<Vec<f64>> = m.iter().map(|r| r.iter().map(|&v| -(v as f64)).collect()).collect();
    let pairs = lsap(&neg);
    let tie =
        g < d && pairs.iter().any(|&(i, j)| m[i].iter().filter(|&&v| v == m[i][j]).count() > 1);
    if !tie {
        return pairs;
    }
    let mut perm: Vec<usize> = pairs.iter().map(|p| p.1).collect();
    let rest: Vec<usize> = (0..d).filter(|j| !perm.contains(j)).collect();
    perm.extend(rest);
    let cost: Vec<Vec<f64>> = pairs
        .iter()
        .map(|&(i, j)| {
            let fp = |&k: &usize| sizes[k].saturating_sub(m[i][k]) as f64;
            perm.iter().map(|k| if m[i][*k] == m[i][j] { fp(k) } else { f64::INFINITY }).collect()
        })
        .collect();
    lsap(&cost).into_iter().map(|(i, k)| (i, perm[k])).collect()
}

fn ratio(a: u32, b: u32) -> f64 {
    if b == 0 { 0.0 } else { a as f64 / b as f64 }
}

fn f1(p: f64, r: f64) -> f64 {
    if p + r == 0.0 { 0.0 } else { 2.0 * p * r / (p + r) }
}

pub fn prom(found: &Sets, gt: &Sets, average: Average, off_target: OffTarget) -> Prom {
    let m = contingency(found, gt);
    let sizes: Vec<u32> = found.iter().map(|s| s.len() as u32).collect();
    let pairs = assign(&m, &sizes);
    let sets: Vec<SetScore> = (0..gt.len())
        .map(|i| {
            let j = pairs.iter().find(|p| p.0 == i).map(|p| p.1);
            let tp = j.map_or(0, |j| m[i][j]);
            let fp = j.map_or(0, |j| sizes[j].saturating_sub(tp));
            let fn_ = (gt[i].len() as u32).saturating_sub(tp);
            let (precision, recall) = (ratio(tp, tp + fp), ratio(tp, tp + fn_));
            SetScore { gt: i, found: j, tp, fp, fn_, precision, recall, f1: f1(precision, recall) }
        })
        .collect();
    let tp: u32 = sets.iter().map(|s| s.tp).sum();
    let fn_: u32 = sets.iter().map(|s| s.fn_).sum();
    let fp = match off_target {
        OffTarget::Ignore => sets.iter().map(|s| s.fp).sum(),
        OffTarget::Penalize => sizes.iter().sum::<u32>().saturating_sub(tp),
    };
    let (g, d, k) = (gt.len(), found.len(), pairs.len());
    let (precision, recall, f) = match average {
        Average::Micro => {
            let (p, r) = (ratio(tp, tp + fp), ratio(tp, tp + fn_));
            (p, r, f1(p, r))
        }
        Average::Macro => {
            let paired = sets.iter().filter(|s| s.found.is_some());
            let sum = |f: fn(&SetScore) -> f64| paired.clone().map(f).sum::<f64>();
            let pd = if off_target == OffTarget::Penalize { d } else { k };
            let fd = if d >= g { pd } else { g };
            let div = |x: f64, n: usize| if n == 0 { 0.0 } else { x / n as f64 };
            (div(sum(|s| s.precision), pd), div(sum(|s| s.recall), g), div(sum(|s| s.f1), fd))
        }
    };
    Prom { tp, fp, fn_, precision, recall, f1: f, sets, average, off_target }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn iou_matching() {
        assert_eq!(iou((0, 10), (0, 10)), 1.0);
        assert!((iou((20, 10), (22, 10)) - 8.0 / 12.0).abs() < 1e-12);
        assert_eq!(iou((0, 10), (10, 10)), 0.0);
        let (r, p) =
            match_occurrences(&[(0, 10), (20, 10), (50, 10)], &[(0, 10), (22, 10), (100, 10)]);
        assert!((r - 2.0 / 3.0).abs() < 1e-12 && (p - 2.0 / 3.0).abs() < 1e-12);
        let (r, p) = match_occurrences(&[(0, 10), (1, 10)], &[(0, 10)]);
        assert_eq!((r, p), (1.0, 0.5));
        assert_eq!(match_occurrences(&[(0, 10)], &[(6, 10)]), (0.0, 0.0));
        assert_eq!(match_occurrences(&[], &[(6, 10)]), (0.0, 0.0));
    }

    fn brute(m: &[Vec<u32>], row: usize, used: &mut Vec<bool>) -> u32 {
        if row == m.len() {
            return 0;
        }
        let free = used.iter().filter(|u| !**u).count();
        let mut best = if m.len() - row > free { brute(m, row + 1, used) } else { 0 };
        for j in 0..used.len() {
            if !used[j] {
                used[j] = true;
                best = best.max(m[row][j] + brute(m, row + 1, used));
                used[j] = false;
            }
        }
        best
    }

    fn sets(max_sets: usize) -> impl Strategy<Value = Vec<Vec<(u32, u32)>>> {
        prop::collection::vec(prop::collection::vec((0u32..20, 5u32..20), 1..5), 0..max_sets)
            .prop_map(|v| v.into_iter().map(|s| s.into_iter().map(|(p, l)| (p * 20, l)).collect()))
            .prop_map(|v| v.collect())
    }

    fn disjoint(v: Vec<Vec<(u32, u32)>>) -> Vec<Vec<(u32, u32)>> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for s in v {
            let s: Vec<(u32, u32)> =
                s.into_iter().filter(|o| seen.insert(o.0)).map(|(p, l)| (p, l.min(19))).collect();
            if !s.is_empty() {
                out.push(s);
            }
        }
        out
    }

    proptest! {
        #[test]
        fn lsap_is_optimal(m in prop::collection::vec(prop::collection::vec(0u32..5, 1..7), 1..7)) {
            let d = m.iter().map(Vec::len).min().unwrap();
            let m: Vec<Vec<u32>> = m.into_iter().map(|r| r[..d].to_vec()).collect();
            let neg: Vec<Vec<f64>> =
                m.iter().map(|r| r.iter().map(|&v| -(v as f64)).collect()).collect();
            let pairs = lsap(&neg);
            prop_assert_eq!(pairs.len(), m.len().min(d));
            let total: u32 = pairs.iter().map(|&(i, j)| m[i][j]).sum();
            prop_assert_eq!(total, brute(&m, 0, &mut vec![false; d]));
        }

        #[test]
        fn prom_is_bounded(gt in sets(5), found in sets(6), macro_ in any::<bool>(), pen in any::<bool>()) {
            let gt = disjoint(gt);
            let avg = if macro_ { Average::Macro } else { Average::Micro };
            let off = if pen { OffTarget::Penalize } else { OffTarget::Ignore };
            let p = prom(&found, &gt, avg, off);
            for v in [p.precision, p.recall, p.f1] {
                prop_assert!((0.0..=1.0).contains(&v));
            }
            prop_assert_eq!(p.tp + p.fn_, gt.iter().map(|s| s.len() as u32).sum::<u32>());
        }
    }
}
