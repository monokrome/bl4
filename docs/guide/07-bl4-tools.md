# Chapter 7: Using bl4 Tools

This chapter is your practical reference for using the bl4 command-line tools. Everything we've learned comes together here.

---

## Building the Tools

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone the repository
git clone https://github.com/monokrome/bl4
cd bl4
```

### Build All Tools

```bash
# Build everything in release mode
cargo build --release

# Binaries are in ./target/release/
ls -la target/release/bl4*
```

### Individual Builds

```bash
# Just the main CLI
cargo build --release -p bl4-cli

# Just the research tools
cargo build --release -p bl4-research

# Just uextract
cargo build --release -p uextract
```

---

## bl4 — Main CLI

The primary tool for working with BL4 data.

### Save File Operations

#### Decrypt a Save

```bash
bl4 decrypt profile.sav --steam-id 76561198012345678

# Output to file
bl4 decrypt profile.sav --steam-id 76561198012345678 -o profile.yaml
```

#### Encrypt a Save

```bash
bl4 encrypt profile.yaml --steam-id 76561198012345678 -o profile.sav
```

#### Query Save Data

```bash
# Get character level
bl4 query profile.sav "characters[0].level" --steam-id 76561198012345678

# List all items
bl4 query profile.sav "state.inventory.items[*].serial" --steam-id 76561198012345678
```

### Item Serial Operations

#### Decode a Serial

```bash
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

Output:
```
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Weapon)
Manufacturer: Unknown (180928)
Decoded bytes: 18
Hex: 21 30 C0 42 0C 48 08 32 0C 4E 08 86 74 72 4B 5C 00 8E
Tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
Named:  Unknown(180928) | 50 | {0:1} 21 {ReloadSpeed} , 2 , , 105 102 41
```

#### Verbose Mode

```bash
bl4 decode --verbose '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

Shows additional details:
- Raw decoded bytes
- Bit positions
- Token breakdown

#### Debug Mode

```bash
bl4 decode --debug '@Ugr$ZCm/&tH!t{KgK/Shxu>k' 2>&1
```

Shows bit-by-bit parsing decisions (stderr output).

### Memory Operations

#### Read Memory

```bash
# From dump file
bl4 memory --dump share/dumps/game.raw read 0x1513878f0 --size 64

# Output format
bl4 memory --dump share/dumps/game.raw read 0x1513878f0 --size 64 --format hex
bl4 memory --dump share/dumps/game.raw read 0x1513878f0 --size 64 --format ascii
```

#### Discover Structures

```bash
# Find GNames pool
bl4 memory --dump share/dumps/game.raw discover gnames

# Find GUObjectArray
bl4 memory --dump share/dumps/game.raw discover guobjectarray
```

#### Dump Names

```bash
# Dump all FNames
bl4 memory --dump share/dumps/game.raw dump-names

# Limit output
bl4 memory --dump share/dumps/game.raw dump-names --limit 100
```

#### Generate Usmap

```bash
bl4 memory --dump share/dumps/game.raw dump-usmap

# Output: BL4.usmap in current directory
```

### Usmap Information

```bash
bl4 usmap-info share/manifest/mappings.usmap
```

Output:
```
=== share/manifest/mappings.usmap ===
Magic: 0x30c4
Version: 3
HasVersionInfo: false
Compression: 0 (None)
CompressedSize: 2199074 bytes
DecompressedSize: 2199074 bytes

Names: 64917
Enums: 2986
Enum values: 17291
Structs: 16849
Properties: 58793

File size: 2199090 bytes
```

### Backup Management

```bash
# Create backup
bl4 backup profile.sav

# List backups
bl4 backup --list profile.sav

# Restore backup
bl4 restore profile.sav --timestamp 2025-01-15T10:30:00
```

---

## bl4-research — Data Extraction

Tools for extracting and organizing game data.

### Generate Pak Manifest

```bash
bl4-research pak-manifest \
    -e share/manifest/extracted \
    -o share/manifest
