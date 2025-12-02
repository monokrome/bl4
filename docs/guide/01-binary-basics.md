# Chapter 1: Binary Basics

Before we can reverse engineer anything, we need to speak the language of computers: binary and hexadecimal. This chapter builds the foundation for everything that follows.

---

## Why Binary Matters

Games store everything as bytes—items, player stats, save data, even the code itself. To understand game internals, you need to:

1. Read hex dumps and recognize patterns
2. Understand how numbers and strings are encoded
3. Know how data is laid out in memory

!!! tip
    You don't need to memorize hex-to-decimal conversions. Use a calculator. What matters is recognizing *patterns* and *structures*.

---

## Number Systems

### Decimal (Base 10)

What you use every day: `0, 1, 2, 3, 4, 5, 6, 7, 8, 9`

Each position is a power of 10:
```
  1   3   7
  ↓   ↓   ↓
100  10   1  →  1×100 + 3×10 + 7×1 = 137
```

### Binary (Base 2)

Computers use: `0, 1`

Each position is a power of 2:
```
  1   0   0   0   1   0   0   1
  ↓   ↓   ↓   ↓   ↓   ↓   ↓   ↓
128  64  32  16   8   4   2   1  →  128 + 8 + 1 = 137
```

### Hexadecimal (Base 16)

More compact than binary: `0-9, A-F` (A=10, B=11, C=12, D=13, E=14, F=15)

Each position is a power of 16:
```
  8   9
  ↓   ↓
 16   1  →  8×16 + 9×1 = 137
```

So: `137` (decimal) = `10001001` (binary) = `0x89` (hex)

!!! note
    **Why hex?** One hex digit = exactly 4 bits. Two hex digits = exactly 1 byte. This makes hex perfect for representing binary data compactly.

---

## Common Conversions

| Decimal | Hex | Binary | Notes |
|---------|-----|--------|-------|
| 0 | 0x00 | 00000000 | Zero/null |
| 1 | 0x01 | 00000001 | |
| 10 | 0x0A | 00001010 | First two-digit decimal |
| 15 | 0x0F | 00001111 | Max single hex digit |
| 16 | 0x10 | 00010000 | First two-digit hex |
| 127 | 0x7F | 01111111 | Max signed byte |
| 128 | 0x80 | 10000000 | Min negative signed byte |
| 255 | 0xFF | 11111111 | Max unsigned byte |
| 256 | 0x100 | 100000000 | First value needing 2 bytes |

---

## Data Types

Computers group bits into standard sizes. Memorize these—you'll see them constantly.

### Integer Types

| Type | Size | Signed Range | Unsigned Range |
|------|------|--------------|----------------|
| `i8` / `u8` | 1 byte | -128 to 127 | 0 to 255 |
| `i16` / `u16` | 2 bytes | -32,768 to 32,767 | 0 to 65,535 |
| `i32` / `u32` | 4 bytes | ±2.1 billion | 0 to 4.3 billion |
| `i64` / `u64` | 8 bytes | ±9.2 quintillion | 0 to 18.4 quintillion |

!!! note
    **Signed vs Unsigned**: Signed integers use one bit for the sign (positive/negative). Unsigned integers use all bits for magnitude, doubling the positive range.

### Floating Point Types

| Type | Size | Precision | Example |
|------|------|-----------|---------|
| `f32` | 4 bytes | ~7 digits | Damage multipliers, coordinates |
| `f64` | 8 bytes | ~15 digits | High-precision calculations |

### Common Game Values

| Value | Typical Type | Why |
|-------|--------------|-----|
| Item level | `u8` or `u16` | Levels rarely exceed 100 |
| Health | `f32` | Allows decimals, negative for damage |
| Item count | `u32` | Stack sizes, currency |
| Pointers | `u64` | Memory addresses on 64-bit systems |
| Flags | `u8` or `u32` | Boolean flags packed into bits |

---

## Endianness

Multi-byte values can be stored in two orders:

### Little-Endian (LE)

**Least significant byte first.** Used by x86/x64 (PCs), most game files.

