use crate::motif::{Ctx, Scratch, Segment, dist_to_proto};
use crate::series::Stats;

pub const PAA: usize = 16;
const GRID: usize = 8;
const FILTER: usize = 32;

#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub len: u32,
    pub stride: u32,
    pub starts: Vec<u32>,
    pub sketches: Vec<[f32; PAA]>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CandidateIndex {
    pub levels: Vec<Level>,
}

fn block(j: usize, l: usize) -> (usize, usize) {
    let a = j * l / PAA;
    (a, ((j + 1) * l / PAA).max(a + 1).min(l))
}

pub fn paa(v: &[f64], out: &mut [f64; PAA]) {
    for (j, o) in out.iter_mut().enumerate() {
        let (a, b) = block(j, v.len());
        *o = v[a..b].iter().sum::<f64>() / (b - a) as f64;
    }
}

// Block-weighted RMS distance of two sketches: a lower bound of the RMS distance of the windows
// when both have length `l` (Jensen on each block).
fn lb(q: &[f64; PAA], s: &[f32; PAA], l: usize) -> f64 {
    let mut acc = 0.0;
    for j in 0..PAA {
        let (a, b) = block(j, l);
        acc += (b - a) as f64 * (q[j] - s[j] as f64).powi(2);
    }
    (acc / (0..PAA).map(|j| block(j, l)).map(|(a, b)| (b - a) as f64).sum::<f64>()).sqrt()
}

pub fn grid(lo: f64, hi: f64, n: usize) -> Vec<u32> {
    let (lo, hi) = (lo.max(2.0), hi.max(2.0).min(n as f64));
    let mut lens: Vec<u32> = (0..GRID)
        .map(|i| (lo * (hi / lo).powf(i as f64 / (GRID - 1) as f64)).round() as u32)
        .filter(|&l| l >= 2 && l as usize <= n)
        .collect();
    lens.dedup();
    lens
}

impl CandidateIndex {
    pub fn build(x: &[f64], stats: &Stats, lens: &[u32], znorm: bool, min_std: f64) -> Self {
        let mut sum = Vec::with_capacity(x.len() + 1);
        sum.push(0.0);
        x.iter().fold(0.0, |s, v| {
            sum.push(s + v);
            s + v
        });
        let levels = lens
            .iter()
            .map(|&len| {
                let (l, stride) = (len as usize, (len / 8).max(1));
                let (mut starts, mut sketches) = (Vec::new(), Vec::new());
                for st in (0..=x.len() - l).step_by(stride as usize) {
                    let (mean, std) = stats.mean_std(st, l);
                    if std < min_std {
                        continue;
                    }
                    let mut sk = [0f32; PAA];
                    for (j, o) in sk.iter_mut().enumerate() {
                        let (a, b) = block(j, l);
                        let m = (sum[st + b] - sum[st + a]) / (b - a) as f64;
                        *o = if znorm { (m - mean) / std.max(1e-12) } else { m } as f32;
                    }
                    starts.push(st as u32);
                    sketches.push(sk);
                }
                Level { len, stride, starts, sketches }
            })
            .collect();
        Self { levels }
    }

    pub fn bytes(&self) -> usize {
        self.levels.iter().map(|l| l.starts.len() * (4 + 4 * PAA)).sum()
    }

    pub fn level(&self, len: f64) -> Option<&Level> {
        self.levels.iter().min_by(|a, b| {
            (a.len as f64 - len)
                .abs()
                .total_cmp(&(b.len as f64 - len).abs())
                .then(a.len.cmp(&b.len))
        })
    }
}

// Best position in [lo, hi]: ~16 evenly spaced probes, then step 1 around the best one, so the
// cost does not grow with the motif length (exhaustive when the range is <= 16). `f` returns None
// for inadmissible positions.
pub fn coarse_fine(lo: i64, hi: i64, mut f: impl FnMut(i64) -> Option<f64>) -> Option<(f64, i64)> {
    let step = ((hi - lo) / 16).max(1);
    let mut best: Option<(f64, i64)> = None;
    let mut probe = |p: i64, best: &mut Option<(f64, i64)>| {
        if let Some(d) = f(p)
            && best.is_none_or(|b| d < b.0)
        {
            *best = Some((d, p));
        }
    };
    let mut p = lo;
    while p <= hi {
        probe(p, &mut best);
        p += step;
    }
    if let (true, Some((_, c))) = (step > 1, best) {
        for q in (c - step + 1).max(lo)..=(c + step - 1).min(hi) {
            probe(q, &mut best);
        }
    }
    best
}

pub fn clash(taken: &[Segment], g: &Segment, gap: u32) -> bool {
    taken.iter().any(|h| g.start < h.end() + gap && h.start < g.end() + gap)
}

fn scan(
    lev: &Level,
    l: usize,
    taken: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
) -> Option<(f64, Segment)> {
    let gap = ctx.motif().min_gap;
    let mut q = [0.0; PAA];
    paa(&s.proto[..l], &mut q);
    s.cand.clear();
    for (i, (&st, sk)) in lev.starts.iter().zip(&lev.sketches).enumerate() {
        if !clash(taken, &Segment { start: st, len: lev.len }, gap) {
            s.cand.push((lb(&q, sk, l) as f32, i as u32));
        }
    }
    s.cand.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let exact = lev.len as usize == l && !ctx.dist.is_dtw() && !ctx.cfg.fitness.cid;
    let mut best: Option<(f64, Segment)> = None;
    for k in 0..s.cand.len() {
        let (bound, i) = s.cand[k];
        let cut = best.map_or(f64::INFINITY, |b| b.0 * (1.0 + 1e-5) + 1e-9);
        if (exact && bound as f64 > cut) || (!exact && k >= FILTER) {
            break;
        }
        let g = Segment { start: lev.starts[i as usize], len: lev.len };
        let d = dist_to_proto(&g, l, ctx, s);
        if best.is_none_or(|b| d < b.0) {
            best = Some((d, g));
        }
    }
    let (d0, g) = best?;
    let half = (lev.stride / 2) as i64;
    let (lo, hi) =
        (g.start as i64 - half, (g.start as i64 + half).min((ctx.n() - lev.len as usize) as i64));
    let local = coarse_fine(lo.max(0), hi, |p| {
        let h = Segment { start: p as u32, len: lev.len };
        (!ctx.is_flat(&h) && !clash(taken, &h, gap)).then(|| dist_to_proto(&h, l, ctx, s))
    });
    match local {
        Some((d, p)) if d < d0 => Some((d, Segment { start: p as u32, len: lev.len })),
        _ => best,
    }
}

// Closest feasible window to the prototype in `s.proto[..l]` that does not touch `taken`, over
// lengths l·{ρ^-½, 1, ρ^½}.
pub fn nearest_outsider(
    ctx: &Ctx,
    l: usize,
    taken: &[Segment],
    s: &mut Scratch,
) -> Option<(f64, Segment)> {
    let r = ctx.motif().max_stretch.sqrt();
    let mut seen: [u32; 3] = [0; 3];
    let mut best: Option<(f64, Segment)> = None;
    for (k, f) in [1.0, 1.0 / r, r].into_iter().enumerate() {
        let Some(lev) = ctx.index.level(l as f64 * f) else { continue };
        if seen[..k].contains(&lev.len) {
            continue;
        }
        seen[k] = lev.len;
        if let Some(hit) = scan(lev, l, taken, ctx, s)
            && best.is_none_or(|b| hit.0 < b.0)
        {
            best = Some(hit);
        }
    }
    best
}
