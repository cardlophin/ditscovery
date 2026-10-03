use ditscover_core::config::{Config, yaml_value};
use ditscover_core::result::RunResult;
use ditscover_core::{Error, VERSION, load_data, synth};
use numpy::PyReadonlyArray1;
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::{depythonize, pythonize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use tracing_subscriber::filter::LevelFilter;

fn err(e: Error) -> PyErr {
    match e {
        Error::Io { .. } => PyOSError::new_err(e.to_string()),
        _ => PyValueError::new_err(e.to_string()),
    }
}

fn pairs(dict: Option<&Bound<'_, PyDict>>, yaml: bool) -> PyResult<Vec<(String, Value)>> {
    let Some(dict) = dict else { return Ok(Vec::new()) };
    dict.iter()
        .map(|(k, v)| {
            let key: String = k.extract()?;
            let value = match yaml {
                true => yaml_value(&key, &v.extract::<String>()?).map_err(err)?,
                false => {
                    depythonize(&v).map_err(|e| PyValueError::new_err(format!("{key}: {e}")))?
                }
            };
            Ok((key, value))
        })
        .collect()
}

#[pyfunction]
#[pyo3(signature = (path=None, base=None, overrides=None, yaml_overrides=None))]
fn build_config<'py>(
    py: Python<'py>,
    path: Option<String>,
    base: Option<&Bound<'py, PyAny>>,
    overrides: Option<&Bound<'py, PyDict>>,
    yaml_overrides: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let yaml = match &path {
        Some(p) => {
            Some(std::fs::read_to_string(p).map_err(|e| PyOSError::new_err(format!("{p}: {e}")))?)
        }
        None => None,
    };
    let base: Option<Value> = base.map(depythonize).transpose()?;
    let mut ov = pairs(overrides, false)?;
    ov.extend(pairs(yaml_overrides, true)?);
    let cfg = Config::build(yaml.as_deref(), base, &ov).map_err(err)?;
    Ok(pythonize(py, &cfg)?)
}

fn init_tracing(level: &str) {
    let level: LevelFilter = level.parse().unwrap_or(LevelFilter::INFO);
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(level)
        .with_target(false)
        .try_init();
}

// Runs the engine off the GIL and polls Python signals every 100 ms; Ctrl+C cancels the run
// and the partial result is returned.
#[pyfunction]
fn discover<'py>(
    py: Python<'py>,
    values: Option<PyReadonlyArray1<'py, f64>>,
    config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let cfg: Config = depythonize(config)?;
    init_tracing(&cfg.log.level);
    let values = match values {
        Some(a) => a.as_slice()?.to_vec(),
        None => py.detach(|| load_data(&cfg)).map_err(err)?,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let rx = Mutex::new(rx);
    let flag = cancel.clone();
    std::thread::spawn(move || {
        let _ = tx.send(ditscover_core::discover(values, &cfg, &flag));
    });
    let result = loop {
        let wait = || rx.lock().expect("receiver lock").recv_timeout(Duration::from_millis(100));
        match py.detach(wait) {
            Ok(r) => break r,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if py.check_signals().is_err() {
                    cancel.store(true, Ordering::Relaxed);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(PyRuntimeError::new_err("discovery thread panicked"));
            }
        }
    };
    Ok(pythonize(py, &result.map_err(err)?)?)
}

#[pyfunction]
fn load_result<'py>(py: Python<'py>, dir: PathBuf) -> PyResult<Bound<'py, PyAny>> {
    Ok(pythonize(py, &RunResult::load(&dir).map_err(err)?)?)
}

#[pyfunction]
fn save_result(result: &Bound<'_, PyAny>, dir: PathBuf) -> PyResult<()> {
    let res: RunResult = depythonize(result)?;
    res.save(&dir).map_err(err)
}

#[pyfunction]
fn synthetic(
    n: usize,
    lens: Vec<u32>,
    occ: usize,
    stretch: f64,
    noise: f64,
    seed: u64,
) -> (Vec<f64>, Vec<Vec<(u32, u32)>>) {
    synth::planted(n, &lens, occ, stretch, noise, seed)
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", VERSION)?;
    m.add_function(wrap_pyfunction!(build_config, m)?)?;
    m.add_function(wrap_pyfunction!(discover, m)?)?;
    m.add_function(wrap_pyfunction!(load_result, m)?)?;
    m.add_function(wrap_pyfunction!(save_result, m)?)?;
    m.add_function(wrap_pyfunction!(synthetic, m)?)?;
    Ok(())
}
