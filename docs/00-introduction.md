# Borderlands 4 Reverse Engineering Guide

> **Note:** The information in this guide is based on ongoing reverse engineering analysis. Some details may be incomplete or subject to revision as our understanding improves. If you find errors or have corrections, please reach out at hey@monokro.me.

When you pick up a legendary weapon in Borderlands 4 and share it with a friend using a serial code, something remarkable happens. That short string of characters—maybe 40 characters long—encodes everything about your weapon: its manufacturer, every part attached to it, the random seed that determined its stats, even which rarity tier it rolled. Your friend pastes the code, and they get an exact duplicate.

But how does it work?

This guide exists because someone asked that question. What started as curiosity about save file formats turned into a deep dive through binary data, encryption schemes, Unreal Engine internals, and ultimately a complete understanding of how Borderlands 4 stores and encodes its item system.

---

## The Journey Ahead

You're about to learn reverse engineering by doing it. We won't just explain concepts—we'll use them immediately to solve real problems. By the time you finish this guide, you'll be able to:

**Decrypt and edit save files.** BL4 saves are encrypted with AES-256, compressed, and structured as YAML. We'll walk through the entire process of opening them up, making changes, and putting them back together.

**Decode item serials.** Those cryptic strings that encode weapons use a custom Base85 alphabet, bit mirroring, and a token-based format. We'll parse them byte by byte until they make perfect sense.

**Extract data from game files.** Unreal Engine 5 packs everything into `.pak` containers with a custom format. We'll build tools to crack them open and pull out the good stuff—weapon definitions, part databases, drop tables.

**Understand memory analysis.** Sometimes the only way to find what you need is to take a snapshot of the running game. We'll learn to navigate gigabytes of process memory to locate specific structures.

None of this requires prior reverse engineering experience. If you can write basic code and use a command line, you have everything you need to start.

---

## How This Guide Works

Each chapter builds on the previous, taking you from fundamentals to practical application. The structure follows the natural progression of a reverse engineering project:

**First**, we establish foundations. Binary representation, data types, memory layout—these concepts appear everywhere in game data. Understanding them makes everything else click into place.

**Then**, we learn Unreal Engine's architecture. BL4 runs on UE5, and knowing how Unreal organizes data (UObjects, reflection, property serialization) explains patterns we'll see repeatedly in memory dumps and pak files.

**Next**, we apply these concepts. We'll analyze save files, decode serials, dump process memory, and extract game assets. Each technique opens new doors.

**Finally**, we use the tools. The bl4 project provides command-line utilities for everything we've learned. The tools chapter serves as your practical reference for day-to-day use.

---

## A Word on Philosophy

The best reverse engineers share a common trait: they form hypotheses and test them ruthlessly. "I think this byte controls weapon damage" leads to "let me change it and see what happens." Wrong guesses are valuable—they narrow down possibilities.

Keep notes as you explore. The documentation you're reading started as scratch files filled with hex dumps and question marks. Over time, patterns emerged, and those scratch notes became explanations. Your discoveries might become documentation too.

---

## Ethics and Intent

This guide exists for education and personal use. It's for:

- Understanding how games work beneath the surface
- Building save editors that modify your own single-player experience
- Learning reverse engineering techniques applicable far beyond gaming
- Satisfying the curiosity that comes from wanting to know *how things work*

It's not for cheating in multiplayer, bypassing DRM, or violating terms of service. The techniques here are powerful—use them responsibly.

---

## What You'll Need

Before we begin, make sure you have the Rust toolchain installed. We'll be building and using the bl4 tools throughout:

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/monokrome/bl4
cd bl4
cargo build --release
```

A hex editor (any will do) and familiarity with your system's terminal will help. If you're on Linux, tools like `xxd` for hex dumps come pre-installed. On Windows, HxD is a solid free option.

For deeper analysis work, Ghidra (free) or IDA (commercial) let you decompile binaries. Rizin or Radare2 provide scriptable binary analysis. We'll use some of these in later chapters, but they're not strictly required to follow along.

---

## Finding Your Path

This guide is meant to be read sequentially, but you might have a specific goal in mind:

**Want to decode a weapon serial?** The encoding process is fascinating, but it also requires understanding binary basics and the token format. Start at Chapter 1 and work through Chapter 5. You'll have full context.

**Need to edit a save file?** Chapter 4 covers the format in detail. If you're comfortable with encryption concepts and binary data, jump there. Otherwise, Chapters 1-2 provide helpful background.

**Interested in extracting game assets?** Chapter 6 walks through pak file parsing. Chapter 2's coverage of Unreal Engine architecture explains why assets are structured the way they are.

**Curious about the whole picture?** Start from the beginning. Each chapter adds a piece to the puzzle.

---

## Let's Begin

The first step in any reverse engineering project is understanding how computers represent data. It sounds basic, but it's foundational. Knowing that 0x41 means 'A' in ASCII, that little-endian systems store bytes backwards, and that a 4-byte integer can represent about 4 billion values—these facts become second nature, and they unlock everything else.

Turn the page to Chapter 1. We'll start with binary basics, and before you know it, you'll be reading hex dumps like they're plain English.

---

*This guide accompanies the bl4 project: https://github.com/monokrome/bl4*
