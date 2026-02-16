# Chapter 4: Save File Format

Your entire Borderlands 4 experience—hundreds of hours, thousands of items, every skill point and completed mission—lives in a handful of files. These saves are encrypted, compressed, and structured in ways that seem designed to keep you out. But once you understand the layers, editing them becomes straightforward.

This chapter peels back those layers. We'll decrypt the encryption, decompress the compression, and find human-readable YAML waiting underneath.

---

## Finding Your Saves

Save files live in predictable locations. On Linux with Proton, they're in your Steam compatdata folder (not the userdata folder):

```text
~/.local/share/Steam/steamapps/compatdata/1285190/pfx/drive_c/users/steamuser/
  Documents/My Games/Borderlands 4/Saved/SaveGames/<steam_id>/Profiles/
├── profile.sav          # Your main profile (bank, golden keys, unlocks)
├── client/
│   ├── 1.sav            # Character slot 1
│   ├── 2.sav            # Character slot 2
│   ├── 3.sav            # Character slot 3
│   ├── 4.sav            # Character slot 4
│   └── 5.sav            # Character slot 5
└── ...
```

On Windows, they're typically in:
```text
%USERPROFILE%\Documents\My Games\Borderlands 4\Saved\SaveGames\<steam_id>\
```

Your Steam ID is a 17-digit number starting with 7656119. The game syncs these to Steam Cloud. When editing saves, temporarily disable cloud sync to prevent the game from overwriting your modifications or vice versa.

---

## The Three Layers

BL4 saves are an onion. The outer layer is AES-256-ECB encryption. Peel that away, and you find zlib compression. Decompress that, and you reach YAML—the actual save data in a format you can read and edit.

```{mermaid}
flowchart LR
    A[".sav file"] -->|AES-256-ECB| B["Encrypted blob"]
    B -->|zlib decompress| C["Compressed data"]
    C -->|Parse| D["YAML document"]
```

To edit a save, you reverse this process: decrypt, decompress, edit the YAML, compress, encrypt. The bl4 tools handle the first four steps automatically. Let's understand each layer.

---

## The Encryption Layer

Open a save file in a hex editor and the first bytes tell you what you're dealing with:

```hexdump
00000000: 41 45 53 2D 32 35 36 2D 45 43 42 00 ...
          A  E  S  -  2  5  6  -  E  C  B  \0
```

"AES-256-ECB" in plain ASCII. The game literally labels its encryption scheme. Following that header (32 bytes total) comes the encrypted payload.

AES-256-ECB means:
- **AES**: Advanced Encryption Standard, the industry standard block cipher
- **256**: 256-bit key (32 bytes)
- **ECB**: Electronic Codebook mode, where each 16-byte block is encrypted independently

ECB mode is considered weak for security purposes—identical plaintext blocks produce identical ciphertext blocks, revealing patterns. But for save files, it doesn't matter. The goal isn't Fort Knox security; it's preventing casual tampering. And once you know the key derivation, the encryption is no obstacle at all.

---

## Key Derivation: Your Steam ID Is the Key

The encryption key is derived from your Steam ID. Not generated randomly, not fetched from a server—just computed from a number you can easily find.

The process:

1. Start with a 32-byte base key (constant for all players)
2. Take your Steam ID as a 64-bit integer
3. Convert to 8 bytes, little-endian
4. XOR those 8 bytes with the first 8 bytes of the base key
5. Result: your personal 32-byte encryption key

```rust
const BASE_KEY: [u8; 32] = [
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA,  // XOR'd with Steam ID
    0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29,
    0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87,
];

fn derive_key(steam_id: u64) -> [u8; 32] {
    let mut key = BASE_KEY;
    let steam_bytes = steam_id.to_le_bytes();
    for i in 0..8 {
        key[i] ^= steam_bytes[i];
    }
    key
}
```

