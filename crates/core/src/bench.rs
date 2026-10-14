use crate::config::{Error, PERIODS, cfg_err, is_date};
use crate::metrics::{Average, OffTarget};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use serde_yaml_ng::Deserializer;
use serde_yaml_ng::with::singleton_map_recursive;
use std::path::PathBuf;

pub const INTERVALS: [&str; 13] =
    ["1m", "2m", "5m", "15m", "30m", "60m", "90m", "1h", "1d", "5d", "1wk", "1mo", "3mo"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "KRaw", into = "KRaw")]
pub enum K {
    #[default]
    Oracle,
    Fixed(u32),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum KRaw {
    N(u32),
    S(String),
}

impl TryFrom<KRaw> for K {
    type Error = String;
    fn try_from(k: KRaw) -> Result<Self, String> {
        match k {
            KRaw::N(n) => Ok(K::Fixed(n)),
            KRaw::S(s) if s == "oracle" => Ok(K::Oracle),
            KRaw::S(s) => Err(format!("expected `oracle` or a number, got `{s}`")),
        }
    }
}

impl From<K> for KRaw {
    fn from(k: K) -> Self {
        match k {
            K::Oracle => KRaw::S("oracle".into()),
            K::Fixed(n) => KRaw::N(n),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BenchConfig {
    pub datasets: Vec<DatasetSpec>,
    pub methods: Vec<MethodSpec>,
    pub seeds: Vec<u64>,
    pub k: K,
    pub timeout_s: f64,
    pub threads: Option<usize>,
    pub jobs: usize,
    pub eval: BenchEval,
    pub scaling: Option<Scaling>,
    pub tune: Option<Tune>,
    pub max_runs: u64,
    pub stats: bool,
    pub save_sets: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tune {
    pub n_series: usize,
    pub metric: String,
}

impl Default for Tune {
    fn default() -> Self {
        Self { n_series: 10, metric: "f1".into() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct BenchEval {
    pub average: Average,
    pub off_target: OffTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatasetSpec {
    Synthetic(Synthetic),
    Tsmd(Tsmd),
    Files(Files),
    Yfinance(Yfinance),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Synthetic {
    pub name: String,
    pub split: Split,
    pub n_series: usize,
    pub n: usize,
    pub lens: Vec<u32>,
    pub occ: usize,
    pub stretch: f64,
    pub noise: f64,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub enum TsmdName {
    ECG5000,
    Fungi,
    Plane,
    Mallat,
    Symbols,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Split {
    Test,
    Validation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tsmd {
    pub name: TsmdName,
    #[serde(default = "test_split")]
    pub split: Split,
    #[serde(default = "twenty")]
    pub n_series: usize,
}

fn test_split() -> Split {
    Split::Test
}

fn twenty() -> usize {
    20
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Files {
    pub name: String,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Yfinance {
    pub name: String,
    pub tickers: Vec<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub period: Option<String>,
    pub interval: String,
    pub field: String,
    pub min_len: u32,
    pub max_len: u32,
    pub k: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodSpec {
    pub method: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub grid: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scaling {
    pub lengths: Vec<usize>,
    #[serde(default = "three")]
    pub repeats: u32,
    #[serde(default)]
    pub methods: Option<Vec<String>>,
}

fn three() -> u32 {
    3
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            datasets: Vec::new(),
            methods: Vec::new(),
            seeds: vec![0, 1, 2],
            k: K::Oracle,
            timeout_s: 600.0,
            threads: None,
            jobs: 1,
            eval: BenchEval::default(),
            scaling: None,
            tune: None,
            max_runs: 20_000,
            stats: true,
            save_sets: true,
        }
    }
}

impl Default for Synthetic {
    fn default() -> Self {
        Self {
            name: String::new(),
            split: Split::Test,
            n_series: 20,
            n: 5000,
            lens: vec![40, 60, 80],
            occ: 6,
            stretch: 0.3,
            noise: 0.1,
            seed: 0,
        }
    }
}

impl Default for Yfinance {
    fn default() -> Self {
        Self {
            name: String::new(),
            tickers: Vec::new(),
            start: None,
            end: None,
            period: None,
            interval: "1d".into(),
            field: "Close".into(),
            min_len: 0,
            max_len: 0,
            k: 5,
        }
    }
}

impl DatasetSpec {
    pub fn name(&self) -> String {
        match self {
            DatasetSpec::Synthetic(s) => s.name.clone(),
            DatasetSpec::Tsmd(t) => format!("{:?}", t.name),
            DatasetSpec::Files(f) => f.name.clone(),
            DatasetSpec::Yfinance(y) => y.name.clone(),
        }
    }

    pub fn split(&self) -> Split {
        match self {
            DatasetSpec::Synthetic(s) => s.split,
            DatasetSpec::Tsmd(t) => t.split,
            DatasetSpec::Files(_) | DatasetSpec::Yfinance(_) => Split::Test,
        }
    }
}

// A bare override key sets every matching node of the tree (top level and every list entry).
fn set_all(node: &mut Value, key: &str, value: &Value) -> usize {
    match node {
        Value::Object(obj) => obj
            .iter_mut()
            .map(|(k, v)| {
                if k != key {
                    return set_all(v, key, value);
                }
                *v = value.clone();
                1
            })
            .sum(),
        Value::Array(items) => items.iter_mut().map(|v| set_all(v, key, value)).sum(),
        _ => 0,
    }
}

impl BenchConfig {
    pub fn build(
        yaml: Option<&str>,
        base: Option<Value>,
        overrides: &[(String, Value)],
    ) -> Result<Self, Error> {
        let mut cfg: Self = match (yaml, base) {
            (Some(y), _) => singleton_map_recursive::deserialize(Deserializer::from_str(y))
                .map_err(|e| cfg_err("bench", e))?,
            (None, Some(b)) => serde_json::from_value(b).map_err(|e| cfg_err("bench", e))?,
            (None, None) => Self::default(),
        };
        if !overrides.is_empty() {
            let mut tree = serde_json::to_value(&cfg).expect("config serialises");
            for (key, value) in overrides {
                if set_all(&mut tree, key, value) == 0 {
                    return Err(cfg_err(key, "unknown parameter"));
                }
            }
            cfg = serde_json::from_value(tree).map_err(|e| cfg_err("bench", e))?;
        }
        for m in &mut cfg.methods {
            m.label.get_or_insert_with(|| m.method.clone());
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
        check(!self.datasets.is_empty(), "datasets", "must not be empty")?;
        check(!self.methods.is_empty(), "methods", "must not be empty")?;
        check(!self.seeds.is_empty(), "seeds", "must not be empty")?;
        check(self.k != K::Fixed(0), "k", "must be >= 1")?;
        check(self.timeout_s > 0.0, "timeout_s", "must be > 0")?;
        check(self.jobs >= 1, "jobs", "must be >= 1")?;
        check(self.threads.is_none_or(|t| t >= 1), "threads", "must be >= 1")?;
        check(self.max_runs >= 1, "max_runs", "must be >= 1")?;
        if let Some(t) = &self.tune {
            check(t.n_series >= 1, "tune.n_series", "must be >= 1")?;
            check(t.metric == "f1", "tune.metric", "must be f1")?;
        }
        let mut names = Vec::new();
        for (i, d) in self.datasets.iter().enumerate() {
            let id = (d.name(), d.split());
            let key = format!("datasets[{i}].name");
            check(!id.0.is_empty(), &key, "required")?;
            check(!names.contains(&id), &key, &format!("duplicate `{}` on {:?}", id.0, id.1))?;
            validate_dataset(d, &format!("datasets[{i}]"))?;
            names.push(id);
        }
        let mut labels = Vec::new();
        for (i, m) in self.methods.iter().enumerate() {
            let label = m.label.clone().unwrap_or_else(|| m.method.clone());
            let msg = format!("duplicate `{label}`; give each entry a distinct label");
            check(!labels.contains(&label), &format!("methods[{i}].label"), &msg)?;
            labels.push(label);
            self.validate_grid(m, i)?;
        }
        self.scaling.as_ref().map_or(Ok(()), |s| validate_scaling(s, &labels))
    }

    fn validate_grid(&self, m: &MethodSpec, i: usize) -> Result<(), Error> {
        let at = format!("methods[{i}].grid");
        check(m.grid.is_empty() || self.tune.is_some(), &at, "needs `tune`")?;
        for (k, v) in &m.grid {
            let ok = v.as_array().is_some_and(|a| !a.is_empty());
            check(ok, &format!("{at}.{k}"), "must be a non-empty list")?;
        }
        Ok(())
    }
}

fn validate_dataset(d: &DatasetSpec, at: &str) -> Result<(), Error> {
    let key = |k: &str| format!("{at}.{k}");
    match d {
        DatasetSpec::Synthetic(s) => {
            check(s.n_series >= 1, &key("n_series"), "must be >= 1")?;
            check(
                !s.lens.is_empty() && s.lens.iter().all(|&l| l >= 2),
                &key("lens"),
                "must be >= 2",
            )?;
            check(s.occ >= 2, &key("occ"), "must be >= 2")?;
            check((0.0..1.0).contains(&s.stretch), &key("stretch"), "must be in [0, 1)")?;
            check(s.noise >= 0.0, &key("noise"), "must be >= 0")
        }
        DatasetSpec::Tsmd(t) => {
            let max = if t.split == Split::Test { 200 } else { 50 };
            let ok = (1..=max).contains(&t.n_series);
            check(ok, &key("n_series"), &format!("must be in [1, {max}]"))
        }
        DatasetSpec::Files(f) => check(!f.paths.is_empty(), &key("paths"), "must not be empty"),
        DatasetSpec::Yfinance(y) => validate_yfinance(y, at),
    }
}

fn validate_yfinance(y: &Yfinance, at: &str) -> Result<(), Error> {
    let key = |k: &str| format!("{at}.{k}");
    check(!y.tickers.is_empty(), &key("tickers"), "must not be empty")?;
    check(y.min_len >= 1, &key("min_len"), "required (> 0): no ground truth to infer it")?;
    let msg = format!("must be >= min_len ({})", y.min_len);
    check(y.max_len >= y.min_len, &key("max_len"), &msg)?;
    check(y.k >= 1, &key("k"), "must be >= 1")?;
    check(
        y.period.is_none() || y.start.is_none(),
        &key("period"),
        "give period or start, not both",
    )?;
    check(y.period.is_some() || y.start.is_some(), &key("start"), "give start or period")?;
    let ok = y.period.as_deref().is_none_or(|p| PERIODS.contains(&p));
    check(ok, &key("period"), &format!("must be one of {}", PERIODS.join("|")))?;
    for (v, k) in [(&y.start, "start"), (&y.end, "end")] {
        check(v.as_deref().is_none_or(is_date), &key(k), "must be a YYYY-MM-DD date")?;
    }
    let ok = INTERVALS.contains(&y.interval.as_str());
    check(ok, &key("interval"), &format!("must be one of {}", INTERVALS.join("|")))
}

fn validate_scaling(s: &Scaling, labels: &[String]) -> Result<(), Error> {
    let ascending = !s.lengths.is_empty() && s.lengths.windows(2).all(|w| w[0] < w[1]);
    check(ascending, "scaling.lengths", "must be non-empty and strictly ascending")?;
    check(s.lengths[0] >= 500, "scaling.lengths", "must be >= 500")?;
    check(s.repeats >= 1, "scaling.repeats", "must be >= 1")?;
    let known = s.methods.iter().flatten().all(|m| labels.contains(m));
    check(known, "scaling.methods", &format!("must be method labels ({})", labels.join(", ")))
}

fn check(ok: bool, path: &str, msg: &str) -> Result<(), Error> {
    if ok { Ok(()) } else { Err(cfg_err(path, msg)) }
}
