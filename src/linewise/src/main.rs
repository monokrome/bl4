mod analysis;
mod commands;
mod config;
mod interactive;
mod records;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "linewise")]
#[command(about = "Pattern analysis tool for record-oriented data")]
struct Cli {
    /// Open file in interactive mode
    #[arg(short = 'i', long = "interactive", global = true)]
    interactive: Option<PathBuf>,

    /// Input format for -i mode
    #[arg(
        short = 'f',
        long = "format",
        default_value = "length16",
        global = true
    )]
    format: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze byte patterns across records
    Analyze {
        /// Input file
        input: PathBuf,

        /// Input format: 'lines' (hex per line), 'length16' (u16 length-prefixed binary)
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Maximum positions to analyze
        #[arg(short = 'n', long, default_value = "64")]
        max_positions: usize,

        /// Show bit-level analysis
        #[arg(long)]
        bits: bool,
    },

    /// Find common byte sequences (n-grams)
    Ngrams {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// N-gram size in bytes
        #[arg(short = 'n', long, default_value = "4")]
        size: usize,

        /// Minimum occurrences to report
        #[arg(short, long, default_value = "10")]
        min_count: usize,
    },

    /// Show entropy per position
    Entropy {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Maximum positions to analyze
        #[arg(short = 'n', long, default_value = "64")]
        max_positions: usize,
    },

    /// Compare two sets of records to find differing positions
    Diff {
        /// First input file
        file_a: PathBuf,

        /// Second input file
        file_b: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,
    },

    /// Group records by byte value at a position and analyze each group
    Group {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Position to group by
        #[arg(short = 'p', long)]
        position: usize,

        /// Maximum positions to analyze per group
        #[arg(short = 'n', long, default_value = "32")]
        max_positions: usize,
    },

    /// Filter records by byte value and analyze
    Filter {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Position to filter on
        #[arg(short = 'p', long)]
        position: usize,

        /// Value to match (hex, e.g. '21' or '0x21')
        #[arg(short = 'v', long)]
        value: String,

        /// Maximum positions to analyze
        #[arg(short = 'n', long, default_value = "64")]
        max_positions: usize,
    },

    /// Compare groups side-by-side to find differing positions
    Compare {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Position to group by
        #[arg(short = 'p', long)]
        position: usize,

        /// Maximum positions to compare
        #[arg(short = 'n', long, default_value = "32")]
        max_positions: usize,
    },

    /// Split records into separate files by header bytes
    Split {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Number of header bytes to group by
        #[arg(short = 'n', long, default_value = "4")]
        header_len: usize,

        /// Output directory
        #[arg(short, long, default_value = ".")]
        output_dir: PathBuf,
    },

    /// Analyze (position, value) frequency to find field boundaries
    Frequency {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Maximum positions to analyze
        #[arg(short = 'n', long, default_value = "64")]
        max_positions: usize,

        /// Minimum frequency % to highlight as potential boundary
        #[arg(short = 't', long, default_value = "80")]
        threshold: usize,
    },

    /// Detect field boundaries from frequency patterns
    Boundaries {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,

        /// Maximum positions to analyze
        #[arg(short = 'n', long, default_value = "64")]
        max_positions: usize,
    },

    /// Interactive TUI for exploring binary data
    #[command(name = "interactive", alias = "i")]
    Interactive {
        /// Input file
        input: PathBuf,

        /// Input format
        #[arg(short, long, default_value = "length16")]
        format: String,
    },
}

fn read_records(path: &PathBuf, format: &str) -> Result<Vec<Vec<u8>>> {
    let file = File::open(path).with_context(|| format!("Failed to open {:?}", path))?;

    match format {
        "length16" => {
            let mut reader = BufReader::new(file);
            let mut records = Vec::new();

            loop {
                let mut len_buf = [0u8; 2];
                match reader.read_exact(&mut len_buf) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e.into()),
                }

                let len = u16::from_le_bytes(len_buf) as usize;
                if len == 0 {
                    records.push(Vec::new());
                    continue;
                }

                let mut data = vec![0u8; len];
                reader.read_exact(&mut data)?;
                records.push(data);
            }

            Ok(records)
        }
        "lines" => {
            let reader = BufReader::new(file);
            let mut records = Vec::new();

            for line in reader.lines() {
                let line = line?;
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Parse hex string
                let bytes: Result<Vec<u8>, _> = (0..line.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&line[i..i + 2], 16))
                    .collect();

                records.push(bytes.context("Invalid hex")?);
            }

            Ok(records)
        }
        _ => anyhow::bail!("Unknown format: {}", format),
    }
}