Your Steam ID is a 17-digit number starting with 7656119. You can find it in your Steam profile URL, in the save file path, or in memory while the game runs. The bl4 tools require this ID to decrypt your saves.

---

## The Compression Layer

Decrypt the payload and you'll find bytes starting with `78 9C`—the signature of zlib compression with default settings.

Zlib is straightforward. Every programming language has libraries for it. Decompress, and you get raw YAML text.

The compression is effective. A 500KB save might decompress to several megabytes of YAML. All that inventory data, skill trees, mission progress—it compresses well because YAML has lots of repeated structure.

---

## The YAML Structure

Underneath everything, BL4 saves are YAML documents. Human-readable, text-based, editable with any text editor. This is where the interesting data lives.

### Character Save Structure

Character saves (1.sav through 5.sav) contain all character-specific data:

```yaml
state:
  char_guid: EAFFA60B46492388B1ED39807437595D
  class: Char_Paladin              # Char_Paladin, Char_DarkSiren, etc.
  char_name: Amon
  player_difficulty: Easy
  experience:
    - type: Character
      level: 50
      points: 3430207
    - type: Specialization
      level: 3
      points: 3084

  inventory:
    items:
      backpack:
        slot_0:
          serial: '@Uge8Cmm/%Dy!gy?;m8e7QLd...'
          flags: 1
          state_flags: 513
        slot_1:
          serial: '@Ugr$)Nm/)}}!eIEIM^$QlZ...'
          flags: 1
          state_flags: 1
        # ... up to slot_21 or more depending on backpack SDUs

    equipped_inventory:
      equipped:
        slot_0:                    # Primary weapon 1
          - serial: '@Ugd77*Fg_4r=3dZfRG}KRs6...'
            flags: 1
            state_flags: 517
        slot_1:                    # Primary weapon 2
          - serial: '@UgxFw!3C0H^%<l*)jVe^47S...'
            flags: 1
            state_flags: 517
        slot_2:                    # Primary weapon 3
          - serial: '@Ugct)%Fg_4rU>wkBRG/`es7...'
            flags: 1
            state_flags: 517
        slot_3:                    # Primary weapon 4
          - serial: '@Ugydj=3C0H^Ow0rtjVjck61...'
            flags: 1
            state_flags: 517
        slot_4:                    # SHIELD SLOT
          - serial: '@Uge9B?m/)}}!tjfrM>VQ_Z$...'
            flags: 1
            state_flags: 1
        slot_5:                    # Additional weapon/item
          - serial: '@Ugr$fEm/%P$!f1b>P^eCgL6...'
            flags: 1
            state_flags: 517
        slot_6:                    # Gear slot (varies)
          - serial: '@Ugr$xKm/)}}!pQufM-}RPG}...'
            flags: 1
            state_flags: 3
        slot_7:                    # Gear slot (varies)
          - serial: '@Uge8Usm/)}}!sNQ3NWCv7s8...'
            flags: 1
            state_flags: 1
        slot_8:                    # Class mod slot
          - serial: '@Ug!pHG2}TYgOpFIQhx*jtRN...'
            flags: 1
            state_flags: 3

    equip_slots_unlocked:
      - 2
      - 3
      - 6
      - 7
      - 8
    active_slot: 2                 # Currently selected weapon slot

  currencies:
    cash: 44971
    eridium: 210
    golden_key: shift

  ammo:
    assaultrifle: 0
    pistol: 148
    shotgun: 40
    smg: 0
    sniper: 47
    repairkit: 10

  checkpoint_name: World_P.RS_Grasslands_ClaptrapBeach
  total_playtime: 4050.224121

globals:
  time_of_day: Day
  prologue_completed: TRUE
  mainmissioncomplete: TRUE
  # ... mission flags, unlocks, etc.

stats:
  achievements:
    00_level_10: 1
    01_level_30: 1
    # ... achievement tracking
