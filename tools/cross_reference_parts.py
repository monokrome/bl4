#!/usr/bin/env python3
"""Cross-reference spreadsheet parts with our extracted parts to find patterns.

This helps identify:
1. Which parts from the spreadsheet we already have
2. Which parts are missing
3. Patterns in naming that could help locate data in pak files
"""

import json
import csv
from pathlib import Path
from collections import defaultdict


def load_spreadsheet_parts(csv_path: Path) -> dict[str, list[dict]]:
    """Load parts from spreadsheet, grouped by category prefix."""
    parts_by_prefix = defaultdict(list)

    with open(csv_path, 'r', encoding='utf-8-sig') as f:
        content = f.read().replace('\r\n', '\n').replace('\xa0', ' ')

    lines = content.split('\n')
    reader = csv.reader(lines)

    header = None
    for row in reader:
        if not row or not row[0]:
            continue
        if row[0] == 'Manufacturer':
            header = row
            continue
        if header is None or len(row) < 5:
            continue

        manufacturer = row[0].strip()
        weapon_type = row[1].strip()

        try:
            part_id = int(row[2].strip())
        except ValueError:
            continue

        part_type = row[3].strip() if len(row) > 3 else ""
        part_string = row[4].strip() if len(row) > 4 else ""

        if not part_string or '.' not in part_string:
            continue

        prefix = part_string.split('.')[0]
        parts_by_prefix[prefix].append({
            'id': part_id,
            'manufacturer': manufacturer,
            'weapon_type': weapon_type,
            'part_type': part_type,
            'name': part_string,
        })

    return dict(parts_by_prefix)


def load_our_parts(json_path: Path) -> dict[str, list[str]]:
    """Load our extracted parts grouped by prefix."""
    with open(json_path, 'r') as f:
        return json.load(f)


def analyze_patterns(spreadsheet_parts: dict, our_parts: dict):
    """Analyze patterns between spreadsheet and our data."""

    print("=" * 70)
    print("CROSS-REFERENCE ANALYSIS")
    print("=" * 70)

    # Check which prefixes match
    spreadsheet_prefixes = set(spreadsheet_parts.keys())
    our_prefixes = set(our_parts.keys())

    common = spreadsheet_prefixes & our_prefixes
    spreadsheet_only = spreadsheet_prefixes - our_prefixes
    our_only = our_prefixes - spreadsheet_prefixes

    print(f"\nPrefix coverage:")
    print(f"  Common: {len(common)}")
    print(f"  Spreadsheet only: {len(spreadsheet_only)}")
    print(f"  Our data only: {len(our_only)}")

    if spreadsheet_only:
        print(f"\n  Missing from our data: {sorted(spreadsheet_only)}")
    if our_only:
        print(f"\n  Extra in our data: {sorted(our_only)}")

    # For each common prefix, check part coverage
    print("\n" + "=" * 70)
    print("PER-CATEGORY ANALYSIS")
    print("=" * 70)

    total_matched = 0
    total_missing = 0
    total_extra = 0

    for prefix in sorted(common):
        ss_parts = {p['name'] for p in spreadsheet_parts[prefix]}
        our_part_set = set(our_parts[prefix])

        matched = ss_parts & our_part_set
        missing = ss_parts - our_part_set
        extra = our_part_set - ss_parts

        total_matched += len(matched)
        total_missing += len(missing)
        total_extra += len(extra)

        if missing or extra:
            print(f"\n{prefix}:")
            print(f"  Matched: {len(matched)}, Missing: {len(missing)}, Extra: {len(extra)}")
            if missing and len(missing) <= 5:
                for m in sorted(missing):
                    print(f"    - {m}")
            elif missing:
                print(f"    (missing {len(missing)} parts)")

    print("\n" + "=" * 70)
    print("SUMMARY")
    print("=" * 70)
    print(f"Total matched parts: {total_matched}")
    print(f"Total missing from our data: {total_missing}")
    print(f"Total extra in our data: {total_extra}")

    # Index analysis - check if spreadsheet indices follow a pattern
    print("\n" + "=" * 70)
    print("INDEX PATTERN ANALYSIS")
    print("=" * 70)

    for prefix in sorted(common)[:3]:  # Just first 3 for demo
        ss_indexed = {p['name']: p['id'] for p in spreadsheet_parts[prefix]}
        our_list = our_parts[prefix]

        print(f"\n{prefix} - first 10 parts:")
        print(f"  {'Our Index':<10} {'SS Index':<10} Part Name")
        print(f"  {'-'*10} {'-'*10} {'-'*40}")

        for i, part in enumerate(our_list[:10]):
            ss_idx = ss_indexed.get(part, '?')
            print(f"  {i:<10} {ss_idx:<10} {part}")


def main():
    script_dir = Path(__file__).parent.parent

    spreadsheet_path = script_dir / "share/data/Borderlands 4 Deserialization/Borderlands 4 Deserilization - Weapon Parts Lookup Table V2.csv"
    our_parts_path = script_dir / "share/manifest/parts_dump.json"

    if not spreadsheet_path.exists():
        print(f"Error: Spreadsheet not found: {spreadsheet_path}")
        return
    if not our_parts_path.exists():
        print(f"Error: Parts dump not found: {our_parts_path}")
        return

    print(f"Loading spreadsheet: {spreadsheet_path.name}")
    spreadsheet_parts = load_spreadsheet_parts(spreadsheet_path)

    print(f"Loading our parts: {our_parts_path.name}")
    our_parts = load_our_parts(our_parts_path)

    analyze_patterns(spreadsheet_parts, our_parts)


if __name__ == "__main__":
    main()
