use crate::config::Ga;
use crate::motif::{Ctx, Scratch, Segment, dist_to_proto, prototype};
use rand::Rng;
use rand_distr::{Distribution, Normal};

const INIT_ATTEMPTS: usize = 20;
const ADD_ATTEMPTS: usize = 3;

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

fn donate(
    a: &[Segment],
    b: &[Segment],
    ctx: &Ctx,
    s: &mut Scratch,
    rng: &mut impl Rng,
) -> Vec<Segment> {
    let l = prototype(a, ctx, s);
    let limit = a.iter().map(|g| dist_to_proto(g, l, ctx, s)).fold(0.0, f64::max);
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

pub fn add(segs: &[Segment], ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Vec<Segment> {
    let mut out = segs.to_vec();
    if out.len() < ctx.motif().max_occ as usize {
        let l = prototype(segs, ctx, s);
        let limit = segs.iter().map(|g| dist_to_proto(g, l, ctx, s)).fold(0.0, f64::max);
        try_add(&mut out, l, Some(limit), ctx, s, rng);
    }
    out
}

pub fn remove(segs: &[Segment], ctx: &Ctx, s: &mut Scratch, rng: &mut impl Rng) -> Vec<Segment> {
    let mut out = segs.to_vec();
    if out.len() > ctx.motif().min_occ as usize {
        let i = match rng.random_bool(0.5) {
            true => worst(segs, prototype(segs, ctx, s), ctx, s),
            false => rng.random_range(0..segs.len()),
        };
        out.remove(i);
    }
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
