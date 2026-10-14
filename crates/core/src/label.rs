use crate::motif::Segment;

pub struct Cand {
    pub occ: Vec<Segment>,
    pub weights: Vec<f64>,
}

// Maximum-weight set of intervals at least `gap` apart (sort by end, DP on the last compatible
// predecessor). Returns the chosen indices in input order.
pub fn schedule(items: &[(Segment, f64)], gap: u32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].0.end(), items[i].0.start, i));
    let ends: Vec<u64> = order.iter().map(|&i| items[i].0.end() as u64 + gap as u64).collect();
    let mut best = vec![0.0f64; order.len() + 1];
    let mut prev = vec![0; order.len()];
    for (j, &i) in order.iter().enumerate() {
        prev[j] = ends[..j].partition_point(|&e| e <= items[i].0.start as u64);
        best[j + 1] = best[j].max(items[i].1 + best[prev[j]]);
    }
    let mut out = Vec::new();
    let mut j = order.len();
    while j > 0 {
        let i = order[j - 1];
        if items[i].1 + best[prev[j - 1]] >= best[j - 1] && best[j] > best[j - 1] {
            out.push(i);
            j = prev[j - 1];
        } else {
            j -= 1;
        }
    }
    out.sort_unstable();
    out
}

// Up to `k` candidates in order, occurrence conflicts resolved by maximum total weight; a motif
// left with fewer than `min_occ` occurrences, or that `ok` rejects, is dropped and the next
// candidate is tried.
pub fn resolve(
    cands: &[Cand],
    k: usize,
    min_occ: usize,
    gap: u32,
    mut ok: impl FnMut(usize, &[Segment]) -> bool,
) -> Vec<(usize, Vec<Segment>)> {
    let mut chosen: Vec<usize> = Vec::new();
    let mut next = 0;
    loop {
        while chosen.len() < k && next < cands.len() {
            chosen.push(next);
            next += 1;
        }
        let items: Vec<(Segment, f64)> = chosen
            .iter()
            .flat_map(|&c| cands[c].occ.iter().copied().zip(cands[c].weights.iter().copied()))
            .collect();
        let owner: Vec<usize> =
            chosen.iter().flat_map(|&c| std::iter::repeat_n(c, cands[c].occ.len())).collect();
        let picked = schedule(&items, gap);
        let kept = |c: usize| -> Vec<Segment> {
            let mut v: Vec<Segment> =
                picked.iter().filter(|&&i| owner[i] == c).map(|&i| items[i].0).collect();
            v.sort();
            v
        };
        // Dropping the lowest-ranked starved motif may free occurrences for the others.
        let starved = chosen.iter().rev().find(|&&c| {
            let occ = kept(c);
            occ.len() < min_occ || occ != cands[c].occ && !ok(c, &occ)
        });
        match starved {
            None => return chosen.iter().map(|&c| (c, kept(c))).collect(),
            Some(&c) => chosen.retain(|&x| x != c),
        }
    }
}

// Motif index per point (occurrences in original coordinates), -1 where none.
pub fn labels(n: usize, motifs: &[Vec<(u32, u32)>]) -> Vec<i32> {
    let mut out = vec![-1; n];
    for (m, occ) in motifs.iter().enumerate() {
        for &(s, len) in occ {
            let end = ((s + len) as usize).min(n);
            out[(s as usize).min(n)..end].fill(m as i32);
        }
    }
    out
}
