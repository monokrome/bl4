# Chapter 4: Save File Format

BL4 save files contain your character's complete state: inventory, progress, skills, and cosmetics. This chapter covers how they're encrypted, compressed, and structured.

---

## Save File Locations

### Steam (Linux/Proton)

```
~/.steam/steam/userdata/<steam_id>/2144700/remote/
├── profile.sav          # Main profile
├── <character_id>.sav   # Per-character saves
└── ...
```

### Steam (Windows)

```
C:\Users\<username>\AppData\Local\<game_folder>\Saved\SaveGames\
```

Or via Steam Cloud:
```
C:\Program Files (x86)\Steam\userdata\<steam_id>\2144700\remote\
```

!!! tip
    Steam syncs saves to the cloud. Disable cloud sync while editing to prevent conflicts.

---

## File Structure Overview

BL4 saves are layered:

```
┌─────────────────────────────┐
│     Encrypted Container     │  ← AES-256-ECB
├─────────────────────────────┤
│     Compressed Data         │  ← zlib
├─────────────────────────────┤
│     YAML Document           │  ← Human-readable structure
└─────────────────────────────┘
```

**Pipeline**:
- **Save**: YAML → zlib compress → AES encrypt → .sav file
- **Load**: .sav file → AES decrypt → zlib decompress → YAML

---

## The Encryption Layer

### File Header

The first bytes reveal the encryption scheme:

```
Offset  Bytes           ASCII
0x00    41 45 53 2D     AES-
0x04    32 35 36 2D     256-
0x08    45 43 42 00     ECB.
0x0C    00 00 00 00     ....  (padding)
0x10    00 00 00 00     ....  (padding)
0x14    00 00 00 00     ....  (padding)
0x18    A8 DE 07 00     ....  (compressed size, little-endian)
0x1C    00 00 00 00     ....  (padding)
0x20    [encrypted payload begins]
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0x00 | 12 | Magic | "AES-256-ECB\0" |
| 0x0C | 12 | Reserved | Zeros/padding |
| 0x18 | 4 | CompSize | Compressed payload size (LE u32) |
| 0x1C | 4 | Reserved | Zeros |
| 0x20 | N | Payload | Encrypted, compressed YAML |

### AES-256-ECB Encryption

BL4 uses AES-256 in ECB mode:
- **Block size**: 16 bytes
- **Key size**: 32 bytes
- **Mode**: ECB (each block encrypted independently)

!!! warning
    **ECB mode is weak** — identical plaintext blocks produce identical ciphertext. This is a design flaw that helps us, but games shouldn't use ECB.

### Key Derivation

The encryption key is derived from your Steam ID:

```rust
// Base key (constant across all saves)
const BASE_KEY: [u8; 32] = [
    0x8E, 0x62, 0xA9, 0x4C, 0x50, 0x60, 0x1A, 0x9D,  // First 8 bytes: XOR target
    0x1C, 0x72, 0xD2, 0xAB, 0x95, 0xFC, 0x10, 0xD0,
    0xD7, 0xA9, 0x26, 0x95, 0x70, 0x56, 0x72, 0x7D,
    0xB4, 0x24, 0x2C, 0x77, 0xAD, 0xF2, 0xB1, 0x51,
];

fn derive_key(steam_id: &str) -> [u8; 32] {
    // Extract only digits from Steam ID
    let digits: String = steam_id.chars().filter(|c| c.is_ascii_digit()).collect();

    // Parse as u64
    let steam_id_num: u64 = digits.parse().unwrap();

    // Convert to 8-byte little-endian
    let steam_bytes = steam_id_num.to_le_bytes();

    // XOR with first 8 bytes of base key
    let mut key = BASE_KEY;
    for i in 0..8 {
        key[i] ^= steam_bytes[i];
    }

    key
}
```

**Example**:
```
Steam ID:    76561198012345678
As u64:      76561198012345678
LE bytes:    [0x4E, 0xF3, 0x92, 0x7E, 0xEB, 0x42, 0x10, 0x01]

