# Chapter 7: Using bl4 Tools

Everything we've learned—binary decoding, memory analysis, save file encryption, serial parsing—comes together in the bl4 command-line tools. This chapter serves as your practical reference for day-to-day use.

The tools are designed to be composable. Pipe output between commands. Chain operations together. Build your own workflows for tasks we haven't anticipated.

---

## Building the Tools

### Prerequisites

```bash
# Install Rust (if you haven't already)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone the repository
git clone https://github.com/monokrome/bl4
cd bl4
```

### Build

```bash
cargo build --release -p bl4-cli

# Binary appears in ./target/release/bl4
```

---

## CLI Structure

The bl4 CLI uses subcommands organized by function:

```text
bl4 <COMMAND>

Commands:
  save       Save file operations (decrypt, encrypt, edit, get, set)
  inspect    Inspect a save file (decrypt and display info)
  configure  Configure default settings
  serial     Item serial operations (decode, encode, compare, modify)
  parts      Query parts database
  memory     Read/analyze game memory (live process or dump file)
  ncs        NCS format operations (decompress, scan, show, search)
  idb        Manage the verified items database
  launch     Launch Borderlands 4 with instrumentation
```

Aliases exist for common commands: `s` (save), `i` (inspect), `r` (serial), `m` (memory), `p` (parts).

---

## Save File Operations

### Inspect a Save

Quick view of save contents:

```bash
bl4 inspect 1.sav
bl4 inspect 1.sav --full  # Show complete YAML
```

### Decrypt/Encrypt

```bash
# Decrypt to stdout
bl4 save decrypt 1.sav

# Decrypt to file
bl4 save decrypt 1.sav character.yaml

# Encrypt back
bl4 save encrypt character.yaml 1.sav
```

Steam ID is auto-detected from configuration or can be specified:

```bash
bl4 save decrypt 1.sav --steam-id 76561198012345678
```

### Edit Interactively

Opens decrypted save in your `$EDITOR`, then re-encrypts on save:

```bash
bl4 save edit 1.sav
```

### Get/Set Values

Query specific paths:

```bash
bl4 save get 1.sav "state.currencies.cash"
bl4 save get 1.sav --level   # Character level
bl4 save get 1.sav --money   # Currencies
bl4 save get 1.sav --all     # Everything
```

Set values:

```bash
bl4 save set 1.sav "state.currencies.cash" 999999999
```

---

## Serial Operations

### Decode

```bash
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

Output:

```text
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Item)
Category: Vladof SMG (22)
Level: 50
Tokens: 180928 | 50 | {0:1} 21 {4} , 2 , , 105 102 41
```

!!! note "Part Name Resolution"
    Part names are resolved from `share/manifest/parts_database.json`. Currently ~40% of parts have known mappings from memory extraction. Unknown parts display as `[category:index]` placeholders (e.g., `[22:5]`). See [Chapter 6](06-data-extraction.md) for details on part data coverage.

Options:

```bash
bl4 serial decode --verbose '@Ugr...'  # Byte breakdown
bl4 serial decode --debug '@Ugr...'    # Bit-by-bit parsing
bl4 serial decode --analyze '@Ugr...'  # Token analysis
```

### Compare

Side-by-side comparison of two serials:

```bash
bl4 serial compare '@Ugr$ZCm...' '@Ugr$ABC...'
```

### Modify

Swap parts between serials:

```bash
bl4 serial modify '@base...' '@source...' '4,12'
```

### Batch Decode

Decode many serials to binary for analysis:

```bash
bl4 serial batch-decode serials.txt serials.bin
```

---

## Items Database (idb)

The items database tracks verified item data with source attribution.

### Basic Operations

```bash
bl4 idb init                    # Create database
bl4 idb stats                   # Show counts
bl4 idb list                    # List all items
bl4 idb show '@Ugr...'          # Show item details
```

### Import Items

```bash
# From save file
bl4 idb import-save 1.sav --decode --legal --source monokrome

# Decode all and populate metadata
bl4 idb decode-all
```

### Attachments

```bash
bl4 idb attach '@Ugr...' screenshot.png
```

### Value Attribution

Track values from different sources:

```bash
bl4 idb set-value '@Ugr...' rarity Epic --source ingame --confidence verified
bl4 idb get-values '@Ugr...' rarity
```

---

## Memory Operations

Memory commands work with live processes or dump files.

### With Dump File

```bash
bl4 memory --dump game.dmp info
bl4 memory --dump game.dmp discover gnames
bl4 memory --dump game.dmp discover guobjectarray
```

### Generate Usmap

```bash
bl4 memory --dump game.dmp dump-usmap
```

### FName Operations

```bash
bl4 memory --dump game.dmp fname 12345
bl4 memory --dump game.dmp fname-search "Damage"
```

### Parts Extraction

```bash
bl4 memory --dump game.dmp dump-parts -o parts.json
bl4 memory --dump game.dmp build-parts-db -i parts.json -o parts_db.json
```

### String Search

```bash
bl4 memory --dump game.dmp scan-string "DAD_AR.part_body" -B 128 -A 128
```

---

## NCS Operations

NCS (Nexus Config Store) contains item pools, loot config, and other game data not in standard PAK assets.

### Decompress from Pak

```bash
# Extract all NCS chunks from a pak file
bl4 ncs decompress pakchunk0.pak -o ./ncs_output/

