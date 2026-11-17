use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct WeightedSection {
    pub id: String,
    pub weight: f64,
}

/// Largest remainder. Remainder ties break toward the smaller section id,
/// which matches FINRA's published SIE and Series 7 counts.
pub fn allocate_questions(sections: &[WeightedSection], total: usize) -> HashMap<String, usize> {
    let mut result: HashMap<String, usize> = sections.iter().map(|section| (section.id.clone(), 0)).collect();
    if total == 0 || sections.is_empty() {
        return result;
    }
    let weight_sum: f64 = sections.iter().map(|section| section.weight).sum();
    if weight_sum <= 0.0 {
        return result;
    }
    let mut rows: Vec<(String, usize, f64)> = sections
        .iter()
        .map(|section| {
            let exact = (section.weight / weight_sum) * total as f64;
            let n = exact.floor() as usize;
            (section.id.clone(), n, exact - n as f64)
        })
        .collect();
    let mut used: usize = rows.iter().map(|row| row.1).sum();
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| rows[b].2.partial_cmp(&rows[a].2).unwrap_or(std::cmp::Ordering::Equal).then_with(|| rows[a].0.cmp(&rows[b].0)));
    let mut index = 0;
    while used < total {
        let slot = order[index % order.len()];
        rows[slot].1 += 1;
        used += 1;
        index += 1;
    }
    for (id, n, _) in rows {
        result.insert(id, n);
    }
    result
}

#[derive(Debug, Clone)]
pub struct DrawItem<T> {
    pub id: String,
    pub section: String,
    pub value: T,
}

pub fn draw_weighted<T, F>(items: &[DrawItem<T>], sections: &[WeightedSection], count: usize, mut rng: F) -> Vec<DrawItem<T>>
where
    T: Clone,
    F: FnMut() -> f64,
{
    let target = count.min(items.len());
    if target == 0 {
        return Vec::new();
    }
    let quotas = allocate_questions(sections, target);
    let mut pools: HashMap<String, Vec<DrawItem<T>>> = HashMap::new();
    for item in items {
        pools.entry(item.section.clone()).or_default().push(item.clone());
    }
    for pool in pools.values_mut() {
        shuffle(pool, &mut rng);
    }
    let mut picked = Vec::new();
    for section in sections {
        let pool = pools.get(&section.id).cloned().unwrap_or_default();
        let take = quotas.get(&section.id).copied().unwrap_or(0).min(pool.len());
        picked.extend(pool.into_iter().take(take));
    }
    if picked.len() < target {
        let picked_ids: HashSet<String> = picked.iter().map(|item| item.id.clone()).collect();
        let mut rest: Vec<DrawItem<T>> = items
            .iter()
            .filter(|item| !picked_ids.contains(&item.id))
            .cloned()
            .collect();
        shuffle(&mut rest, &mut rng);
        while picked.len() < target {
            if let Some(next) = rest.pop() {
                picked.push(next);
            } else {
                break;
            }
        }
    }
    shuffle(&mut picked, &mut rng);
    picked
}

pub fn shuffle<T>(list: &mut [T], rng: &mut dyn FnMut() -> f64) {
    for index in (1..list.len()).rev() {
        let swap_at = (rng() * (index as f64 + 1.0)).floor() as usize;
        list.swap(index, swap_at.min(index));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_finra_counts() {
        let sie = sections(&[("SIE-1", 16.0), ("SIE-2", 44.0), ("SIE-3", 31.0), ("SIE-4", 9.0)]);
        let got = allocate_questions(&sie, 75);
        assert_eq!(got["SIE-1"], 12);
        assert_eq!(got["SIE-2"], 33);
        assert_eq!(got["SIE-3"], 23);
        assert_eq!(got["SIE-4"], 7);
        let s7 = sections(&[("S7-1", 7.0), ("S7-2", 9.0), ("S7-3", 73.0), ("S7-4", 11.0)]);
        let got = allocate_questions(&s7, 125);
        assert_eq!(got["S7-1"], 9);
        assert_eq!(got["S7-2"], 11);
        assert_eq!(got["S7-3"], 91);
        assert_eq!(got["S7-4"], 14);
    }

    #[test]
    fn sums_to_the_total() {
        let sie = sections(&[("SIE-1", 16.0), ("SIE-2", 44.0), ("SIE-3", 31.0), ("SIE-4", 9.0)]);
        let got = allocate_questions(&sie, 10);
        assert_eq!(got.values().sum::<usize>(), 10);
    }

    fn sections(rows: &[(&str, f64)]) -> Vec<WeightedSection> {
        rows.iter()
            .map(|(id, weight)| WeightedSection {
                id: (*id).into(),
                weight: *weight,
            })
            .collect()
    }
}
