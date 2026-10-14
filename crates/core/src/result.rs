use crate::config::{Column, Config, Error, TopK};
use crate::distance::{euclid, resample, znorm};
use crate::events::{Event, GenerationRecord, write_line};
use crate::fitness::members;
use crate::io::{CsvLoader, Loader};
use crate::label::{Cand, resolve};
use crate::metrics::Prom;
use crate::motif::{Ctx, Individual, Scratch, Segment, align, containment, evaluate};
use crate::series::Series;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    MaxGenerations,
    TimeBudget,
    Stagnation,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Motif {
    pub occurrences: Vec<(u32, u32)>,
    pub prototype: Vec<f64>,
    pub aligned: Option<Vec<Vec<f64>>>,
    pub support: u32,
    pub cohesion: f64,
    pub mean_len: f64,
    pub stretch: f64,
    pub objectives: [f64; 3],
    #[serde(default)]
    pub extent: f64,
    #[serde(default)]
    pub margin: f64,
    #[serde(default)]
    pub coverage: f64,
    #[serde(default)]
    pub quality: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunResult {
    pub motifs: Vec<Motif>,
    pub front: Vec<Motif>,
    #[serde(default)]
    pub history: Vec<GenerationRecord>,
    pub stop: StopReason,
    pub generations: u32,
    pub elapsed_s: f64,
    pub dropped_events: u64,
    pub config: Config,
    pub version: String,
    #[serde(default)]
    pub series: Vec<f64>,
    #[serde(default)]
    pub metrics: Option<Prom>,
    #[serde(default)]
    pub timestamps: Option<Vec<String>>,
    #[serde(default)]
    pub labels: Option<Vec<i32>>,
}

fn io_err(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |source| Error::Io { path: path.to_path_buf(), source }
}

// A results directory: results.json (without history and series), history.jsonl, config.yaml
// and series.csv (original series, NaN kept).
impl RunResult {
    pub fn save(&self, dir: &Path) -> Result<(), Error> {
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        let mut json = serde_json::to_value(self).map_err(|e| Error::Data(e.to_string()))?;
        let obj = json.as_object_mut().expect("result is an object");
        for key in ["history", "series", "timestamps", "labels"] {
            obj.remove(key);
        }
        let path = dir.join("results.json");
        let text = serde_json::to_string_pretty(&json).expect("json serialises");
        std::fs::write(&path, text).map_err(io_err(&path))?;
        let path = dir.join("config.yaml");
        std::fs::write(&path, self.config.to_yaml()).map_err(io_err(&path))?;
        self.write_history(&dir.join("history.jsonl"))?;
        let path = dir.join("series.csv");
        std::fs::write(&path, self.series_csv()).map_err(io_err(&path))
    }

    // [timestamp,]value[,label]; label is the 1-based motif number or empty.
    fn series_csv(&self) -> String {
        let ts = self.timestamps.as_ref();
        let lab = self.labels.as_ref();
        let mut out = String::new();
        out += if ts.is_some() { "timestamp,value" } else { "value" };
        out += if lab.is_some() { ",label\n" } else { "\n" };
        for (i, v) in self.series.iter().enumerate() {
            if let Some(t) = ts {
                out += &format!("{},", t[i]);
            }
            out += &v.to_string();
            match lab.map(|l| l[i]) {
                Some(l) if l >= 0 => out += &format!(",{}\n", l + 1),
                Some(_) => out += ",\n",
                None => out += "\n",
            }
        }
        out
    }

    fn write_history(&self, path: &Path) -> Result<(), Error> {
        let file = std::fs::File::create(path).map_err(io_err(path))?;
        let mut w = std::io::BufWriter::new(file);
        let start =
            Event::Start { config: Box::new(self.config.clone()), version: self.version.clone() };
        let finish = Event::Finish { reason: self.stop, dropped: self.dropped_events };
        let events = self.history.iter().cloned().map(Event::Generation);
        for e in std::iter::once(start).chain(events).chain(std::iter::once(finish)) {
            write_line(&mut w, &e).map_err(io_err(path))?;
        }
        w.flush().map_err(io_err(path))
    }

    pub fn load(dir: &Path) -> Result<Self, Error> {
        let path = dir.join("results.json");
        let text = std::fs::read_to_string(&path).map_err(io_err(&path))?;
        let bad = |p: &Path, e: serde_json::Error| Error::Data(format!("{}: {e}", p.display()));
        let mut res: RunResult = serde_json::from_str(&text).map_err(|e| bad(&path, e))?;
        let path = dir.join("history.jsonl");
        let text = std::fs::read_to_string(&path).map_err(io_err(&path))?;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            if let Event::Generation(r) = serde_json::from_str(line).map_err(|e| bad(&path, e))? {
                res.history.push(r);
            }
        }
        let path = dir.join("series.csv");
        let csv = CsvLoader::new(&path).column(Column::Name("value".into()));
        res.series = csv.load()?;
        let mut head = String::new();
        let file = std::fs::File::open(&path).map_err(io_err(&path))?;
        std::io::BufReader::new(file).read_line(&mut head).map_err(io_err(&path))?;
        if head.starts_with("timestamp,") {
            res.timestamps = Some(csv.text(&Column::Name("timestamp".into()))?);
        }
        if head.trim_end().ends_with(",label") {
            let text = csv.text(&Column::Name("label".into()))?;
            res.labels =
                Some(text.iter().map(|t| t.parse::<i32>().map_or(-1, |l| l - 1)).collect());
        }
        Ok(res)
    }
}

