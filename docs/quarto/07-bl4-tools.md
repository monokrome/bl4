# Chapter 7: Using bl4 Tools

Everything we've learned—binary decoding, memory analysis, save file encryption, serial parsing—comes together in the bl4 command-line tools. This chapter serves as your practical reference for day-to-day use.

The tools are designed to be composable. Pipe output between commands. Chain operations together. Build your own workflows for tasks we haven't anticipated. The goal is giving you capabilities, not restricting you to pre-defined paths.

---

## Building the Tools

Before using the tools, you need to build them from source.

### Prerequisites

```bash
# Install Rust (if you haven't already)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone the repository
git clone https://github.com/monokrome/bl4
cd bl4
```

### Build Everything

```bash
cargo build --release

# Binaries appear in ./target/release/
ls -la target/release/bl4*
```

::: {.callout-tip}
Use `--release` for significantly faster execution. Debug builds are useful during development but noticeably slower for memory analysis operations.

:::

### Individual Builds

If you only need specific tools:

```bash
cargo build --release -p bl4-cli      # Main CLI
cargo build --release -p bl4-research # Research tools
cargo build --release -p uextract     # Asset extraction
```

---

## bl4 — The Main CLI

This is your primary interface for most operations.

### Save File Operations

**Decrypt a save to YAML:**

```bash
bl4 decrypt profile.sav --steam-id 76561198012345678

# Write to specific file
bl4 decrypt profile.sav --steam-id 76561198012345678 -o profile.yaml
```

**Re-encrypt after editing:**

```bash
bl4 encrypt profile.yaml --steam-id 76561198012345678 -o profile.sav
```

**Query specific data without full decryption:**

```bash
# Get character level
bl4 query profile.sav "characters[0].level" --steam-id 76561198012345678

# List all item serials
bl4 query profile.sav "state.inventory.items[*].serial" --steam-id 76561198012345678
```

::: {.callout-note title="Steam ID Required"}
The encryption key derives from your Steam ID. Using the wrong ID produces garbage output—not an error message. If decryption produces unreadable data, double-check the ID.

:::

### Item Serial Operations

**Decode a serial:**

```bash
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

Output shows item type, manufacturer, raw hex, and parsed tokens:

```
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Weapon)
Part Group: 22 (Vladof SMG)
Tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

**Verbose mode** shows raw bytes and bit positions:

```bash
bl4 decode --verbose '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

**Debug mode** outputs bit-by-bit parsing decisions to stderr:

```bash
bl4 decode --debug '@Ugr$ZCm/&tH!t{KgK/Shxu>k' 2>&1
```

::: {.callout-tip title="Quoting Serials"}
Serials contain special characters (`$`, `!`, `@`). Always wrap them in single quotes to prevent shell interpretation.

:::

### Memory Operations

Memory commands require a dump file specified with `--dump`:

**Read raw memory:**

```bash
bl4 memory --dump share/dumps/game.dmp read 0x1513878f0 --size 64

# Different output formats
bl4 memory --dump share/dumps/game.dmp read 0x1513878f0 --size 64 --format hex
bl4 memory --dump share/dumps/game.dmp read 0x1513878f0 --size 64 --format ascii
```

**Discover Unreal structures:**

```bash
bl4 memory --dump share/dumps/game.dmp discover gnames
bl4 memory --dump share/dumps/game.dmp discover guobjectarray
```

**Generate usmap from memory:**

```bash
bl4 memory --dump share/dumps/game.dmp dump-usmap
# Output: BL4.usmap in current directory
```

**Search for strings:**

```bash
bl4 memory --dump share/dumps/game.dmp scan-string "DAD_AR.part_body" \
    -B 128 -A 128 -l 10
```

Shows matches with context bytes before (`-B`) and after (`-A`).

**FName operations:**

```bash
# Look up by index
bl4 memory --dump share/dumps/game.dmp fname 12345

# Search for names containing pattern
bl4 memory --dump share/dumps/game.dmp fname-search "Damage"
```

### Parts Database Extraction

::: {.callout-note title="Parts Database Preview"}
This builds the mappings between serial tokens and actual game parts. See Chapter 6 for why this requires memory extraction rather than pak file analysis.

:::

```bash
# Step 1: Extract part names from memory
bl4 memory --dump share/dumps/game.dmp dump-parts \
    -o share/manifest/parts_dump.json