fn print_bit_analysis(records: &[Vec<u8>], pos: usize) {
    let values: Vec<u8> = records.iter().filter_map(|r| r.get(pos).copied()).collect();
    for bit in (0..8).rev() {
        let ones: usize = values.iter().filter(|&&v| (v >> bit) & 1 == 1).count();
        let zeros = values.len() - ones;
        let ones_ratio = ones as f64 / values.len() as f64;
        let zeros_ratio = zeros as f64 / values.len() as f64;
        if ones > 0 && zeros > 0 && ones_ratio > 0.1 && zeros_ratio > 0.1 {
            println!(
                "       bit {}: 0={:<5} 1={:<5} ({:.1}% ones)",
                bit,
                zeros,
                ones,
                100.0 * ones_ratio
            );
        }
    }
}

fn analyze(records: &[Vec<u8>], max_positions: usize, show_bits: bool) {
    if records.is_empty() {
        println!("No records to analyze");
        return;
    }

    let max_len = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);
    let record_refs: Vec<&Vec<u8>> = records.iter().collect();

    println!("Records: {}", records.len());
    println!(
        "Length range: {} - {}",
        records.iter().map(|r| r.len()).min().unwrap_or(0),
        max_len
    );
    println!();
    println!(
        "{:>4}  {:>6}  {:>8}  {:>6}  {:>8}  Distribution",
        "Pos", "Count", "Unique", "Entropy", "Common"
    );
    println!("{}", "-".repeat(70));

    for pos in 0..positions {
        let Some(stats) = analysis::PositionStats::from_records(&record_refs, pos) else {
            continue;
        };

        println!(
            "{:>4}  {:>6}  {:>8}  {:>6.2}  0x{:02x}:{:<4}  {}",
            pos,
            stats.count,
            stats.unique,
            stats.entropy,
            stats.most_common.0,
            stats.most_common.1,
            stats.distribution_summary()
        );

        if show_bits && stats.unique > 1 && stats.unique < 16 {
            print_bit_analysis(records, pos);
        }
    }
}

fn ngrams(records: &[Vec<u8>], size: usize, min_count: usize) {
    let mut freq: HashMap<Vec<u8>, usize> = HashMap::new();

    for record in records {
        if record.len() < size {
            continue;
        }
        for window in record.windows(size) {
            *freq.entry(window.to_vec()).or_insert(0) += 1;
        }
    }

    let mut pairs: Vec<_> = freq.into_iter().filter(|(_, c)| *c >= min_count).collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1));

    println!("Top {}-grams (min count {}):", size, min_count);
    println!("{:>8}  Bytes", "Count");
    println!("{}", "-".repeat(40));

    for (bytes, count) in pairs.iter().take(50) {
        let hex: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
        println!("{:>8}  {}", count, hex);
    }
}

fn entropy_analysis(records: &[Vec<u8>], max_positions: usize) {
    if records.is_empty() {
        println!("No records");
        return;
    }

    let max_len = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);

    println!("Entropy by position (0=fixed, 8=random):\n");

    for pos in 0..positions {
        let values: Vec<u8> = records.iter().filter_map(|r| r.get(pos).copied()).collect();
        if values.is_empty() {
            continue;
        }

        let mut freq: HashMap<u8, usize> = HashMap::new();
        for &v in &values {
            *freq.entry(v).or_insert(0) += 1;
        }

        let total = values.len() as f64;
        let entropy: f64 = freq
            .values()
            .map(|&count| {
                let p = count as f64 / total;
                if p > 0.0 {
                    -p * p.log2()
                } else {
                    0.0
                }
            })
            .sum();

        // Visual bar
        let bar_len = (entropy * 8.0) as usize;
        let bar: String = "#".repeat(bar_len) + &" ".repeat(64 - bar_len);

        println!(
            "{:>3}: [{:.2}] |{}|",
            pos,
            entropy,
            &bar[..64.min(bar.len())]
        );
    }
}

fn diff_analysis(records_a: &[Vec<u8>], records_b: &[Vec<u8>]) {
    println!("Set A: {} records", records_a.len());
    println!("Set B: {} records", records_b.len());

    let max_len = records_a
        .iter()
        .chain(records_b.iter())
        .map(|r| r.len())
        .max()
        .unwrap_or(0);

    println!("\nPositions with different distributions:\n");
    println!(
        "{:>4}  {:>10}  {:>10}  Notes",
        "Pos", "A common", "B common"
    );
    println!("{}", "-".repeat(50));

    for pos in 0..max_len.min(64) {
        let values_a: Vec<u8> = records_a
            .iter()
            .filter_map(|r| r.get(pos).copied())
            .collect();
        let values_b: Vec<u8> = records_b
            .iter()
            .filter_map(|r| r.get(pos).copied())
            .collect();

        if values_a.is_empty() || values_b.is_empty() {
            continue;
        }

        let common_a = most_common(&values_a);
        let common_b = most_common(&values_b);

        if common_a != common_b {
            println!(
                "{:>4}  0x{:02x} ({:>3}%)  0x{:02x} ({:>3}%)  DIFFERS",
                pos,
                common_a.0,
                common_a.1 * 100 / values_a.len(),
                common_b.0,
                common_b.1 * 100 / values_b.len()
            );
        }
    }
}

