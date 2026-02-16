# Borderlands 4 Reverse Engineering Guide

**Zero to Hero**

A comprehensive guide to understanding game internals, reverse engineering techniques, and using the bl4 tooling to analyze and modify Borderlands 4.

---

## Chapters

### Part I: Core Concepts

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 1 | [Binary Basics](#sec-binary-basics) | Hexadecimal, endianness, data types, and memory layout |
| 2 | [Unreal Engine Architecture](#sec-unreal-architecture) | UObjects, reflection system, pak files, and usmap |
| 3 | [Memory Analysis](#sec-memory-analysis) | Process memory, dumps, pattern scanning, pointer chains |

### Part II: Game Formats

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 4 | [Save File Format](#sec-save-files) | Encryption, compression, YAML structure, key derivation |
| 5 | [Item Serials](#sec-item-serials) | Base85 encoding, bit manipulation, token parsing |
| 6 | [NCS Format](#sec-ncs-format) | Nexus Config Store: compression, content format, binary section |

### Part III: Practical Application

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 7 | [Data Extraction](#sec-data-extraction) | Pak files, NCS parsing, memory dumps, manifest generation |
| 8 | [Parts System](#sec-parts-system) | Part categories, compositions, licensed parts, validation |
| 9 | [Using bl4 Tools](#sec-bl4-tools) | Complete CLI reference and practical workflows |

### Appendices

| Appendix | Title | Description |
|:--------:|-------|-------------|
| A | [SDK Class Layouts](#sec-sdk-layouts) | Memory layouts for UObject, AOakCharacter, AWeapon, etc. |
| B | [Weapon Parts Reference](#sec-weapon-parts) | Complete catalog of weapon parts by manufacturer |
| C | [Loot System Internals](#sec-loot-system) | Drop pools, rarity weights, luck system |
| D | [Game File Structure](#sec-game-files) | Full asset tree and file organization |

### Reference

| | Title | Description |
|:--:|-------|-------------|
| | [Glossary](#sec-glossary) | Terms, definitions, and quick reference tables |

---

## Quick Start

**New to reverse engineering?**
Start with [Chapter 1: Binary Basics](#sec-binary-basics) and work through sequentially.

**Want to edit saves?**
Jump to [Chapter 4: Save File Format](#sec-save-files).

**Need to decode an item?**
See [Chapter 5: Item Serials](#sec-item-serials).

**Interested in the NCS format?**
See [Chapter 6: NCS Format](#sec-ncs-format) for the complete format specification.

**Just want the tool reference?**
Go to [Chapter 9: Using bl4 Tools](#sec-bl4-tools).

---

## Prerequisites

Before starting, ensure you have:

- Basic programming knowledge (any language)
- Command line familiarity
- Rust toolchain installed ([rustup.rs](https://rustup.rs))
- The bl4 repository cloned and built

```bash
git clone https://github.com/monokrome/bl4
cd bl4
cargo build --release
```

---

## About This Guide

This guide accompanies the **bl4** project---a Borderlands 4 save file editor and item serial decoder. It documents not just *how* to use the tools, but *why* they work, giving you the knowledge to explore further on your own.

Each chapter includes:

- **Concept explanations** with visual diagrams
- **Practical examples** you can try immediately
- **Exercises** to test your understanding
- **Tips** from real reverse engineering sessions

---

*For the latest version of this guide, visit [book.bl4.dev](https://book.bl4.dev)*
