# Downloads

## Guide Books

Download the complete Borderlands 4 Reverse Engineering Guide in your preferred format for offline reading.

<div class="grid cards" markdown>

-   :material-file-pdf-box:{ .lg .middle } **PDF Format**

    ---

    Best for desktop reading and printing. Full-featured with cover page, table of contents, and professional typesetting.

    [:material-download: Download PDF](bl4-guide.pdf){ .md-button .md-button--primary }

-   :material-book-open-variant:{ .lg .middle } **EPUB Format**

    ---

    Best for e-readers (Kobo, Nook) and mobile devices. Reflowable text adapts to your screen size.

    [:material-download: Download EPUB](bl4-guide.epub){ .md-button .md-button--primary }

-   :material-kindle:{ .lg .middle } **MOBI Format**

    ---

    Best for Kindle devices. Native format for Amazon e-readers.

    [:material-download: Download MOBI](bl4-guide.mobi){ .md-button .md-button--primary }

</div>

### What's Included

- **8 chapters** covering binary basics through advanced tool usage
- **4 appendices** with SDK layouts, weapon parts, loot system, and game files
- **Comprehensive glossary** with 70+ terms and quick reference tables
- **Code examples** and exercises throughout

!!! note "Book Generation"
    All book formats are automatically generated from the markdown source on each release.
    It may take a few minutes after a push for the latest version to be available.

## Source Files

All documentation is available as Markdown in the [GitHub repository](https://github.com/monokrome/bl4/tree/main/docs).

### Guide Chapters

| Chapter | Title | Description |
|---------|-------|-------------|
| 00 | [Introduction](../guide/00-introduction.md) | Prerequisites and overview |
| 01 | [Binary Basics](../guide/01-binary-basics.md) | Hex, endianness, data types |
| 02 | [Unreal Architecture](../guide/02-unreal-architecture.md) | UE5 internals |
| 03 | [Memory Analysis](../guide/03-memory-analysis.md) | Process memory techniques |
| 04 | [Save Files](../guide/04-save-files.md) | Encryption and structure |
| 05 | [Item Serials](../guide/05-item-serials.md) | Serial encoding format |
| 06 | [Data Extraction](../guide/06-data-extraction.md) | Pak file extraction |
| 07 | [bl4 Tools](../guide/07-bl4-tools.md) | CLI reference |

### Appendices

| Appendix | Title | Description |
|----------|-------|-------------|
| A | [SDK Class Layouts](../guide/appendix-a-sdk-layouts.md) | Memory layouts for UObject, AOakCharacter, AWeapon |
| B | [Weapon Parts Reference](../guide/appendix-b-weapon-parts.md) | Complete catalog of weapon parts by manufacturer |
| C | [Loot System Internals](../guide/appendix-c-loot-system.md) | Drop pools, rarity weights, luck system |
| D | [Game File Structure](../guide/appendix-d-game-files.md) | Full asset tree and file organization |
| | [Glossary](../guide/glossary.md) | Terms, definitions, and quick reference tables |

## Manifest Data

Pre-extracted game data is available in the repository under `share/manifest/`:

| File | Description |
|------|-------------|
| `pak_manifest.json` | 81,097 indexed game assets |
| `mappings.usmap` | UE5 reflection data (16,849 structs) |
| `items_database.json` | Item pools and stats |
| `manufacturers.json` | All 10 manufacturers |

!!! warning "Large Files"
    Manifest files are stored with Git LFS. Ensure you have LFS installed:
    ```bash
    git lfs install
    git lfs pull
    ```