fn most_common(values: &[u8]) -> (u8, usize) {
    let mut freq: HashMap<u8, usize> = HashMap::new();
    for &v in values {
        *freq.entry(v).or_insert(0) += 1;
    }
    freq.into_iter().max_by_key(|(_, c)| *c).unwrap_or((0, 0))
}

#[tokio::main]
#[allow(clippy::too_many_lines)] // CLI command dispatch
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle -i flag for quick interactive mode
    if let Some(input) = cli.interactive {
        let records = read_records(&input, &cli.format)?;
        let cfg = config::Config::load().await?;
        let auto_preset = cfg.detect_preset(&records, 50);
        return interactive::run_interactive(records, auto_preset);
    }

    let command = cli
        .command
        .ok_or_else(|| anyhow::anyhow!("No command specified. Use -i <file> or a subcommand."))?;

    match command {
        Command::Analyze {
            input,
            format,
            max_positions,
            bits,
        } => {
            let records = read_records(&input, &format)?;
            analyze(&records, max_positions, bits);
        }
        Command::Ngrams {
            input,
            format,
            size,
            min_count,
        } => {
            let records = read_records(&input, &format)?;
            ngrams(&records, size, min_count);
        }
        Command::Entropy {
            input,
            format,
            max_positions,
        } => {
            let records = read_records(&input, &format)?;
            entropy_analysis(&records, max_positions);
        }
        Command::Diff {
            file_a,
            file_b,
            format,
        } => {
            let records_a = read_records(&file_a, &format)?;
            let records_b = read_records(&file_b, &format)?;
            diff_analysis(&records_a, &records_b);
        }
        Command::Group {
            input,
            format,
            position,
            max_positions,
        } => {
            let records = read_records(&input, &format)?;
            commands::group_analysis(&records, position, max_positions);
        }
        Command::Filter {
            input,
            format,
            position,
            value,
            max_positions,
        } => {
            let records = read_records(&input, &format)?;
            let v = parse_hex_value(&value)?;
            commands::filter_analysis(&records, position, v, max_positions);
        }
        Command::Compare {
            input,
            format,
            position,
            max_positions,
        } => {
            let records = read_records(&input, &format)?;
            commands::compare_groups(&records, position, max_positions);
        }
        Command::Split {
            input,
            format,
            header_len,
            output_dir,
        } => {
            let records = read_records(&input, &format)?;
            split_by_header(&records, header_len, &output_dir)?;
        }
        Command::Frequency {
            input,
            format,
            max_positions,
            threshold,
        } => {
            let records = read_records(&input, &format)?;
            frequency_analysis(&records, max_positions, threshold);
        }
        Command::Boundaries {
            input,
            format,
            max_positions,
        } => {
            let records = read_records(&input, &format)?;
            boundary_detection(&records, max_positions);
        }
        Command::Interactive { input, format } => {
            let records = read_records(&input, &format)?;
            let cfg = config::Config::load().await?;
            let auto_preset = cfg.detect_preset(&records, 50);
            interactive::run_interactive(records, auto_preset)?;
        }
    }

    Ok(())
}

fn split_by_header(records: &[Vec<u8>], header_len: usize, output_dir: &PathBuf) -> Result<()> {
    std::fs::create_dir_all(output_dir)?;

    // Group records by their header bytes
    let mut groups: HashMap<Vec<u8>, Vec<&Vec<u8>>> = HashMap::new();
    for record in records {
        let header: Vec<u8> = record.iter().take(header_len).copied().collect();
        groups.entry(header).or_default().push(record);
    }

    // Sort groups by size (largest first) and assign letters
    let mut sorted_groups: Vec<_> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    println!(
        "Split {} records into {} groups by {}-byte header:\n",
        records.len(),
        sorted_groups.len(),
        header_len
    );

    for (idx, (header, group_records)) in sorted_groups.iter().enumerate() {
        // Generate filename: group_a.bin, group_b.bin, etc.
        let letter = (b'a' + (idx as u8 % 26)) as char;
        let suffix = if idx >= 26 {
            format!("{}", idx / 26)
        } else {
            String::new()
        };
        let filename = format!("group_{}{}.bin", letter, suffix);
        let path = output_dir.join(&filename);

        // Write records in length16 format
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);

        for record in group_records {
            let len = record.len() as u16;
            writer.write_all(&len.to_le_bytes())?;
            writer.write_all(record)?;
        }

        // Display header as hex
        let header_hex: String = header.iter().map(|b| format!("{:02x}", b)).collect();
        println!(
            "  {} : {:>5} records  header={}",
            filename,
            group_records.len(),
            header_hex
        );
    }

    println!("\nFiles written to {:?}", output_dir);
    Ok(())
}