```

### Equipped Slot Mapping

The `equipped_inventory.equipped` section uses numbered slots:

| Slot | Purpose |
|------|---------|
| slot_0 | Primary weapon 1 |
| slot_1 | Primary weapon 2 |
| slot_2 | Primary weapon 3 |
| slot_3 | Primary weapon 4 |
| slot_4 | **Shield** |
| slot_5 | Additional weapon slot |
| slot_6 | Gear slot |
| slot_7 | Gear slot |
| slot_8 | Class mod |

### State Flags

The `state_flags` field is a bitmask indicating item status and labels:

**Bit Definitions (verified in-game):**

| Bit | Value | Meaning |
|-----|-------|---------|
| 0 | 1 | Item exists/valid (always set) |
| 1 | 2 | Favorite |
| 2 | 4 | Junk |
| 4 | 16 | Label 1 |
| 5 | 32 | Label 2 |
| 6 | 64 | Label 3 |
| 7 | 128 | Label 4 |
| 9 | 512 | Backpack only (NOT equipped) |

**Note:** Favorite, Junk, and Labels 1-4 are mutually exclusive—only one can be set at a time.

**How Equipping Works:**

When you equip an item, its serial is **copied** from `inventory.items` to `equipped_inventory.equipped`. Both copies keep bit 9 **clear** (0) to indicate the item is equipped. When unequipped, bit 9 is **set** (1) on the backpack copy and the equipped_inventory copy is removed.

**Common Values:**

| Value | Binary | Meaning |
|-------|--------|---------|
| 1 | `0000000001` | Equipped item |
| 3 | `0000000011` | Equipped item + favorite |
| 513 | `1000000001` | Backpack only (not equipped) |
| 515 | `1000000011` | Backpack only + Favorite |
| 517 | `1000000101` | Backpack only + Junk |
| 529 | `1000010001` | Backpack only + Label 1 |
| 545 | `1000100001` | Backpack only + Label 2 |
| 577 | `1001000001` | Backpack only + Label 3 |
| 641 | `1010000001` | Backpack only + Label 4 |

Items in inventory appear as serials—those Base85-encoded strings we'll decode in Chapter 5. Each item also has `flags` (various item properties) and `state_flags` (the bitmask above).

---

## Working with Saves

The bl4 tools make save editing straightforward.

**Decrypt a save to YAML:**
```bash
bl4 save decrypt 1.sav character.yaml
# or use stdout
bl4 save decrypt 1.sav > character.yaml
```

**Edit the YAML** with any text editor. Add items, change currency, modify stats.

**Re-encrypt:**
```bash
bl4 save encrypt character.yaml 1.sav
```

**Or use the interactive editor** (decrypts, opens in `$EDITOR`, re-encrypts on save):
```bash
bl4 save edit 1.sav
```

The Steam ID is configured once and stored, so you don't need to specify it each time.

---

## Item Injection

To add items to a save, you need to:

1. **Decrypt the save**
2. **Add the item serial to the appropriate location**
3. **Re-encrypt the save**

### Adding to Backpack

Add a new slot entry under `state.inventory.items.backpack`:

```yaml
backpack:
  slot_0:
    serial: '@Uge8Cmm/...'
    flags: 1
    state_flags: 513
  # Add new item as the next slot number
  slot_22:
    serial: '@Uge92<m/)}}!gNodNkyuCbwInLxgj=C`_2FW'
    state_flags: 513
```

### Equipping an Item

**Important**: The equipped_inventory is a *reference* to a backpack item. To equip an item:

1. First, add the item to the backpack
2. Then, add a reference to the same item in equipped_inventory

```yaml
# Step 1: Add to backpack (bit 9 clear = equipped)
state:
  inventory:
    items:
      backpack:
        slot_22:
          serial: '@Uge8jxm/)@{!bAp5s!;381FF>eS^@w'
          flags: 1
          state_flags: 1    # Equipped (bit 9 = 0)

