#!/usr/bin/env python3
"""Import part indices from community spreadsheet into our parts database format.

Parses the Weapon Parts Lookup Table V2.csv and creates a parts_database.json
with the correct SerialIndex values (Category, Index) from the spreadsheet.
"""

import csv
import json
import sys
from pathlib import Path

# Manufacturer + Weapon Type -> Category ID mapping
# Based on our existing category mappings from parts.rs
CATEGORY_MAP = {
    # Pistols (2-7)
    ("Daedalus", "Pistol"): 2,
    ("Jakobs", "Pistol"): 3,
    ("Tediore", "Pistol"): 4,
    ("Torgue", "Pistol"): 5,
    ("Order", "Pistol"): 6,
    ("Vladof", "Pistol"): 7,
    # Shotguns (8-12, 19)
    ("Daedalus", "Shotgun"): 8,
    ("Jakobs", "Shotgun"): 9,
    ("Tediore", "Shotgun"): 10,
    ("Torgue", "Shotgun"): 11,
    ("Bor", "Shotgun"): 12,
    ("Ripper", "Shotgun"): 12,  # Bor/Ripper alias
    ("Maliwan", "Shotgun"): 19,
    # Assault Rifles (13-18)
    ("Daedalus", "Assault Rifle"): 13,
    ("Jakobs", "Assault Rifle"): 14,
    ("Tediore", "Assault Rifle"): 15,
    ("Torgue", "Assault Rifle"): 16,
    ("Vladof", "Assault Rifle"): 17,
    ("Order", "Assault Rifle"): 18,
    # SMGs (20-24)
    ("Daedalus", "SMG"): 20,
    ("Bor", "SMG"): 21,
    ("Ripper", "SMG"): 21,  # Bor/Ripper alias
    ("Vladof", "SMG"): 22,
    ("Maliwan", "SMG"): 23,
    # Snipers (25-29)
    ("Bor", "Sniper"): 25,
    ("Ripper", "Sniper"): 25,  # Bor/Ripper alias
    ("Jakobs", "Sniper"): 26,
    ("Vladof", "Sniper"): 27,
    ("Order", "Sniper"): 28,
    ("Maliwan", "Sniper"): 29,
    # Heavy Weapons (244-247)
    ("Vladof", "Heavy"): 244,
    ("Torgue", "Heavy"): 245,
    ("Bor", "Heavy"): 246,
    ("Ripper", "Heavy"): 246,
    ("Maliwan", "Heavy"): 247,
}

# Reverse lookup for group names
CATEGORY_NAMES = {
    2: "Daedalus Pistol",
    3: "Jakobs Pistol",
    4: "Tediore Pistol",
    5: "Torgue Pistol",
    6: "Order Pistol",
    7: "Vladof Pistol",
    8: "Daedalus Shotgun",
    9: "Jakobs Shotgun",
    10: "Tediore Shotgun",
    11: "Torgue Shotgun",
    12: "Bor Shotgun",
    13: "Daedalus Assault Rifle",
    14: "Jakobs Assault Rifle",
    15: "Tediore Assault Rifle",
    16: "Torgue Assault Rifle",
    17: "Vladof Assault Rifle",
    18: "Order Assault Rifle",
    19: "Maliwan Shotgun",
    20: "Daedalus SMG",
    21: "Bor SMG",
    22: "Vladof SMG",
    23: "Maliwan SMG",
    25: "Bor Sniper",
    26: "Jakobs Sniper",
    27: "Vladof Sniper",
    28: "Order Sniper",
    29: "Maliwan Sniper",
    244: "Vladof Heavy",
    245: "Torgue Heavy",
    246: "Bor Heavy",
    247: "Maliwan Heavy",
}


