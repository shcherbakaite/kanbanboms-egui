#!/usr/bin/env python3
"""
Export db.sqlite3 to kanbanboms_export.json and merge Part Number Master XLS.
- Exports BOMs and bom_entries from Django db.sqlite3
- Merges part master entries from Part Number Master_007.xls (partno + description)
- Deduplicates by partno to avoid PocketBase uuid validation_not_unique
"""

import argparse
import json
import re
import sqlite3
import uuid
from pathlib import Path

try:
    import xlrd
except ImportError:
    xlrd = None

DB_PATH = Path(__file__).parent / "db.sqlite3"
XLS_PATH = Path(__file__).parent / "Part Number Master_007.xls"
EXPORT_PATH = Path(__file__).parent / "kanbanboms_export.json"
NAMESPACE = uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8")

PARTNO_RE = re.compile(r"^[0-9]{4,5}-[A-Za-z0-9]{2}-[0-9A-Za-z]{2,5}$")
INCOMPLETE_RE = re.compile(r"^([0-9]{4,5})-([A-Za-z0-9]{2})-([0-9A-Za-z]*)$")


def partno_to_uuid(partno: str, existing_map: dict[str, str]) -> str:
    if partno in existing_map:
        return existing_map[partno]
    return str(uuid.uuid5(NAMESPACE, f"kanbanboms:{partno}"))


def normalize_partno(v) -> str:
    if v is None:
        return ""
    s = str(v).strip()
    if re.match(r"^\d+\.0+$", s):
        s = str(int(float(s)))
    return s


def is_partno(s: str) -> bool:
    return bool(s and PARTNO_RE.match(s.strip()))


def repair_partno(raw: str, level_val=None, prev_suffix: str = "") -> str | None:
    s = raw.strip()
    if not s or "-" not in s:
        return None
    m = INCOMPLETE_RE.match(s)
    if not m:
        return None
    prefix, middle, suffix = m.group(1), m.group(2), m.group(3)
    if len(suffix) >= 2 and not suffix.endswith("-"):
        return s
    suffix = suffix.rstrip("-")
    candidate = ""
    if level_val is not None and str(level_val).strip():
        v = str(level_val).strip()
        if "." in v and re.match(r"^[\d.]+$", v):
            v = v.split(".")[0] or v
        if re.match(r"^[0-9A-Za-z]{2,5}$", v):
            candidate = v
    if not candidate and prev_suffix:
        candidate = prev_suffix
    if not candidate:
        return None
    return f"{prefix}-{middle}-{candidate}"


def has_description(desc: str) -> bool:
    s = (desc or "").strip()
    return bool(s and s != "-" and len(s) >= 5)


def parse_tags(tags_str: str | None) -> list[str]:
    if not tags_str or not tags_str.strip():
        return []
    return [t.strip() for t in tags_str.split(",") if t.strip()]


def extract_parts_from_xls() -> dict[str, str]:
    wb = xlrd.open_workbook(XLS_PATH)
    result: dict[str, str] = {}

    for sname in wb.sheet_names():
        sh = wb.sheet_by_index(wb.sheet_names().index(sname))
        pn_col = desc_col = sales_col = level_col = hdr_row = None

        for r in range(min(10, sh.nrows)):
            for c in range(sh.ncols):
                v = str(sh.cell_value(r, c)).upper().replace(" ", "")
                if "PARTNUMBER" in v:
                    hdr_row = r
                    for c2 in range(sh.ncols):
                        v2 = str(sh.cell_value(r, c2)).lower()
                        if "internal" in v2 and "description" in v2:
                            desc_col = c2
                        elif "sales" in v2 and "description" in v2:
                            sales_col = c2
                        elif "level" in v2:
                            level_col = c2
                    if desc_col is None:
                        desc_col = c + 1
                    break
            if hdr_row is not None:
                break

        pn_col = None
        if hdr_row is not None:
            for c in range(sh.ncols):
                found = False
                for r in range(hdr_row + 1, min(hdr_row + 20, sh.nrows)):
                    pn = normalize_partno(sh.cell_value(r, c))
                    if is_partno(pn):
                        pn_col = c
                        found = True
                        break
                if found:
                    break

        if pn_col is not None and hdr_row is not None:
            prev_suffix = ""
            for r in range(hdr_row + 1, sh.nrows):
                raw_pn = normalize_partno(sh.cell_value(r, pn_col))
                pn = raw_pn
                if not is_partno(pn):
                    level_val = level_col is not None and level_col < sh.ncols and sh.cell_value(r, level_col)
                    repaired = repair_partno(raw_pn, level_val, prev_suffix)
                    if repaired and is_partno(repaired):
                        pn = repaired
                    else:
                        continue
                m = INCOMPLETE_RE.match(pn)
                if m and m.group(3):
                    prev_suffix = m.group(3)
                desc = ""
                if desc_col is not None and desc_col < sh.ncols:
                    desc = str(sh.cell_value(r, desc_col) or "").strip()
                if sales_col is not None and sales_col < sh.ncols:
                    sales = str(sh.cell_value(r, sales_col) or "").strip()
                    if sales:
                        desc = sales if not desc else desc
                if not has_description(desc):
                    continue
                if pn not in result or desc:
                    result[pn] = desc
            continue

        prev_suffix = ""
        for r in range(sh.nrows):
            for c in range(sh.ncols):
                raw_pn = normalize_partno(sh.cell_value(r, c))
                pn = raw_pn
                if not is_partno(pn):
                    level_val = None
                    for lc in [c + 4, c + 3, c - 2]:
                        if 0 <= lc < sh.ncols:
                            level_val = sh.cell_value(r, lc)
                            if level_val is not None and str(level_val).strip():
                                break
                    repaired = repair_partno(raw_pn, level_val, prev_suffix)
                    if repaired and is_partno(repaired):
                        pn = repaired
                    else:
                        continue
                m = INCOMPLETE_RE.match(pn)
                if m and m.group(3):
                    prev_suffix = m.group(3)
                desc = ""
                for dc in [1, 2, -1]:
                    c2 = c + dc
                    if 0 <= c2 < sh.ncols:
                        v = str(sh.cell_value(r, c2) or "").strip()
                        if v and len(v) > 2 and not re.match(r"^[\d.-]+$", v):
                            desc = v
                            break
                if not has_description(desc):
                    continue
                if pn not in result or desc:
                    result[pn] = desc
                break

    return result


