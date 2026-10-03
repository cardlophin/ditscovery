use crate::config::{Column, Config, Error};
use crate::events::{Event, GenerationRecord, write_line};
use crate::io::{CsvLoader, Loader};
use crate::motif::{Ctx, Individual, align, jaccard};
use crate::series::Series;
use serde::{Deserialize, Serialize};
use std::io::Write;
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
        obj.remove("history");
        obj.remove("series");
        let path = dir.join("results.json");
        let text = serde_json::to_string_pretty(&json).expect("json serialises");
        std::fs::write(&path, text).map_err(io_err(&path))?;
        let path = dir.join("config.yaml");
        std::fs::write(&path, self.config.to_yaml()).map_err(io_err(&path))?;
        self.write_history(&dir.join("history.jsonl"))?;
        let path = dir.join("series.csv");
        let mut out = String::from("value\n");
        self.series.iter().for_each(|v| out += &format!("{v}\n"));
        std::fs::write(&path, out).map_err(io_err(&path))
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
        res.series =
            CsvLoader::new(dir.join("series.csv")).column(Column::Name("value".into())).load()?;
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

pub fn top_k(pop: &[Individual], k: usize, dedupe: f64) -> Vec<usize> {
    let mut chosen: Vec<usize> = Vec::with_capacity(k);
    for i in order(pop) {
        if chosen.len() == k {
            break;
        }
        if chosen.iter().all(|&c| jaccard(&pop[i].segs, &pop[c].segs) <= dedupe) {
            chosen.push(i);
        }
    }
    chosen
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
    }
}

pub fn build(pop: &[Individual], ctx: &Ctx, series: &Series) -> (Vec<Motif>, Vec<Motif>) {
    let out = &ctx.cfg.output;
    let motifs = top_k(pop, out.top_k, out.dedupe_jaccard)
        .into_iter()
        .map(|i| to_motif(&pop[i], ctx, series, true))
        .collect();
    let front = order(pop)
        .into_iter()
        .filter(|&i| pop[i].rank == 0)
        .map(|i| to_motif(&pop[i], ctx, series, false))
        .collect();
    (motifs, front)
}
