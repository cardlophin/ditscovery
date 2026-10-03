pub mod config;
pub mod distance;
pub mod engine;
pub mod events;
pub mod io;
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
use motif::Ctx;
use result::RunResult;
use series::Series;
use std::sync::atomic::AtomicBool;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn load_data(cfg: &Config) -> Result<Vec<f64>, Error> {
    let d = &cfg.data;
    let path = d.path.as_ref().ok_or_else(|| Error::Config {
        path: "data.path".into(),
        msg: "required when no array is given".into(),
    })?;
    CsvLoader::new(path)
        .column(d.column.clone())
        .delimiter(d.delimiter as u8)
        .header(d.header)
        .load()
}

pub fn discover(values: Vec<f64>, cfg: &Config, cancel: &AtomicBool) -> Result<RunResult, Error> {
    cfg.validate()?;
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
    };
    if let Some(dir) = &cfg.output.dir {
        res.save(dir)?;
    }
    Ok(res)
}
