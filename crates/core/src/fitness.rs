use crate::config::Config;
use crate::distance::{Distance, cid_factor, znorm};
use crate::index::nearest_outsider;
use crate::motif::{Ctx, Raw, Scratch, Segment, eval_len, load, prototype};
use crate::series::Stats;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

const NULL_STREAM: u64 = 0x6e75_6c6c_7363_616c;

pub fn pair_dist(dist: Distance, cid: bool, a: &[f64], b: &[f64], row: &mut [f64]) -> f64 {
    let d = dist.dist(a, b, row);
    if cid { d * cid_factor(a, b) } else { d }
}

fn mean_loo(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> usize {
    let l = prototype(segs, ctx, s);
    let k = segs.len() as f64;
    let loo = ctx.cfg.fitness.loo && segs.len() > 1;
    for (i, g) in segs.iter().enumerate() {
        load(g, ctx, &mut s.v[..l]);
        if loo {
            for j in 0..l {
                s.alt[j] = (k * s.proto[j] - s.v[j]) / (k - 1.0);
            }
        }
        let p = if loo { &s.alt[..l] } else { &s.proto[..l] };
        s.member[i] = ctx.d(&s.v[..l], p, &mut s.row);
    }
    l
}

// Occurrences resampled to the target length in `s.rows`, distances in `s.pair` (k × k).
pub fn pairwise(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> (usize, usize) {
    let (k, l) = (segs.len(), eval_len(segs));
    s.rows.resize(k * l, 0.0);
    s.pair.resize(k * k, 0.0);
    for (i, g) in segs.iter().enumerate() {
        load(g, ctx, &mut s.rows[i * l..(i + 1) * l]);
    }
    for i in 0..k {
        s.pair[i * k + i] = 0.0;
        for j in i + 1..k {
            let d = ctx.d(&s.rows[i * l..(i + 1) * l], &s.rows[j * l..(j + 1) * l], &mut s.row);
            (s.pair[i * k + j], s.pair[j * k + i]) = (d, d);
        }
    }
    (k, l)
}

// DTW kinds: the medoid is the prototype.
fn medoid(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> usize {
    let (k, l) = pairwise(segs, ctx, s);
    let total = |i: usize| s.pair[i * k..(i + 1) * k].iter().sum::<f64>();
    let m = (0..k).min_by(|&a, &b| total(a).total_cmp(&total(b)).then(a.cmp(&b))).unwrap_or(0);
    let loo = ctx.cfg.fitness.loo && k > 1;
    for i in 0..k {
        s.member[i] = match (i == m, loo) {
            (false, _) => s.pair[i * k + m],
            (true, true) => {
                (0..k).filter(|&j| j != m).map(|j| s.pair[m * k + j]).fold(f64::INFINITY, f64::min)
            }
            (true, false) => 0.0,
        };
    }
    s.proto[..l].copy_from_slice(&s.rows[m * l..(m + 1) * l]);
    l
}

// Member distances in `s.member[..k]` (leave-one-out unless disabled); the prototype stays in
// `s.proto[..l]`. Returns l.
pub fn member_distances(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> usize {
    s.member.resize(segs.len(), 0.0);
    if ctx.dist.is_dtw() { medoid(segs, ctx, s) } else { mean_loo(segs, ctx, s) }
}

pub fn members(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> Vec<f64> {
    member_distances(segs, ctx, s);
    s.member[..segs.len()].to_vec()
}

// Ranking score: closed (margin), tight (extent well below the random-pair scale) and covering
// (square root): linear coverage ranked a loose tiling of 10 instance-aligned windows (74 % of a
// Plane series) above the planted 3-occurrence sets.
pub fn quality(coverage: f64, margin: f64, rel_extent: f64) -> f64 {
    coverage.powf(0.5) * margin.max(0.0) * (1.0 - rel_extent).max(0.0)
}

// (target length, extent); the prototype stays in `s.proto`.
pub fn extent_of(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> (usize, f64) {
    let k = segs.len();
    let l = member_distances(segs, ctx, s);
    s.sorted.clear();
    s.sorted.extend_from_slice(&s.member[..k]);
    s.sorted.sort_by(f64::total_cmp);
    let rank = ((ctx.cfg.fitness.extent_q * k as f64).ceil() as usize).clamp(1, k);
    (l, s.sorted[rank - 1])
}

pub fn evaluate(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> Raw {
    let k = segs.len();
    let (l, extent) = extent_of(segs, ctx, s);
    let outsider = nearest_outsider(ctx, l, segs, s).map_or(extent, |o| o.0);
    let scale = ctx.null.at(l as f64);
    let margin = (outsider - extent) / outsider.max(extent).max(1e-12);
    let total: u32 = segs.iter().map(|g| g.len).sum();
    let (lo, hi) = segs.iter().fold((u32::MAX, 0), |(a, b), g| (a.min(g.len), b.max(g.len)));
    let coverage = total as f64 / ctx.n() as f64;
    Raw {
        support: k as u32,
        cohesion: s.member[..k].iter().sum::<f64>() / k as f64,
        mean_len: total as f64 / k as f64,
        stretch: hi as f64 / lo as f64,
        extent,
        outsider,
        scale,
        margin,
        coverage,
        quality: quality(coverage, margin, extent / scale),
    }
}

// Support, extent and cohesion, the last two relative to the initial population's worst cohesion.
// The margin, coverage and null-scale objectives lost the ablation (research R8); margin and δ
// still drive ranking, labelling and the guarded operators.
pub fn objectives(raw: &Raw, ctx: &Ctx, c_ref: f64) -> [f64; 3] {
    let m = ctx.motif();
    let support = match m.max_occ == m.min_occ {
        true => 0.0,
        false => (m.max_occ as f64 - raw.support as f64) / (m.max_occ - m.min_occ) as f64,
    };
    let unit = |v: f64| v.clamp(0.0, 1.5) / 1.5;
    [support, unit(raw.extent / c_ref), unit(raw.cohesion / c_ref)]
}

// Typical distance between two random non-overlapping feasible windows, per length.
#[derive(Debug, Clone, PartialEq)]
pub struct NullScale {
    pub lens: Vec<u32>,
    pub delta: Vec<f64>,
}

impl NullScale {
    pub fn build(x: &[f64], stats: &Stats, cfg: &Config, dist: Distance, min_std: f64) -> Self {
        let m = &cfg.motif;
        let n = x.len();
        let mut lens: Vec<u32> = (0..8)
            .map(|i| m.min_len as f64 + (m.max_len - m.min_len) as f64 * i as f64 / 7.0)
            .map(|l| (l.round() as u32).clamp(1, n.max(1) as u32))
            .collect();
        lens.dedup();
        let fallback =
            if dist.znorm() { 2f64.sqrt() } else { stats.mean_std(0, n).1 * 2f64.sqrt() };
        let mut rng = ChaCha8Rng::seed_from_u64(cfg.seed ^ NULL_STREAM);
        let delta = lens
            .iter()
            .map(|&len| {
                let l = len as usize;
                let (mut a, mut b, mut row) = (vec![0.0; l], vec![0.0; l], vec![0.0; l + 1]);
                let mut ds = Vec::new();
                let want = cfg.fitness.null_pairs as usize;
                for _ in 0..8 * want {
                    if ds.len() == want || 2 * l > n {
                        break;
                    }
                    let (i, j) = (rng.random_range(0..=n - l), rng.random_range(0..=n - l));
                    let flat = |s: usize| stats.mean_std(s, l).1 < min_std;
                    if i.abs_diff(j) < l || flat(i) || flat(j) {
                        continue;
                    }
                    for (buf, s) in [(&mut a, i), (&mut b, j)] {
                        buf.copy_from_slice(&x[s..s + l]);
                        if dist.znorm() {
                            znorm(buf);
                        }
                    }
                    ds.push(pair_dist(dist, cfg.fitness.cid, &a, &b, &mut row));
                }
                ds.sort_by(f64::total_cmp);
                let d = if ds.is_empty() { fallback } else { ds[ds.len() / 2] };
                d.max(1e-12)
            })
            .collect();
        Self { lens, delta }
    }

    pub fn at(&self, l: f64) -> f64 {
        let k = self.lens.partition_point(|&g| (g as f64) < l);
        if k == 0 {
            return self.delta[0];
        }
        if k == self.lens.len() {
            return self.delta[k - 1];
        }
        let (l0, l1) = (self.lens[k - 1] as f64, self.lens[k] as f64);
        let t = (l - l0) / (l1 - l0);
        self.delta[k - 1] + t * (self.delta[k] - self.delta[k - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn setup(kind: &str) -> (Vec<f64>, Config) {
        let (x, _) = crate::synth::planted(4000, &[40, 70], 6, 0.2, 0.1, 5);
        let o: Vec<(String, Value)> = [
            ("min_len", json!(30)),
            ("max_len", json!(90)),
            ("kind", json!(kind)),
            ("seed", json!(3)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        (x, Config::build(None, None, &o).unwrap())
    }

    #[test]
    fn null_scale_is_deterministic_positive_and_interpolates() {
        for kind in ["zeuclid", "euclid", "dtw", "zdtw"] {
            let (x, c) = setup(kind);
            let st = Stats::new(&x);
            let d = c.distance.distance();
            let a = NullScale::build(&x, &st, &c, d, 1e-6);
            assert_eq!(a, NullScale::build(&x, &st, &c, d, 1e-6), "{kind}");
            assert_eq!(a.lens.len(), 8);
            assert!(a.delta.iter().all(|&v| v > 0.0 && v.is_finite()), "{kind}: {:?}", a.delta);
            for w in 0..7 {
                let (l0, l1) = (a.lens[w] as f64, a.lens[w + 1] as f64);
                assert_eq!(a.at(l0), a.delta[w]);
                let mid = a.at((l0 + l1) / 2.0);
                let (lo, hi) = (a.delta[w].min(a.delta[w + 1]), a.delta[w].max(a.delta[w + 1]));
                assert!(mid >= lo - 1e-12 && mid <= hi + 1e-12, "{kind} {mid} not in [{lo}, {hi}]");
            }
            assert_eq!(a.at(1.0), a.delta[0]);
            assert_eq!(a.at(1e6), a.delta[7]);
        }
    }

    #[test]
    fn too_short_series_falls_back() {
        let (_, c) = setup("zeuclid");
        let x: Vec<f64> = (0..100).map(|i| (i as f64 * 0.3).sin()).collect();
        let a = NullScale::build(&x, &Stats::new(&x), &c, c.distance.distance(), 1e-6);
        assert_eq!(*a.delta.last().unwrap(), 2f64.sqrt());
    }
}
