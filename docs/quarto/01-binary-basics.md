# Chapter 1: Binary Basics

Open any game file in a hex editor and you'll see a wall of numbers and letters that looks like gibberish. But it's not gibberish—it's data, organized according to rules we can learn. This chapter teaches you to read that wall of bytes like a map.

---

## The Language of Computers

Everything in a computer reduces to numbers. Your character's health? A number. The name of that legendary weapon? A sequence of numbers representing characters. The damage calculation when you shoot an enemy? Numbers in, numbers out.

Humans count in base 10 because we have ten fingers. Computers count in base 2 because transistors have two states: on and off. This creates an immediate translation problem. The number 137 is easy for us to read, but computers see it as `10001001`—eight binary digits representing 128 + 8 + 1.

Writing out binary gets tedious fast. An 8-byte pointer becomes 64 ones and zeros. So we use hexadecimal—base 16—as a compact representation. Each hex digit represents exactly four bits, so two hex digits represent exactly one byte. The number 137 becomes `0x89`. Much better.

Here's the relationship:

```text
Decimal:     137
Binary:      1000 1001
Hexadecimal: 8    9     →  0x89
```

You don't need to do these conversions in your head. Use a calculator. What matters is recognizing that when you see `0x89` in a hex dump, you're looking at one byte with a value of 137.

---

## How Data Lives in Memory

When a game stores your character's level, it doesn't just write "50" somewhere. It chooses a *data type*—a container of a specific size with rules about what values it can hold.

The most common types you'll encounter:

**Integers** store whole numbers. An `i32` (signed 32-bit integer) takes 4 bytes and can hold values from about -2 billion to +2 billion. A `u8` (unsigned 8-bit integer) takes 1 byte and holds 0-255. The 'u' means unsigned (no negative values), and the number indicates bits.

**Floating point** numbers store decimals. Damage multipliers, coordinates, health values—anything that needs fractional precision uses `f32` (4 bytes) or `f64` (8 bytes).

**Pointers** are addresses pointing to other memory locations. On 64-bit systems like modern PCs, pointers are 8 bytes (`u64`).

| Type | Size | Range |
|------|------|-------|
| `u8` | 1 byte | 0 to 255 |
| `i16` | 2 bytes | -32,768 to 32,767 |
| `u32` | 4 bytes | 0 to ~4.3 billion |
| `i64` | 8 bytes | ±9.2 quintillion |
| `f32` | 4 bytes | ~7 digits precision |

When reverse engineering, your job is figuring out which type holds which value. Is that item level a `u8` or `u16`? Only one way to find out: look at the data and test your hypothesis.

---

## The Backwards Number Problem

Here's something that trips up every beginner. Let's say you're looking for the value 0x12345678 in a file. You search for those bytes and find... nothing. Then you search for `78 56 34 12` and there it is.

Welcome to little-endian byte order.

Intel CPUs—which means basically every PC—store multi-byte numbers with the *least significant byte first*. The "small end" comes first, hence "little-endian." It feels backwards because we read numbers from left to right, most significant digit first.

The value 0x12345678 stored on a little-endian system:

```text
Memory address:  0x00  0x01  0x02  0x03
Stored bytes:    0x78  0x56  0x34  0x12
```

Big-endian systems store bytes in the order we'd expect: 0x12 first, then 0x34, then 0x56, then 0x78. Network protocols often use big-endian (it's sometimes called "network byte order").

BL4 uses both. Save files are little-endian because they're made for x86 processors. But item serials—those shareable weapon codes—use big-endian for the Base85 decoding step. Always verify which you're dealing with before assuming.

---

## Reading a Hex Dump

Let's look at real data. Here's the first 64 bytes of a BL4 save file:

```hexdump
00000000: 4145 532d 3235 362d 4543 4200 0000 0000  AES-256-ECB.....
00000010: 0000 0000 0000 0000 a8de 0700 0000 0000  ................
00000020: 789c 0bc9 c82c 5600 5346 4b23 0b32 4b32  x....,.V.SFK#.2K2
00000030: 93cb 4a8b cb52 f34b 148c f513 73f3 5212  ..J..R.K....s.R.
```

Three columns: offset on the left, hex bytes in the middle, ASCII representation on the right.

The offset tells you where in the file you are. `00000020` means byte 32 (0x20 in hex).

The hex bytes are the actual data, two hex digits per byte. `4145` represents two bytes: 0x41 and 0x45.

The ASCII column shows printable characters for bytes that fall in the displayable range. Non-printable bytes show as dots. This is where strings jump out at you—look at the first line: "AES-256-ECB" is clearly visible.

Now let's decode what we're seeing:

**Bytes 0x00-0x0B**: The string "AES-256-ECB" followed by a null byte. This is a magic marker identifying the encryption scheme.

**Bytes 0x0C-0x17**: Zeros. Padding or reserved space.

**Bytes 0x18-0x1B**: `a8 de 07 00`. Four bytes, little-endian. That's 0x0007DEA8 = 515,752 in decimal. This is the size of the encrypted payload.

**Byte 0x20**: `78 9c`. These two bytes are a zlib magic number—the compressed data starts here.

Pattern recognition is the core skill. After seeing a few zlib-compressed files, you'll instantly recognize that `78 9c` signature. After enough save files, the AES-256-ECB header becomes as readable as English.

---

## Packing Bits Together

Sometimes a single byte holds multiple values. Games do this to save space or because the values are logically related.

