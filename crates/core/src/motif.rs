use crate::config::{Config, MotifCfg};
use crate::distance::{Distance, resample, znorm};
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
}

impl<'a> Ctx<'a> {
    pub fn new(x: &'a [f64], cfg: &'a Config) -> Self {
        Self { x, stats: Stats::new(x), cfg, dist: cfg.distance.distance() }
    }

    pub fn n(&self) -> usize {
        self.x.len()
    }

    pub fn motif(&self) -> &MotifCfg {
        &self.cfg.motif
    }

    pub fn is_flat(&self, g: &Segment) -> bool {
        self.stats.mean_std(g.start as usize, g.len as usize).1 < self.cfg.motif.min_std
    }
}

// Per-worker buffers so evaluation never allocates.
pub struct Scratch {
    proto: Vec<f64>,
    v: Vec<f64>,
    row: Vec<f64>,
}

impl Scratch {
    pub fn new(max_len: usize) -> Self {
        Self { proto: vec![0.0; max_len], v: vec![0.0; max_len], row: vec![0.0; max_len + 1] }
    }
}

pub fn target_len(segs: &[Segment]) -> usize {
    let total: u32 = segs.iter().map(|g| g.len).sum();
    (total as f64 / segs.len() as f64).round() as usize
}

fn load(g: &Segment, ctx: &Ctx, out: &mut [f64]) {
    resample(&ctx.x[g.start as usize..g.end() as usize], out);
    if ctx.dist.znorm() {
        znorm(out);
    }
}

pub fn prototype(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> usize {
    let l = target_len(segs);
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
    ctx.dist.dist(&s.v[..l], &s.proto[..l], &mut s.row)
}

pub fn evaluate(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> Raw {
    let l = prototype(segs, ctx, s);
    let k = segs.len() as f64;
    let cohesion = segs.iter().map(|g| dist_to_proto(g, l, ctx, s)).sum::<f64>() / k;
    let (lo, hi, sum) =
        segs.iter().fold((u32::MAX, 0, 0), |(a, b, c), g| (a.min(g.len), b.max(g.len), c + g.len));
    Raw {
        support: segs.len() as u32,
        cohesion,
        mean_len: sum as f64 / k,
        stretch: hi as f64 / lo as f64,
    }
}

pub fn objectives(raw: &Raw, m: &MotifCfg, c_ref: f64) -> [f64; 3] {
    let scaled =
        |hi: u32, lo: u32, v: f64| if hi == lo { 0.0 } else { (hi as f64 - v) / (hi - lo) as f64 };
    [
        scaled(m.max_occ, m.min_occ, raw.support as f64),
        raw.cohesion / c_ref,
        scaled(m.max_len, m.min_len, raw.mean_len),
    ]
}

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