```

Produces:
- `pak_manifest.json` — 81,097 indexed assets
- `pak_summary.json` — Statistics

### Generate Items Database

```bash
bl4-research items-db -m share/manifest
```

Produces:
- `items_database.json` — Drop pools, stats

### Generate Weapon Breakdown

```bash
bl4-research weapons
```

Produces:
- `weapons_breakdown.json` — Counts by type/manufacturer

### Extract Manufacturers

```bash
bl4-research manufacturers
```

Produces:
- `manufacturers.json` — All 10 manufacturers

---

## uextract — Asset Parsing

Custom tool for parsing UE5 assets.

### List Assets

```bash
./target/release/uextract /path/to/Paks --list
```

### Filter Assets

```bash
# Case-insensitive filter
./target/release/uextract /path/to/Paks --list --ifilter "weapon"

# Case-sensitive filter
./target/release/uextract /path/to/Paks --list --filter "Struct_Weapon"
```

### Extract Assets

```bash
./target/release/uextract /path/to/Paks -o ./output
```

### With Usmap

```bash
./target/release/uextract /path/to/Paks \
    -o ./output \
    --usmap share/manifest/mappings.usmap
```

---

## Common Workflows

### Workflow 1: Edit Save File

```bash
# 1. Create backup
bl4 backup ~/.steam/steam/userdata/.../profile.sav

# 2. Decrypt
bl4 decrypt ~/.steam/steam/userdata/.../profile.sav \
    --steam-id 76561198012345678 \
    -o profile.yaml

# 3. Edit with text editor
nano profile.yaml

# 4. Re-encrypt
bl4 encrypt profile.yaml \
    --steam-id 76561198012345678 \
    -o ~/.steam/steam/userdata/.../profile.sav

# 5. Start game and verify
```

### Workflow 2: Analyze Item

```bash
# 1. Get item serial from save
bl4 query profile.sav "state.inventory.items[0].serial" \
    --steam-id 76561198012345678

# 2. Decode the serial
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# 3. Look up part meanings in manifest
cat share/manifest/items_database.json | jq '.items_with_stats'
```

### Workflow 3: Update Game Data After Patch

```bash
# 1. Create new memory dump while game running
sudo gcore -o bl4_new $(pgrep -f wine64-preloader)

# 2. Generate new usmap
bl4 memory --dump bl4_new.* dump-usmap
mv BL4.usmap share/manifest/mappings.usmap

# 3. Re-extract pak data
./target/release/uextract /path/to/Paks \
    -o share/manifest/extracted \
    --usmap share/manifest/mappings.usmap

# 4. Regenerate manifest
bl4-research pak-manifest \
    -e share/manifest/extracted \
    -o share/manifest
```

### Workflow 4: Find a Specific Weapon

```bash
# 1. Search in pak manifest
cat share/manifest/pak_manifest.json | jq '.[] | select(.path | contains("Linebacker"))'

# 2. Extract that specific asset
./target/release/uextract /path/to/Paks \
    -o ./linebacker \
    --ifilter "Linebacker" \
    --usmap share/manifest/mappings.usmap

