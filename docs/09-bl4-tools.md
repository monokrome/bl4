# Chapter 9: Using bl4 Tools

Everything we've learned---binary decoding, memory analysis, save file encryption, serial parsing---comes together in the bl4 command-line tools. This chapter serves as your practical reference for day-to-day use.

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

The `uextract` tool builds separately:

```bash
cargo build --release -p uextract

# Binary appears in ./target/release/uextract
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
  ncs        NCS format operations (decompress, scan, show, search, extract, debug)
  idb        Manage the verified items database
  drops      Query drop rates and locations for legendary items
  launch     Launch Borderlands 4 with instrumentation
```

Aliases exist for common commands: `s` (save), `i` (inspect), `r` (serial), `m` (memory), `n` (ncs), `d` (drops), `p` (parts), `l` (launch).

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

::: {.callout-note title="Part Name Resolution"}
Part names are resolved from `share/manifest/parts_database.json`. Currently ~40% of parts have known mappings from memory extraction. Unknown parts display as `[category:index]` placeholders (e.g., `[22:5]`). See [Chapter 7](07-data-extraction.md) for details on part data coverage.
:::

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

## Drop Rate Queries

The `drops` command queries a manifest of legendary item drop sources, extracted from NCS data. It tells you where items drop and what each source drops.

### Find Where an Item Drops

```bash
bl4 drops find Hellwalker
bl4 drops find "Plasma Coil"
```

Output is sorted by drop rate (highest first), showing the source, source type, tier, and chance for each location.

### List All Drops from a Source

```bash
bl4 drops source Timekeeper
bl4 drops source "Black Market"
bl4 drops source "Fish Collector"
```

### List All Known Items or Sources

```bash
bl4 drops list              # List all known legendary items
bl4 drops list --sources    # List all known drop sources
```

### Generate Drops Manifest

Build the drops manifest from decompressed NCS data:

```bash
bl4 drops generate ./ncs_output/ -o share/manifest/drops.json
```

By default, the `find`, `source`, and `list` commands use `share/manifest/drops.json`. Override with `--manifest <PATH>`.

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

The `list` command supports filtering and output format control:

```bash
bl4 idb list --manufacturer JAK --rarity Legendary
bl4 idb list --format json
bl4 idb list --format csv -f serial,name,rarity
```

### Import Items

```bash
# From save file
bl4 idb import-save 1.sav --decode --legal --source monokrome

# From share/weapons directory structure
bl4 idb import share/weapons/

# Decode all and populate metadata
bl4 idb decode-all
```

### Attachments

```bash
bl4 idb attach '@Ugr...' screenshot.png
bl4 idb attach '@Ugr...' card.png --popup    # Mark as item card view
bl4 idb attach '@Ugr...' inspect.png --detail # Mark as 3D inspect view
```

### Value Attribution

Track values from different sources:

```bash
bl4 idb set-value '@Ugr...' rarity Epic --source ingame --confidence verified
bl4 idb get-values '@Ugr...' rarity
```

### Verification and Curation

```bash
bl4 idb verify '@Ugr...' verified --notes "Confirmed in-game screenshot"
bl4 idb mark-legal '@Ugr...'
bl4 idb mark-legal all                   # Mark everything as legal
bl4 idb set-source monokrome '@Ugr...'
bl4 idb set-source community --where "legal = 0"
```

### Export and Merge

```bash
bl4 idb export '@Ugr...' ./item_dir/
bl4 idb merge source.db destination.db
```

### Community Server Sync

Publish items to and pull items from the community server:

```bash
bl4 idb publish                             # Publish all items
bl4 idb publish --serial '@Ugr...'          # Publish one item
bl4 idb publish --attachments --dry-run     # Preview with screenshots

bl4 idb pull                                # Pull all items
bl4 idb pull --authoritative                # Prefer remote values
bl4 idb pull --dry-run                      # Preview what would change
```

The default server is `https://items.bl4.dev`. Override with `--server <URL>`.

### Database Maintenance

