use crate::config::{Config, Error};
use crate::events::{Emitter, Event, GenerationRecord};
use crate::fitness::objectives;
use crate::motif::{Ctx, Individual, Scratch, Segment, evaluate};
use crate::nsga::{hv3d, survive, tournament};
use crate::ops;
use crate::result::StopReason;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const INIT: u64 = 0x9e37_79b9_7f4a_7c15;
const VARY: u64 = 0xc2b2_ae3d_27d4_eb4f;
const HV_REF: f64 = 1.1;

pub struct Outcome {
    pub pop: Vec<Individual>,
    pub stop: StopReason,
    pub generations: u32,
    pub elapsed_s: f64,
}

fn splitmix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

// One independent stream per (seed, generation, index): results do not depend on scheduling.
fn rng(seed: u64, generation: u32, idx: usize, stream: u64) -> ChaCha8Rng {
    let mix = splitmix64(seed)
        ^ splitmix64(generation as u64 ^ stream)
        ^ splitmix64(idx as u64).rotate_left(17);
    ChaCha8Rng::seed_from_u64(splitmix64(mix))
}

fn evaluated(segs: Vec<Segment>, ctx: &Ctx, s: &mut Scratch) -> Individual {
    let raw = evaluate(&segs, ctx, s);
    Individual { raw, ..Individual::new(segs) }
}

// The first `seed_nn` share grows nearest-neighbour seeds; the rest keeps the greedy/random mix.
fn initial(ctx: &Ctx, cfg: &Config, seeds: &[Segment]) -> Result<Vec<Individual>, Error> {
    let n = cfg.ga.population as usize;
    let n_seed = if seeds.is_empty() { 0 } else { (cfg.ga.seed_nn * n as f64).round() as usize };
    let greedy = (cfg.ga.init_greedy * (n - n_seed) as f64).round() as usize;
    let max_len = cfg.motif.max_len as usize;
    let pop: Option<Vec<Individual>> = (0..n)
        .into_par_iter()
        .map_init(
            || Scratch::new(max_len),
            |s, i| {
                let r = &mut rng(cfg.seed, 0, i, INIT);
                let segs = match i < n_seed {
                    true => ops::grow(seeds[i % seeds.len()], ctx, s, r)
                        .or_else(|| ops::init(true, ctx, s, r)),
                    false => ops::init(i - n_seed < greedy, ctx, s, r),
                }?;
                Some(evaluated(segs, ctx, s))
            },
        )
        .collect();
    pop.ok_or_else(|| {
        Error::Data(
            "could not build feasible motifs: series too short or too flat for \
             min_len/min_occ/min_std/min_gap"
                .into(),
        )
    })
}

// Pairs not started once `cancel` is raised or the deadline passed are skipped, so either stop
// lands within one evaluation.
fn offspring(
    pop: &[Individual],
    ctx: &Ctx,
    generation: u32,
    halt: &(dyn Fn() -> bool + Sync),
) -> Vec<Individual> {
    let cfg = ctx.cfg;
    let pairs: Vec<Option<[Individual; 2]>> = (0..pop.len() / 2)
        .into_par_iter()
        .map_init(
            || Scratch::new(cfg.motif.max_len as usize),
            |s, p| {
                if halt() {
                    return None;
                }
                let r = &mut rng(cfg.seed, generation, p, VARY);
                let (i, j) = (tournament(pop, r), tournament(pop, r));
                let (a, b) = ops::vary(&pop[i].segs, &pop[j].segs, &cfg.ga, ctx, s, r);
                Some([a, b].map(|segs| evaluated(segs, ctx, s)))
            },
        )
        .collect();
    pairs.into_iter().flatten().flatten().collect()
}