# Step 2: Add to equipped_inventory (same serial, same flags)
    equipped_inventory:
      equipped:
        slot_4:               # Shield slot
          - serial: '@Uge8jxm/)@{!bAp5s!;381FF>eS^@w'
            flags: 1
            state_flags: 1    # Equipped
```

The same serial appears in both places—the backpack holds the actual item data, and equipped_inventory references it.

**Critical**: Only ONE item per slot type can have `state_flags: 1` (equipped). If you have multiple shields all marked as equipped, the game will refuse to equip any of them. Make sure all other shields in your backpack have `state_flags: 513` (backpack only, not equipped).

### Live Editing Limitations

The game **caches character data in memory** once loaded. This means:

- Editing a save file on disk has **no effect** until the game restarts
- Switching characters doesn't reload from disk—the cache persists
- You must **fully quit and restart** the game to see save edits

**Workflow for save editing:**
1. Quit the game completely
2. Edit the save file
3. Restart the game
4. Load the character

**Warning**: Never edit a save for a character you've already loaded this session—your edits will be ignored and potentially overwritten when the game saves.

---

## Common Edits

**Adding currency:**
```yaml
state:
  currencies:
    cash: 999999999
    eridium: 9999
```

**Changing character name:**
```yaml
state:
  char_name: NewName
```

**Changing character level** requires updating experience points to match:
```yaml
state:
  experience:
    - type: Character
      level: 50
      points: 3430207
```

Known character XP thresholds:

| Level | XP Required |
|-------|-------------|
| 1 | 0 |
| 2 | 1,100 |
| 30 | 821,362 |
| 50 | 3,430,207 |

The curve follows approximately `XP ≈ 202 × level^2.44`.

**Specialization levels** use separate XP tracked independently:

| Level | XP Required |
|-------|-------------|
| 2 | ~1,265 |
| 3 | ~2,599 |
| 4 | ~4,690 |
| 5 | ~7,948 |
| 6 | ~12,718 |

**Adding items** requires valid serials. You can copy serials from other saves, the items database, or generate them (once you understand the format from Chapter 5):
```yaml
state:
  inventory:
    items:
      backpack:
        slot_22:
          serial: '@UgYOUR_ITEM_SERIAL_HERE'
          state_flags: 513
```

Invalid serials cause problems—items may not appear, or the game might crash. Always test with a backup save.

---

## Map Exploration Data (foddatas)

The `foddatas` section stores your map exploration progress—the areas you've uncovered as you explore each zone. This data is surprisingly large, as it tracks exploration state at a granular level for every map in the game.

### Structure

```yaml
fodsaveversion: 2
foddatas:
  - levelname: World_P
    foddimensionx: 128
    foddimensiony: 128
    compressiontype: Zlib
    foddata: eJztW3tYTVkb37kMkec...  # Base64-encoded zlib data
  - levelname: Fortress_Grasslands_P
    foddimensionx: 128
    foddimensiony: 128
    compressiontype: Zlib
    foddata: eJztm3k8lO0axv...
  # ... one entry per visited zone
```

Each zone entry contains:

| Field | Description |
|-------|-------------|
| `levelname` | Internal zone identifier (e.g., `World_P`, `Fortress_Grasslands_P`) |
| `foddimensionx` | Grid width (typically 128) |
| `foddimensiony` | Grid height (typically 128) |
| `compressiontype` | Always `Zlib` |
| `foddata` | Base64-encoded, zlib-compressed exploration bitmap |

### Zone Names

| Level Name | In-Game Zone |
|------------|--------------|
| `Intro_P` | Tutorial area |
| `World_P` | Main open world hub |
| `Fortress_Grasslands_P` | Grasslands region |
| `Fortress_Shatteredlands_P` | Shattered Lands region |
| `Fortress_Mountains_P` | Mountains region |
| `ElpisElevator_P` | Elpis elevator zone |
| `Elpis_P` | Moon base |
| `UpperCity_P` | Upper city |

### Copying Exploration Progress

To copy exploration data from one character to another, extract the entire `foddatas` block (including `fodsaveversion`) and replace it in the target save:

```bash
# Decrypt both saves
bl4 save decrypt source.sav source.yaml
bl4 save decrypt target.sav target.yaml