# Step 2: Build database with category mappings
bl4 memory --dump share/dumps/game.dmp build-parts-db \
    -i share/manifest/parts_dump.json \
    -o share/manifest/parts_database.json
```

### Usmap Information

Inspect a usmap file:

```bash
bl4 usmap-info share/manifest/mappings.usmap
```

Output shows names, enums, structs, and property counts.

**Search for specific structures:**

```bash
bl4 usmap-search share/manifest/mappings.usmap "GbxSerialNumberIndex"
```

### Backup Management

**Create backup before editing:**

```bash
bl4 backup profile.sav
```

**List existing backups:**

```bash
bl4 backup --list profile.sav
```

**Restore a specific backup:**

```bash
bl4 restore profile.sav --timestamp 2025-01-15T10:30:00
```

::: {.callout-important}
Always backup before editing. The game might reject modified saves, and cloud sync can overwrite your work.

:::

---

## bl4-research — Data Extraction

Research tools for organizing extracted game data.

**Generate pak manifest:**

```bash
bl4-research pak-manifest \
    -e share/manifest/extracted \
    -o share/manifest

# Produces: pak_manifest.json, pak_summary.json
```

**Generate items database:**

```bash
bl4-research items-db -m share/manifest
```

**Generate weapons breakdown:**

```bash
bl4-research weapons
```

**Extract manufacturer data:**

```bash
bl4-research manufacturers
```

---

## uextract — Asset Parsing

Custom tool for parsing UE5 assets from pak files.

**List all assets:**

```bash
./target/release/uextract /path/to/Paks --list
```

**Filter by pattern:**

```bash
# Case-insensitive
./target/release/uextract /path/to/Paks --list --ifilter "weapon"

# Case-sensitive
./target/release/uextract /path/to/Paks --list --filter "Struct_Weapon"
```

**Extract with usmap:**

```bash
./target/release/uextract /path/to/Paks \
    -o ./output \
    --usmap share/manifest/mappings.usmap
```

---

## Common Workflows

### Workflow 1: Edit a Save File

The complete flow from backup through verification:

```bash
# 1. Backup
bl4 backup ~/.steam/steam/userdata/.../profile.sav

# 2. Decrypt
bl4 decrypt ~/.steam/steam/userdata/.../profile.sav \
    --steam-id 76561198012345678 \
    -o profile.yaml

# 3. Edit
nano profile.yaml  # or your preferred editor

# 4. Re-encrypt
bl4 encrypt profile.yaml \
    --steam-id 76561198012345678 \
    -o ~/.steam/steam/userdata/.../profile.sav

# 5. Start game and verify changes took effect
```

### Workflow 2: Analyze an Item

From serial string to understanding what you're looking at:

```bash
# Get serial from save
bl4 query profile.sav "state.inventory.items[0].serial" \
    --steam-id 76561198012345678

# Decode the serial
bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Look up part meanings
cat share/manifest/parts_database.json | jq '.categories["22"]'
```

### Workflow 3: Update After Game Patch

When Gearbox releases an update, regenerate your data:

```bash
# 1. New memory dump while game running
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

Trace from weapon name to game data:

```bash
# Search manifest
cat share/manifest/pak_manifest.json | \
    jq '.[] | select(.path | contains("Linebacker"))'

# Extract that asset
./target/release/uextract /path/to/Paks \
    -o ./linebacker \
    --ifilter "Linebacker" \
    --usmap share/manifest/mappings.usmap

# Examine
cat ./linebacker/*.json | jq .
```

---

## Environment Variables

Set these to avoid repeating common options:

```bash
# Add to ~/.bashrc or ~/.zshrc
export BL4_STEAM_ID=76561198012345678
export BL4_DUMP_FILE=share/dumps/game.dmp
```

Commands will use these as defaults when corresponding flags aren't provided.

---

## Shell Integration

### Useful Aliases

```bash
alias bl4d='bl4 decode'
alias bl4q='bl4 query --steam-id $BL4_STEAM_ID'
alias bl4m='bl4 memory --dump $BL4_DUMP_FILE'
```

### Piping and Composition

