# Borderlands 4 Reverse Engineering Guide

A zero-to-hero guide for understanding game internals, reverse engineering techniques, and using the bl4 tooling to analyze and modify Borderlands 4.

---

## Who This Guide Is For

This guide assumes you:
- Can write basic code (any language—we'll use Rust, but concepts transfer)
- Have used a command line before
- Are curious about how games work under the hood

No prior reverse engineering experience is required. We'll build up from fundamentals.

---

## What You'll Learn

By the end of this guide, you'll understand:

1. **Binary Fundamentals** — How data is represented in memory and files
2. **Unreal Engine Architecture** — How UE5 organizes game data
3. **Memory Analysis** — Reading and interpreting process memory
4. **Save File Structure** — BL4's encryption, compression, and YAML format
5. **Item Serial Format** — How items are encoded as shareable strings
6. **Data Extraction** — Pulling game assets from pak files
7. **Using bl4 Tools** — Practical usage of the tooling we've built

---

## Guide Structure

Each chapter builds on the previous. Chapters include:

!!! note
    **Concept boxes** explain key ideas you'll need later.

!!! tip
    **Tips** share practical advice from real reverse engineering sessions.

!!! warning
    **Warnings** highlight common pitfalls and mistakes.

```
Code blocks show actual commands, data structures, and examples.
```

**Data type references** appear in tables like this:

| Type | Size | Range | Notes |
|------|------|-------|-------|
| `u8` | 1 byte | 0–255 | Unsigned byte |
| `i32` | 4 bytes | ±2.1 billion | Signed 32-bit integer |

---

## Chapters

1. **[Binary Basics](01-binary-basics.md)** — Hex, endianness, data types, and memory layout
2. **[Unreal Engine Architecture](02-unreal-architecture.md)** — UObjects, reflection, pak files, and usmap
3. **[Memory Analysis](03-memory-analysis.md)** — Process memory, dumps, pattern scanning
4. **[Save File Format](04-save-files.md)** — Encryption, compression, YAML structure
5. **[Item Serials](05-item-serials.md)** — Base85, bit manipulation, token parsing
6. **[Data Extraction](06-data-extraction.md)** — Pak files, asset parsing, manifest generation
7. **[Using bl4 Tools](07-bl4-tools.md)** — CLI reference and practical workflows

---

## Prerequisites

### Required Software

```bash
# Rust toolchain (for building bl4)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone the repository
git clone https://github.com/monokrome/bl4
cd bl4

# Build all tools
cargo build --release
```

### Recommended Tools

| Tool | Purpose | Install |
|------|---------|---------|
| `xxd` | Hex dumps | Usually pre-installed on Linux |
| `radare2` | Binary analysis | `apt install radare2` |
| `Ghidra` | Decompilation | [ghidra-sre.org](https://ghidra-sre.org) |
| `Process Hacker` | Windows memory inspection | [processhacker.sourceforge.io](https://processhacker.sourceforge.io) |

---

## Philosophy

### Learning by Doing

This guide emphasizes *doing* over *reading*. Each chapter includes exercises you can try immediately. The best way to learn reverse engineering is to:

1. **Form a hypothesis** — "I think this byte controls damage"
2. **Test it** — Modify the value, observe the result
3. **Iterate** — Refine your understanding based on what you see

### Document Everything

Keep notes as you explore. The `docs/` folder in this repository started as scratch notes and evolved into comprehensive documentation. Your discoveries might help others.

### Ethical Considerations

This guide is for:
- ✅ Understanding how games work
- ✅ Creating save editors for personal use
- ✅ Educational purposes and CTF-style challenges
- ✅ Modding within game terms of service

This guide is **not** for:
- ❌ Cheating in multiplayer
- ❌ Piracy or bypassing DRM
- ❌ Violating game terms of service

---

## Quick Start

Already comfortable with the basics? Jump to what interests you:

**"I want to decode an item serial"**
→ [Chapter 5: Item Serials](05-item-serials.md)

**"I want to edit my save file"**
→ [Chapter 4: Save File Format](04-save-files.md)

**"I want to extract game data"**
→ [Chapter 6: Data Extraction](06-data-extraction.md)

**"I want to understand memory analysis"**
→ [Chapter 3: Memory Analysis](03-memory-analysis.md)

---

## Conventions Used

### Command Examples

```bash
# Commands you should run
$ bl4 decode '@Ugr$ZCm/&tH!t{KgK/Shxu>k'

# Output is shown without the $ prefix
Serial: @Ugr$ZCm/&tH!t{KgK/Shxu>k
Item type: r (Weapon)
Tokens: 180928 | 51 | {0:1} 21 {4} ...
```

### File Paths

- `/path/to/file` — Absolute path (replace with your actual path)
- `./relative/path` — Relative to the bl4 repository root
- `~/.steam/...` — Relative to your home directory

### Hex Notation

- `0x1A` — Hexadecimal value (26 in decimal)
- `\x1A` — Hex byte in strings
- `01 1A FF` — Hex dump format (space-separated bytes)

---

## Let's Begin

Ready to dive in? Start with [Chapter 1: Binary Basics](01-binary-basics.md).

---

*This guide accompanies the bl4 project: https://github.com/monokrome/bl4*
