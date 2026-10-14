use crate::config::{Config, MotifCfg};
use crate::distance::{Distance, resample, znorm};
use crate::fitness::{NullScale, pair_dist};
use crate::index::{CandidateIndex, grid};
use crate::series::Stats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Segment {
    pub start: u32,
    pub len: u32,
}

impl Segment {
    pub fn end(&self) -> u32 {
        self.start + self.len
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Raw {
    pub support: u32,
    pub cohesion: f64,
    pub mean_len: f64,
    pub stretch: f64,
    pub extent: f64,
    pub outsider: f64,
    pub scale: f64,
    pub margin: f64,
    pub coverage: f64,
    pub quality: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Individual {
    pub segs: Vec<Segment>,
    pub raw: Raw,
    pub obj: [f64; 3],
    pub rank: u32,
    pub crowd: f64,
}

impl Individual {
    pub fn new(segs: Vec<Segment>) -> Self {
        Self { segs, raw: Raw::default(), obj: [0.0; 3], rank: 0, crowd: 0.0 }
    }
}

pub struct Ctx<'a> {
    pub x: &'a [f64],
    pub stats: Stats,
    pub cfg: &'a Config,
    pub dist: Distance,
    pub min_std: f64,
    pub index: CandidateIndex,
    pub null: NullScale,
}

impl<'a> Ctx<'a> {
    pub fn new(x: &'a [f64], cfg: &'a Config) -> Self {
        let (stats, dist, m) = (Stats::new(x), cfg.distance.distance(), &cfg.motif);
        let min_std = m.min_std;
        let r = m.max_stretch.sqrt();
        let lens = grid(m.min_len as f64 / r, m.max_len as f64 * r, x.len());
        let index = CandidateIndex::build(x, &stats, &lens, dist.znorm(), min_std);
        let null = NullScale::build(x, &stats, cfg, dist, min_std);
        Self { x, stats, cfg, dist, min_std, index, null }
    }

    pub fn n(&self) -> usize {
        self.x.len()
    }

    pub fn motif(&self) -> &MotifCfg {
        &self.cfg.motif
    }

    pub fn is_flat(&self, g: &Segment) -> bool {
        self.stats.mean_std(g.start as usize, g.len as usize).1 < self.min_std
    }

    pub fn d(&self, a: &[f64], b: &[f64], row: &mut [f64]) -> f64 {
        pair_dist(self.dist, self.cfg.fitness.cid, a, b, row)
    }
}

// Per-worker buffers so evaluation never allocates (after the first call grows `cand`).
pub struct Scratch {
    pub(crate) proto: Vec<f64>,
    pub(crate) v: Vec<f64>,
    pub(crate) alt: Vec<f64>,
    pub(crate) row: Vec<f64>,
    pub(crate) cand: Vec<(f32, u32)>,
    pub(crate) rows: Vec<f64>,
    pub(crate) pair: Vec<f64>,
    pub(crate) member: Vec<f64>,
    pub(crate) sorted: Vec<f64>,
}

impl Scratch {
    pub fn new(max_len: usize) -> Self {
        let buf = vec![0.0; max_len];
        Self {
            proto: buf.clone(),
            v: buf.clone(),
            alt: buf,
            row: vec![0.0; max_len + 1],
            cand: Vec::new(),
            rows: Vec::new(),
            pair: Vec::new(),
            member: Vec::new(),
            sorted: Vec::new(),
        }
    }
}

pub fn target_len(segs: &[Segment]) -> usize {
    let total: u32 = segs.iter().map(|g| g.len).sum();
    (total as f64 / segs.len() as f64).round() as usize
}

pub(crate) fn load(g: &Segment, ctx: &Ctx, out: &mut [f64]) {
    resample(&ctx.x[g.start as usize..g.end() as usize], out);
    if ctx.dist.znorm() {
        znorm(out);
    }
}

// Occurrences are compared resampled to at most EVAL_RES points, so the cost does not grow with
// the motif length (Mallat motifs are ~1000 points).
pub const EVAL_RES: usize = 256;

pub fn eval_len(segs: &[Segment]) -> usize {
    target_len(segs).min(EVAL_RES)
}

pub fn prototype(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> usize {
    let l = eval_len(segs);
    s.proto[..l].fill(0.0);
    for g in segs {
        load(g, ctx, &mut s.v[..l]);
        s.proto[..l].iter_mut().zip(&s.v[..l]).for_each(|(p, v)| *p += v);
    }
    let k = segs.len() as f64;
    s.proto[..l].iter_mut().for_each(|p| *p /= k);
    l
}

pub fn dist_to_proto(g: &Segment, l: usize, ctx: &Ctx, s: &mut Scratch) -> f64 {
    load(g, ctx, &mut s.v[..l]);
    ctx.d(&s.v[..l], &s.proto[..l], &mut s.row)
}

pub use crate::fitness::evaluate;

pub fn align(segs: &[Segment], ctx: &Ctx) -> (Vec<f64>, Vec<Vec<f64>>) {
    let l = target_len(segs);
    let rows: Vec<Vec<f64>> = segs
        .iter()
        .map(|g| {
            let mut row = vec![0.0; l];
            load(g, ctx, &mut row);
            row
        })
        .collect();
    let k = rows.len() as f64;
    let proto = (0..l).map(|i| rows.iter().map(|r| r[i]).sum::<f64>() / k).collect();
    (proto, rows)
}

pub fn jaccard(a: &[Segment], b: &[Segment]) -> f64 {
    let (mut i, mut j, mut inter) = (0, 0, 0u64);
    while i < a.len() && j < b.len() {
        let lo = a[i].start.max(b[j].start);
        let hi = a[i].end().min(b[j].end());
        inter += hi.saturating_sub(lo) as u64;
        if a[i].end() < b[j].end() { i += 1 } else { j += 1 }
    }
    let total = |s: &[Segment]| s.iter().map(|g| g.len as u64).sum::<u64>();
    let union = total(a) + total(b) - inter;
    if union == 0 { 0.0 } else { inter as f64 / union as f64 }
}

// Overlap relative to the smaller set (sorted segments).
pub fn containment(a: &[Segment], b: &[Segment]) -> f64 {
    let (mut i, mut j, mut inter) = (0, 0, 0u64);
    while i < a.len() && j < b.len() {
        inter += a[i].end().min(b[j].end()).saturating_sub(a[i].start.max(b[j].start)) as u64;
        if a[i].end() < b[j].end() { i += 1 } else { j += 1 }
    }
    let covered = |s: &[Segment]| s.iter().map(|g| g.len as u64).sum::<u64>();
    let small = covered(a).min(covered(b));
    if small == 0 { 0.0 } else { inter as f64 / small as f64 }
}