Base key[0..8]:  [0x8E, 0x62, 0xA9, 0x4C, 0x50, 0x60, 0x1A, 0x9D]
XOR result:      [0xC0, 0x91, 0x3B, 0x32, 0xBB, 0x22, 0x0A, 0x9C]

Final key: [0xC0, 0x91, 0x3B, ... remaining 24 bytes unchanged]
```

---

## The Compression Layer

After decryption, data is zlib-compressed.

### Zlib Header

```
78 9C ...  ← Standard zlib header
│  │
│  └── Compression level (9C = default)
└──── Compression method (78 = deflate)
```

### Decompression

```rust
use flate2::read::ZlibDecoder;

fn decompress(data: &[u8]) -> Vec<u8> {
    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).unwrap();
    output
}
```

After decompression, you have readable YAML.

---

## The YAML Structure

BL4 saves are YAML documents with a specific schema.

### Top-Level Structure

```yaml
version: 1
profile:
  steam_id: "76561198012345678"
  created: "2025-01-15T10:30:00Z"

characters:
  - id: "char_001"
    class: "DarkSiren"
    level: 50
    ...

state:
  inventory:
    items:
      - serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
        flags: 0
      - serial: "@Uge8jxm/)@{!gQaYMipv(G&-b*Z~_"
        flags: 1
    ...
```

### Key Sections

| Section | Contents |
|---------|----------|
| `version` | Save format version |
| `profile` | Account-level data |
| `characters` | Character list |
| `state.inventory` | All items (as serials) |
| `state.equipped` | Currently equipped items |
| `state.skills` | Skill point allocations |
| `state.missions` | Mission progress |
| `state.discoveries` | Map exploration |

### Item Entry

Each item in inventory:

```yaml
items:
  - serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"
    flags: 0
    seen: true
```

| Field | Type | Description |
|-------|------|-------------|
| `serial` | string | Base85-encoded item data |
| `flags` | u32 | Item state flags (equipped, favorite, etc.) |
| `seen` | bool | Whether item notification was dismissed |

---

## Using bl4 to Work with Saves

### Decrypt and View

```bash
# Decrypt save to YAML
bl4 decrypt profile.sav --steam-id 76561198012345678

# Output: profile.yaml (or stdout)
```

### Edit and Re-encrypt

```bash
# Edit the YAML file
nano profile.yaml

# Re-encrypt
bl4 encrypt profile.yaml --steam-id 76561198012345678 -o profile.sav
```

### Query Specific Data

```bash
# List all items
bl4 query profile.sav "state.inventory.items[*].serial" --steam-id 76561198012345678

# Get character level
bl4 query profile.sav "characters[0].level" --steam-id 76561198012345678
```

---

## Backup System

The bl4 tool includes smart backup management.

### How It Works

Before modifying a save, bl4:
1. Computes SHA-256 hash of the file
2. Checks if a backup with that hash exists
3. Creates backup only if it's a new version

```bash
# Create backup before editing
bl4 backup profile.sav

# List backups
bl4 backup --list profile.sav

# Restore specific backup
bl4 restore profile.sav --timestamp 2025-01-15T10:30:00
```

### Backup Location

```
~/.config/bl4/backups/
├── profile.sav/
│   ├── 2025-01-15T10:30:00_abc123.sav
│   └── 2025-01-16T14:22:00_def456.sav
└── character_001.sav/
    └── ...
```

---

## Common Modifications

### Adding Items

1. Decrypt the save
2. Add an item entry to `state.inventory.items`
3. Re-encrypt

```yaml
items:
  - serial: "@Ugr$ZCm/&tH!t{KgK/Shxu>k"  # Existing
    flags: 0
  - serial: "@UgYOUR_NEW_ITEM_SERIAL"     # New item
    flags: 0
```

!!! warning
    Invalid serials will crash the game or corrupt inventory. Test with expendable saves first.

### Modifying Currency

```yaml
state:
  currency:
    cash: 999999999
    eridium: 9999
```

### Changing Level

```yaml
characters:
  - id: "char_001"
    level: 72        # Max level
    experience: 999999999
