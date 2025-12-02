# BL4 - Borderlands 4 Save Editor

A Borderlands 4 save file editor and item serial decoder.

---

## Features

- **Save File Decryption/Encryption** — Read and modify save files
- **Item Serial Decoding** — Understand how items are encoded
- **Memory Analysis** — Extract data from running game
- **Data Extraction** — Parse game assets from pak files
- **Usmap Generation** — Create reflection data for asset parsing

## Quick Start

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/monokrome/bl4
cd bl4
cargo build --release

# Decode an item serial
./target/release/bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

## Documentation

<div class="grid cards" markdown>

-   :material-book-open-variant:{ .lg .middle } **Reverse Engineering Guide**

    ---

    Learn reverse engineering from scratch with our zero-to-hero guide.

    [:octicons-arrow-right-24: Start the Guide](guide/00-introduction.md)

-   :material-file-document:{ .lg .middle } **Reference Documentation**

    ---

    Detailed technical documentation on data structures and formats.

    [:octicons-arrow-right-24: Data Structures](data_structures.md)

-   :material-download:{ .lg .middle } **Downloads**

    ---

    Download the complete guide as a PDF for offline reading.

    [:octicons-arrow-right-24: Downloads](downloads/index.md)

-   :material-github:{ .lg .middle } **Source Code**

    ---

    View the source code and contribute on GitHub.

    [:octicons-arrow-right-24: GitHub](https://github.com/monokrome/bl4)

</div>

## Project Status

| Component | Status |
|-----------|--------|
| Save Decryption/Encryption | ✅ Complete |
| Item Serial Decoding | ✅ Complete |
| Memory Analysis | ✅ Complete |
| Usmap Generation | ✅ Complete |
| Data Extraction | ✅ Complete |
| Serial Encoding | 🚧 In Progress |
| WASM Bindings | 🚧 Planned |

## Extracted Data

The project includes pre-extracted game data:

| Data | Count |
|------|-------|
| Game Assets | 81,097 |
| Usmap Structs | 16,849 |
| Usmap Properties | 58,793 |
| Manufacturers | 10 |
| Item Pools | 62 |

## License

BSD-2-Clause — See [LICENSE](https://github.com/monokrome/bl4/blob/main/LICENSE) for details.
