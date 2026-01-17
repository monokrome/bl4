
## UPDATE 2026-01-15 (Structure Hypothesis)

### The Question: Is the First Entry a Schema Definition?

**User hypothesis**: Does the bit-packed first entry define the structure for all other entries?

**Answer**: No - each entry has different fields. For example:
- "ammo" (entry 0): 16 fields including `rarity`, `pingfeedback`, `cantpickupprompt`
- "ammo_assaultrifle" (entry 1): 9 fields including `basetype`, `body`, `aspects` 
- "classmod" (entry 6): 33 fields including `class`, `slot`, `parttypes`

Each entry type has its own field schema.

### Metadata Section Discovery

The 28 bytes at offset 0x100-0x11b (between format code and string table) contain:
```
43 41 61 42 c1 52 51 4a  01 49 45 5a b4 08 00 00
00 00 00 00 d7 d1 00 00  00 00 00 00
```

When bl4 parses this file, it shows these as the first "Entry Names":
- CAaB
- RQJ  
- IEZ

These bytes match the metadata! So the metadata section contains **inline entry names** 
that aren't stored in the string table. This is likely an optimization for short/special identifiers.

### Remaining Questions

1. Why does only the first entry get bit-packed encoding?
2. What do the format code letters (abcehijl) actually mean?
3. How is the structured section encoded?
4. How do the 107 bit-packed indices map to the "ammo" entry's 16 JSON fields?

