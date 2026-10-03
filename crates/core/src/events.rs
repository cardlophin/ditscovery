use crate::config::{Config, Error, Log};
use crate::result::StopReason;
use crossbeam_channel::{Receiver, Sender, bounded};
use serde::{Deserialize, Serialize};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationRecord {
    #[serde(rename = "gen")]
    pub generation: u32,
    pub elapsed_s: f64,
    pub evals: u64,
    pub hv: f64,
    pub front_size: u32,
    pub best: [f64; 3],
    pub mean: [f64; 3],
    pub front: Option<Vec<[f64; 3]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Start { config: Box<Config>, version: String },
    Generation(GenerationRecord),
    Finish { reason: StopReason, dropped: u64 },
}

pub struct Emitter {
    tx: Sender<Event>,
    dropped: Arc<AtomicU64>,
}

impl Emitter {
    pub fn send(&self, event: Event) {
        if self.tx.try_send(event).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

pub fn channel(capacity: usize) -> (Emitter, Receiver<Event>) {
    let (tx, rx) = bounded(capacity);
    (Emitter { tx, dropped: Arc::default() }, rx)
}

pub struct Logger(JoinHandle<Result<Vec<GenerationRecord>, Error>>);

impl Logger {
    pub fn join(self) -> Result<Vec<GenerationRecord>, Error> {
        self.0.join().expect("logger thread panicked")
    }
}

pub fn spawn(log: &Log, dir: Option<&Path>) -> (Emitter, Logger) {
    let (em, rx) = channel(log.channel_capacity);
    let file = dir.filter(|_| log.history).map(|d| d.join("history.jsonl"));
    let (keep, every) = (log.history, log.log_every);
    let handle = std::thread::spawn(move || consume(rx, file, keep, every));
    (em, Logger(handle))
}

fn log(event: &Event, every: u32) {
    match event {
        Event::Generation(r) if r.generation % every == 0 => tracing::info!(
            "gen {} hv {:.4} front {} best support {} cohesion {:.4} length {:.1}",
            r.generation,
            r.hv,
            r.front_size,
            r.best[0],
            r.best[1],
            r.best[2]
        ),
        Event::Generation(_) => {}
        Event::Start { version, .. } => tracing::debug!("ditscover {version} started"),
        Event::Finish { reason, dropped } => {
            tracing::info!("finished: {reason:?}, {dropped} log events dropped")
        }
    }
}

fn consume(
    rx: Receiver<Event>,
    file: Option<PathBuf>,
    keep: bool,
    every: u32,
) -> Result<Vec<GenerationRecord>, Error> {
    let mut out = match &file {
        Some(p) => Some(create(p).map_err(|source| Error::Io { path: p.clone(), source })?),
        None => None,
    };
    let mut history = Vec::new();
    for event in rx {
        if let (Some(w), Some(p)) = (out.as_mut(), &file) {
            write_line(w, &event).map_err(|source| Error::Io { path: p.clone(), source })?;
        }
        log(&event, every);
        if let (Event::Generation(r), true) = (event, keep) {
            history.push(r);
        }
    }
    if let (Some(mut w), Some(p)) = (out, file) {
        w.flush().map_err(|source| Error::Io { path: p, source })?;
    }
    Ok(history)
}

fn create(path: &Path) -> std::io::Result<BufWriter<std::fs::File>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(BufWriter::new(std::fs::File::create(path)?))
}

pub(crate) fn write_line(w: &mut impl Write, event: &Event) -> std::io::Result<()> {
    serde_json::to_writer(&mut *w, event)?;
    w.write_all(b"\n")
}