fn order(pop: &[Individual]) -> Vec<usize> {
    let norm = |i: &Individual| i.obj.iter().map(|v| v * v).sum::<f64>();
    let mut idx: Vec<usize> = (0..pop.len()).collect();
    idx.sort_by(|&a, &b| {
        let (x, y) = (&pop[a], &pop[b]);
        x.rank.cmp(&y.rank).then(norm(x).total_cmp(&norm(y))).then(a.cmp(&b))
    });
    idx
}

pub fn to_motif(ind: &Individual, ctx: &Ctx, series: &Series, aligned: bool) -> Motif {
    let occurrences: Vec<(u32, u32)> =
        ind.segs.iter().map(|g| series.to_original(g.start, g.len)).collect();
    let mean_len = occurrences.iter().map(|o| o.1 as f64).sum::<f64>() / occurrences.len() as f64;
    let (prototype, rows) = align(&ind.segs, ctx);
    Motif {
        occurrences,
        prototype,
        aligned: aligned.then_some(rows),
        support: ind.raw.support,
        cohesion: ind.raw.cohesion,
        mean_len,
        stretch: ind.raw.stretch,
        objectives: ind.obj,
        extent: ind.raw.extent,
        margin: ind.raw.margin,
        coverage: ind.raw.coverage,
        quality: ind.raw.quality,
    }
}

fn by_quality(pop: &[Individual]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..pop.len()).collect();
    idx.sort_by(|&a, &b| {
        let (x, y) = (&pop[a], &pop[b]);
        y.raw.quality.total_cmp(&x.raw.quality).then(x.rank.cmp(&y.rank)).then(a.cmp(&b))
    });
    idx
}

// Min over lags |τ| <= L/4 of the RMS distance between the re-z-normalised overlapping parts of
// two prototypes resampled to a common length.
pub fn shape_distance(a: &[f64], b: &[f64]) -> f64 {
    let l = a.len().min(b.len());
    let (mut x, mut y) = (vec![0.0; l], vec![0.0; l]);
    resample(a, &mut x);
    resample(b, &mut y);
    let lag = (l / 4) as i64;
    (-lag..=lag)
        .map(|t| {
            let (sa, sb) = if t >= 0 { (t as usize, 0) } else { (0, (-t) as usize) };
            let n = l - t.unsigned_abs() as usize;
            let (mut p, mut q) = (x[sa..sa + n].to_vec(), y[sb..sb + n].to_vec());
            znorm(&mut p);
            znorm(&mut q);
            euclid(&p, &q)
        })
        .fold(f64::INFINITY, f64::min)
}

