# bl4

> **Pre-alpha / prototype.** Do not use in production. Expect breaking changes, incorrect decoding, and incomplete features. Back up your saves.

Borderlands 4 reverse engineering toolkit — save editor, item serial decoder, NCS parser, items database, drop rate analysis, and memory tools.

## Disclaimer

These are unofficial tools. Borderlands and related trademarks are property of
Gearbox Software and 2K. This project is not affiliated with or endorsed by
Gearbox or 2K, nor does it claim to be.

## Features

- [Configuration](#configuration)
- [Save Files](#save-files) — decrypt, encrypt, edit, query, modify
  - [Level Scaling](#level-scaling) — set all items to a target level
  - [Map Reveal](#map-reveal) — reveal or clear fog-of-discovery
  - [Mission Progress](#mission-progress) — list and set campaign/DLC/side mission progression
  - [Item Validation](#item-validation) — check all items for legality
- [Item Serials](#item-serials) — decode, encode, compare, modify, validate
- [Items Database](#items-database) — track, identify, and analyze items across saves
- [Drop Rates](#drop-rates) — query drop sources and legendary locations
- [Parts Database](#parts-database) — query weapon/equipment part manifests
- [NCS Parser](#ncs-parser) — parse game binary configuration files
- [UE5 Asset Extraction](#ue5-asset-extraction) — extract assets from pak files
- [Library Usage](#library-usage) — use bl4 as a Rust dependency

## Crates

| Crate | Description |
|-------|-------------|
| `bl4` | Core library (crypto, serial codec, save manipulation, manifest) |
| `bl4-cli` | Command-line interface (`bl4` binary) |
| `bl4-idb` | Items database (SQLite/PostgreSQL, sync/async) |
| `bl4-ncs` | Nexus Config Store parser (game binary config format) |
| `bl4-community` | Community API server (Axum) |
| `bl4-save-editor` | GUI save editor (Tauri + WASM) |
| `uextract` | UE5 IoStore/pak extractor |

## Installation

From source:
```bash
git clone https://github.com/monokrome/bl4.git
cd bl4
cargo build --release -p bl4-cli
```

Binary location: `target/release/bl4`

## Configuration

```bash
# Set your Steam ID (required for save encryption/decryption)
bl4 configure --steam-id 76561197960521364

# Show current configuration
bl4 configure --show
```

Steam ID is required for encryption/decryption. Find it in your Steam profile URL.

## Save Files

`bl4 save 1.sav` or shorthand `bl4 1.sav` — both work.

```bash
# Decrypt to YAML
bl4 1.sav decrypt -o save.yaml
bl4 1.sav decrypt > save.yaml

# Encrypt from YAML
bl4 1.sav encrypt save.yaml

# Edit in $EDITOR
bl4 1.sav edit

# Query values
bl4 1.sav get --all
bl4 1.sav get --level
bl4 1.sav get --money
bl4 1.sav get --info
bl4 1.sav get "state.currencies.cash"

# Set values
bl4 1.sav set "state.experience[0].level" 50
bl4 1.sav set "state.currencies.cash" 999999
```

### Level Scaling

```bash
# Set all items in a save to level 60
bl4 1.sav --set-item-level 60

# Change level on a single serial
bl4 serial decode --level 60 '@Ugr$xKm/)}}!pQufM-}RPG}y!%8r1pL0ss'
```

### Map Reveal

```bash
# Reveal entire fog-of-discovery map
bl4 1.sav --map reveal

# Clear the map
bl4 1.sav --map clear

# Reveal/clear a specific zone only
bl4 1.sav --map reveal --zone "Crimson Badlands"
```

### Mission Progress

List and modify campaign progression, DLC completion, and side missions.

```bash
# List main story progress
bl4 1.sav missions list

# List all mission categories
bl4 1.sav missions list all

# List a specific category (main, side, micro, dlc, vault, zoneactivity)
bl4 1.sav missions list side

# Set campaign progress to a specific point
# Shows what will change and asks for confirmation
bl4 1.sav missions set grasslands1
bl4 1.sav missions set mountains2a
bl4 1.sav missions set searchforlilith
bl4 1.sav missions set elpis
bl4 1.sav missions set cityepilogue

# Skip confirmation
bl4 1.sav missions set mountains1 -y

# Complete a DLC
bl4 1.sav missions set cowbell
bl4 1.sav missions set cello

# Complete a specific mission
bl4 1.sav missions set huntedpart1
```

Setting campaign progress automatically marks all prerequisite missions as
completed. At the three-way branch point (Grasslands/Mountains/Shattered Lands),
setting progress past `searchforlilith` completes all three branches.

### Item Validation

```bash
# Validate all items in a save
bl4 1.sav --validate-items
```

## Item Serials

```bash
# Decode a serial
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Verbose decode (shows all parts, tokens, hex dump)
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --verbose

# Decode with rarity estimation
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --rarity

# Change item level
bl4 serial decode --level 60 '@Ugr$xKm/)}}!pQufM-}RPG}y!%8r1pL0ss'

# Test round-trip encoding
bl4 serial encode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Validate serial legality
bl4 serial validate '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
bl4 serial validate '@Ugr$ZCm/&tH!t{KgK/Shxu>k' --verbose

# Compare two serials
bl4 serial compare '<serial1>' '<serial2>'

# Modify serial (swap parts from source into base)
bl4 serial modify '<base>' '<source>' --parts barrel,grip

# Add or remove parts
bl4 serial decode '<serial>' --add part_barrel_01 --remove part_barrel_02

# Batch decode serials to binary
bl4 serial batch-decode serials.txt output.bin
```

## Items Database

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

## Drop Rates

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

## Parts Database

Query the parts manifest:

```bash
# Find parts for a weapon type
bl4 parts "Jakobs Pistol"
```

## NCS Parser

Parse and extract data from the game's binary configuration format:

```bash
# Show contents of an NCS file (fully expanded)
bl4 ncs show inv0.bin

# Show as JSON
bl4 ncs show inv0.bin --json

# Scan a directory for NCS file types
bl4 ncs scan /path/to/decompressed/

# Search for patterns across NCS files
bl4 ncs search /path/to/decompressed/ "weapon_ps"

# Extract parts manifest
bl4 ncs extract -t manifest /path/to/decompressed/

# Extract mission graph (dependency chain + metadata)
bl4 ncs extract -t missions /path/to/decompressed/

# Decompress NCS from a .pak file
bl4 ncs decompress game.pak -o ./ncs/ --raw
```

## UE5 Asset Extraction

```bash
# Extract all assets from pak files
uextract /path/to/Paks -o extracted/

# Filter by path pattern
uextract /path/to/Paks -f "ItemSerialPart" -o parts/

# Limit parallelism
uextract /path/to/Paks -o extracted/ -j 4
```

## Library Usage

```toml
[dependencies]
bl4 = { git = "https://github.com/monokrome/bl4" }
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
use bl4::{ItemSerial, resolve};

// Full pipeline: decode -> resolve category/identity/parts/rarity/name -> validate
let item = resolve::full_resolve("@Ugr$ZCm/&tH!t{KgK/Shxu>k")?;
println!("{:?} {:?} {:?}", item.name, item.manufacturer, item.level);

// Or decode without resolution
let serial = ItemSerial::decode("@Ugr$ZCm/&tH!t{KgK/Shxu>k")?;
let validation = serial.validate();
println!("Legality: {}", validation.legality);
```

### Campaign Progress

```rust
use bl4::{SaveFile, missions, save::campaign};

let save = SaveFile::from_yaml(&yaml_data)?;

// List main story status
for entry in save.campaign_status() {
    println!("{}: {}", entry.mission_set, entry.status);
}

// Plan and apply progress
let changes = campaign::plan_campaign_progress("mountains2a").unwrap();
save.apply_campaign_progress(&changes)?;
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

**Use at your own risk. Always back up your saves before editing.**

## Special thanks

- [glacierpiece](https://github.com/glacierpiece) for figuring out the save file crypto
- [Cr4nkSt4r](https://github.com/Cr4nkSt4r) for helping me understand the NCS format
- [trumank](https://github.com/trumank) for their work on [retoc](https://github.com/trumank/retoc)

## License

BSD-2-Clause