```

---

## Validation

BL4 performs some validation on load:

| Check | Result of Failure |
|-------|-------------------|
| Invalid YAML syntax | Crash / won't load |
| Unknown fields | Usually ignored |
| Invalid item serial | Item may not appear |
| Out-of-range values | Clamped or ignored |
| Wrong Steam ID key | Decryption fails completely |

### Testing Modifications

1. **Backup first** — Always
2. **Make one change** — Isolate what broke if it fails
3. **Start the game** — Check if character loads
4. **Verify in-game** — Confirm changes took effect

---

## Practical: Decrypting a Save

Let's walk through manual decryption:

### Step 1: Read the Header

```bash
xxd -l 32 profile.sav
```

Output:
```
00000000: 4145 532d 3235 362d 4543 4200 0000 0000  AES-256-ECB.....
00000010: 0000 0000 0000 0000 a8de 0700 0000 0000  ................
```

Compressed size: `0x0007dea8` = 515,752 bytes

### Step 2: Derive the Key

```python
import struct

STEAM_ID = "76561198012345678"
BASE_KEY = bytes([
    0x8E, 0x62, 0xA9, 0x4C, 0x50, 0x60, 0x1A, 0x9D,
    0x1C, 0x72, 0xD2, 0xAB, 0x95, 0xFC, 0x10, 0xD0,
    0xD7, 0xA9, 0x26, 0x95, 0x70, 0x56, 0x72, 0x7D,
    0xB4, 0x24, 0x2C, 0x77, 0xAD, 0xF2, 0xB1, 0x51,
])

digits = ''.join(c for c in STEAM_ID if c.isdigit())
steam_bytes = struct.pack('<Q', int(digits))

key = bytearray(BASE_KEY)
for i in range(8):
    key[i] ^= steam_bytes[i]

print("Key:", key.hex())
```

### Step 3: Decrypt

```python
from Crypto.Cipher import AES

with open('profile.sav', 'rb') as f:
    data = f.read()

# Skip 32-byte header
encrypted = data[32:]

# Pad to 16-byte boundary
padded = encrypted + b'\x00' * (16 - len(encrypted) % 16)

cipher = AES.new(bytes(key), AES.MODE_ECB)
decrypted = cipher.decrypt(padded)[:len(encrypted)]

print("First bytes:", decrypted[:16].hex())
# Should start with 78 9c (zlib header)
```

### Step 4: Decompress

```python
import zlib

decompressed = zlib.decompress(decrypted)
print(decompressed[:500].decode('utf-8'))
# Should be YAML
```

---

## Exercises

### Exercise 1: Find Your Steam ID

1. Open your profile.sav
2. Decrypt it using the bl4 tool
3. Find your Steam ID in the YAML
4. Verify it matches your Steam profile

### Exercise 2: Count Your Items

1. Decrypt a character save
2. Navigate to `state.inventory.items`
3. Count the items
4. Compare with in-game backpack count

### Exercise 3: Backup and Modify

1. Create a backup of your save
2. Add 1 million cash
3. Load the game and verify
4. Restore the backup

---

## Security Notes

### Why ECB is Weak

ECB encrypts each 16-byte block independently:

```
Plaintext:   [Block A] [Block B] [Block A]
Ciphertext:  [Enc(A)]  [Enc(B)]  [Enc(A)]   ← Identical!
```

This allows:
- **Pattern detection** — Repeated data creates repeated ciphertext
- **Block manipulation** — Swap encrypted blocks without knowing the key

### Why This Doesn't Matter (Much)

For save files:
- You need the Steam ID anyway (it's in the filename path)
- Local save editing is expected behavior
- No multiplayer security depends on save integrity

!!! note
    Gearbox knows about this. It's a reasonable tradeoff for a single-player game with cross-play considerations.

---

## Key Takeaways

1. **Saves are YAML** — Human-readable once decrypted
2. **Encryption key = Steam ID + base key** — Derivation is simple
3. **Backup before editing** — Always have a way back
4. **Items are serials** — Chapter 5 covers decoding them

---

## Next Chapter

Now let's decode those item serial strings and understand how weapons and gear are represented.

**Next: [Chapter 5: Item Serials](05-item-serials.md)**