fn signature(ind: &Individual, ctx: &Ctx) -> Vec<f64> {
    let mut p = align(&ind.segs, ctx).0;
    znorm(&mut p);
    p
}

// Candidates in ranking order: closed ones only under `top_k: auto`, duplicates (containment or
// lag-tolerant shape) of a better one removed.
pub fn rank(pop: &[Individual], ctx: &Ctx) -> Vec<usize> {
    let out = &ctx.cfg.output;
    let auto = out.top_k == TopK::Auto;
    let mut kept: Vec<(usize, Vec<f64>)> = Vec::new();
    for i in by_quality(pop).into_iter().filter(|&i| !auto || pop[i].raw.margin > 0.0) {
        let segs = &pop[i].segs;
        if kept.iter().any(|(c, _)| containment(segs, &pop[*c].segs) > out.dedupe_overlap) {
            continue;
        }
        let sig = signature(&pop[i], ctx);
        let limit = out.dedupe_shape * 2f64.sqrt();
        if kept.iter().all(|(_, s)| shape_distance(&sig, s) >= limit) {
            kept.push((i, sig));
        }
    }
    kept.into_iter().map(|p| p.0).collect()
}

// Labelling weight: how clearly each occurrence belongs to its motif.
fn cand(ind: &Individual, ctx: &Ctx, s: &mut Scratch) -> Cand {
    let d = members(&ind.segs, ctx, s);
    let scale = if ind.raw.scale > 0.0 { ind.raw.scale } else { 1.0 };
    let weights = d.iter().map(|di| (ind.raw.outsider - di).max(1e-6) / scale).collect();
    Cand { occ: ind.segs.clone(), weights }
}

// Final motifs: ranked, deduplicated, and (with labels on) occurrence conflicts resolved.
fn chosen(pop: &[Individual], ctx: &Ctx) -> Vec<Individual> {
    let out = &ctx.cfg.output;
    let ranked = rank(pop, ctx);
    let k = match out.top_k {
        TopK::N(k) => k,
        TopK::Auto => ranked.len(),
    };
    if !out.labels {
        return ranked.into_iter().take(k).map(|i| pop[i].clone()).collect();
    }
    let mut s = Scratch::new(ctx.cfg.motif.max_len as usize);
    let cands: Vec<Cand> = ranked.iter().map(|&i| cand(&pop[i], ctx, &mut s)).collect();
    let (m, gap) = (ctx.motif().min_occ as usize, ctx.motif().min_gap);
    // A motif trimmed by the conflicts must still be closed.
    let mut t = Scratch::new(ctx.cfg.motif.max_len as usize);
    let closed = |_: usize, occ: &[Segment]| evaluate(occ, ctx, &mut t).margin > 0.0;
    resolve(&cands, k, m, gap, closed)
        .into_iter()
        .map(|(c, occ)| {
            let ind = &pop[ranked[c]];
            match occ == ind.segs {
                true => ind.clone(),
                false => Individual {
                    raw: evaluate(&occ, ctx, &mut s),
                    obj: ind.obj,
                    ..Individual::new(occ)
                },
            }
        })
        .filter(|i| out.top_k != TopK::Auto || i.raw.margin > 0.0)
        .collect()
}

pub fn build(pop: &[Individual], ctx: &Ctx, series: &Series) -> (Vec<Motif>, Vec<Motif>) {
    let motifs = chosen(pop, ctx).iter().map(|i| to_motif(i, ctx, series, true)).collect();
    let front = order(pop)
        .into_iter()
        .filter(|&i| pop[i].rank == 0)
        .map(|i| to_motif(&pop[i], ctx, series, false))
        .collect();
    (motifs, front)
}
