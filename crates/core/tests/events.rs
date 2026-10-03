use ditscover_core::config::{Config, Log};
use ditscover_core::events::{self, Event, GenerationRecord};
use ditscover_core::result::StopReason;
use serde_json::Value;

fn rec(g: u32) -> GenerationRecord {
    GenerationRecord {
        generation: g,
        elapsed_s: g as f64 * 0.1,
        evals: 10 * g as u64,
        hv: 0.5,
        front_size: 3,
        best: [5.0, 0.1, 12.0],
        mean: [3.0, 0.4, 10.0],
        front: (g == 0).then(|| vec![[5.0, 0.1, 12.0]]),
    }
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join("ditscover-events-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn full_channel_never_blocks_and_counts_drops() {
    let (em, rx) = events::channel(2);
    for g in 0..10 {
        em.send(Event::Generation(rec(g)));
    }
    assert_eq!(em.dropped(), 8);
    assert_eq!(rx.len(), 2);
}

#[test]
fn consumer_streams_jsonl_and_returns_history() {
    let dir = tmp("stream");
    let (em, logger) = events::spawn(&Log::default(), Some(&dir));
    em.send(Event::Start { config: Box::new(Config::default()), version: "0.1.0".into() });
    for g in 0..3 {
        em.send(Event::Generation(rec(g)));
    }
    em.send(Event::Finish { reason: StopReason::MaxGenerations, dropped: 0 });
    drop(em);
    let hist = logger.join().unwrap();
    assert_eq!(hist.iter().map(|r| r.generation).collect::<Vec<_>>(), [0, 1, 2]);
    let text = std::fs::read_to_string(dir.join("history.jsonl")).unwrap();
    let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0]["event"], "start");
    assert_eq!(lines[0]["version"], "0.1.0");
    for k in ["gen", "elapsed_s", "evals", "hv", "front_size", "best", "mean", "front"] {
        assert!(lines[1].get(k).is_some(), "missing {k}");
    }
    assert_eq!(lines[1]["event"], "generation");
    assert!(lines[2]["front"].is_null());
    assert_eq!(lines[4]["event"], "finish");
    assert_eq!(lines[4]["reason"], "max_generations");
}

#[test]
fn without_dir_or_history() {
    let (em, logger) = events::spawn(&Log::default(), None);
    em.send(Event::Generation(rec(0)));
    drop(em);
    assert_eq!(logger.join().unwrap().len(), 1);

    let dir = tmp("off");
    let log = Log { history: false, ..Log::default() };
    let (em, logger) = events::spawn(&log, Some(&dir));
    em.send(Event::Generation(rec(0)));
    drop(em);
    assert!(logger.join().unwrap().is_empty());
    assert!(!dir.join("history.jsonl").exists());
}
