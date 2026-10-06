use crate::bench::Synthetic;
use crate::config::{Error, cfg_err};
use crate::metrics::GroundTruth;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::{Distribution, Normal};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

fn shape(kind: usize, t: f64) -> f64 {
    match kind % 3 {
        0 => (2.0 * PI * 2.0 * t).sin() * (-2.0 * t).exp(),
        1 => 1.0 - (2.0 * t - 1.0).abs() * 2.0,
        _ => (8.0 * (t - 0.5)).tanh() * (1.0 + (kind / 3) as f64 * 0.5),
    }
}

fn background(n: usize, rng: &mut ChaCha8Rng) -> Vec<f64> {
    let step = Normal::new(0.0, 0.05).expect("valid sigma");
    let mut b = 0.0;
    (0..n)
        .map(|_| {
            b = 0.98 * b + step.sample(rng);
            b
        })
        .collect()
}

// Mean-reverting random walk background; each occurrence replaces the background with
// `level + shape` (amplitude 1), so occurrences differ only by duration and gaussian noise.
pub fn planted(
    n: usize,
    lens: &[u32],
    occ: usize,
    stretch: f64,
    noise: f64,
    seed: u64,
) -> (Vec<f64>, Vec<Vec<(u32, u32)>>) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut x = background(n, &mut rng);
    let mut items: Vec<(usize, u32)> = (0..lens.len())
        .flat_map(|m| (0..occ).map(move |_| m))
        .map(|m| {
            (m, (lens[m] as f64 * rng.random_range(1.0 - stretch..=1.0 + stretch)).round() as u32)
        })
        .collect();
    items.shuffle(&mut rng);
    let used: usize = items.iter().map(|&(_, l)| l as usize).sum();
    assert!(used < n, "series too short for the planted motifs");
    let weights: Vec<f64> = (0..=items.len()).map(|_| rng.random::<f64>()).collect();
    let total: f64 = weights.iter().sum();
    let mut out = vec![Vec::new(); lens.len()];
    let mut pos = 0usize;
    for (&(m, len), w) in items.iter().zip(&weights) {
        pos += (w / total * (n - used) as f64) as usize;
        let level = x[pos];
        for j in 0..len as usize {
            x[pos + j] = level + shape(m, j as f64 / (len - 1) as f64);
        }
        out[m].push((pos as u32, len));
        pos += len as usize;
    }
    let noise = Normal::new(0.0, noise).expect("valid sigma");
    x.iter_mut().for_each(|v| *v += noise.sample(&mut rng));
    out.iter_mut().for_each(|o| o.sort());
    (x, out)
}

// Series i uses seed + i; files are `<dir>/<i>.json` ground-truth files.
pub fn suite(dir: &Path, s: &Synthetic) -> Result<Vec<PathBuf>, Error> {
    let need: f64 = s.lens.iter().map(|&l| (l as f64 * (1.0 + s.stretch)).round() + 1.0).sum();
    if need * s.occ as f64 >= s.n as f64 {
        let msg = format!("n = {} too short for {} occurrences of {:?}", s.n, s.occ, s.lens);
        return Err(cfg_err("n", msg));
    }
    std::fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.into(), source })?;
    (0..s.n_series)
        .map(|i| {
            let (series, sets) =
                planted(s.n, &s.lens, s.occ, s.stretch, s.noise, s.seed + i as u64);
            let path = dir.join(format!("{i}.json"));
            GroundTruth { name: format!("{}#{i}", s.name), series, sets }.save(&path)?;
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_writes_ground_truth_files() {
        let dir = std::env::temp_dir().join("ditscover-suite-test");
        let s = Synthetic {
            name: "s".into(),
            n_series: 2,
            n: 1000,
            lens: vec![30],
            occ: 3,
            ..Default::default()
        };
        let files = suite(&dir, &s).unwrap();
        assert_eq!(files.len(), 2);
        let gt = GroundTruth::load(&files[1]).unwrap();
        assert_eq!((gt.name.as_str(), gt.series.len(), gt.sets[0].len()), ("s#1", 1000, 3));
        assert_eq!(gt.series, planted(1000, &[30], 3, 0.3, 0.1, 1).0);
        let short = Synthetic { n: 100, ..s };
        assert!(suite(&dir, &short).unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn planted_occurrences_are_valid() {
        let lens = [40, 60, 80];
        let (x, motifs) = planted(5000, &lens, 8, 0.3, 0.1, 1);
        assert_eq!(x.len(), 5000);
        assert!(x.iter().all(|v| v.is_finite()));
        assert_eq!(motifs.len(), 3);
        let mut all: Vec<(u32, u32)> = motifs.iter().flatten().copied().collect();
        all.sort();
        assert!(all.windows(2).all(|w| w[0].0 + w[0].1 <= w[1].0));
        assert!(all.last().is_none_or(|&(s, l)| s + l <= 5000));
        for (m, occ) in motifs.iter().enumerate() {
            assert_eq!(occ.len(), 8);
            let (lo, hi) = (lens[m] as f64 * 0.7, lens[m] as f64 * 1.3);
            assert!(occ.iter().all(|&(_, l)| l as f64 >= lo.floor() && l as f64 <= hi.ceil()));
        }
        assert_eq!(planted(5000, &lens, 8, 0.3, 0.1, 1).0, x);
    }
}
