# Chapter 4: Save File Format

Your entire Borderlands 4 experience—hundreds of hours, thousands of items, every skill point and completed mission—lives in a handful of files. These saves are encrypted, compressed, and structured in ways that seem designed to keep you out. But once you understand the layers, editing them becomes straightforward.

This chapter peels back those layers. We'll decrypt the encryption, decompress the compression, and find human-readable YAML waiting underneath.

---

## Finding Your Saves

Save files live in predictable locations. On Linux with Proton, look in your Steam userdata folder:

```
~/.steam/steam/userdata/<steam_id>/2144700/remote/
├── profile.sav          # Your main profile
├── <character_id>.sav   # Individual character saves
└── ...
```

On Windows, they're in your local app data or Steam's userdata depending on how you installed the game.

The game syncs these to Steam Cloud. When editing saves, temporarily disable cloud sync to prevent the game from overwriting your modifications or vice versa.

---

## The Three Layers

BL4 saves are an onion. The outer layer is AES-256-ECB encryption. Peel that away, and you find zlib compression. Decompress that, and you reach YAML—the actual save data in a format you can read and edit.

```
.sav file
    └── AES-256-ECB encrypted
        └── zlib compressed
            └── YAML document
```

To edit a save, you reverse this process: decrypt, decompress, edit the YAML, compress, encrypt. The bl4 tools handle the first four steps automatically. Let's understand each layer.

---

## The Encryption Layer

Open a save file in a hex editor and the first bytes tell you what you're dealing with:

```
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
    0x8E, 0x62, 0xA9, 0x4C, 0x50, 0x60, 0x1A, 0x9D,  // XOR'd with Steam ID
    0x1C, 0x72, 0xD2, 0xAB, 0x95, 0xFC, 0x10, 0xD0,
    0xD7, 0xA9, 0x26, 0x95, 0x70, 0x56, 0x72, 0x7D,
    0xB4, 0x24, 0x2C, 0x77, 0xAD, 0xF2, 0xB1, 0x51,
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

The top-level structure:

```yaml
version: 1
profile:
  steam_id: "76561198012345678"
  created: "2025-01-15T10:30:00Z"

characters:
  - id: "char_001"
    class: "DarkSiren"
    level: 50
    experience: 4500000

state:
  inventory:
    items:
      - serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
        flags: 0
      - serial: "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_"
        flags: 1
  currency:
    cash: 15000000
    eridium: 500
  skills:
    # Skill point allocations
  missions:
    # Completed and active missions
```

The `version` field indicates the save format version. The `profile` section contains account-level data. `characters` lists your characters with their stats. And `state` contains the actual game state—inventory, equipped items, progress, discoveries.

Items in inventory appear as serials—those Base85-encoded strings we'll decode in Chapter 5. Each item also has flags (equipped, favorited, etc.) and metadata.

---

## Working with Saves

The bl4 tools make save editing straightforward.

**Decrypt a save to YAML:**
```bash
bl4 decrypt profile.sav --steam-id 76561198012345678
```

**Edit the YAML** with any text editor. Add items, change currency, modify stats.

**Re-encrypt:**
```bash
bl4 encrypt profile.yaml --steam-id 76561198012345678 -o profile.sav
```

**Query specific data** without full decryption:
```bash
bl4 query profile.sav "state.inventory.items[*].serial" --steam-id 76561198012345678
```

---

## Common Edits

**Adding currency:**
```yaml
state:
  currency:
    cash: 999999999
    eridium: 9999
```

**Changing character level:**
```yaml
characters:
  - id: "char_001"
    level: 72
    experience: 999999999
```

**Adding items** requires valid serials. You can copy serials from other saves, community databases, or generate them (once you understand the format from Chapter 5):
```yaml
state:
  inventory:
    items:
      - serial: "@UgYOUR_ITEM_SERIAL_HERE"
        flags: 0
```

Invalid serials cause problems—items may not appear, or worse, the game might crash. Always test with a backup.

---

## The Backup System

Never edit saves without a backup. The bl4 tools include smart backup management.

```bash
# Create a backup before editing
bl4 backup profile.sav

# List all backups for a file
bl4 backup --list profile.sav

# Restore a specific backup
bl4 restore profile.sav --timestamp 2025-01-15T10:30:00
```

Backups are stored with content hashes, so identical saves don't create duplicate backups. You can accumulate a history of significant states without filling your disk.

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
    0x8E, 0x62, 0xA9, 0x4C, 0x50, 0x60, 0x1A, 0x9D,
    0x1C, 0x72, 0xD2, 0xAB, 0x95, 0xFC, 0x10, 0xD0,
    0xD7, 0xA9, 0x26, 0x95, 0x70, 0x56, 0x72, 0x7D,
    0xB4, 0x24, 0x2C, 0x77, 0xAD, 0xF2, 0xB1, 0x51,
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

