# bl4

Borderlands 4 save editor toolkit - library, CLI, and analysis tools.

## Features

- Decrypt and encrypt .sav files using Steam ID
- Interactive editing via $EDITOR
- Query and modify save data (level, currencies, XP, etc.)
- Decode and analyze item serial numbers
- Items database for tracking and identifying gear
- Hash-based backup system
- WebAssembly support for browser/Node.js
- Binary pattern analysis with linewise TUI

## Installation

```bash
cargo install bl4-cli
```

From source:
```bash
git clone https://github.com/monokrome/bl4.git
cd bl4
cargo build --release -p bl4-cli
```

Binary location: `target/release/bl4`

JavaScript/TypeScript (NPM):
```bash
npm install @monokrome/bl4
```

## Usage

### Configuration

```bash
bl4 configure --steam-id 76561197960521364
bl4 configure --show
```

Steam ID is required for encryption/decryption. Find it in your Steam profile URL.

### Save Commands

```bash
# Edit in $EDITOR
bl4 save edit 1.sav
bl4 save edit 1.sav -s 76561197960521364  # Override Steam ID

# Query values
bl4 save get 1.sav
bl4 save get 1.sav --level
bl4 save get 1.sav --money
bl4 save get 1.sav --info
bl4 save get 1.sav "state.currencies.cash"

# Set values
bl4 save set 1.sav "state.experience[0].level" 50
bl4 save set 1.sav "state.currencies.cash" 999999
```

### Encryption/Decryption

```bash
bl4 decrypt 1.sav -o save.yaml
bl4 decrypt 1.sav > save.yaml
bl4 encrypt save.yaml -o 1.sav
```

### Inspection

```bash
bl4 inspect 1.sav
bl4 inspect 1.sav --full
```

### Item Serial Commands

```bash
# Decode a serial
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
bl4 serial decode '@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_' --verbose

# Modify serial (swap parts)
bl4 serial modify <base> <source> <parts>

# Batch decode to binary
bl4 serial batch-decode serials.txt output.bin
```

### Items Database

Track and identify items across saves:

```bash
# Import items from a save
bl4 idb import-save 1.sav --decode

# Query items
bl4 idb query --weapon-type "Assault Rifle"
bl4 idb query --manufacturer "Vladof"

# Set item metadata
bl4 idb set-value "<serial>" name "My Legendary"
```

## Backup System

Hash-based tracking with `.sav.bak` files:
- First edit creates backup
- Subsequent edits preserve original
- File replacement triggers new backup

## Library Usage

```toml
[dependencies]
bl4 = "0.4"
```

```rust
use bl4::{decrypt_sav, encrypt_sav, SaveFile};
use std::fs;

let encrypted = fs::read("1.sav")?;
let yaml_data = decrypt_sav(&encrypted, "76561197960521364")?;
let mut save = SaveFile::from_yaml(&yaml_data)?;

println!("Character: {:?}", save.get_character_name());
println!("Cash: {:?}", save.get_cash());

save.set_cash(999999)?;
let modified_yaml = save.to_yaml()?;
let encrypted = encrypt_sav(&modified_yaml, "76561197960521364")?;
fs::write("1.sav", encrypted)?;
```

Documentation: https://docs.rs/bl4

### JavaScript/TypeScript

```javascript
import init, { SaveFile, decryptSav, encryptSav } from '@monokrome/bl4';

await init();

const encrypted = new Uint8Array(await file.arrayBuffer());
const yaml = decryptSav(encrypted, '76561197960521364');
const save = new SaveFile(yaml);

save.setCash(999999);
const modified = save.toYaml();
const encryptedNew = encryptSav(modified, '76561197960521364');
```

TypeScript definitions included.

## Development

```bash
cargo test
cargo clippy --workspace
```

### WASM Build

```bash
wasm-pack build src/bl4 --target web --features wasm
```

### Save File Format

```
.sav -> decrypt (AES-256-ECB) -> decompress (zlib) -> YAML
```

Key derivation: Steam ID XOR'd with hardcoded BASE_KEY.

## Tools

### linewise

TUI for binary pattern analysis of length-prefixed records:

```bash
cargo build --release -p linewise

# Interactive exploration
linewise explore data.bin

# Statistical analysis
linewise analyze data.bin -n 64
linewise frequency data.bin
linewise ngram data.bin --size 4
```

Interactive controls:
- `j/k` - move between records
- `h/l` - shift field alignment
- `w/b` - move between fields
- `Tab` - cycle data types (u8, u16, u32, varint, hex, ascii)
- `L` - lock current field (e.g., `24L` locks 24 bytes)
- `:q` - quit

### uextract

UE5 IoStore extractor for game assets:

```bash
cargo build --release -p uextract

# Extract all assets
uextract /path/to/Paks -o extracted/

# Filter by path
uextract /path/to/Paks -f "ItemSerialPart" -o parts/

# Extract specific patterns
uextract /path/to/Paks -s "**/Weapons/**" -o weapons/

# List contents without extracting
uextract list /path/to/Paks
```

Outputs JSON representations of UE5 assets for analysis.

## Releases

1. Update version in root `Cargo.toml`
2. Tag and push: `git tag v0.4.x && git push origin v0.4.x`
3. GitHub Actions publishes to crates.io, NPM, and GitHub Releases

## Disclaimer

Unofficial tools. Borderlands and related trademarks are property of
Gearbox Software and 2K. Not affiliated with or endorsed by Gearbox or 2K.

**Use at your own risk.**

## License

BSD-2-Clause
