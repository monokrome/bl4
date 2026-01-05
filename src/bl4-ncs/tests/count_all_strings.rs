/// Test to count ALL strings in the file and find where they actually end
#[test]
#[ignore]
fn count_all_strings_in_file() {
    let data = std::fs::read("/home/polar/Documents/Borderlands 4/ncsdata/pakchunk4-Windows_0_P/Nexus-Data-inv4.bin")
        .expect("File not found");

    // Start from string table offset
    let start = 0x225;
    let mut pos = start;
    let mut count = 0u32;

    // Count ALL null-terminated strings until end of file
    while pos < data.len() {
        let string_start = pos;

        // Find null terminator
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }

        if pos > string_start {
            count += 1;

            // Print first few and last few
            if count <= 5 || count % 5000 == 0 {
                let s = String::from_utf8_lossy(&data[string_start..pos]);
                eprintln!("[{}] 0x{:x}: {}", count, string_start, s);
            }
        }

        // Skip null
        if pos < data.len() {
            pos += 1;
        }
    }

    eprintln!("\nTotal strings found: {}", count);
    eprintln!("End position: 0x{:x}", pos);
    eprintln!("File size: 0x{:x}", data.len());
}
