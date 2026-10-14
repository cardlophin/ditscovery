use crate::bench::INTERVALS;
use crate::distance::Distance;
use crate::metrics::{Average, OffTarget};
use crate::preprocess::Step;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml_ng::Deserializer;
use serde_yaml_ng::with::singleton_map_recursive;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {msg}")]
    Config { path: String, msg: String },
    #[error("{0}")]
    Data(String),
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
}

pub(crate) fn cfg_err(path: impl Into<String>, msg: impl ToString) -> Error {
    Error::Config { path: path.into(), msg: msg.to_string() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Column {
    Index(usize),
    Name(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Zeuclid,
    Euclid,
    Dtw,
    Zdtw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "TopKRaw", into = "TopKRaw")]
pub enum TopK {
    N(usize),
    Auto,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum TopKRaw {
    N(usize),
    S(String),
}

impl TryFrom<TopKRaw> for TopK {
    type Error = String;
    fn try_from(k: TopKRaw) -> Result<Self, String> {
        match k {
            TopKRaw::N(n) => Ok(TopK::N(n)),
            TopKRaw::S(s) if s == "auto" => Ok(TopK::Auto),
            TopKRaw::S(s) => Err(format!("top_k: expected `auto` or a number, got `{s}`")),
        }
    }
}

impl From<TopK> for TopKRaw {
    fn from(k: TopK) -> Self {
        match k {
            TopK::N(n) => TopKRaw::N(n),
            TopK::Auto => TopKRaw::S("auto".into()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    pub seed: u64,
    pub data: Data,
    pub preprocess: Vec<Step>,
    pub motif: MotifCfg,
    pub distance: DistanceCfg,
    pub fitness: Fitness,
    pub ga: Ga,
    pub stop: Stop,
    pub output: Output,
    pub log: Log,
    pub eval: Eval,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Eval {
    pub ground_truth: Option<PathBuf>,
    pub average: Average,
    pub off_target: OffTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Data {
    pub path: Option<PathBuf>,
    pub column: Column,
    pub delimiter: char,
    pub header: bool,
    pub time_column: Option<Column>,
    pub ticker: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub period: Option<String>,
    pub interval: String,
    pub field: String,
    pub refresh: bool,
}

pub const PERIODS: [&str; 11] =
    ["1d", "5d", "1mo", "3mo", "6mo", "1y", "2y", "5y", "10y", "ytd", "max"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct MotifCfg {
    pub min_len: u32,
    pub max_len: u32,
    pub max_stretch: f64,
    pub min_occ: u32,
    pub max_occ: u32,
    pub min_gap: u32,
    pub min_std: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DistanceCfg {
    pub kind: Kind,
    pub band: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Fitness {
    pub extent_q: f64,
    pub null_pairs: u32,
    pub cid: bool,
    pub loo: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Ga {
    pub population: u32,
    pub crossover: f64,
    pub shift: f64,
    pub stretch: f64,
    pub add: f64,
    pub remove: f64,
    pub sigma_shift: f64,
    pub sigma_stretch: f64,
    pub scan_step: f64,
    pub init_greedy: f64,
    pub niche_jaccard: f64,
    pub refine: f64,
    pub refine_radius: f64,
    pub guarded_add: bool,
    pub seed_nn: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Stop {
    pub max_generations: u32,
    pub min_generations: u32,
    pub time_budget_s: Option<f64>,
    pub stagnation_window: Option<u32>,
    pub stagnation_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Output {
    pub dir: Option<PathBuf>,
    pub top_k: TopK,
    pub dedupe_overlap: f64,
    pub dedupe_shape: f64,
    pub labels: bool,
    pub snapshot_every: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Log {
    pub level: String,
    pub log_every: u32,
    pub history: bool,
    pub channel_capacity: usize,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            path: None,
            column: Column::Index(0),
            delimiter: ',',
            header: true,
            time_column: None,
            ticker: None,
            start: None,
            end: None,
            period: None,
            interval: "1d".into(),
            field: "Close".into(),
            refresh: false,
        }
    }
}

impl Default for MotifCfg {
    fn default() -> Self {
        Self {
            min_len: 0,
            max_len: 0,
            max_stretch: 1.5,
            min_occ: 2,
            max_occ: 20,
            min_gap: 0,
            min_std: 1e-6,
        }
    }
}

impl Default for DistanceCfg {
    fn default() -> Self {
        Self { kind: Kind::Zeuclid, band: 0.1 }
    }
}

impl Default for Ga {
    fn default() -> Self {
        Self {
            population: 200,
            crossover: 0.9,
            shift: 0.3,
            stretch: 0.2,
            add: 0.2,
            remove: 0.1,
            sigma_shift: 0.1,
            sigma_stretch: 0.1,
            scan_step: 0.125,
            init_greedy: 0.5,
            niche_jaccard: 0.5,
            refine: 0.3,
            refine_radius: 0.1,
            guarded_add: true,
            seed_nn: 0.4,
        }
    }
}

impl Default for Fitness {
    fn default() -> Self {
        Self { extent_q: 0.9, null_pairs: 256, cid: false, loo: true }
    }
}

impl Default for Stop {
    fn default() -> Self {
        Self {
            max_generations: 300,
            min_generations: 100,
            time_budget_s: None,
            stagnation_window: Some(50),
            stagnation_delta: 1e-3,
        }
    }
}

impl Default for Output {
    fn default() -> Self {
        Self {
            dir: None,
            top_k: TopK::N(5),
            dedupe_overlap: 0.5,
            dedupe_shape: 0.3,
            labels: true,
            snapshot_every: 10,
        }
    }
}

impl Default for Log {
    fn default() -> Self {
        Self { level: "info".into(), log_every: 10, history: true, channel_capacity: 1024 }
    }
}

impl DistanceCfg {
    pub fn distance(&self) -> Distance {
        match self.kind {
            Kind::Zeuclid => Distance::ZEuclid,
            Kind::Euclid => Distance::Euclid,
            Kind::Dtw => Distance::Dtw { band: self.band },
            Kind::Zdtw => Distance::ZDtw { band: self.band },
        }
    }
}

impl Config {
    pub fn build(
        yaml: Option<&str>,
        base: Option<Value>,
        overrides: &[(String, Value)],
    ) -> Result<Self, Error> {
        let mut cfg = match (yaml, base) {
            (Some(y), _) => singleton_map_recursive::deserialize(Deserializer::from_str(y))
                .map_err(|e| cfg_err("config", e))?,
            (None, Some(b)) => serde_json::from_value(b).map_err(|e| cfg_err("config", e))?,
            (None, None) => Config::default(),
        };
        if !overrides.is_empty() {
            let mut tree = serde_json::to_value(&cfg).expect("config serialises");
            for (key, value) in overrides {
                let path = set_leaf(&mut tree, key, value.clone())?;
                cfg = serde_json::from_value(tree.clone()).map_err(|e| cfg_err(path, e))?;
            }
        }
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn to_yaml(&self) -> String {
        let mut out = Vec::new();
        singleton_map_recursive::serialize(self, &mut serde_yaml_ng::Serializer::new(&mut out))
            .expect("config serialises");
        String::from_utf8(out).expect("yaml is utf-8")
    }

    pub fn validate(&self) -> Result<(), Error> {
        self.validate_motif()?;
        self.validate_ga()?;
        self.validate_data()?;
        self.validate_rest()
    }

    fn validate_data(&self) -> Result<(), Error> {
        let d = &self.data;
        let both = d.ticker.is_some() && d.path.is_some();
        check(!both, "data.ticker", "give data.path or data.ticker, not both")?;
        check(
            d.period.is_none() || d.start.is_none(),
            "data.period",
            "give period or start, not both",
        )?;
        let period = d.period.as_deref().is_none_or(|p| PERIODS.contains(&p));
        check(period, "data.period", &format!("must be one of {}", PERIODS.join("|")))?;
        let interval = INTERVALS.contains(&d.interval.as_str());
        check(interval, "data.interval", &format!("must be one of {}", INTERVALS.join("|")))?;
        for (v, key) in [(&d.start, "data.start"), (&d.end, "data.end")] {
            check(v.as_deref().is_none_or(is_date), key, "must be a YYYY-MM-DD date")?;
        }
        Ok(())
    }

    fn validate_motif(&self) -> Result<(), Error> {
        let m = &self.motif;
        check(m.min_len > 0, "motif.min_len", "required (> 0)")?;
        let msg = format!("must be >= min_len ({})", m.min_len);
        check(m.max_len >= m.min_len, "motif.max_len", &msg)?;
        check(m.max_stretch >= 1.0, "motif.max_stretch", "must be >= 1")?;
        check(m.min_occ >= 2, "motif.min_occ", "must be >= 2")?;
        let msg = format!("must be >= min_occ ({})", m.min_occ);
        check(m.max_occ >= m.min_occ, "motif.max_occ", &msg)?;
        check(m.min_std >= 0.0, "motif.min_std", "must be >= 0")?;
        check(self.data.delimiter.is_ascii(), "data.delimiter", "must be one ASCII character")?;
        let b = self.distance.band;
        check(b > 0.0 && b <= 1.0, "distance.band", "must be in (0, 1]")
    }

    fn validate_ga(&self) -> Result<(), Error> {
        let g = &self.ga;
        let even = g.population >= 4 && g.population % 2 == 0;
        check(even, "ga.population", "must be even and >= 4")?;
        for (v, k) in [
            (g.crossover, "crossover"),
            (g.shift, "shift"),
            (g.stretch, "stretch"),
            (g.add, "add"),
            (g.remove, "remove"),
            (g.init_greedy, "init_greedy"),
            (g.niche_jaccard, "niche_jaccard"),
            (g.refine, "refine"),
            (g.seed_nn, "seed_nn"),
        ] {
            check(unit(v), &format!("ga.{k}"), "must be in [0, 1]")?;
        }
        let r = g.refine_radius;
        check(r > 0.0 && r <= 0.5, "ga.refine_radius", "must be in (0, 0.5]")?;
        let f = &self.fitness;
        let q = f.extent_q;
        check(q > 0.0 && q <= 1.0, "fitness.extent_q", "must be in (0, 1]")?;
        check(f.null_pairs >= 16, "fitness.null_pairs", "must be >= 16")?;
        check(g.sigma_shift >= 0.0, "ga.sigma_shift", "must be >= 0")?;
        check(g.sigma_stretch >= 0.0, "ga.sigma_stretch", "must be >= 0")?;
        check(g.scan_step > 0.0 && g.scan_step <= 1.0, "ga.scan_step", "must be in (0, 1]")
    }

    fn validate_rest(&self) -> Result<(), Error> {
        let s = &self.stop;
        check(s.max_generations >= 1, "stop.max_generations", "must be >= 1")?;
        check(s.time_budget_s.is_none_or(|t| t > 0.0), "stop.time_budget_s", "must be > 0")?;
        let window = s.stagnation_window.is_none_or(|w| w >= 1);
        check(window, "stop.stagnation_window", "must be >= 1")?;
        check(s.stagnation_delta >= 0.0, "stop.stagnation_delta", "must be >= 0")?;
        let o = &self.output;
        check(o.top_k != TopK::N(0), "output.top_k", "must be >= 1 or auto")?;
        check(unit(o.dedupe_overlap), "output.dedupe_overlap", "must be in [0, 1]")?;
        check(o.dedupe_shape >= 0.0, "output.dedupe_shape", "must be >= 0")?;
        check(o.snapshot_every >= 1, "output.snapshot_every", "must be >= 1")?;
        let l = &self.log;
        let level = ["error", "warn", "info", "debug", "trace"].contains(&l.level.as_str());
        check(level, "log.level", "must be one of error|warn|info|debug|trace")?;
        check(l.log_every >= 1, "log.log_every", "must be >= 1")?;
        check(l.channel_capacity >= 1, "log.channel_capacity", "must be >= 1")?;
        for (i, step) in self.preprocess.iter().enumerate() {
            match *step {
                Step::Smooth(w) => {
                    check(w >= 2, &format!("preprocess[{i}].smooth"), "must be >= 2")?
                }
                Step::Paa(f) => check(f >= 2, &format!("preprocess[{i}].paa"), "must be >= 2")?,
                _ => {}
            }
        }
        Ok(())
    }
}

fn check(ok: bool, path: &str, msg: &str) -> Result<(), Error> {
    if ok { Ok(()) } else { Err(cfg_err(path, msg)) }
}

fn unit(v: f64) -> bool {
    (0.0..=1.0).contains(&v)
}

pub fn is_date(s: &str) -> bool {
    let p: Vec<&str> = s.split('-').collect();
    let num = |x: &str, n: usize| x.len() == n && x.bytes().all(|b| b.is_ascii_digit());
    if p.len() != 3 || !num(p[0], 4) || !num(p[1], 2) || !num(p[2], 2) {
        return false;
    }
    let (m, d): (u32, u32) = (p[1].parse().unwrap_or(0), p[2].parse().unwrap_or(0));
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

pub fn validate_step(i: usize, step: &Step, n: usize) -> Result<(), Error> {
    let too_long = |name: &str, w: usize| {
        let msg = format!("window {w} > series length {n}");
        check(w <= n, &format!("preprocess[{i}].{name}"), &msg)
    };
    match *step {
        Step::Smooth(w) => too_long("smooth", w),
        Step::Paa(f) => too_long("paa", f),
        Step::Detrend(_) => check(n >= 2, &format!("preprocess[{i}].detrend"), "needs >= 2 points"),
        Step::Nan(_) | Step::Normalize(_) => Ok(()),
    }
}

pub fn yaml_value(key: &str, text: &str) -> Result<Value, Error> {
    serde_yaml_ng::from_str(text).map_err(|e| cfg_err(key, e))
}

pub fn validate_len(n: usize, cfg: &Config) -> Result<(), Error> {
    let need = cfg.motif.min_occ as usize * cfg.motif.min_len as usize;
    if n < need {
        return Err(cfg_err("series", format!("length {n} < min_occ * min_len = {need}")));
    }
    Ok(())
}

fn set_leaf(tree: &mut Value, key: &str, value: Value) -> Result<String, Error> {
    fn find<'a>(node: &'a mut Value, key: &str, path: &str) -> Option<(String, &'a mut Value)> {
        let obj = node.as_object_mut()?;
        for (k, v) in obj.iter_mut() {
            let here = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
            if k == key && !v.is_object() {
                return Some((here, v));
            }
            if let Some(hit) = find(v, key, &here) {
                return Some(hit);
            }
        }
        None
    }
    let (path, slot) = find(tree, key, "").ok_or_else(|| cfg_err(key, "unknown parameter"))?;
    *slot = value;
    Ok(path)
}
