use crate::config::{Error, validate_step};
use crate::series::Series;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    Nan(Fill),
    Detrend(Trend),
    Smooth(usize),
    Normalize(Norm),
    Paa(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    Interpolate,
    Ffill,
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Trend {
    Linear,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Norm {
    Zscore,
    Minmax,
}

pub fn apply(mut s: Series, steps: &[Step]) -> Result<Series, Error> {
    for (i, step) in steps.iter().enumerate() {
        validate_step(i, step, s.len())?;
        s = match *step {
            Step::Nan(Fill::Drop) => drop_nan(s),
            Step::Nan(fill) => Series { values: fill_nan(&s.values, fill), ..s },
            Step::Detrend(Trend::Linear) => Series { values: detrend(&s.values), ..s },
            Step::Detrend(Trend::Diff) => Series {
                values: s.values.windows(2).map(|w| w[1] - w[0]).collect(),
                origin: s.origin[1..].to_vec(),
                scale: s.scale,
            },
            Step::Smooth(w) => Series { values: smooth(&s.values, w), ..s },
            Step::Normalize(norm) => Series { values: normalize(&s.values, norm), ..s },
            Step::Paa(f) => paa(&s, f),
        };
    }
    Ok(s)
}

fn finite(x: &[f64]) -> impl Iterator<Item = (usize, f64)> + '_ {
    x.iter().copied().enumerate().filter(|(_, v)| v.is_finite())
}

fn drop_nan(s: Series) -> Series {
    let keep: Vec<usize> = finite(&s.values).map(|(i, _)| i).collect();
    Series {
        values: keep.iter().map(|&i| s.values[i]).collect(),
        origin: keep.iter().map(|&i| s.origin[i]).collect(),
        scale: s.scale,
    }
}

// Gaps are filled linearly (interpolate) or with the previous value (ffill); edges take the
// nearest valid value.
fn fill_nan(x: &[f64], fill: Fill) -> Vec<f64> {
    let valid: Vec<(usize, f64)> = finite(x).collect();
    let Some(&(first, v0)) = valid.first() else { return x.to_vec() };
    let mut out = x.to_vec();
    out[..first].fill(v0);
    for w in valid.windows(2) {
        let ((a, va), (b, vb)) = (w[0], w[1]);
        for (i, o) in out.iter_mut().enumerate().take(b).skip(a + 1) {
            *o = match fill {
                Fill::Interpolate => va + (vb - va) * (i - a) as f64 / (b - a) as f64,
                _ => va,
            };
        }
    }
    let (last, vl) = valid[valid.len() - 1];
    out[last + 1..].fill(vl);
    out
}

fn detrend(x: &[f64]) -> Vec<f64> {
    let pts: Vec<(f64, f64)> = finite(x).map(|(i, v)| (i as f64, v)).collect();
    let n = pts.len() as f64;
    let (mx, my) = pts.iter().fold((0.0, 0.0), |(a, b), &(i, v)| (a + i / n, b + v / n));
    let sxx: f64 = pts.iter().map(|&(i, _)| (i - mx).powi(2)).sum();
    let sxy: f64 = pts.iter().map(|&(i, v)| (i - mx) * (v - my)).sum();
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    x.iter().enumerate().map(|(i, v)| v - my - slope * (i as f64 - mx)).collect()
}

fn smooth(x: &[f64], w: usize) -> Vec<f64> {
    let mut prefix = vec![0.0; x.len() + 1];
    for (i, v) in x.iter().enumerate() {
        prefix[i + 1] = prefix[i] + v;
    }
    (0..x.len())
        .map(|i| {
            let (lo, hi) = (i.saturating_sub((w - 1) / 2), (i + w / 2).min(x.len() - 1));
            (prefix[hi + 1] - prefix[lo]) / (hi + 1 - lo) as f64
        })
        .collect()
}

fn normalize(x: &[f64], norm: Norm) -> Vec<f64> {
    let vals: Vec<f64> = finite(x).map(|(_, v)| v).collect();
    let n = vals.len() as f64;
    let (shift, scale) = match norm {
        Norm::Zscore => {
            let mean = vals.iter().sum::<f64>() / n;
            (mean, (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n).sqrt())
        }
        Norm::Minmax => {
            let (lo, hi) =
                vals.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
            (lo, hi - lo)
        }
    };
    x.iter().map(|v| if scale > 0.0 { (v - shift) / scale } else { 0.0 }).collect()
}

fn paa(s: &Series, f: usize) -> Series {
    let blocks = s.len() / f;
    Series {
        values: (0..blocks)
            .map(|b| s.values[b * f..(b + 1) * f].iter().sum::<f64>() / f as f64)
            .collect(),
        origin: (0..blocks).map(|b| s.origin[b * f]).collect(),
        scale: s.scale * f as u32,
    }
}