# 3. Examine the output
cat ./linebacker/*.json | jq .
```

---

## Quick Reference

### Environment Variables

```bash
# Set Steam ID for all commands
export BL4_STEAM_ID=76561198012345678

# Set default dump file
export BL4_DUMP_FILE=share/dumps/game.raw
```

### Common Flags

| Flag | Description |
|------|-------------|
| `-o, --output` | Output file/directory |
| `--steam-id` | Steam ID for encryption |
| `--dump` | Memory dump file |
| `--usmap` | Usmap schema file |
| `--verbose` | More output |
| `--debug` | Debug output (stderr) |
| `-h, --help` | Show help |

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | File not found |
| 4 | Parse error |
| 5 | Encryption/decryption error |

---

## Troubleshooting

### "Invalid Steam ID"

```
Error: Could not parse Steam ID
```

**Solution**: Ensure Steam ID is numeric (e.g., `76561198012345678`)

### "Decryption failed"

```
Error: AES decryption failed
```

**Causes**:
- Wrong Steam ID
- Corrupted save file
- Not a BL4 save file

**Solution**: Verify Steam ID matches the account that created the save

### "Invalid serial"

```
Error: Invalid Base85 character
```

**Causes**:
- Copied serial incorrectly
- Extra whitespace
- Wrong quote characters

**Solution**: Copy serial exactly, including `@Ug` prefix

### "Usmap not found"

```
Error: Could not load usmap file
```

**Solution**:
```bash
# Generate from memory dump
bl4 memory --dump share/dumps/game.raw dump-usmap

# Or use the bundled one
ls share/manifest/mappings.usmap
```

### "Memory read failed"

```
Error: Could not read memory at address 0x...
```

**Causes**:
- Address not in dump
- Dump file corrupted
- Wrong file format

**Solution**: Verify dump covers the address range needed

---

## Tips and Tricks

### Piping Output

```bash
# Decode and extract specific field
bl4 decode '@Ugr...' | grep "Manufacturer"

# Query and count items
bl4 query profile.sav "state.inventory.items[*]" --steam-id ... | jq length
```

### Batch Processing

```bash
# Decode all serials from a file
while read serial; do
    bl4 decode "$serial" 2>/dev/null | head -3
done < serials.txt
```

### JSON Processing

```bash
# Pretty print manifest
cat share/manifest/manufacturers.json | jq .

# Find specific manufacturer
cat share/manifest/manufacturers.json | jq '.DAD'

# Count weapons by type
cat share/manifest/weapons_breakdown.json | jq '.by_type'
```

### Creating Aliases

```bash
# Add to ~/.bashrc
alias bl4d='bl4 decode'
alias bl4q='bl4 query --steam-id $BL4_STEAM_ID'
alias bl4m='bl4 memory --dump share/dumps/game.raw'
```

---

## What's Next?

You now have all the tools to:

1. ✅ Decrypt and edit save files
2. ✅ Decode item serials
3. ✅ Analyze game memory
4. ✅ Extract game data
5. ✅ Generate and use usmap files

### Future Development

Areas still being worked on:

- **Serial encoding** — Create new items from scratch
- **WASM bindings** — Browser-based editor
- **Part pool mapping** — Fully decode what each part index means
- **Inventory manipulation API** — High-level item editing

### Contributing

Found something interesting? Want to help?

1. Document your findings in `docs/`
2. Add test cases for edge cases
3. Submit PRs to https://github.com/monokrome/bl4

---

## Appendix: Complete Command Reference

### bl4

```
bl4 <COMMAND>

Commands:
  decrypt    Decrypt a save file to YAML
  encrypt    Encrypt YAML to a save file
  query      Query save file data
  decode     Decode an item serial
  backup     Manage save backups
  restore    Restore a save backup
  memory     Memory analysis commands
  usmap-info Display usmap file information
  help       Show help
```

### bl4-research

```
bl4-research <COMMAND>

Commands:
  pak-manifest   Generate pak manifest
  items-db       Generate items database
  weapons        Generate weapons breakdown
  manufacturers  Extract manufacturer data
  manifest       Generate all manifests
  help           Show help
```

### uextract

```
uextract <PAK_PATH> [OPTIONS]

Options:
  -o, --output <PATH>   Output directory
  --list                List assets only
  --filter <PATTERN>    Filter by pattern (case-sensitive)
  --ifilter <PATTERN>   Filter by pattern (case-insensitive)
  --usmap <PATH>        Usmap file for property resolution
  -h, --help            Show help
```

---

*Congratulations! You've completed the Borderlands 4 Reverse Engineering Guide.*

*Happy hunting, Vault Hunter!*