fn frequency_analysis(records: &[Vec<u8>], max_positions: usize, threshold: usize) {
    if records.is_empty() {
        println!("No records");
        return;
    }

    let max_len = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);
    let total = records.len();

    println!(
        "Frequency analysis: {} records, {} positions\n",
        total, positions
    );
    println!(
        "{:>4}  {:>6}  {:>6}  {:>8}  Frequency Bar",
        "Pos", "Top%", "Top2%", "TopVal"
    );
    println!("{}", "-".repeat(70));

    for pos in 0..positions {
        let values: Vec<u8> = records.iter().filter_map(|r| r.get(pos).copied()).collect();
        if values.is_empty() {
            continue;
        }

        let mut freq: HashMap<u8, usize> = HashMap::new();
        for &v in &values {
            *freq.entry(v).or_insert(0) += 1;
        }

        // Get top two values
        let mut sorted: Vec<_> = freq.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        let (top_val, top_count) = sorted.first().map(|(&v, &c)| (v, c)).unwrap_or((0, 0));
        let top_pct = top_count * 100 / values.len();

        let top2_pct = if sorted.len() > 1 {
            (sorted[0].1 + sorted[1].1) * 100 / values.len()
        } else {
            top_pct
        };

        // Visual frequency bar
        let bar_len = top_pct * 40 / 100;
        let bar: String = "█".repeat(bar_len) + &"░".repeat(40 - bar_len);

        // Mark high-frequency positions
        let marker = if top_pct >= threshold {
            " ◀ FIXED"
        } else {
            ""
        };

        println!(
            "{:>4}  {:>5}%  {:>5}%  0x{:02x}     |{}|{}",
            pos, top_pct, top2_pct, top_val, bar, marker
        );
    }
}

fn detect_field_boundaries(stats: &[analysis::PositionStats]) -> Vec<(usize, usize, bool)> {
    let mut fields = Vec::new();
    let mut prev_fixed = false;
    let mut field_start = 0;

    for (i, s) in stats.iter().enumerate() {
        let is_fixed = s.entropy < 1.0;
        if i == 0 {
            prev_fixed = is_fixed;
            field_start = s.position;
        } else if is_fixed != prev_fixed {
            fields.push((field_start, s.position - 1, prev_fixed));
            field_start = s.position;
            prev_fixed = is_fixed;
        }
    }
    if let Some(s) = stats.last() {
        fields.push((field_start, s.position, prev_fixed));
    }
    fields
}

fn field_description(is_fixed: bool, len: usize) -> &'static str {
    match (is_fixed, len) {
        (true, 1..=4) => "likely header/delimiter",
        (true, _) => "padding or constant",
        (false, 1..=2) => "small field (ID?)",
        (false, 3..=4) => "medium field (value?)",
        (false, _) => "large field (data block)",
    }
}

fn boundary_detection(records: &[Vec<u8>], max_positions: usize) {
    if records.is_empty() {
        println!("No records");
        return;
    }

    let max_len = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let positions = max_len.min(max_positions);
    let record_refs: Vec<&Vec<u8>> = records.iter().collect();

    let stats: Vec<_> = (0..positions)
        .filter_map(|pos| analysis::PositionStats::from_records(&record_refs, pos))
        .collect();

    let fields = detect_field_boundaries(&stats);

    println!("Field boundary detection: {} records\n", records.len());
    println!("Legend: ═══ fixed field, ─── variable field, │ boundary\n");
    println!("{:>4}-{:<4}  {:>8}  Description", "Start", "End", "Type");
    println!("{}", "-".repeat(50));

    for &(start, end, is_fixed) in &fields {
        let len = end - start + 1;
        let field_type = if is_fixed { "FIXED" } else { "VARIABLE" };
        println!(
            "{:>4}-{:<4}  {:>8}  {} ({} bytes)",
            start,
            end,
            field_type,
            field_description(is_fixed, len),
            len
        );
    }

    // Visual representation
    println!("\nVisual map (each char = 1 byte):");
    let visual: String = fields
        .iter()
        .flat_map(|&(start, end, is_fixed)| {
            let len = end - start + 1;
            let sym = if is_fixed { '═' } else { '─' };
            std::iter::once('│').chain(std::iter::repeat(sym).take(len))
        })
        .chain(std::iter::once('│'))
        .collect();
    println!("{}", visual);

    // Position markers
    print!("0");
    let mut pos = 0;
    for &(start, end, _) in &fields {
        let len = end - start + 1;
        pos += len + 1;
        if pos < 70 {
            print!("{:>width$}", end + 1, width = len);
        }
    }
    println!();
}

fn parse_hex_value(s: &str) -> Result<u8> {
    let s = s.trim().trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(s, 16).context("Invalid hex value")
}
