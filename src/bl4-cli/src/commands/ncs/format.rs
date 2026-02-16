//! Output formatting for NCS commands

use bl4_ncs::ParsedDocument;
use std::fmt::Write;

/// Output parsed document as TSV to stdout
pub fn output_tsv(doc: &ParsedDocument) {
    print!("{}", format_tsv(doc));
}

/// Format parsed document as TSV string
pub fn format_tsv(doc: &ParsedDocument) -> String {
    let mut output = String::new();

    for (table_name, table) in &doc.tables {
        writeln!(output, "# {}", table_name).unwrap();

        for (i, record) in table.records.iter().enumerate() {
            for entry in &record.entries {
                write!(output, "record_{}\t{}\t", i, entry.key).unwrap();
                format_value(&entry.value, &mut output);
                writeln!(output).unwrap();
            }
        }
    }

    output
}

fn format_value(value: &bl4_ncs::ParsedValue, output: &mut String) {
    match value {
        bl4_ncs::ParsedValue::Null => write!(output, "null").unwrap(),
        bl4_ncs::ParsedValue::Leaf(s) => write!(output, "{}", s).unwrap(),
        bl4_ncs::ParsedValue::Array(arr) => {
            write!(output, "[").unwrap();
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    write!(output, ", ").unwrap();
                }
                format_value(v, output);
            }
            write!(output, "]").unwrap();
        }
        bl4_ncs::ParsedValue::Map(map) => {
            write!(output, "{{").unwrap();
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    write!(output, ", ").unwrap();
                }
                write!(output, "{}: ", k).unwrap();
                format_value(v, output);
            }
            write!(output, "}}").unwrap();
        }
        bl4_ncs::ParsedValue::Ref { r#ref } => write!(output, "ref({})", r#ref).unwrap(),
    }
}