# Copy foddatas section (use text manipulation or YAML tools)
# Then re-encrypt
bl4 save encrypt target_modified.yaml target.sav
```

The foddata is substantial—a fully-explored save can have 40KB+ of exploration data compared to a fresh character's few hundred bytes.

---

## Safehouse and World Progress

The `openworld` section tracks your progression through the open world activities: safehouses captured, silos cleared, bounties completed, and collectibles found.

### Safehouses

```yaml
openworld:
  activities:
    safehouses:
      safehouse_grasslands_1: 1
      safehouse_grasslands_3: 1
      safehouse_grasslands_4: 1
      safehouse_mountains_1: 1
      safehouse_mountains_2: 1
      safehouse_mountains_3: 1
      safehouse_mountains_4: 1
      safehouse_shatteredlands_2: 1
      safehouse_city_1: 1
      safehouse_city_3: 1
```

A value of `1` indicates the safehouse is captured. Missing entries or `0` means uncaptured.

### Silos

```yaml
    silos:
      silo_grasslands_1: 1
      silo_grasslands_2: 1
      silo_grasslands_3: 1
      silo_mountains_1: 1
      silo_mountains_2: 1
      silo_mountains_3: 1
      silo_shatteredlands_1: 1
      silo_shatteredlands_2: 1
      silo_shatteredlands_3: 1
```

### Bounties

Three bounty types track different faction activities:

```yaml
    bounties_augur:
      augurbounty_mountains_1: 1
      augurbounty_mountains_2: 1
      augurbounty_shatteredlands_1: 1
    bounties_order:
      orderbounty_grasslands_1: 1
      orderbounty_grasslands_2: 1
    bounties_vanguard:
      vanguardbounty_grasslands_1: 1
      vanguardbounty_mountains_1: 1
```

### Collectibles

```yaml
    collectibles:
      vaultsymbols:
        vaultsymbol_grasslands_4: 1
        vaultsymbol_grasslands_5: 1
      shrines:
        shrine_mountains_10: 1
      safes:
        safe_shatteredlands_10: 1
      echologs_general:
        el_g_grasslands:
          gra_gen_02: 1
          gra_gen_10: 1
          gra_mis_04: 1
```

---

## Unlockables

The `unlockables` section tracks cosmetic items and vehicle customizations you've collected.

### Hoverdrive Skins

```yaml
unlockables:
  unlockable_hoverdrives:
    entries:
      - unlockable_hoverdrives.jakobs_01
      - unlockable_hoverdrives.daedalus_01
      - unlockable_hoverdrives.jakobs_03
      - unlockable_hoverdrives.borg_03
      - unlockable_hoverdrives.vladof_01
      - unlockable_hoverdrives.maliwan_02
      - unlockable_hoverdrives.order_02
      - unlockable_hoverdrives.tediore_01
```

Each entry represents a vehicle skin tied to a manufacturer. The naming pattern is `unlockable_hoverdrives.<manufacturer>_<number>`.

### Vault Hunter Rank

```yaml
highest_unlocked_vault_hunter_level: 6
```

This tracks your progression through the Vault Hunter Rank challenges. Values typically range from 1 (starting) to 6 (max rank).

---

## Backups

Never edit saves without a backup. Before making any changes, copy your save file:

```bash
# Simple backup
cp 1.sav 1.sav.backup

