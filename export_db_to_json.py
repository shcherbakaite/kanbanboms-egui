#!/usr/bin/env python3
"""
Export db.sqlite3 (Django kanbanbomsapp) to kanbanboms_export.json format.
Preserves UUIDs from existing export when partno matches; generates deterministic
UUID5 for new parts.
"""

import json
import sqlite3
import uuid
from pathlib import Path

DB_PATH = Path(__file__).parent / "db.sqlite3"
EXPORT_PATH = Path(__file__).parent / "kanbanboms_export.json"
NAMESPACE = uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8")  # DNS namespace for UUID5


def partno_to_uuid(partno: str, existing_map: dict[str, str]) -> str:
    """Use existing UUID if partno known, else deterministic UUID5."""
    if partno in existing_map:
        return existing_map[partno]
    return str(uuid.uuid5(NAMESPACE, f"kanbanboms:{partno}"))


def parse_tags(tags_str: str | None) -> list[str]:
    """Parse tags from DB: comma-separated or single value."""
    if not tags_str or not tags_str.strip():
        return []
    return [t.strip() for t in tags_str.split(",") if t.strip()]


def main():
    # Load existing export to preserve partno->uuid mapping
    existing_partno_to_uuid: dict[str, str] = {}
    if EXPORT_PATH.exists():
        with open(EXPORT_PATH) as f:
            data = json.load(f)
        for b in data.get("boms", []):
            existing_partno_to_uuid[b["partno"]] = b["id"]
        part_master_categories = data.get("part_master_categories", ["All", "EA", "ME", "SD", "TR", "Others"])
    else:
        part_master_categories = ["All", "EA", "ME", "SD", "TR", "Others"]

    # Filter out empty category names (PocketBase requires non-empty)
    part_master_categories = [c for c in part_master_categories if c and str(c).strip()]
    if not part_master_categories:
        part_master_categories = ["All", "EA", "ME", "SD", "TR", "Others"]
    if "All" not in part_master_categories:
        part_master_categories.insert(0, "All")

    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    # id -> partno for all boms (skip empty partno to avoid PocketBase 400 "missing required value")
    cur.execute("SELECT id, partno, description, batch_quantity FROM kanbanbomsapp_bom")
    id_to_partno: dict[int, str] = {}
    boms_out = []
    skipped_empty_partno = 0
    for row in cur.fetchall():
        partno = (row["partno"] or "").strip()
        if not partno:
            skipped_empty_partno += 1
            continue
        id_to_partno[row["id"]] = partno
        uid = partno_to_uuid(partno, existing_partno_to_uuid)
        boms_out.append({
            "id": uid,
            "partno": partno,
            "description": (row["description"] or "").strip(),
            "batch_quantity": int(row["batch_quantity"]) if row["batch_quantity"] is not None else 0,
            "location": "",
            "custom_fields": {},
            "bom_entry_count": 0,  # computed below
        })
    if skipped_empty_partno:
        print(f"Skipped {skipped_empty_partno} boms with empty partno")

    # Compute bom_entry_count
    cur.execute("""
        SELECT bom_id, COUNT(*) as cnt
        FROM kanbanbomsapp_bomentry
        GROUP BY bom_id
    """)
    bom_counts = {row["bom_id"]: row["cnt"] for row in cur.fetchall()}
    partno_to_db_id = {pn: db_id for db_id, pn in id_to_partno.items()}
    for b in boms_out:
        db_id = partno_to_db_id.get(b["partno"])
        b["bom_entry_count"] = bom_counts.get(db_id, 0) if db_id else 0

    # bom_entries
    cur.execute("""
        SELECT bom_id, part_id, quantity, uom, disabled, expand, tags
        FROM kanbanbomsapp_bomentry
    """)
    bom_entries_out = []
    for row in cur.fetchall():
        bom_partno = id_to_partno.get(row["bom_id"])
        part_partno = id_to_partno.get(row["part_id"])
        if bom_partno is None or part_partno is None:
            continue  # skip orphaned entries
        qty = int(row["quantity"]) if row["quantity"] is not None else 1
        bom_entries_out.append({
            "bom_id": partno_to_uuid(bom_partno, existing_partno_to_uuid),
            "part_id": partno_to_uuid(part_partno, existing_partno_to_uuid),
            "quantity": qty,
            "uom": (row["uom"] or "EA").strip() or "EA",
            "disabled": bool(row["disabled"]),
            "expand": bool(row["expand"]),
            "tags": parse_tags(row["tags"]),
        })

    conn.close()

    out = {
        "boms": boms_out,
        "bom_entries": bom_entries_out,
        "bom_revisions": {},
        "bom_revision_next": {},
        "part_master_categories": part_master_categories,
    }

    with open(EXPORT_PATH, "w") as f:
        json.dump(out, f, indent=2)

    print(f"Exported {len(boms_out)} boms, {len(bom_entries_out)} bom_entries to {EXPORT_PATH}")


if __name__ == "__main__":
    main()
