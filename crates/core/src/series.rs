#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub values: Vec<f64>,
    pub origin: Vec<u32>,
    pub scale: u32,
}

impl Series {
    pub fn new(values: Vec<f64>) -> Self {
        let origin = (0..values.len() as u32).collect();
        Self { values, origin, scale: 1 }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn to_original(&self, start: u32, len: u32) -> (u32, u32) {
        let s = self.origin[start as usize];
        (s, self.origin[(start + len - 1) as usize] + self.scale - s)
    }
}

pub struct Stats {
    sum: Vec<f64>,
    sum2: Vec<f64>,
}

impl Stats {
    pub fn new(x: &[f64]) -> Self {
        let mut sum = Vec::with_capacity(x.len() + 1);
        let mut sum2 = Vec::with_capacity(x.len() + 1);
        let (mut s, mut s2) = (0.0, 0.0);
        sum.push(0.0);
        sum2.push(0.0);
        for &v in x {
            s += v;
            s2 += v * v;
            sum.push(s);
            sum2.push(s2);
        }
        Self { sum, sum2 }
    }

    pub fn mean_std(&self, start: usize, len: usize) -> (f64, f64) {
        let n = len as f64;
        let mean = (self.sum[start + len] - self.sum[start]) / n;
        let var = (self.sum2[start + len] - self.sum2[start]) / n - mean * mean;
        (mean, var.max(0.0).sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_and_origin() {
        let st = Stats::new(&[1.0, 2.0, 3.0, 4.0, 4.0]);
        let (m, s) = st.mean_std(1, 3);
        assert!((m - 3.0).abs() < 1e-12 && (s - (2.0f64 / 3.0).sqrt()).abs() < 1e-12);
        assert_eq!(st.mean_std(3, 2).1, 0.0);
        let s = Series { values: vec![0.0; 3], origin: vec![0, 4, 8], scale: 4 };
        assert_eq!(s.to_original(1, 2), (4, 8));
        assert_eq!(Series::new(vec![0.0; 5]).to_original(2, 3), (2, 3));
    }
}
