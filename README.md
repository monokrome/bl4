# bl4

Borderlands 4 reverse engineering toolkit — save editor, item serial decoder, NCS parser, items database, drop rate analysis, and memory tools.

## Crates

| Crate | Description |
|-------|-------------|
| `bl4` | Core library (crypto, serial codec, save manipulation, manifest) |
| `bl4-cli` | Command-line interface (`bl4` binary) |
| `bl4-idb` | Items database (SQLite/PostgreSQL, sync/async) |
| `bl4-ncs` | Nexus Config Store parser (game binary config format) |
| `bl4-community` | Community API server (Axum) |
| `bl4-save-editor` | GUI save editor (Tauri) |
| `uextract` | UE5 IoStore/pak extractor |
| `bl4-preload` | LD_PRELOAD instrumentation library |

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

## Usage

### Configuration

```bash
bl4 configure --steam-id 76561197960521364
bl4 configure --show
```

Steam ID is required for encryption/decryption. Find it in your Steam profile URL.

### Save Commands

```bash
# Decrypt to YAML
bl4 save 1.sav decrypt -o save.yaml
bl4 save 1.sav decrypt > save.yaml

# Encrypt from YAML
bl4 save 1.sav encrypt save.yaml

# Edit in $EDITOR
bl4 save 1.sav edit

# Query values
bl4 save 1.sav get
bl4 save 1.sav get --level
bl4 save 1.sav get --money
bl4 save 1.sav get --info
bl4 save 1.sav get "state.currencies.cash"

# Set values
bl4 save 1.sav set "state.experience[0].level" 50
bl4 save 1.sav set "state.currencies.cash" 999999

# Fog-of-discovery map manipulation
bl4 save 1.sav --map reveal
bl4 save 1.sav --map clear --zone "Crimson Badlands"

# Validate all items in a save
bl4 save 1.sav --validate-items
```

### Item Serial Commands

```bash
# Decode a serial
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --verbose
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --rarity

# Validate serial legality
bl4 serial validate '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
bl4 serial validate '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --verbose

# Compare two serials
bl4 serial compare '<serial1>' '<serial2>'

# Modify serial (swap parts from source into base)
bl4 serial modify '<base>' '<source>' <parts>

# Batch decode serials to binary
bl4 serial batch-decode serials.txt output.bin
```

### Items Database

Track, identify, and analyze items across saves:

```bash
# Initialize database
bl4 idb init

# Import items from a save file
bl4 idb import-save 1.sav --decode

# Decode all serials and populate metadata
bl4 idb decode-all

# List items with filters
bl4 idb list --weapon-type Pistol --manufacturer Jakobs
bl4 idb list --rarity Legendary --format csv

# Show item details
bl4 idb show '<serial>'

# Set metadata with source attribution
bl4 idb set-value '<serial>' name "Terminus" --source ingame

# Publish/pull items to community server
bl4 idb publish --server https://api.example.com
bl4 idb pull --server https://api.example.com
```

### NCS (Nexus Config Store)

Parse and extract data from the game's binary configuration format:

```bash
# Scan a directory for NCS file types
bl4 ncs scan /path/to/extracted/

# Show contents of an NCS file
bl4 ncs show inv0.bin

# Extract manifests (parts database, category names)
bl4 ncs extract --extract-type manifest /path/to/inv/

# Search for patterns across NCS files
bl4 ncs search /path/to/extracted/ "weapon_ps"
```

### Drop Rates

Query drop rates and legendary item sources:

```bash
# Find where an item drops
bl4 drops find "Terminus"

# List all drops from a specific source
bl4 drops source "Dryl"

# List all known sources or items
bl4 drops list --sources
bl4 drops list --items
```

### Parts Database

Query the parts manifest:

```bash
# Find parts for a weapon type
bl4 parts "Jakobs Pistol"
```

### Memory Tools

Read and analyze live game process memory or dump files:

```bash
# Attach to live process
bl4 memory info
bl4 memory objects --class WeaponData
bl4 memory fname 12345

# Work with dump files
bl4 memory --dump game.bin info
bl4 memory --dump game.bin scan "48 8B 05 ?? ?? ?? ??"

# LD_PRELOAD instrumentation
bl4 memory preload run -- wine Borderlands4.exe
bl4 memory preload watch /tmp/bl4_log
```

### UE5 Asset Extraction

```bash
# Extract all assets from pak files
uextract /path/to/Paks -o extracted/

# Filter by path pattern
uextract /path/to/Paks -f "ItemSerialPart" -o parts/

# List contents without extracting
uextract list /path/to/Paks
```

## Library Usage

```toml
[dependencies]
bl4 = "0.6"
```

```rust
use bl4::{decrypt_sav, encrypt_sav, SaveFile, ItemSerial};
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

### Serial Decoding

```rust
use bl4::ItemSerial;

let item = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k")?;

if let Some((mfr, wtype)) = item.weapon_info() {
    println!("{} {}", mfr, wtype);
}

let validation = item.validate();
println!("Legality: {}", validation.legality);
```

## Save File Format

```
.sav -> decrypt (AES-256-ECB) -> decompress (zlib) -> YAML
```

Key derivation: Steam ID XOR'd with hardcoded base key.

## Development

```bash
cargo test
cargo clippy --workspace
```

## Disclaimer

Unofficial tools. Borderlands and related trademarks are property of
Gearbox Software and 2K. Not affiliated with or endorsed by Gearbox or 2K.

**Use at your own risk.**

## License

BSD-2-Clause
