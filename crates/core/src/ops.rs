use crate::config::Ga;
use crate::fitness::{extent_of, member_distances};
use crate::index::{Level, PAA, clash, coarse_fine, nearest_outsider, paa};
use crate::motif::{Ctx, Scratch, Segment, dist_to_proto, evaluate, load, prototype, target_len};
use rand::Rng;
use rand_distr::{Distribution, Normal};
use rayon::prelude::*;

const INIT_ATTEMPTS: usize = 20;
const ADD_ATTEMPTS: usize = 3;
const REFINE_PASSES: usize = 4;
// Refinement compares occurrences resampled to at most this many points.
const REFINE_RES: usize = 128;
const NN_QUERIES: usize = 256;

fn fit(start: i64, len: i64, ctx: &Ctx) -> Segment {
    let m = ctx.motif();
    let n = ctx.n() as i64;
    let len = len.clamp(m.min_len as i64, (m.max_len as i64).min(n));
    Segment { start: start.clamp(0, n - len) as u32, len: len as u32 }
}

fn resize(g: Segment, len: i64, ctx: &Ctx) -> Segment {
    let centre2 = 2 * g.start as i64 + g.len as i64;
    let len = len.clamp(1, ctx.n() as i64);
    fit((centre2 - len) / 2, len, ctx)
}

fn clamp_stretch(segs: &mut [Segment], ctx: &Ctx) {
    let mut lens: Vec<u32> = segs.iter().map(|g| g.len).collect();
    lens.sort_unstable();
    let (m, r) = (lens[lens.len() / 2] as f64, ctx.motif().max_stretch.sqrt());
    let lo = (m / r - 1e-9).ceil() as i64;
    let hi = (m * r + 1e-9).floor() as i64;
    for g in segs.iter_mut() {
        let len = (g.len as i64).clamp(lo, hi);
        if len != g.len as i64 {
            *g = resize(*g, len, ctx);
        }
    }
}

fn overlaps(segs: &[Segment], g: &Segment, gap: u32) -> bool {
    segs.iter().any(|h| g.start < h.end() + gap && h.start < g.end() + gap)
}

fn worst(segs: &[Segment], l: usize, ctx: &Ctx, s: &mut Scratch) -> usize {
    let d: Vec<f64> = segs.iter().map(|g| dist_to_proto(g, l, ctx, s)).collect();
    (0..segs.len()).max_by(|&a, &b| d[a].total_cmp(&d[b]).then(b.cmp(&a))).unwrap_or(0)
}

fn scan_step(len: u32, ctx: &Ctx) -> u32 {
    ((len as f64 * ctx.cfg.ga.scan_step).round() as u32).max(1)
}

fn consider(
    g: Segment,
    l: usize,
    taken: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    best: &mut Option<(f64, Segment)>,
) {
    if ctx.is_flat(&g) || overlaps(taken, &g, ctx.motif().min_gap) {
        return;
    }
    let d = dist_to_proto(&g, l, ctx, s);
    if best.is_none_or(|(bd, _)| d < bd) {
        *best = Some((d, g));
    }
}