fn record(
    pop: &[Individual],
    generation: u32,
    elapsed_s: f64,
    evals: u64,
    cfg: &Config,
) -> GenerationRecord {
    let raw = |i: &Individual| [i.raw.support as f64, i.raw.cohesion, i.raw.mean_len];
    let front: Vec<&Individual> = pop.iter().filter(|i| i.rank == 0).collect();
    let hv = hv3d(&front.iter().map(|i| i.obj.map(|v| v.min(HV_REF))).collect::<Vec<_>>(), HV_REF);
    let n = pop.len() as f64;
    let mut best = [f64::MIN, f64::MAX, f64::MIN];
    let mut mean = [0.0; 3];
    for v in pop.iter().map(raw) {
        best = [best[0].max(v[0]), best[1].min(v[1]), best[2].max(v[2])];
        (0..3).for_each(|k| mean[k] += v[k] / n);
    }
    let snapshot = generation % cfg.output.snapshot_every == 0;
    GenerationRecord {
        generation,
        elapsed_s,
        evals,
        hv,
        front_size: front.len() as u32,
        best,
        mean,
        front: snapshot.then(|| front.iter().map(|i| raw(i)).collect()),
    }
}

fn should_stop(
    cfg: &Config,
    generation: u32,
    elapsed: f64,
    hvs: &[f64],
    cancel: &AtomicBool,
) -> Option<StopReason> {
    let s = &cfg.stop;
    if cancel.load(Ordering::Relaxed) {
        return Some(StopReason::Cancelled);
    }
    if generation >= s.max_generations {
        return Some(StopReason::MaxGenerations);
    }
    if s.time_budget_s.is_some_and(|t| elapsed >= t) {
        return Some(StopReason::TimeBudget);
    }
    if generation < s.min_generations {
        return None;
    }
    let w = s.stagnation_window? as usize;
    let g = hvs.len() - 1;
    (g >= w && hvs[g] - hvs[g - w] < s.stagnation_delta).then_some(StopReason::Stagnation)
}

pub fn run(ctx: &Ctx, cfg: &Config, cancel: &AtomicBool, em: &Emitter) -> Result<Outcome, Error> {
    let t0 = Instant::now();
    let (n, niche) = (cfg.ga.population as usize, cfg.ga.niche_jaccard);
    let seeds = if cfg.ga.seed_nn > 0.0 { ops::nn_seeds(ctx, 4 * n) } else { Vec::new() };
    let mut pop = initial(ctx, cfg, &seeds)?;
    let c_ref = pop.iter().map(|i| i.raw.cohesion).fold(0.0, f64::max);
    let c_ref = if c_ref > 0.0 { c_ref } else { 1.0 };
    let score = |pop: &mut [Individual]| {
        pop.iter_mut().for_each(|i| i.obj = objectives(&i.raw, ctx, c_ref));
    };
    score(&mut pop);
    pop = survive(pop, n, niche);
    if cancel.load(Ordering::Relaxed) {
        pop.clear();
        let stop = StopReason::Cancelled;
        return Ok(Outcome { pop, stop, generations: 0, elapsed_s: 0.0 });
    }
    let deadline = cfg.stop.time_budget_s.map(|b| t0 + std::time::Duration::from_secs_f64(b));
    let halt = || cancel.load(Ordering::Relaxed) || deadline.is_some_and(|d| Instant::now() >= d);
    let mut evals = n as u64;
    let mut hvs = Vec::new();
    let mut generation = 0;
    let stop = loop {
        if generation > 0 {
            let mut children = offspring(&pop, ctx, generation, &halt);
            score(&mut children);
            evals += children.len() as u64;
            pop.extend(children);
            pop = survive(pop, n, niche);
        }
        let rec = record(&pop, generation, t0.elapsed().as_secs_f64(), evals, cfg);
        // Clearing can drop front members, so stagnation tracks the best HV seen so far.
        hvs.push(hvs.last().map_or(rec.hv, |&b: &f64| rec.hv.max(b)));
        em.send(Event::Generation(rec));
        let elapsed = t0.elapsed().as_secs_f64();
        if let Some(reason) = should_stop(cfg, generation, elapsed, &hvs, cancel) {
            break reason;
        }
        generation += 1;
    };
    Ok(Outcome { pop, stop, generations: generation, elapsed_s: t0.elapsed().as_secs_f64() })
}