```bash
bl4 idb migrate-values              # Migrate column values to item_values table
bl4 idb migrate-values --dry-run    # Preview migration
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

### Object Inspection

```bash
bl4 memory --dump game.dmp objects --class ItemPoolDef --limit 50
bl4 memory --dump game.dmp list-objects --class-filter "Part" --limit 100
bl4 memory --dump game.dmp list-objects --stats
bl4 memory --dump game.dmp list-uclasses --filter "Inventory"
bl4 memory --dump game.dmp find-class-uclass
```

### Parts Extraction

```bash
bl4 memory --dump game.dmp dump-parts -o parts.json
bl4 memory --dump game.dmp build-parts-db -i parts.json -o parts_db.json
```

Extract parts with authoritative Category/Index values from UObjects:

```bash
bl4 memory --dump game.dmp extract-parts -o parts_with_categories.json
bl4 memory --dump game.dmp extract-parts --list-fnames    # Debug: list part FNames
bl4 memory --dump game.dmp extract-parts-raw -o parts_raw.json
```

### Object Discovery

Find objects matching a name pattern and generate lookup maps:

```bash
bl4 memory --dump game.dmp find-objects-by-pattern ".part_" --limit 20
bl4 memory --dump game.dmp generate-object-map -o objects.json
```

### NCS Schema Extraction

Extract NCS field hash-to-name mappings from process memory:

```bash
bl4 memory --dump game.dmp extract-ncs-schema -o share/manifests/bl4.ncsmap
```

### String Search

```bash
bl4 memory --dump game.dmp scan-string "DAD_AR.part_body" -B 128 -A 128
```

### Low-Level Memory Access

These commands work with live processes (no dump file):

```bash
bl4 memory read 0x7f1234567890 --size 256
bl4 memory write 0x7f1234567890 "90 90 90"
bl4 memory scan "48 8B 05 ?? ?? ?? ??"
bl4 memory patch 0x7f1234567890 --nop 5
bl4 memory patch 0x7f1234567890 --bytes "EB 05"
```

### Preload Library

The preload library intercepts file I/O for NCS extraction and debugging:

```bash
bl4 memory preload info                     # Show LD_PRELOAD command
bl4 memory preload run --capture ./out -- wine64 game.exe
bl4 memory preload watch                    # Tail the preload log
bl4 memory monitor --filter "fopen"         # Monitor log with function filter
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

# Output as JSON
bl4 ncs scan ./ncs_output/ --json
```

### Show File Contents

```bash
# Display parsed content
bl4 ncs show ./ncs_output/itempool0.bin

# Show all strings
bl4 ncs show ./ncs_output/itempool0.bin --all-strings

# Output as JSON or TSV
bl4 ncs show ./ncs_output/itempool0.bin --json
bl4 ncs show ./ncs_output/itempool0.bin --tsv
```

### Search

```bash
# Search for pattern in entry names
bl4 ncs search ./ncs_output/ "legendary"

# Search all strings
bl4 ncs search ./ncs_output/ "damage" --all

# Limit results
bl4 ncs search ./ncs_output/ "barrel" --all -n 50
```

### Extract Data

The `extract` command pulls structured data from NCS files. The `-t` flag selects the extraction type:

```bash
# Extract categorized parts manifest (produces parts_database.json + category_names.json)
bl4 ncs extract ./ncs_output/ -t manifest -o share/manifest/parts_database.json

# Extract part serial indices from inv.bin
bl4 ncs extract ./ncs_output/ -t parts

# Extract item-to-parts mapping
bl4 ncs extract ./ncs_output/ -t item-parts --json

# Extract NexusSerialized display name mappings
bl4 ncs extract ./ncs_output/ -t names

# Extract manufacturer code-to-name mappings
bl4 ncs extract ./ncs_output/ -t manufacturers --json

# Extract raw string table
bl4 ncs extract ./ncs_output/ -t strings

# Extract string-numeric pairs
bl4 ncs extract ./ncs_output/ -t pairs

# Build serial index decoder from all inv*.bin files
bl4 ncs extract ./ncs_output/ -t decoder --json

# Extract using the binary parser (structured document output)
bl4 ncs extract ./ncs_output/ -t binary --json

