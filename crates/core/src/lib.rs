pub mod bench;
pub mod config;
pub mod distance;
pub mod engine;
pub mod events;
pub mod io;
pub mod metrics;
pub mod motif;
pub mod nsga;
pub mod ops;
pub mod preprocess;
pub mod result;
pub mod series;
pub mod synth;

pub use config::{Config, Error};

use config::validate_len;
use events::Event;
use io::{CsvLoader, Loader};
use metrics::GroundTruth;
use motif::Ctx;
use result::RunResult;
use series::Series;
use std::sync::atomic::AtomicBool;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub type Dated = (Vec<f64>, Option<Vec<String>>);

pub fn load_data(cfg: &Config) -> Result<Dated, Error> {
    let d = &cfg.data;
    if d.ticker.is_some() {
        let msg = "market data is downloaded by the Python package (pip install \
                   'ditscover[finance]'); pass the values instead";
        return Err(config::cfg_err("data.ticker", msg));
    }
    let path = d.path.as_ref().ok_or_else(|| Error::Config {
        path: "data.path".into(),
        msg: "required when no array is given".into(),
    })?;
    let csv =
        CsvLoader::new(path).column(d.column.clone()).delimiter(d.delimiter as u8).header(d.header);
    let times = d.time_column.as_ref().map(|c| csv.text(c)).transpose()?;
    Ok((csv.load()?, times))
}

pub fn discover(values: Vec<f64>, cfg: &Config, cancel: &AtomicBool) -> Result<RunResult, Error> {
    discover_dated(values, None, cfg, cancel)
}

pub fn discover_dated(
    values: Vec<f64>,
    timestamps: Option<Vec<String>>,
    cfg: &Config,
    cancel: &AtomicBool,
) -> Result<RunResult, Error> {
    cfg.validate()?;
    if timestamps.as_ref().is_some_and(|t| t.len() != values.len()) {
        return Err(Error::Data("timestamps and values have different lengths".into()));
    }
    let truth = cfg.eval.ground_truth.as_deref().map(GroundTruth::load).transpose()?;
    let series = preprocess::apply(Series::new(values.clone()), &cfg.preprocess)?;
    if series.values.iter().any(|v| !v.is_finite()) {
        return Err(Error::Data(
            "series has missing or infinite values; add a `nan` preprocessing step".into(),
        ));
    }
    validate_len(series.len(), cfg)?;
    let ctx = Ctx::new(&series.values, cfg);
    let (em, logger) = events::spawn(&cfg.log, cfg.output.dir.as_deref());
    em.send(Event::Start { config: Box::new(cfg.clone()), version: VERSION.into() });
    let outcome = engine::run(&ctx, cfg, cancel, &em);
    if let Ok(o) = &outcome {
        em.send(Event::Finish { reason: o.stop, dropped: em.dropped() });
    }
    let dropped_events = em.dropped();
    drop(em);
    let history = logger.join();
    let outcome = outcome?;
    let (motifs, front) = result::build(&outcome.pop, &ctx, &series);
    let metrics = truth.map(|t| {
        let found: Vec<_> = motifs.iter().map(|m| m.occurrences.clone()).collect();
        metrics::prom(&found, &t.sets, cfg.eval.average, cfg.eval.off_target)
    });
    let res = RunResult {
        motifs,
        front,
        history: history?,
        stop: outcome.stop,
        generations: outcome.generations,
        elapsed_s: outcome.elapsed_s,
        dropped_events,
        config: cfg.clone(),
        version: VERSION.into(),
        series: values,
        metrics,
        timestamps,
    };
    if let Some(dir) = &cfg.output.dir {
        res.save(dir)?;
    }
    Ok(res)
}
