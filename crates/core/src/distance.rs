#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Distance {
    ZEuclid,
    Euclid,
    Dtw { band: f64 },
    ZDtw { band: f64 },
}

impl Distance {
    pub fn znorm(&self) -> bool {
        matches!(self, Distance::ZEuclid | Distance::ZDtw { .. })
    }

    pub fn is_dtw(&self) -> bool {
        matches!(self, Distance::Dtw { .. } | Distance::ZDtw { .. })
    }

    pub fn dist(&self, a: &[f64], b: &[f64], row: &mut [f64]) -> f64 {
        match *self {
            Distance::ZEuclid | Distance::Euclid => euclid(a, b),
            Distance::Dtw { band } | Distance::ZDtw { band } => {
                dtw(a, b, (band * a.len() as f64).ceil() as usize, row)
            }
        }
    }
}

fn complexity(a: &[f64]) -> f64 {
    a.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f64>().sqrt()
}

// Batista et al. (2014): >= 1, and 1 when both inputs are equally complex.
pub fn cid_factor(a: &[f64], b: &[f64]) -> f64 {
    let (ca, cb) = (complexity(a), complexity(b));
    let lo = ca.min(cb);
    if lo < 1e-12 { if ca.max(cb) < 1e-12 { 1.0 } else { f64::INFINITY } } else { ca.max(cb) / lo }
}

pub fn resample(src: &[f64], dst: &mut [f64]) {
    let (n, m) = (src.len(), dst.len());
    if n == 1 || m == 1 {
        dst.fill(src[0]);
        return;
    }
    let step = (n - 1) as f64 / (m - 1) as f64;
    for (j, d) in dst.iter_mut().enumerate() {
        let t = j as f64 * step;
        let i = (t as usize).min(n - 2);
        *d = src[i] + (src[i + 1] - src[i]) * (t - i as f64);
    }
}

pub fn znorm(v: &mut [f64]) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    let std = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    if std < 1e-8 {
        v.fill(0.0);
    } else {
        v.iter_mut().for_each(|x| *x = (*x - mean) / std);
    }
}

pub fn euclid(a: &[f64], b: &[f64]) -> f64 {
    let s: f64 = a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum();
    (s / a.len() as f64).sqrt()
}

// Sakoe-Chiba DTW with a single row; `row` needs len >= b.len() + 1.
pub fn dtw(a: &[f64], b: &[f64], w: usize, row: &mut [f64]) -> f64 {
    let (n, m) = (a.len(), b.len());
    row[0] = 0.0;
    row[1..=m].fill(f64::INFINITY);
    for i in 1..=n {
        let (lo, hi) = (i.saturating_sub(w).max(1), (i + w).min(m));
        let mut diag = row[lo - 1];
        row[lo - 1] = f64::INFINITY;
        for j in lo..=hi {
            let best = diag.min(row[j]).min(row[j - 1]);
            diag = row[j];
            row[j] = (a[i - 1] - b[j - 1]).powi(2) + best;
        }
    }
    (row[m] / n as f64).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12)
    }

    #[test]
    fn resample_identity_and_endpoints() {
        let src = [1.0, 2.0, 5.0, 4.0];
        let mut dst = [0.0; 4];
        resample(&src, &mut dst);
        assert!(close(&dst, &src));
        let mut up = [0.0; 5];
        resample(&[0.0, 10.0], &mut up);
        assert!(close(&up, &[0.0, 2.5, 5.0, 7.5, 10.0]));
        let mut down = [0.0; 3];
        resample(&[0.0, 1.0, 2.0, 3.0, 4.0], &mut down);
        assert!(close(&down, &[0.0, 2.0, 4.0]));
    }

    #[test]
    fn znorm_flat_is_zero_and_unit_otherwise() {
        let mut flat = [3.0; 6];
        znorm(&mut flat);
        assert!(flat.iter().all(|&v| v == 0.0));
        let mut v = [1.0, 2.0, 3.0, 4.0];
        znorm(&mut v);
        let mean = v.iter().sum::<f64>() / 4.0;
        let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / 4.0;
        assert!(mean.abs() < 1e-12 && (var - 1.0).abs() < 1e-12);
    }

    #[test]
    fn dtw_bounds() {
        let a = [0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0, 1.0];
        let b = [0.0, 0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0];
        let mut row = vec![0.0; a.len() + 1];
        let e = euclid(&a, &b);
        assert!((dtw(&a, &b, 0, &mut row) - e).abs() < 1e-12);
        let d = dtw(&a, &b, 2, &mut row);
        assert!(d <= e && d < e * 0.5);
        assert_eq!(dtw(&a, &a, 2, &mut row), 0.0);
    }

    #[test]
    fn cid_factor_is_at_least_one() {
        let a = [0.0, 1.0, 0.0, 1.0, 0.0];
        let b = [0.0, 2.0, 0.0, 2.0, 0.0];
        assert_eq!(cid_factor(&a, &a), 1.0);
        assert_eq!(cid_factor(&a, &[1.0, 0.0, 1.0, 0.0, 1.0]), 1.0);
        assert!((cid_factor(&a, &b) - 2.0).abs() < 1e-12);
        assert_eq!(cid_factor(&b, &a), cid_factor(&a, &b));
        assert_eq!(cid_factor(&[1.0; 4], &[2.0; 4]), 1.0);
    }

    #[test]
    fn zdtw_normalises_and_bands_like_dtw() {
        let d = Distance::ZDtw { band: 0.2 };
        assert!(d.znorm() && d.is_dtw() && !Distance::ZEuclid.is_dtw());
        let mut row = vec![0.0; 9];
        let (a, b) =
            ([0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 2.0, 1.0, 0.0, -1.0, 0.0]);
        assert_eq!(d.dist(&a, &b, &mut row), Distance::Dtw { band: 0.2 }.dist(&a, &b, &mut row));
    }

    #[test]
    fn distance_kinds() {
        let mut row = vec![0.0; 5];
        let (a, b) = ([1.0, 2.0, 3.0, 4.0], [1.0, 2.0, 3.0, 6.0]);
        assert!((Distance::Euclid.dist(&a, &b, &mut row) - 1.0).abs() < 1e-12);
        let dtw = Distance::Dtw { band: 0.5 }.dist(&a, &b, &mut row);
        assert!(dtw <= 1.0 + 1e-12);
        assert!(Distance::ZEuclid.znorm() && !Distance::Euclid.znorm());
    }
}