# Or extract by NCS type name directly (e.g., itempool, rarity)
bl4 ncs extract ./ncs_output/ -t itempool --json
```

### Debug

Inspect the binary structure of an NCS file:

```bash
bl4 ncs debug ./ncs_output/inv0.bin
bl4 ncs debug ./ncs_output/inv0.bin --hex         # Show hex dump
bl4 ncs debug ./ncs_output/inv0.bin --parse        # Parse binary with bit reader
bl4 ncs debug ./ncs_output/inv0.bin --offsets      # Show all section offsets
```

### Statistics

```bash
bl4 ncs stats ./ncs_output/
bl4 ncs stats ./ncs_output/ --formats  # Show format code breakdown
```

---

## Launch

Launch BL4 through Steam with the preload instrumentation library:

```bash
bl4 launch          # Shows Steam launch options, prompts for confirmation
bl4 launch -y       # Skip confirmation prompt
```

This finds `libbl4_preload.so`, prints the `LD_PRELOAD` launch option string for Steam, and optionally launches the game via `steam://rungameid/1285190`. The preload library must be built first:

```bash
cargo build --release -p bl4-preload
```

Environment variables control preload behavior: `BL4_RNG_BIAS` (max/high/low/min), `BL4_PRELOAD_ALL` (1 to intercept all I/O), `BL4_PRELOAD_STACKS` (1 for stack traces). The log goes to `/tmp/bl4_preload.log`.

---

## uextract

The `uextract` tool handles UE5 IoStore asset extraction---a separate binary from bl4, focused on unpacking game assets.

### Extract from IoStore

```bash
# Extract all assets to JSON + uasset
uextract /path/to/Paks -o ./extracted/

# Extract with property schema and class resolution
uextract /path/to/Paks -o ./extracted/ --usmap BL4.usmap --scriptobjects scriptobjects.json

# Filter by path and class
uextract /path/to/Paks -o ./extracted/ --filter "ItemPool" --class-filter "ItemPoolDef"

# List matching files without extracting
uextract /path/to/Paks --list --filter "Weapon"

# JSON only, no raw uasset
uextract /path/to/Paks -o ./extracted/ --format json
```

### Extract from Traditional Pak Files

```bash
uextract pak /path/to/file.pak -o ./extracted/
uextract pak /path/to/file.pak --extension ncs --list
```

### Dump ScriptObjects

Generate the class hash-to-path lookup used by other commands:

```bash
uextract script-objects /path/to/Paks -o scriptobjects.json
```

### Find Assets by Class

```bash
uextract find-by-class /path/to/Paks InventoryPartDef --scriptobjects scriptobjects.json
uextract find-by-class /path/to/Paks ItemPoolDef -o itempool_paths.txt
```

### List Classes

```bash
uextract list-classes /path/to/Paks --scriptobjects scriptobjects.json --samples 5
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

# Check where it drops
bl4 drops find "Hellwalker"
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

# Rebuild parts manifest from NCS
bl4 ncs extract ./ncs_output/ -t manifest -o share/manifest/parts_database.json

# Regenerate drops manifest
bl4 drops generate ./ncs_output/ -o share/manifest/drops.json
```

### Full Asset Extraction Pipeline

```bash
# 1. Dump ScriptObjects for class resolution
uextract script-objects /path/to/Paks -o scriptobjects.json

# 2. Extract all assets with property schema
uextract /path/to/Paks -o ./extracted/ --usmap BL4.usmap --scriptobjects scriptobjects.json

# 3. Find specific asset types
uextract find-by-class /path/to/Paks ItemPoolDef --scriptobjects scriptobjects.json
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
alias bl4f='bl4 drops find'
```

### Piping