Consider item flags. An item might be equipped (yes/no), favorited (yes/no), marked as junk (yes/no), and new (yes/no). Four boolean values could use four bytes, but why waste three? Pack them into one:

```text
Bit 7  6  5  4  3  2  1  0
    _  _  _  _  N  J  F  E

E = Equipped (bit 0)
F = Favorited (bit 1)
J = Junk (bit 2)
N = New (bit 3)
```

If you see the byte 0x0B (`00001011` in binary), that means:
- Equipped: yes (bit 0 = 1)
- Favorited: yes (bit 1 = 1)
- Junk: no (bit 2 = 0)
- New: yes (bit 3 = 1)

Extracting individual bits requires bitwise operations:

```rust
let flags: u8 = 0x0B;

// Check if equipped (bit 0)
let equipped = (flags & 0x01) != 0;  // true

// Check if junk (bit 2)
let junk = (flags & 0x04) != 0;  // false

// Get bits 0-1 together
let low_two_bits = flags & 0x03;  // 0x03 = 3
```

The `&` operator (bitwise AND) masks off the bits you don't care about. Shifting with `>>` moves bits to the right so you can isolate them. You'll use these operations constantly when parsing packed data formats.

---

## Text as Numbers

Strings are just sequences of numbers representing characters. The mapping depends on the encoding.

**ASCII** maps characters to single bytes. 'A' is 65 (0x41), 'a' is 97 (0x61), space is 32 (0x20). Only covers 128 characters.

**UTF-8** is ASCII-compatible but extends to all Unicode characters. Multi-byte sequences for non-ASCII characters.

**UTF-16** uses 2 bytes per character (or 4 for rare characters). Common in Windows APIs and some game engines.

In hex dumps, ASCII strings are easy to spot because the bytes fall in the printable range (0x20-0x7E). You'll see the actual text in the right column.

Two common string storage formats:

**Null-terminated**: The string ends when you hit 0x00.
```text
48 65 6C 6C 6F 00  →  "Hello"
```

**Length-prefixed**: A length value precedes the characters.
```text
05 00 00 00 48 65 6C 6C 6F  →  5 characters, then "Hello"
```

Unreal Engine uses length-prefixed strings with additional metadata. We'll cover the exact format in Chapter 2.

---

## Why Alignment Creates Gaps

If you're looking at a hex dump of a struct and see mysterious zero bytes between fields, you've found alignment padding.

CPUs access memory most efficiently when data aligns to certain boundaries. A 4-byte integer reads fastest from an address divisible by 4. A 64-bit pointer wants an address divisible by 8.

Compilers automatically insert padding to maintain alignment:

```c
struct WeaponStats {
    u8  rarity;        // Offset 0x00, 1 byte
    // 3 bytes padding  // Offset 0x01-0x03
    u32 damage;        // Offset 0x04, 4 bytes (aligned to 4)
    u8  element;       // Offset 0x08, 1 byte
    // 7 bytes padding  // Offset 0x09-0x0F
    u64 serial_ptr;    // Offset 0x10, 8 bytes (aligned to 8)
};  // Total: 24 bytes
```

The struct logically contains 14 bytes of data (1+4+1+8), but its actual size is 24 bytes because of padding. Knowing this prevents confusion when offsets don't match what you'd calculate by adding field sizes.

---

## Putting It Together

Let's decode a small example. Say you find these bytes at the start of an item record:

```text
32 00 00 00 01 00 00 00 E8 03 00 00
```

Breaking it down:

- `32 00 00 00`: Little-endian u32 = 0x00000032 = 50. Probably item level.
- `01 00 00 00`: Little-endian u32 = 1. Maybe a type ID or flag.
- `E8 03 00 00`: Little-endian u32 = 0x000003E8 = 1000. Could be damage, price, or some other stat.

Is this interpretation correct? Only way to know is test it. Change the first value to `64 00 00 00` (100 in decimal), reload the save, and see if the item is now level 100. If yes, hypothesis confirmed. If no, back to the drawing board.

This is the cycle of reverse engineering: observe, hypothesize, test, repeat.

---

## Exercises

**Exercise 1: Reading Hex**

Convert these values:
1. `0xFF` to decimal
2. `1000` (decimal) to hex
3. What's the decimal value of `a8 de 07 00` as a little-endian u32?

**Exercise 2: Endianness**

You're looking for the value 305,419,896 (0x12345678) in a file. What byte sequence do you search for on a little-endian system?

**Exercise 3: Bit Extraction**

Given the byte `0xD6` (binary: `11010110`):
1. What's bit 0?
2. What's bit 7?
3. What are bits 4-6 as a 3-bit value?

<details>
<summary>Answers</summary>

**Exercise 1:**
1. 255
2. 0x3E8
3. 0x0007DEA8 = 515,752

**Exercise 2:**
`78 56 34 12` (least significant byte first)

**Exercise 3:**
1. Bit 0 = 0 (rightmost bit)
2. Bit 7 = 1 (leftmost bit)
3. Bits 4-6 = `(0xD6 >> 4) & 0x07` = `0b101` = 5

</details>

---

## What's Next

You now have the foundation to read binary data. But raw bytes only get you so far—you need to understand how the game engine organizes that data into meaningful structures.

Next, we'll explore Unreal Engine 5's architecture: how it tracks objects with reflection, serializes properties, and stores assets in pak files. Understanding UE5's patterns makes the difference between staring at random bytes and recognizing game data instantly.

**Next: [Chapter 2: Unreal Engine Architecture](02-unreal-architecture.md)**
