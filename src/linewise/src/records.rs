use std::collections::HashMap;

pub fn group_by_position(records: &[Vec<u8>], position: usize) -> HashMap<u8, Vec<&Vec<u8>>> {
    let mut groups: HashMap<u8, Vec<&Vec<u8>>> = HashMap::new();

    for record in records {
        if let Some(&byte) = record.get(position) {
            groups.entry(byte).or_default().push(record);
        }
    }

    groups
}

pub fn filter_by_position(records: &[Vec<u8>], position: usize, value: u8) -> Vec<&Vec<u8>> {
    records
        .iter()
        .filter(|r| r.get(position) == Some(&value))
        .collect()
}
