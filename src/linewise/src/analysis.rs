use std::collections::HashMap;

pub fn calculate_entropy(values: &[u8]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut freq: HashMap<u8, usize> = HashMap::new();
    for &v in values {
        *freq.entry(v).or_insert(0) += 1;
    }

    let total = values.len() as f64;
    freq.values()
        .map(|&count| {
            let p = count as f64 / total;
            if p > 0.0 {
                -p * p.log2()
            } else {
                0.0
            }
        })
        .sum()
}

pub fn most_common(values: &[u8]) -> Option<(u8, usize)> {
    if values.is_empty() {
        return None;
    }

    let mut freq: HashMap<u8, usize> = HashMap::new();
    for &v in values {
        *freq.entry(v).or_insert(0) += 1;
    }

    freq.into_iter().max_by_key(|(_, c)| *c)
}

pub fn byte_frequency(values: &[u8]) -> HashMap<u8, usize> {
    let mut freq: HashMap<u8, usize> = HashMap::new();
    for &v in values {
        *freq.entry(v).or_insert(0) += 1;
    }
    freq
}

pub struct PositionStats {
    pub position: usize,
    pub count: usize,
    pub unique: usize,
    pub entropy: f64,
    pub most_common: (u8, usize),
    pub frequency: HashMap<u8, usize>,
}

impl PositionStats {
    pub fn from_records(records: &[&Vec<u8>], position: usize) -> Option<Self> {
        let values: Vec<u8> = records
            .iter()
            .filter_map(|r| r.get(position).copied())
            .collect();

        if values.is_empty() {
            return None;
        }

        let frequency = byte_frequency(&values);
        let unique = frequency.len();
        let entropy = calculate_entropy(&values);
        let most_common = most_common(&values).unwrap_or((0, 0));

        Some(PositionStats {
            position,
            count: values.len(),
            unique,
            entropy,
            most_common,
            frequency,
        })
    }

    pub fn distribution_summary(&self) -> String {
        if self.unique == 1 {
            format!("FIXED: 0x{:02x}", self.most_common.0)
        } else if self.unique <= 4 {
            let mut pairs: Vec<_> = self.frequency.iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(a.1));
            pairs
                .iter()
                .take(4)
                .map(|(v, c)| format!("{:02x}:{}", v, c))
                .collect::<Vec<_>>()
                .join(" ")
        } else if self.entropy < 2.0 {
            format!(
                "LOW-ENT (top: 0x{:02x} {}%)",
                self.most_common.0,
                self.most_common.1 * 100 / self.count
            )
        } else {
            format!("varied ({} unique)", self.unique)
        }
    }
}