// Window of length `len` closest to the prototype in `s` (length `l`), not touching `taken`:
// strided scan of the whole series, then a step-1 search around the best hit.
fn best_match(
    len: u32,
    l: usize,
    taken: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Option<(f64, Segment)> {
    let (n, step) = (ctx.n() as u32, scan_step(len, ctx));
    let mut best = None;
    for start in (rng.random_range(0..step)..=n - len).step_by(step as usize) {
        consider(Segment { start, len }, l, taken, ctx, s, &mut best);
    }
    let (_, g) = best?;
    for start in g.start.saturating_sub(step / 2)..=(g.start + step / 2).min(n - len) {
        consider(Segment { start, len }, l, taken, ctx, s, &mut best);
    }
    best
}

fn mean_len(segs: &[Segment]) -> f64 {
    segs.iter().map(|g| g.len as f64).sum::<f64>() / segs.len() as f64
}

// Inserts the best match of the prototype in `s`, if it is no worse than `limit`.
fn try_add(
    segs: &mut Vec<Segment>,
    l: usize,
    limit: Option<f64>,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> bool {
    let m = ctx.motif();
    let len = if segs.is_empty() { l as f64 } else { mean_len(segs) }.round() as u32;
    let len = len.clamp(m.min_len, m.max_len);
    if len as usize > ctx.n() {
        return false;
    }
    match best_match(len, l, segs, ctx, s, rng) {
        Some((d, g)) if limit.is_none_or(|lim| d <= lim) => {
            let at = segs.partition_point(|h| h.start < g.start);
            segs.insert(at, g);
            true
        }
        _ => false,
    }
}

// Sorted sweep; of two conflicting segments the one closer to the prototype stays.
fn resolve_overlaps(segs: &[Segment], l: usize, ctx: &Ctx, s: &mut Scratch) -> Vec<(Segment, f64)> {
    let mut scored: Vec<(Segment, f64)> =
        segs.iter().map(|&g| (g, dist_to_proto(&g, l, ctx, s))).collect();
    scored.sort_by_key(|&(g, _)| g);
    let mut kept: Vec<(Segment, f64)> = Vec::with_capacity(scored.len());
    for (g, d) in scored {
        match kept.last_mut() {
            Some(last) if g.start < last.0.end() + ctx.motif().min_gap => {
                if d < last.1 {
                    *last = (g, d);
                }
            }
            _ => kept.push((g, d)),
        }
    }
    kept
}

pub fn repair(
    segs: Vec<Segment>,
    anchor: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Option<Vec<Segment>> {
    let m = ctx.motif();
    let mut segs: Vec<Segment> =
        segs.into_iter().map(|g| fit(g.start as i64, g.len as i64, ctx)).collect();
    if !segs.is_empty() {
        clamp_stretch(&mut segs, ctx);
    }
    segs.retain(|g| !ctx.is_flat(g));
    let l = match (anchor.is_empty(), segs.is_empty()) {
        (false, _) => prototype(anchor, ctx, s),
        (true, false) => prototype(&segs, ctx, s),
        (true, true) => return None,
    };
    let mut kept = resolve_overlaps(&segs, l, ctx, s);
    while kept.len() > m.max_occ as usize {
        let i = (0..kept.len()).max_by(|&a, &b| kept[a].1.total_cmp(&kept[b].1).then(b.cmp(&a)))?;
        kept.remove(i);
    }
    let mut out: Vec<Segment> = kept.into_iter().map(|(g, _)| g).collect();
    let mut attempts = 0;
    while out.len() < m.min_occ as usize && attempts < ADD_ATTEMPTS {
        attempts += 1;
        try_add(&mut out, l, None, ctx, s, rng);
    }
    (out.len() >= m.min_occ as usize).then_some(out)
}

fn random_len(base: f64, ctx: &Ctx, rng: &mut impl Rng) -> i64 {
    let r = ctx.motif().max_stretch.sqrt();
    (base * rng.random_range(1.0 / r..=r)).round() as i64
}

fn seeded(ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Option<Vec<Segment>> {
    let m = ctx.motif();
    let max_len = m.max_len.min(ctx.n() as u32);
    let len = rng.random_range(m.min_len..=max_len);
    let seed = Segment { start: rng.random_range(0..=ctx.n() as u32 - len), len };
    if ctx.is_flat(&seed) {
        return None;
    }
    let l = prototype(&[seed], ctx, s);
    let r = m.max_stretch.sqrt();
    let mut probes: Vec<(f64, Segment)> = Vec::new();
    for f in [1.0 / r, 1.0, r] {
        let g = fit(0, (len as f64 * f).round() as i64, ctx);
        for start in (0..=ctx.n() as u32 - g.len).step_by(scan_step(g.len, ctx) as usize) {
            let g = Segment { start, len: g.len };
            if !ctx.is_flat(&g) {
                probes.push((dist_to_proto(&g, l, ctx, s), g));
            }
        }
    }
    probes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let k = rng.random_range(m.min_occ..=m.max_occ) as usize;
    let mut segs = vec![seed];
    for (_, g) in probes {
        if segs.len() >= k {
            break;
        }
        if !ctx.is_flat(&g) && !overlaps(&segs, &g, m.min_gap) {
            segs.push(g);
        }
    }
    repair(segs, &[seed], ctx, s, rng)
}

fn random(ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Option<Vec<Segment>> {
    let m = ctx.motif();
    let base = rng.random_range(m.min_len..=m.max_len) as f64;
    let k = rng.random_range(m.min_occ..=m.max_occ);
    let segs = (0..k)
        .map(|_| fit(rng.random_range(0..ctx.n() as i64), random_len(base, ctx, rng), ctx))
        .collect();
    repair(segs, &[], ctx, s, rng)
}

pub fn init(
    seeded_init: bool,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Option<Vec<Segment>> {
    (0..INIT_ATTEMPTS).find_map(|i| match seeded_init == (i % 2 == 0) {
        true => seeded(ctx, s, rng),
        false => random(ctx, s, rng),
    })
}

fn guarded(ctx: &Ctx) -> bool {
    ctx.cfg.ga.guarded_add
}

fn chance(p: f64, rng: &mut impl Rng) -> bool {
    p > 0.0 && rng.random_bool(p)
}

// Guarded: a donated occurrence must be within the receiver's extent of its prototype.
fn donate(
    a: &[Segment],
    b: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Vec<Segment> {
    let (l, limit) = match guarded(ctx) {
        true => extent_of(a, ctx, s),
        false => {
            let l = prototype(a, ctx, s);
            (l, a.iter().map(|g| dist_to_proto(g, l, ctx, s)).fold(0.0, f64::max))
        }
    };
    let mut union = a.to_vec();
    union.extend(b.iter().filter(|g| dist_to_proto(g, l, ctx, s) <= limit));
    repair(union, a, ctx, s, rng).unwrap_or_else(|| a.to_vec())
}

pub fn crossover(
    a: &[Segment],
    b: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> (Vec<Segment>, Vec<Segment>) {
    (donate(a, b, ctx, s, rng), donate(b, a, ctx, s, rng))
}

fn normal(sigma: f64, rng: &mut impl Rng) -> f64 {
    Normal::new(0.0, sigma.max(0.0)).map_or(0.0, |d| d.sample(rng))
}

pub fn shift(
    segs: &[Segment],
    ga: &Ga,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Vec<Segment> {
    let mut out = segs.to_vec();
    let g = &mut out[rng.random_range(0..segs.len())];
    let delta = normal(ga.sigma_shift * g.len as f64, rng).round() as i64;
    *g = fit(g.start as i64 + delta, g.len as i64, ctx);
    repair(out, segs, ctx, s, rng).unwrap_or_else(|| segs.to_vec())
}

pub fn stretch(
    segs: &[Segment],
    ga: &Ga,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Vec<Segment> {
    let mut out = segs.to_vec();
    let i = rng.random_range(0..segs.len());
    let len = (out[i].len as f64 * normal(ga.sigma_stretch, rng).exp()).round() as i64;
    out[i] = resize(out[i], len, ctx);
    repair(out, segs, ctx, s, rng).unwrap_or_else(|| segs.to_vec())
}

// Guarded: the nearest outsider joins only if the extent does not grow.
pub fn add(segs: &[Segment], ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Vec<Segment> {
    let mut out = segs.to_vec();
    if out.len() >= ctx.motif().max_occ as usize {
        return out;
    }
    if !guarded(ctx) {
        let l = prototype(segs, ctx, s);
        let limit = segs.iter().map(|g| dist_to_proto(g, l, ctx, s)).fold(0.0, f64::max);
        try_add(&mut out, l, Some(limit), ctx, s, rng);
        return out;
    }
    let (l, before) = extent_of(segs, ctx, s);
    let Some((_, g)) = nearest_outsider(ctx, l, segs, s) else { return out };
    out.insert(out.partition_point(|h| h.start < g.start), g);
    match repair(out, segs, ctx, s, rng) {
        Some(o) if extent_of(&o, ctx, s).1 <= before => o,
        _ => segs.to_vec(),
    }
}

// Guarded: drops the worst member, kept only if the margin grows.
pub fn remove(segs: &[Segment], ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Vec<Segment> {
    let mut out = segs.to_vec();
    if out.len() <= ctx.motif().min_occ as usize {
        return out;
    }
    if guarded(ctx) {
        member_distances(segs, ctx, s);
        let d = &s.member[..segs.len()];
        let i = (0..d.len()).max_by(|&a, &b| d[a].total_cmp(&d[b]).then(b.cmp(&a))).unwrap_or(0);
        let before = evaluate(segs, ctx, s).margin;
        out.remove(i);
        return if evaluate(&out, ctx, s).margin > before { out } else { segs.to_vec() };
    }
    let i = match rng.random_bool(0.5) {
        true => worst(segs, prototype(segs, ctx, s), ctx, s),
        false => rng.random_range(0..segs.len()),
    };
    out.remove(i);
    out
}

fn mutate(
    mut segs: Vec<Segment>,
    ga: &Ga,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Vec<Segment> {
    if rng.random_bool(ga.shift) {
        segs = shift(&segs, ga, ctx, s, rng);
    }
    if rng.random_bool(ga.stretch) {
        segs = stretch(&segs, ga, ctx, s, rng);
    }
    if rng.random_bool(ga.add) {
        segs = add(&segs, ctx, s, rng);
    }
    if rng.random_bool(ga.remove) {
        segs = remove(&segs, ctx, s, rng);
    }
    if chance(ga.refine, rng) {
        segs = refine(&segs, ctx, s);
    }
    segs
}

pub fn vary(
    a: &[Segment],
    b: &[Segment],
    ga: &Ga,
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> (Vec<Segment>, Vec<Segment>) {
    let (c, d) = match rng.random_bool(ga.crossover) {
        true => crossover(a, b, ctx, s, rng),
        false => (a.to_vec(), b.to_vec()),
    };
    (mutate(c, ga, ctx, s, rng), mutate(d, ga, ctx, s, rng))
}

fn others_mean(out: &[Segment], i: usize, l: usize, ctx: &Ctx, s: &mut Scratch) {
    let k = out.len() as f64;
    load(&out[i], ctx, &mut s.v[..l]);
    for j in 0..l {
        s.proto[j] = (s.alt[j] - s.v[j]) / (k - 1.0);
    }
}

fn admissible(g: &Segment, i: usize, out: &[Segment], ctx: &Ctx) -> bool {
    let m = ctx.motif();
    let (lo, hi) = out
        .iter()
        .enumerate()
        .filter(|p| p.0 != i)
        .fold((u32::MAX, 0), |(a, b), (_, h)| (a.min(h.len), b.max(h.len)));
    let fits = g.len >= m.min_len && g.len <= m.max_len && g.end() as usize <= ctx.n();
    let stretch = g.len.max(hi) as f64 <= g.len.min(lo) as f64 * m.max_stretch + 1e-9;
    let gap = m.min_gap;
    let hit =
        |(j, h): (usize, &Segment)| j != i && g.start < h.end() + gap && h.start < g.end() + gap;
    fits && stretch && !ctx.is_flat(g) && !out.iter().enumerate().any(hit)
}

// (start, len, offset) -> moved (start, len)
type Move = fn(i64, i64, i64) -> (i64, i64);

// Best start (fixed length), or best length with the start, the end or the centre fixed; each
// family of moves is searched coarse-to-fine over offsets in [-r, r].
fn best_move(
    i: usize,
    out: &[Segment],
    resize: bool,
    l: usize,
    ctx: &Ctx,
    s: &mut Scratch,
) -> Segment {
    let cur = out[i];
    let r = (ctx.cfg.ga.refine_radius * cur.len as f64).ceil() as i64;
    let mut best = (dist_to_proto(&cur, l, ctx, s), cur);
    let (start, len) = (cur.start as i64, cur.len as i64);
    let families: &[Move] = match resize {
        false => &[|st, ln, d| (st + d, ln)],
        true => &[
            |st, ln, d| (st, ln + d),
            |st, ln, d| (st - d, ln + d),
            |st, ln, d| (st - d / 2, ln + d),
        ],
    };
    for family in families {
        let seg = |d: i64| {
            let (st, ln) = family(start, len, d);
            (st >= 0 && ln > 0).then_some(Segment { start: st as u32, len: ln as u32 })
        };
        let found = coarse_fine(-r, r, |d| {
            let g = seg(d).filter(|g| *g != cur && admissible(g, i, out, ctx))?;
            Some(dist_to_proto(&g, l, ctx, s))
        });
        if let Some((dist, d)) = found
            && dist < best.0
        {
            best = (dist, seg(d).expect("admissible move"));
        }
    }
    best.1
}

fn refine_pass(out: &mut [Segment], resize: bool, ctx: &Ctx, s: &mut Scratch) -> bool {
    let l = target_len(out).min(REFINE_RES);
    s.alt[..l].fill(0.0);
    for g in out.iter() {
        load(g, ctx, &mut s.v[..l]);
        s.alt[..l].iter_mut().zip(&s.v[..l]).for_each(|(a, v)| *a += v);
    }
    let mut moved = false;
    for i in 0..out.len() {
        others_mean(out, i, l, ctx, s);
        let g = best_move(i, out, resize, l, ctx, s);
        if g != out[i] {
            load(&out[i], ctx, &mut s.v[..l]);
            s.alt[..l].iter_mut().zip(&s.v[..l]).for_each(|(a, v)| *a -= v);
            load(&g, ctx, &mut s.v[..l]);
            s.alt[..l].iter_mut().zip(&s.v[..l]).for_each(|(a, v)| *a += v);
            (out[i], moved) = (g, true);
        }
    }
    moved
}

fn spread(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> f64 {
    member_distances(segs, ctx, s);
    s.member[..segs.len()].iter().map(|d| d * d).sum::<f64>() / segs.len() as f64
}

// Coordinate descent of every occurrence towards the mean of the others: starts first, lengths
// once the starts settled (resizing against misaligned others shrinks every occurrence to a
// consistent sub-window). Kept only if the mean squared member distance does not grow.
pub fn refine(segs: &[Segment], ctx: &Ctx, s: &mut Scratch) -> Vec<Segment> {
    if segs.len() < 2 {
        return segs.to_vec();
    }
    let mut out = segs.to_vec();
    for resize in [false, true] {
        for _ in 0..REFINE_PASSES {
            if !refine_pass(&mut out, resize, ctx, s) {
                break;
            }
        }
    }
    out.sort();
    if out != segs && spread(&out, ctx, s) <= spread(segs, ctx, s) { out } else { segs.to_vec() }
}

// Approximate nearest-neighbour distance of up to NN_QUERIES windows per level (sketch distance,
// trivial matches excluded).
fn level_nn(lev: &Level) -> Vec<(f32, Segment)> {
    let w = lev.starts.len();
    let step = (w / NN_QUERIES).max(1);
    (0..w)
        .step_by(step)
        .collect::<Vec<_>>()
        .par_iter()
        .filter_map(|&q| {
            let (sq, len) = (&lev.sketches[q], lev.len);
            let start = lev.starts[q];
            let best = (0..w)
                .filter(|&j| lev.starts[j].abs_diff(start) >= len)
                .map(|j| {
                    sq.iter().zip(&lev.sketches[j]).map(|(a, b)| (a - b) * (a - b)).sum::<f32>()
                })
                .fold(f32::INFINITY, f32::min);
            best.is_finite().then_some((best, Segment { start, len }))
        })
        .collect()
}

// Seed windows ranked by approximate nearest-neighbour distance, mutually non-overlapping.
pub fn nn_seeds(ctx: &Ctx, count: usize) -> Vec<Segment> {
    let m = ctx.motif();
    let inside: Vec<&Level> =
        ctx.index.levels.iter().filter(|l| l.len >= m.min_len && l.len <= m.max_len).collect();
    let levels = if inside.is_empty() { ctx.index.levels.iter().collect() } else { inside };
    let mut cands: Vec<(f32, Segment)> = levels.into_iter().flat_map(level_nn).collect();
    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut out: Vec<Segment> = Vec::new();
    for (_, g) in cands {
        if out.len() == count {
            break;
        }
        if !clash(&out, &g, 0) {
            out.push(g);
        }
    }
    out
}

// Size cut at the largest relative jump of the sorted member distances.
fn cut(d: &[f64], min_occ: usize) -> usize {
    let m = d.len();
    (min_occ.max(2)..=m)
        .map(|size| (d[size - 1] / d[size - 2].max(1e-9), size))
        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
        .map_or(m + 1, |p| p.1)
}

// Best step-1 position within half a stride of an indexed window (strided windows are misaligned
// by up to stride/2, which would blur the distance jump between true and false members).
fn aligned(
    start: u32,
    lev: &Level,
    seed: Segment,
    l: usize,
    ctx: &Ctx,
    s: &mut Scratch,
) -> Option<(f64, Segment)> {
    let half = (lev.stride / 2) as i64;
    let last = (ctx.n() - lev.len as usize) as i64;
    let (lo, hi) = ((start as i64 - half).max(0), (start as i64 + half).min(last));
    let best = coarse_fine(lo, hi, |p| {
        let g = Segment { start: p as u32, len: lev.len };
        let bad = ctx.is_flat(&g) || clash(&[seed], &g, ctx.motif().min_gap);
        (!bad).then(|| dist_to_proto(&g, l, ctx, s))
    });
    best.map(|(d, p)| (d, Segment { start: p as u32, len: lev.len }))
}

// A seed grown with its nearest non-overlapping windows, cut at the first large jump in distance.
pub fn grow(seed: Segment, ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Option<Vec<Segment>> {
    let m = ctx.motif();
    let seed = fit(seed.start as i64, seed.len as i64, ctx);
    let lev = ctx.index.level(seed.len as f64)?;
    let l = prototype(&[seed], ctx, s);
    let mut q = [0.0; PAA];
    paa(&s.proto[..l], &mut q);
    let mut ranked: Vec<(f64, u32)> = lev
        .starts
        .iter()
        .zip(&lev.sketches)
        .filter(|(st, _)| !clash(&[seed], &Segment { start: **st, len: lev.len }, m.min_gap))
        .map(|(&st, sk)| (q.iter().zip(sk).map(|(a, b)| (a - *b as f64).powi(2)).sum::<f64>(), st))
        .collect();
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut near: Vec<(f64, Segment)> = ranked
        .iter()
        .take(4 * m.max_occ as usize)
        .filter_map(|&(_, start)| aligned(start, lev, seed, l, ctx, s))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut set = vec![seed];
    let mut dist = Vec::new();
    for (d, g) in near {
        if set.len() == m.max_occ as usize {
            break;
        }
        if !clash(&set, &g, m.min_gap) {
            set.push(g);
            dist.push(d);
        }
    }
    let size = cut(&dist, m.min_occ as usize).min(set.len());
    set.truncate(size);
    set.sort();
    repair(set, &[seed], ctx, s, rng)
}