```bash
# Decode and extract specific field
bl4 decode '@Ugr...' | grep "Part Group"

# Count inventory items
bl4 query profile.sav "state.inventory.items[*]" \
    --steam-id 76561198012345678 | jq length

# Batch decode serials
while read serial; do
    bl4 decode "$serial" 2>/dev/null | head -3
done < serials.txt
```

### JSON Processing with jq

```bash
# Pretty print
cat share/manifest/manufacturers.json | jq .

# Find specific manufacturer
cat share/manifest/manufacturers.json | jq '.DAD'

# Count weapons by type
cat share/manifest/weapons_breakdown.json | jq '.by_type'
```

---

## Troubleshooting

### "Invalid Steam ID"

**Cause**: Non-numeric characters in Steam ID.

**Solution**: Use only the 17-digit numeric ID (e.g., `76561198012345678`), not your custom URL or display name.

### "Decryption failed"

**Causes**:
- Wrong Steam ID
- Corrupted save file
- Not a BL4 save file

**Solution**: Verify the Steam ID matches the account that created the save. Check the save file path contains your actual Steam ID.

### "Invalid serial"

**Cause**: Copy error—extra whitespace, wrong quote type, partial copy.

**Solution**: Copy the serial exactly, including the `@Ug` prefix. Use single quotes in shell commands.

### "Usmap not found"

**Solution**:

```bash
# Generate fresh from memory dump
bl4 memory --dump share/dumps/game.dmp dump-usmap

# Or verify bundled file exists
ls -la share/manifest/mappings.usmap
```

### "Memory read failed"

**Causes**:
- Address outside dump's range
- Corrupted dump file
- Wrong dump format

**Solution**: Verify the dump covers the required address range. For Linux core dumps, addresses may need translation from the MDMP-style addresses documented elsewhere.

---

## Quick Reference

### Common Flags

| Flag | Description |
|------|-------------|
| `-o, --output` | Output file or directory |
| `--steam-id` | Steam ID for encryption |
| `--dump` | Memory dump file path |
| `--usmap` | Usmap schema file |
| `--verbose` | Extended output |
| `--debug` | Debug info (stderr) |
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

## Command Reference

### bl4

```
bl4 <COMMAND>

Commands:
  decrypt      Decrypt a save file to YAML
  encrypt      Encrypt YAML to a save file
  query        Query save file data
  decode       Decode an item serial
  backup       Create save backup
  restore      Restore save backup
  memory       Memory analysis commands
  usmap-info   Display usmap file information
  usmap-search Search usmap for struct/property names
  help         Show help
```

### bl4 memory

```
bl4 memory --dump <FILE> <COMMAND>

Commands:
  info           Dump file information
  discover       Find GNames, GUObjectArray
  objects        List UObjects by class
  dump-usmap     Generate usmap
  fname          Look up FName by index
  fname-search   Search FName pool
  read           Read bytes at address
  scan-string    Search for string
  dump-parts     Extract part names
  build-parts-db Build parts database
  help           Show help
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
  --filter <PATTERN>    Filter (case-sensitive)
  --ifilter <PATTERN>   Filter (case-insensitive)
  --usmap <PATH>        Usmap file for parsing
  -h, --help            Show help
```

---

## What's Next?

You now have the practical skills to:

- Decrypt and edit save files
- Decode item serials
- Analyze game memory
- Extract game data
- Generate and use usmap files

The appendices provide deep reference material for specific areas:

- **[Appendix A: SDK Class Layouts](appendix-a-sdk-layouts.md)** — Memory layouts for key UE5 classes
- **[Appendix B: Weapon Parts Reference](appendix-b-weapon-parts.md)** — Complete parts catalog by manufacturer
- **[Appendix C: Loot System Internals](appendix-c-loot-system.md)** — Drop pools and rarity
- **[Appendix D: Game File Structure](appendix-d-game-files.md)** — Asset organization
- **[Glossary](glossary.md)** — Terms and quick reference

### Contributing

Found something interesting? Want to help?

1. Document discoveries in `docs/`
2. Add test cases for edge cases
3. Submit PRs to https://github.com/monokrome/bl4

---

*Congratulations! You've completed the Borderlands 4 Reverse Engineering Guide.*

*Happy hunting, Vault Hunter!*