# Extract from specific offset
bl4 ncs decompress pakchunk0.pak --offset 0x15835

# Use native Oodle DLL for 100% compatibility (Windows only)
bl4 ncs decompress pakchunk0.pak -o ./ncs_output/ --oodle-dll /path/to/oo2core_9_win64.dll

# Use external command for Oodle decompression (cross-platform)
bl4 ncs decompress pakchunk0.pak -o ./ncs_output/ --oodle-exec ./oodle_wrapper.sh
```

The `--oodle-exec` command receives `decompress <size>` arguments, compressed data via stdin, and outputs decompressed data to stdout.

### Scan Decompressed Files

```bash
# List all NCS types in a directory
bl4 ncs scan ./ncs_output/

# Filter by type
bl4 ncs scan ./ncs_output/ -t itempool

# Show detailed info
bl4 ncs scan ./ncs_output/ --verbose
```

### Show File Contents

```bash
# Display parsed content
bl4 ncs show ./ncs_output/itempool0.bin

# Show all strings
bl4 ncs show ./ncs_output/itempool0.bin --all-strings

# Output as JSON
bl4 ncs show ./ncs_output/itempool0.bin --json
```

### Search

```bash
# Search for pattern in entry names
bl4 ncs search ./ncs_output/ "legendary"

# Search all strings
bl4 ncs search ./ncs_output/ "damage" --all
```

### Statistics

```bash
bl4 ncs stats ./ncs_output/
bl4 ncs stats ./ncs_output/ --formats  # Show format code breakdown
```

---

## Configuration

Set defaults to avoid repetition:

```bash
bl4 configure --steam-id 76561198012345678
bl4 configure --show
```

Environment variable `BL4_ITEMS_DB` sets the default items database path.

---

## Common Workflows

### Edit a Save

```bash
bl4 save edit ~/.steam/.../1.sav
# Editor opens, make changes, save & quit
# File is automatically re-encrypted
```

### Analyze an Item

```bash
# Extract serial from save
bl4 save get 1.sav "state.inventory.items[0].serial"

# Decode it
bl4 serial decode '@Ugr...'

# Look up in database
bl4 idb show '@Ugr...'
```

### Import Items from Saves

```bash
for sav in saves/*.sav; do
  bl4 idb import-save "$sav" --decode --legal
done
bl4 idb stats
```

### Update After Game Patch

```bash
# New memory dump
sudo gcore -o bl4_new $(pgrep -f wine64-preloader)

# Generate new usmap
bl4 memory --dump bl4_new.* dump-usmap

# Re-extract parts
bl4 memory --dump bl4_new.* extract-parts -o share/manifest/
```

---

## Shell Tips

### Quoting Serials

Serials contain `$`, `!`, `@`. Always use single quotes:

```bash
bl4 serial decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'
```

### Aliases

```bash
alias bl4d='bl4 serial decode'
alias bl4i='bl4 inspect'
alias bl4e='bl4 save edit'
```

### Piping

```bash
bl4 serial decode '@Ugr...' | grep "Category"
bl4 idb list | wc -l
```

---

## Troubleshooting

### "Decryption failed"

- Wrong Steam ID
- Corrupted save file
- Not a BL4 save

Verify the Steam ID matches the save file path.

### "Invalid serial"

- Missing `@Ug` prefix
- Truncated copy
- Wrong quote type in shell

Copy the complete serial and use single quotes.

### "Memory read failed"

- Address outside dump range
- Corrupted dump

Verify the dump covers the target address.

---

## Quick Reference

| Command | Description |
|---------|-------------|
| `bl4 inspect <FILE>` | Quick save inspection |
| `bl4 save decrypt <IN> [OUT]` | Decrypt save to YAML |
| `bl4 save encrypt <IN> <OUT>` | Encrypt YAML to save |
| `bl4 save edit <FILE>` | Edit in $EDITOR |
| `bl4 save get <FILE> <PATH>` | Query value |
| `bl4 save set <FILE> <PATH> <VAL>` | Set value |
| `bl4 serial decode <SERIAL>` | Decode item serial |
| `bl4 serial compare <S1> <S2>` | Compare serials |
| `bl4 ncs decompress <PAK> -o <DIR>` | Extract NCS from pak (use `--oodle-exec` for full compat) |
| `bl4 ncs scan <DIR>` | List NCS types |
| `bl4 ncs show <FILE>` | Show NCS contents |
| `bl4 idb stats` | Database statistics |
| `bl4 idb import-save <FILE>` | Import from save |
| `bl4 memory --dump <F> <CMD>` | Memory analysis |

---

## What's Next?

The appendices provide deep reference material:

- **[Appendix A: SDK Class Layouts](appendix-a-sdk-layouts.md)** — Memory layouts for key UE5 classes
- **[Appendix B: Weapon Parts Reference](appendix-b-weapon-parts.md)** — Complete parts catalog
- **[Appendix C: Loot System Internals](appendix-c-loot-system.md)** — Drop pools and rarity
- **[Appendix D: Game File Structure](appendix-d-game-files.md)** — Asset organization
- **[Glossary](glossary.md)** — Terms and quick reference