# Or with timestamp
cp 1.sav "1.sav.$(date +%Y%m%d_%H%M%S).backup"
```

Steam Cloud will also sync your saves. If you're experimenting, disable cloud sync temporarily to prevent conflicts.

---

## What Can Go Wrong

The game validates saves on load. Here's what happens with various issues:

**Invalid YAML syntax**: The game won't load the save at all. Usually a crash or error message.

**Unknown fields**: Generally ignored. The game skips what it doesn't recognize.

**Invalid item serials**: Items may not appear in inventory, or appear as corrupted/unnamed items.

**Out-of-range values**: Often clamped to valid ranges. Level 9999 might become level 72 (the cap).

**Wrong encryption key**: Decryption fails completely—you get garbage instead of zlib-compressed data.

The safest approach: make one change at a time, test immediately, keep backups.

---

## Manual Decryption Walkthrough

If you want to understand the process without tools, here's a Python walkthrough:

**Step 1: Read and parse the header**
```python
with open('profile.sav', 'rb') as f:
    data = f.read()

# First 12 bytes: "AES-256-ECB\0"
assert data[:11] == b'AES-256-ECB'

# Bytes 24-28: compressed size (little-endian u32)
import struct
compressed_size = struct.unpack('<I', data[24:28])[0]
print(f"Compressed size: {compressed_size}")

# Encrypted payload starts at byte 32
encrypted = data[32:]
```

**Step 2: Derive the key**
```python
STEAM_ID = 76561198012345678  # Replace with yours

BASE_KEY = bytes([
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA,
    0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29,
    0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87,
])

steam_bytes = struct.pack('<Q', STEAM_ID)
key = bytearray(BASE_KEY)
for i in range(8):
    key[i] ^= steam_bytes[i]
```

**Step 3: Decrypt**
```python
from Crypto.Cipher import AES

# Pad to 16-byte boundary for AES
padded = encrypted + b'\x00' * (16 - len(encrypted) % 16)

cipher = AES.new(bytes(key), AES.MODE_ECB)
decrypted = cipher.decrypt(padded)[:len(encrypted)]

# Should start with 78 9C (zlib header)
print(f"First bytes: {decrypted[:4].hex()}")
```

**Step 4: Decompress**
```python
import zlib
yaml_data = zlib.decompress(decrypted)
print(yaml_data[:500].decode('utf-8'))
```

At this point, you have the raw YAML. Edit it, then reverse the process: compress with zlib, encrypt with AES-256-ECB using the same key, prepend the header.

---

## Why ECB Mode?

Security-conscious readers might wonder why Gearbox chose ECB mode, which cryptographers consider weak. ECB's flaw is that identical plaintext blocks produce identical ciphertext blocks, potentially revealing patterns.

For save files, this doesn't matter much. The files contain compressed data (which looks random), and the threat model is "casual tampering," not nation-state adversaries. ECB is simple to implement, requires no IV management, and works fine for this use case.

More importantly for us, ECB makes analysis easier. You can decrypt blocks independently, which simplifies debugging. It's a reasonable engineering tradeoff.

---

## Exercises

**Exercise 1: Decrypt Your Save**

Use the bl4 tools to decrypt one of your saves. Examine the YAML structure. Find your character level and current cash.

**Exercise 2: Make a Safe Modification**

1. Back up your save
2. Decrypt to YAML
3. Add 1000 cash to your total
4. Re-encrypt
5. Load the game and verify
6. Restore the backup

**Exercise 3: Find the Item List**

Navigate through the decrypted YAML to `state.inventory.items`. Count your items. Notice that each item is just a serial string and some flags. The serial encodes everything—weapon type, parts, level, stats.

---

## What's Next

You've seen that inventory items are stored as compact serial strings. But what do those strings mean? How does `@Ugr$ZCm/&tH!t{KgK/Shxu>k` encode a complete weapon with manufacturer, parts, level, and random rolls?

The next chapter decodes item serials. It's one of the most intricate pieces of the puzzle, and understanding it unlocks the ability to create, modify, or analyze any item in the game.

**Next: [Chapter 5: Item Serials](05-item-serials.md)**