def parse_spreadsheet(csv_path: Path) -> list[dict]:
    """Parse the weapon parts lookup table CSV."""
    parts = []
    skipped = []

    with open(csv_path, 'r', encoding='utf-8-sig') as f:
        # Handle potential BOM and weird encodings
        content = f.read()
        # Normalize line endings and non-breaking spaces
        content = content.replace('\r\n', '\n').replace('\xa0', ' ')

    lines = content.split('\n')
    reader = csv.reader(lines)

    header = None
    for row in reader:
        if not row or not row[0]:
            continue

        # Find header row
        if row[0] == 'Manufacturer':
            header = row
            continue

        if header is None:
            continue

        # Parse data row
        if len(row) < 5:
            continue

        manufacturer = row[0].strip()
        weapon_type = row[1].strip()

        # Skip empty or header-like rows
        if not manufacturer or manufacturer == 'Manufacturer':
            continue

        try:
            part_id = int(row[2].strip())
        except (ValueError, IndexError):
            continue

        part_type = row[3].strip() if len(row) > 3 else ""
        part_string = row[4].strip() if len(row) > 4 else ""

        # Skip rows without part string
        if not part_string or not part_string.startswith(('DAD_', 'JAK_', 'TED_', 'TOR_',
                                                           'ORD_', 'VLA_', 'MAL_', 'BOR_',
                                                           'RIP_', 'COV_', 'ATL_')):
            continue

        # Look up category
        key = (manufacturer, weapon_type)
        if key not in CATEGORY_MAP:
            skipped.append(f"{manufacturer} {weapon_type}: {part_string}")
            continue

        category = CATEGORY_MAP[key]
        group = CATEGORY_NAMES.get(category, f"Category {category}")

        parts.append({
            "category": category,
            "index": part_id,
            "name": part_string,
            "group": group,
            "part_type": part_type,
        })

    if skipped:
        print(f"Skipped {len(skipped)} parts with unknown categories:", file=sys.stderr)
        for s in skipped[:10]:
            print(f"  {s}", file=sys.stderr)
        if len(skipped) > 10:
            print(f"  ... and {len(skipped) - 10} more", file=sys.stderr)

    return parts


def merge_with_existing(new_parts: list[dict], existing_path: Path) -> list[dict]:
    """Merge new parts with existing database, preferring new data for overlapping categories."""
    if not existing_path.exists():
        return new_parts

    with open(existing_path, 'r') as f:
        existing = json.load(f)

    # Get categories covered by new parts
    new_categories = set(p["category"] for p in new_parts)

    # Keep existing parts for categories not in new data
    merged = list(new_parts)
    for p in existing.get("parts", []):
        if p["category"] not in new_categories:
            merged.append(p)

    return merged


def main():
    script_dir = Path(__file__).parent.parent
    csv_path = script_dir / "share/data/Borderlands 4 Deserialization/Borderlands 4 Deserilization - Weapon Parts Lookup Table V2.csv"
    existing_path = script_dir / "share/manifest/parts_database.json"
    output_path = script_dir / "share/manifest/parts_database_spreadsheet.json"

    if not csv_path.exists():
        print(f"Error: CSV file not found: {csv_path}", file=sys.stderr)
        sys.exit(1)

    print(f"Parsing: {csv_path}")
    parts = parse_spreadsheet(csv_path)

    # Merge with existing database for non-weapon categories
    if existing_path.exists():
        print(f"Merging with existing: {existing_path}")
        parts = merge_with_existing(parts, existing_path)

    # Sort by category, then index
    parts.sort(key=lambda p: (p["category"], p["index"]))

    # Remove part_type from output (only needed for debugging)
    for p in parts:
        if "part_type" in p:
            del p["part_type"]

    output = {
        "version": 2,
        "source": "Community Spreadsheet (weapons) + memory extraction (other)",
        "note": "Weapon indices from spreadsheet - requires in-game verification",
        "parts": parts,
    }

    with open(output_path, 'w') as f:
        json.dump(output, f, indent=2)

    # Stats
    categories = {}
    for p in parts:
        cat = p["category"]
        categories[cat] = categories.get(cat, 0) + 1

    print(f"\nImported {len(parts)} parts across {len(categories)} categories")
    print(f"Output: {output_path}")
    print("\nParts per category:")
    for cat in sorted(categories.keys()):
        name = CATEGORY_NAMES.get(cat, f"Category {cat}")
        print(f"  {cat:3d}: {categories[cat]:3d} parts - {name}")


if __name__ == "__main__":
    main()