def deduplicate_boms(boms: list) -> list:
    seen = set()
    out = []
    for b in boms:
        if b["partno"] in seen:
            continue
        seen.add(b["partno"])
        out.append(b)
    return out


def main():
    parser = argparse.ArgumentParser(description="Export db.sqlite3 to JSON, optionally merge XLS part master")
    parser.add_argument("--no-xls", action="store_true", help="Skip XLS part master merge")
    args = parser.parse_args()

    existing_partno_to_uuid: dict[str, str] = {}
    if EXPORT_PATH.exists():
        with open(EXPORT_PATH) as f:
            data = json.load(f)
        for b in data.get("boms", []):
            existing_partno_to_uuid[b["partno"]] = b["id"]
        part_master_categories = data.get("part_master_categories", ["All", "EA", "ME", "SD", "TR", "Others"])
    else:
        part_master_categories = ["All", "EA", "ME", "SD", "TR", "Others"]

    part_master_categories = [c for c in part_master_categories if c and str(c).strip()]
    if not part_master_categories:
        part_master_categories = ["All", "EA", "ME", "SD", "TR", "Others"]
    if "All" not in part_master_categories:
        part_master_categories.insert(0, "All")

    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute("SELECT id, partno, description, batch_quantity FROM kanbanbomsapp_bom")
    id_to_partno: dict[int, str] = {}
    boms_out = []
    skipped_empty = 0
    for row in cur.fetchall():
        partno = (row["partno"] or "").strip()
        if not partno:
            skipped_empty += 1
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
            "bom_entry_count": 0,
        })
    if skipped_empty:
        print(f"Skipped {skipped_empty} boms with empty partno")

    cur.execute("SELECT bom_id, COUNT(*) as cnt FROM kanbanbomsapp_bomentry GROUP BY bom_id")
    bom_counts = {row["bom_id"]: row["cnt"] for row in cur.fetchall()}
    partno_to_db_id = {pn: db_id for db_id, pn in id_to_partno.items()}
    for b in boms_out:
        db_id = partno_to_db_id.get(b["partno"])
        b["bom_entry_count"] = bom_counts.get(db_id, 0) if db_id else 0

    cur.execute("SELECT bom_id, part_id, quantity, uom, disabled, expand, tags FROM kanbanbomsapp_bomentry")
    bom_entries_out = []
    for row in cur.fetchall():
        bom_partno = id_to_partno.get(row["bom_id"])
        part_partno = id_to_partno.get(row["part_id"])
        if bom_partno is None or part_partno is None:
            continue
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

    # Merge XLS part master
    xls_added = xls_updated = 0
    if not args.no_xls and xlrd and XLS_PATH.exists():
        xls_parts = extract_parts_from_xls()
        print(f"Loaded {len(xls_parts)} part numbers from XLS")
        partno_to_bom = {b["partno"]: b for b in boms_out}
        existing_ids = {b["id"] for b in boms_out}
        for pn, desc in xls_parts.items():
            if not pn.strip() or not has_description(desc):
                continue
            if pn in partno_to_bom:
                if partno_to_bom[pn].get("description") != desc:
                    partno_to_bom[pn]["description"] = desc
                    xls_updated += 1
            else:
                uid = str(uuid.uuid5(NAMESPACE, f"kanbanboms:{pn}"))
                if uid in existing_ids:
                    continue
                boms_out.append({
                    "id": uid,
                    "partno": pn,
                    "description": desc,
                    "batch_quantity": 0,
                    "location": "",
                    "custom_fields": {},
                    "bom_entry_count": 0,
                })
                partno_to_bom[pn] = boms_out[-1]
                existing_ids.add(uid)
                xls_added += 1
        if xls_added or xls_updated:
            print(f"XLS: added {xls_added}, updated {xls_updated} descriptions")
    elif not args.no_xls and not xlrd:
        print("Install xlrd for XLS merge: pip install xlrd")
    elif not args.no_xls and not XLS_PATH.exists():
        print(f"XLS not found: {XLS_PATH}, skipping merge")

    # Deduplicate once
    n_before = len(boms_out)
    boms_out = deduplicate_boms(boms_out)
    if len(boms_out) < n_before:
        print(f"Deduplicated {n_before - len(boms_out)} duplicate part numbers")

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
