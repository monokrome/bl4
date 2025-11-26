# bl4

## Features

- Decrypt and encrypt .sav files using Steam ID
- Interactive editing via $EDITOR
- Query and modify save data (level, currencies, XP, etc.)
- Decode item serial numbers (weapons, equipment, etc.)
- Hash-based backup system
- Optional Steam ID configuration
- WebAssembly support for JavaScript/TypeScript
- Rust library with CLI wrapper

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
npm install bl4
```

## Usage

### Configuration

```bash
bl4 configure --steam-id 76561197960521364
bl4 configure --show
```

Steam ID is required for encryption/decryption. Find it in your Steam account page.

### Editing

```bash
bl4 edit -i 1.sav
bl4 edit -i 1.sav -s 76561197960521364  # Override configured Steam ID
```

### Querying

```bash
bl4 get -i 1.sav
bl4 get -i 1.sav --level
bl4 get -i 1.sav --money
bl4 get -i 1.sav --info
bl4 get -i 1.sav "state.currencies.cash"
bl4 get -i 1.sav "state.experience[0].level"
```

### Modifying

```bash
bl4 set -i 1.sav "state.experience[0].level" 50
bl4 set -i 1.sav "state.currencies.cash" 999999
bl4 set -i 1.sav "state.currencies.eridium" 50000
bl4 set -i 1.sav --raw "state.some_field" "{complex: yaml, structure: [1, 2, 3]}"
```

Automatic backups enabled by default (`--backup` flag).

### Decryption

```bash
bl4 decrypt -i 1.sav -o save.yaml
bl4 decrypt -i 1.sav > save.yaml
```

### Encryption

```bash
bl4 encrypt -i save.yaml -o 1.sav
cat save.yaml | bl4 encrypt > 1.sav
```

### Inspection

```bash
bl4 inspect -i 1.sav
bl4 inspect -i 1.sav --full
```

### Item Serial Decoding

```bash
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
bl4 decode '@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_' --verbose
```

Decodes BL4 item serial numbers to show:
- Item type (weapon, equipment, utility, etc.)
- Raw decoded bytes
- Parsed tokens (VarInt, VarBit, Part structures)
- Extracted fields (manufacturer, rarity, level when available)

## Backup System

Hash-based tracking with `.sav.bak` and `.sav.bak.json` metadata:

- First edit creates backup
- Subsequent edits preserve original
- File replacement triggers new backup

## Library Usage

```toml
[dependencies]
bl4 = "0.1"
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
save.set_character_name("NewName")?;

let modified_yaml = save.to_yaml()?;
let encrypted = encrypt_sav(&modified_yaml, "76561197960521364")?;
fs::write("1.sav", encrypted)?;
```

Documentation: https://docs.rs/bl4

### JavaScript/TypeScript

Browser:
```javascript
import init, { SaveFile, decryptSav, encryptSav } from './pkg/bl4.js';

await init();

const encrypted = new Uint8Array(await file.arrayBuffer());
const yaml = decryptSav(encrypted, '76561197960521364');
const save = new SaveFile(yaml);

save.setCash(999999);
const modified = save.toYaml();
const encryptedNew = encryptSav(modified, '76561197960521364');
```

Node.js:
```javascript
const { SaveFile, decryptSav, encryptSav } = require('./pkg/bl4.js');
const fs = require('fs');

const encrypted = fs.readFileSync('1.sav');
const yaml = decryptSav(encrypted, '76561197960521364');
const save = new SaveFile(yaml);

save.setCash(999999);
fs.writeFileSync('1.sav', encryptSav(save.toYaml(), '76561197960521364'));
```

TypeScript definitions included automatically.

## Development

### Tests

```bash
cargo test
```

### WASM Build

```bash
cd crates/bl4
./build-wasm.sh
```

Generates bindings and TypeScript definitions in `pkg/`.

### Save File Format

1. AES-256-ECB encryption (Steam ID-derived key via XOR with BASE_KEY)
2. Zlib compression
3. YAML data

```
.sav -> decrypt (AES-256-ECB) -> decompress (zlib) -> YAML
```

## Contributing

Areas for contribution:
- Item serial decoding
- Save structure documentation
- GUI development
- Platform testing
- Additional convenience methods

## Releases

1. Update versions in `Cargo.toml` files
2. Commit and tag: `git tag v0.1.0 && git push origin v0.1.0`
3. GitHub Actions handles publishing to crates.io, NPM, and binary releases

## Disclaimer

This repository contains unofficial tools. Borderlands and all related
trademarks are property of Gearbox Software and 2K. This project is not
affiliated with or endorsed by Gearbox Software or 2K.

**Use at your own risk.**


## License

BSD-2-Clause