The number `0x12345678` stored in memory:
```
Address:  0x00  0x01  0x02  0x03
Value:    0x78  0x56  0x34  0x12
          └─────────────────────┘
              "Backwards"
```

### Big-Endian (BE)

**Most significant byte first.** Used by network protocols, some file formats.

The number `0x12345678` stored in memory:
```
Address:  0x00  0x01  0x02  0x03
Value:    0x12  0x34  0x56  0x78
          └─────────────────────┘
             "Natural order"
```

!!! warning
    **BL4 uses both!** Save files are little-endian (x86), but item serial Base85 encoding is big-endian. Always check which you're dealing with.

### Quick Detection

If you see a value that seems "backwards," check endianness:
```
Expected: 0x00001000 (4096)
Saw:      0x00 0x10 0x00 0x00  ← Little-endian ✓

Expected: 0x00001000 (4096)
Saw:      0x00 0x00 0x10 0x00  ← Big-endian ✓
```

---

## Reading Hex Dumps

Hex dumps are your window into binary data. Here's how to read them:

```
$ xxd -l 64 save.sav

00000000: 4145 532d 3235 362d 4543 4200 0000 0000  AES-256-ECB.....
00000010: 789c 0bc9 c82c 5600 5346 4b23 0b32 4b32  x....,.V.SFK#.2K2
00000020: 93cb 4a8b cb52 f34b 148c f513 73f3 5212  ..J..R.K....s.R.
00000030: 4b93 7393 7353 4a8a 32f3 d2f3 938a 4a8b  K.s.sSJ.2.....J.
```

### Anatomy of a Hex Dump

```
00000000: 4145 532d 3235 362d 4543 4200 0000 0000  AES-256-ECB.....
│         │                                        │
│         │                                        └── ASCII representation
│         └── Hex bytes (2 digits = 1 byte)
└── Offset (address from start of file)
```

### Reading Tips

1. **Look for ASCII** — The right column shows printable characters. Strings stand out.
2. **Spot patterns** — Repeated bytes (`00 00 00 00`) often mean padding or null values.
3. **Find magic numbers** — File formats start with signatures (`789c` = zlib, `504B` = zip).
4. **Count bytes** — Each pair of hex digits is one byte.

---

## Practical Exercise: Examining a BL4 Save

Let's look at an encrypted BL4 save file:

```bash
$ xxd -l 128 ~/.steam/steam/userdata/<id>/2144700/remote/profile.sav
```

You'll see something like:
```
00000000: 4145 532d 3235 362d 4543 4200 0000 0000  AES-256-ECB.....
00000010: 0000 0000 0000 0000 a8de 0700 0000 0000  ................
00000020: [encrypted data...]
```

**What we can identify:**

| Offset | Bytes | Meaning |
|--------|-------|---------|
| 0x00–0x0B | `41 45 53...` | ASCII "AES-256-ECB" — encryption marker |
| 0x0C–0x17 | `00...` | Padding/reserved |
| 0x18–0x1B | `a8 de 07 00` | Little-endian size: 0x0007dea8 = 515,752 bytes |
| 0x20+ | | Encrypted payload |

!!! tip
    **Pattern recognition** is key. After seeing a few save files, you'll instantly recognize the AES-256-ECB header.

---

## Bit Manipulation

Games often pack multiple values into single bytes using bitwise operations.

### Common Operations

| Operation | Symbol | Example | Result |
|-----------|--------|---------|--------|
| AND | `&` | `0b1010 & 0b1100` | `0b1000` |
| OR | `\|` | `0b1010 \| 0b1100` | `0b1110` |
| XOR | `^` | `0b1010 ^ 0b1100` | `0b0110` |
| NOT | `!` or `~` | `~0b1010` | `0b0101` |
| Left Shift | `<<` | `0b0001 << 2` | `0b0100` |
| Right Shift | `>>` | `0b1000 >> 2` | `0b0010` |

### Extracting Bits

To get specific bits from a byte:

```rust
let byte: u8 = 0b10110100;

// Get bit 5 (counting from 0)
let bit5 = (byte >> 5) & 1;  // = 1

// Get bits 2-4 (3 bits)
let bits_2_to_4 = (byte >> 2) & 0b111;  // = 0b101 = 5

// Check if bit 7 is set
let is_set = (byte & 0b10000000) != 0;  // = true
```

