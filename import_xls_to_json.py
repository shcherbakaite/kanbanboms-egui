#!/usr/bin/env python3
"""
Add part master entries (part number, description) from Part Number Master XLS
into kanbanboms_export.json. New parts are added; existing parts get description
updated from XLS when XLS has a non-empty description.
"""

import json
import re
import uuid
from pathlib import Path

try:
    import xlrd
except ImportError:
    print("Install xlrd: pip install xlrd")
    raise SystemExit(1)

XLS_PATH = Path(__file__).parent / "Part Number Master_007.xls"
EXPORT_PATH = Path(__file__).parent / "kanbanboms_export.json"
NAMESPACE = uuid.UUID("6ba7b810-9dad-11d1-80b4-00c04fd430c8")


def normalize_partno(v) -> str:
    """Convert cell value to part number string."""
    if v is None:
        return ""
    s = str(v).strip()
    # Handle Excel float like 75001.0 -> 75001
    if re.match(r"^\d+\.0+$", s):
        s = str(int(float(s)))
    return s


PARTNO_RE = re.compile(r"^[0-9]{4,5}-[A-Za-z0-9]{2}-[0-9A-Za-z]{2,5}$")


def is_partno(s: str) -> bool:
    return bool(s and PARTNO_RE.match(s.strip()))


def extract_parts_from_xls() -> dict[str, str]:
    """Extract (partno -> description) from all sheets."""
    wb = xlrd.open_workbook(XLS_PATH)
    result: dict[str, str] = {}

    for sname in wb.sheet_names():
        sh = wb.sheet_by_index(wb.sheet_names().index(sname))
        pn_col = desc_col = sales_col = hdr_row = None

        # Find header row with PARTNUMBER and column indices
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
                    if desc_col is None:
                        desc_col = c + 1
                    break
            if hdr_row is not None:
                break

        # Find which column has part numbers in data rows (header col may be wrong)
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
            for r in range(hdr_row + 1, sh.nrows):
                pn = normalize_partno(sh.cell_value(r, pn_col))
                if not is_partno(pn):
                    continue
                desc = ""
                if desc_col is not None and desc_col < sh.ncols:
                    desc = str(sh.cell_value(r, desc_col) or "").strip()
                if sales_col is not None and sales_col < sh.ncols:
                    sales = str(sh.cell_value(r, sales_col) or "").strip()
                    if sales:
                        desc = sales if not desc else desc
                if pn not in result or desc:
                    result[pn] = desc
            continue

        # Fallback: scan all cells for part number pattern
        for r in range(sh.nrows):
            for c in range(sh.ncols):
                pn = normalize_partno(sh.cell_value(r, c))
                if not is_partno(pn):
                    continue
                desc = ""
                for dc in [1, 2, -1]:
                    c2 = c + dc
                    if 0 <= c2 < sh.ncols:
                        v = str(sh.cell_value(r, c2) or "").strip()
                        if v and len(v) > 2 and not re.match(r"^[\d.-]+$", v):
                            desc = v
                            break
                if pn not in result or desc:
                    result[pn] = desc
                break

    return result


def main():
    if not XLS_PATH.exists():
        print(f"XLS not found: {XLS_PATH}")
        raise SystemExit(1)

    xls_parts = extract_parts_from_xls()
    print(f"Loaded {len(xls_parts)} part numbers from XLS")

    if not EXPORT_PATH.exists():
        print(f"JSON not found: {EXPORT_PATH}. Run export_db_to_json.py first.")
        raise SystemExit(1)

    with open(EXPORT_PATH) as f:
        data = json.load(f)

    boms = data.get("boms", [])
    partno_to_bom = {b["partno"]: b for b in boms}
    existing_ids = {b["id"] for b in boms}

    added = 0
    updated = 0
    for pn, desc in xls_parts.items():
        if not pn.strip():
            continue
        if pn in partno_to_bom:
            if desc and partno_to_bom[pn].get("description") != desc:
                partno_to_bom[pn]["description"] = desc
                updated += 1
        else:
            uid = str(uuid.uuid5(NAMESPACE, f"kanbanboms:{pn}"))
            if uid in existing_ids:
                continue
            boms.append({
                "id": uid,
                "partno": pn,
                "description": desc,
                "batch_quantity": 0,
                "location": "",
                "custom_fields": {},
                "bom_entry_count": 0,
            })
            partno_to_bom[pn] = boms[-1]
            existing_ids.add(uid)
            added += 1

    data["boms"] = boms

    with open(EXPORT_PATH, "w") as f:
        json.dump(data, f, indent=2)

    print(f"Added {added} new parts, updated {updated} descriptions. Total boms: {len(boms)}")


if __name__ == "__main__":
    main()