```bash
bl4 serial decode '@Ugr...' | grep "Category"
bl4 idb list | wc -l
bl4 drops list --sources | sort
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

### "Preload library not found"

Build the preload library before using `bl4 launch`:

```bash
cargo build --release -p bl4-preload
```

### "Failed to load drops manifest"

Generate the drops manifest first, or point `--manifest` at an existing one:

```bash
bl4 drops generate ./ncs_output/ -o share/manifest/drops.json
```

---

## Quick Reference

| Command | Description |
|---------|-------------|
| **Save** | |
| `bl4 inspect <FILE>` | Quick save inspection |
| `bl4 save decrypt <IN> [OUT]` | Decrypt save to YAML |
| `bl4 save encrypt <IN> <OUT>` | Encrypt YAML to save |
| `bl4 save edit <FILE>` | Edit in $EDITOR |
| `bl4 save get <FILE> <PATH>` | Query value |
| `bl4 save set <FILE> <PATH> <VAL>` | Set value |
| **Serial** | |
| `bl4 serial decode <SERIAL>` | Decode item serial |
| `bl4 serial compare <S1> <S2>` | Compare serials |
| `bl4 serial modify <BASE> <SRC> <PARTS>` | Swap parts between serials |
| `bl4 serial batch-decode <IN> <OUT>` | Batch decode to binary |
| **Drops** | |
| `bl4 drops find <ITEM>` | Find where an item drops |
| `bl4 drops source <SOURCE>` | List items from a source |
| `bl4 drops list [--sources]` | List all items or sources |
| `bl4 drops generate <DIR> -o <OUT>` | Generate drops manifest |
| **NCS** | |
| `bl4 ncs decompress <PAK> -o <DIR>` | Extract NCS from pak (`--oodle-exec` for full compat) |
| `bl4 ncs scan <DIR>` | List NCS types |
| `bl4 ncs show <FILE>` | Show NCS contents |
| `bl4 ncs search <DIR> <PATTERN>` | Search NCS files |
| `bl4 ncs extract <DIR> -t <TYPE>` | Extract structured data |
| `bl4 ncs debug <FILE>` | Debug binary structure |
| `bl4 ncs stats <DIR>` | Show NCS statistics |
| **Items DB** | |
| `bl4 idb init` | Create database |
| `bl4 idb stats` | Database statistics |
| `bl4 idb list` | List items (supports `--format`, `--manufacturer`, etc.) |
| `bl4 idb show <SERIAL>` | Show item details |
| `bl4 idb import-save <FILE>` | Import from save |
| `bl4 idb verify <SERIAL> <STATUS>` | Set verification status |
| `bl4 idb export <SERIAL> <DIR>` | Export item to directory |
| `bl4 idb merge <SRC> <DEST>` | Merge databases |
| `bl4 idb publish` | Publish to community server |
| `bl4 idb pull` | Pull from community server |
| `bl4 idb set-source <SRC> <IDS...>` | Set item source |
| `bl4 idb mark-legal <IDS...>` | Mark items as legal |
| `bl4 idb migrate-values` | Migrate to item_values table |
| **Memory** | |
| `bl4 memory --dump <F> info` | Process/dump info |
| `bl4 memory --dump <F> discover <TARGET>` | Discover UE5 structures |
| `bl4 memory --dump <F> dump-usmap` | Generate usmap file |
| `bl4 memory --dump <F> fname <INDEX>` | Look up FName |
| `bl4 memory --dump <F> extract-parts` | Extract parts from UObjects |
| `bl4 memory --dump <F> extract-parts-raw` | Extract raw part data |
| `bl4 memory --dump <F> find-objects-by-pattern <PAT>` | Find objects by name |
| `bl4 memory --dump <F> generate-object-map` | Generate object map JSON |
| `bl4 memory --dump <F> extract-ncs-schema` | Extract NCS field schema |
| `bl4 memory preload info` | Show LD_PRELOAD command |
| **Launch** | |
| `bl4 launch [-y]` | Launch BL4 with instrumentation |
| **uextract** | |
| `uextract <PAKS> -o <DIR>` | Extract IoStore assets |
| `uextract pak <FILE> -o <DIR>` | Extract from .pak files |
| `uextract script-objects <PAKS> -o <OUT>` | Dump ScriptObjects to JSON |
| `uextract find-by-class <PAKS> <CLASS>` | Find assets by class |
| `uextract list-classes <PAKS>` | List unique class hashes |

---

## What's Next?

The appendices provide deep reference material:

- **[Appendix A: SDK Class Layouts](appendix-a-sdk-layouts.md)** --- Memory layouts for key UE5 classes
- **[Appendix B: Weapon Parts Reference](appendix-b-weapon-parts.md)** --- Complete parts catalog
- **[Appendix C: Loot System Internals](appendix-c-loot-system.md)** --- Drop pools and rarity
- **[Appendix D: Game File Structure](appendix-d-game-files.md)** --- Asset organization
- **[Glossary](glossary.md)** --- Terms and quick reference