### Bit Flags

Games often use single bytes to store 8 boolean values:

```rust
const FLAG_EQUIPPED: u8   = 0b00000001;  // Bit 0
const FLAG_FAVORITE: u8   = 0b00000010;  // Bit 1
const FLAG_JUNK: u8       = 0b00000100;  // Bit 2
const FLAG_NEW: u8        = 0b00001000;  // Bit 3

let item_flags: u8 = 0b00001011;

// Check if equipped
if item_flags & FLAG_EQUIPPED != 0 {
    println!("Item is equipped");
}

// Check multiple flags
if item_flags & (FLAG_EQUIPPED | FLAG_NEW) != 0 {
    println!("Item is equipped or new");
}
```

---

## Strings in Binary

### Null-Terminated Strings (C-style)

The string ends when you hit a `0x00` byte:

```
48 65 6C 6C 6F 00      →  "Hello"
H  e  l  l  o  NULL
```

### Length-Prefixed Strings

The length comes first, then the characters:

```
05 00 00 00 48 65 6C 6C 6F  →  "Hello"
│           └── 5 characters
└── Length as u32 (little-endian)
```

### UTF-16 Strings (Wide Strings)

Each character uses 2 bytes:

```
48 00 65 00 6C 00 6C 00 6F 00  →  "Hello"
H     e     l     l     o
```

!!! note
    **Unreal Engine** typically uses length-prefixed strings with a character count, followed by null-terminated data. The exact format varies by context.

---

## Memory Alignment

CPUs read memory most efficiently when data is aligned to its size:

| Type | Preferred Alignment |
|------|---------------------|
| `u8` | 1 byte (any address) |
| `u16` | 2 bytes (even addresses) |
| `u32` | 4 bytes (addresses divisible by 4) |
| `u64` | 8 bytes (addresses divisible by 8) |

### Padding

Structures often have "padding" bytes to maintain alignment:

```c
struct Example {
    u8  a;      // Offset 0x00
    // 3 bytes padding (0x01-0x03)
    u32 b;      // Offset 0x04
    u8  c;      // Offset 0x08
    // 7 bytes padding (0x09-0x0F)
    u64 d;      // Offset 0x10
};  // Total size: 0x18 (24 bytes)
```

!!! tip
    When you see unexpected `00` bytes between fields, it's probably padding. This is especially common in memory dumps.

---

## Exercises

### Exercise 1: Hex Conversion

Convert these values (use a calculator if needed):

1. `0xFF` to decimal
2. `1000` (decimal) to hex
3. `0x12345678` to decimal
4. What's `0xDEADBEEF` & `0x0000FFFF`?

<details>
<summary>Answers</summary>

1. 255
2. 0x3E8
3. 305,419,896
4. 0x0000BEEF (48,879)

</details>

### Exercise 2: Endianness

You find these bytes in a file: `78 56 34 12`

1. What's the value if it's little-endian u32?
2. What's the value if it's big-endian u32?

<details>
<summary>Answers</summary>

1. Little-endian: 0x12345678 = 305,419,896
2. Big-endian: 0x78563412 = 2,018,915,346

</details>

### Exercise 3: Bit Extraction

Given the byte `0b11010110`:

1. What's bit 0?
2. What's bit 7?
3. What are bits 2-4 as a 3-bit value?

<details>
<summary>Answers</summary>

1. Bit 0 = 0 (rightmost)
2. Bit 7 = 1 (leftmost)
3. Bits 2-4 = `(0b11010110 >> 2) & 0b111` = `0b101` = 5

</details>

---

## Key Takeaways

1. **Hex is compact binary** — Two hex digits = one byte
2. **Endianness matters** — x86 is little-endian, but formats vary
3. **Data has structure** — Types, alignment, and padding are predictable
4. **Pattern recognition** — Most of RE is spotting familiar patterns

---

## Next Chapter

Now that you understand binary basics, let's see how Unreal Engine organizes game data.

**Next: [Chapter 2: Unreal Engine Architecture](02-unreal-architecture.md)**
