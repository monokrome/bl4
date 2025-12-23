# Borderlands 4 Reverse Engineering Guide

**Zero to Hero**

A comprehensive guide to understanding game internals, reverse engineering techniques, and using the bl4 tooling to analyze and modify Borderlands 4.

---

<div class="grid cards" markdown>

-   :material-download:{ .lg .middle } **Download**

    ---

    Get the complete guide for offline reading.

    [:material-file-pdf-box: PDF](downloads/bl4-guide.pdf){ .md-button .md-button--primary }
    [:material-book-open-variant: EPUB](downloads/bl4-guide.epub){ .md-button }
    [:material-kindle: MOBI](downloads/bl4-guide.mobi){ .md-button }

</div>

---

## Chapters

### Part I: Foundations

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 1 | [Binary Basics](01-binary-basics.md) | Hexadecimal, endianness, data types, and memory layout |
| 2 | [Unreal Engine Architecture](02-unreal-architecture.md) | UObjects, reflection system, pak files, and usmap |

### Part II: Analysis Techniques

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 3 | [Memory Analysis](03-memory-analysis.md) | Process memory, dumps, pattern scanning, pointer chains |
| 4 | [Save File Format](04-save-files.md) | Encryption, compression, YAML structure, key derivation |
| 5 | [Item Serials](05-item-serials.md) | Base85 encoding, bit manipulation, token parsing |

### Part III: Practical Application

| Chapter | Title | Description |
|:-------:|-------|-------------|
| 6 | [Data Extraction](06-data-extraction.md) | Pak files, asset parsing, manifest generation |
| 7 | [Using bl4 Tools](07-bl4-tools.md) | Complete CLI reference and practical workflows |

### Appendices

| Appendix | Title | Description |
|:--------:|-------|-------------|
| A | [SDK Class Layouts](appendix-a-sdk-layouts.md) | Memory layouts for UObject, AOakCharacter, AWeapon, etc. |
| B | [Weapon Parts Reference](appendix-b-weapon-parts.md) | Complete catalog of weapon parts by manufacturer |
| C | [Loot System Internals](appendix-c-loot-system.md) | Drop pools, rarity weights, luck system |
| D | [Game File Structure](appendix-d-game-files.md) | Full asset tree and file organization |

### Reference

| | Title | Description |
|:--:|-------|-------------|
| | [Glossary](glossary.md) | Terms, definitions, and quick reference tables |

---

## Quick Start

**New to reverse engineering?**
Start with [Chapter 1: Binary Basics](01-binary-basics.md) and work through sequentially.

**Want to edit saves?**
Jump to [Chapter 4: Save File Format](04-save-files.md).

**Need to decode an item?**
See [Chapter 5: Item Serials](05-item-serials.md).

**Just want the tool reference?**
Go to [Chapter 7: Using bl4 Tools](07-bl4-tools.md).

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

This guide accompanies the **bl4** project—a Borderlands 4 save file editor and item serial decoder. It documents not just *how* to use the tools, but *why* they work, giving you the knowledge to explore further on your own.

Each chapter includes:

- **Concept explanations** with visual diagrams
- **Practical examples** you can try immediately
- **Exercises** to test your understanding
- **Tips** from real reverse engineering sessions

---

*For the latest version of this guide, visit [book.bl4.dev](https://book.bl4.dev)*
