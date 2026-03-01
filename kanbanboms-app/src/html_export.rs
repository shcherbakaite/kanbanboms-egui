//! HTML generation for print preview (ported from Django print_request template).

use crate::models::{Bom, Request, RequestEntry};
use std::collections::HashMap;
use std::cmp::Ordering;
use uuid::Uuid;

/// Part tuple: (partno, description, quantity, uom, location, tags)
pub type PartTuple = (String, String, i32, String, String, Vec<String>);

fn compare_part_by_column(
    a: &PartTuple,
    b: &PartTuple,
    col: usize,
    asc: bool,
) -> Ordering {
    let ord = match col {
        0 => a.0.cmp(&b.0),
        1 => a.1.cmp(&b.1),
        2 => a.4.cmp(&b.4),   // location
        3 => a.3.cmp(&b.3),   // uom
        4 => a.2.cmp(&b.2),   // quantity
        5 => a.5.join(" ").cmp(&b.5.join(" ")),
        _ => Ordering::Equal,
    };
    if asc {
        ord
    } else {
        ord.reverse()
    }
}

/// Sorts parts by the given sort state (column_index, ascending). Primary sort is last.
pub fn sort_parts_by_state(parts: &mut [PartTuple], sort_state: &[(usize, bool)]) {
    for (col, asc) in sort_state.iter().rev() {
        parts.sort_by(|a, b| compare_part_by_column(a, b, *col, *asc));
    }
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Column indices for BOM table: 0=Part Number, 1=Description, 2=Location, 3=UOM, 4=Qty, 5=Tags.
const BOM_COL_NAMES: &[&str] = &["Part Number", "Description", "Location", "UOM", "Qty", "Tags"];

/// Generates a self-contained HTML document for print preview.
/// Mirrors the Django print_request template structure (without accumatica, IDs, barcodes).
/// `parts` should be in the desired display order (e.g. matching preview data grid sort).
/// `columns_in_html`: per-column flags; when true, that column is included in the output. PDF ignores this.
/// `column_order`: display order of columns (indices 0..5); when None, uses default [0,1,2,3,4,5].
pub fn generate_html_print_preview(
    request: &Request,
    assemblies: &[RequestEntry],
    boms: &HashMap<Uuid, Bom>,
    parts: &[PartTuple],
    columns_in_html: &[bool; 6],
    column_order: Option<&[usize]>,
) -> String {
    let date = chrono::Local::now().format("%m/%d/%Y").to_string();

    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Kitting BOM</title>
<style>
/* Bootstrap 4: box-sizing + typography */
*, *::before, *::after { box-sizing: border-box; }
body { margin: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, "Noto Sans", sans-serif; font-size: 0.875rem; font-weight: 400; line-height: 1.35; color: #212529; text-align: left; background-color: #fff; }
h1, h2, h3, h4, h5, h6 { margin-top: 0; margin-bottom: 0.35rem; font-weight: 500; line-height: 1.2; }
h1 { font-size: 1.5rem; }
h2 { font-size: 1.15rem; }
.pr-wrapper { padding: 8px; margin: 0 auto; width: 8.5in; max-width: 100%; }
.pr-buttons { margin-bottom: 0.5rem; }
@media print { .pr-buttons { display: none; } .pr-wrapper { width: 100%; margin: 0; padding: 0.25in; } }
.pr-table { border: 1px solid black; border-collapse: collapse; table-layout: fixed; width: 100%; -webkit-backface-visibility: visible; font-size: 0.8125rem; }
.pr-table th, .pr-table td { border: 1px solid black; padding: 2px 4px; line-height: 1.25; }
.pr-table thead th:nth-child(1) { width: 110px; }
.pr-table thead th:nth-child(2) { width: auto; }
.pr-table thead th:nth-child(3) { width: 100px; }
.pr-table thead th:nth-child(4) { width: 50px; }
.pr-table tbody tr:nth-child(odd) { background-color: #e3e3e3; }
.pr-table .pr-center { text-align: center; }
.pr-table td.text span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: inline-block; max-width: 100%; }
.pr-assemblies-table { width: 100%; table-layout: fixed; font-size: 0.8125rem; }
.pr-assemblies-table th, .pr-assemblies-table td { padding: 2px 4px; line-height: 1.25; }
.pr-assemblies-table tr td:nth-child(1) { width: 35px; }
.pr-assemblies-table tr td:nth-child(2) { width: 100px; }
.pr-assemblies-table td.text span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: inline-block; width: auto; max-width: 100%; }
.pr-2 { padding-right: 0.5rem; }
.mt-2 { margin-top: 0.35rem; }
.mb-4 { margin-bottom: 0.5rem; }
.btn { padding: 0.375rem 0.75rem; cursor: pointer; border: 1px solid transparent; border-radius: 0.25rem; background: #007bff; color: #fff; font-size: 0.875rem; font-weight: 400; line-height: 1.5; font-family: inherit; }
.btn:hover { background: #0056b3; color: #fff; }
@media print { .printed tbody tr:nth-child(odd) td { background-color: #dee2e6 !important; print-color-adjust: exact; -webkit-print-color-adjust: exact; } }
</style>
</head>
<body>
<div class="pr-wrapper">
<div class="pr-buttons mb-4">
<button type="button" onclick="window.print();" class="btn">Print</button>
</div>

<h1>Kitting BOM</h1>
<table>
<tbody>
<tr><th class="pr-2" scope="col" style="width: auto; white-space: nowrap">Date:</th><td style="width: 100%" title="#);
    html.push_str(&escape_html(&date));
    html.push_str(r#"">"#);
    html.push_str(&escape_html(&date));
    html.push_str("</td></tr>\n");

    if !request.requested_by.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap\">Requested By:</th><td title=\"");
        html.push_str(&escape_html(&request.requested_by));
        html.push_str("\">");
        html.push_str(&escape_html(&request.requested_by));
        html.push_str("</td></tr>\n");
    }
    if !request.machine_number.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap\">Machine #:</th><td title=\"");
        html.push_str(&escape_html(&request.machine_number));
        html.push_str("\">");
        html.push_str(&escape_html(&request.machine_number));
        html.push_str("</td></tr>\n");
    }
    if !request.notes.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap; vertical-align: top\">Notes:</th><td title=\"");
        html.push_str(&escape_html(&request.notes));
        html.push_str("\">");
        html.push_str(&escape_html(&request.notes));
        html.push_str("</td></tr>\n");
    }

    html.push_str(r#"</tbody>
</table>

<h2 class="mt-2">Assemblies</h2>
<table class="pr-assemblies-table">
<colgroup>
<col style="width: 35px">
<col style="width: 100px">
<col>
</colgroup>
<tbody>
"#);

    for ae in assemblies {
        if let Some(bom) = boms.get(&ae.part_id) {
            let partno_esc = escape_html(&bom.partno);
            let desc_esc = escape_html(&bom.description);
            html.push_str("<tr><td title=\"x");
            html.push_str(&ae.quantity.to_string());
            html.push_str("\">x");
            html.push_str(&ae.quantity.to_string());
            html.push_str("</td><td title=\"");
            html.push_str(&partno_esc);
            html.push_str("\">");
            html.push_str(&partno_esc);
            html.push_str("</td><td class=\"text\" title=\"");
            html.push_str(&desc_esc);
            html.push_str("\"><span>");
            html.push_str(&desc_esc);
            html.push_str("</span></td></tr>\n");
        }
    }

    html.push_str(r#"</tbody>
</table>

<h2 class="mt-2">Bill Of Materials</h2>
<table class="pr-table printed">
<colgroup>
"#);

    // Ordered list of columns to emit (respects preview column order)
    let order: Vec<usize> = match column_order {
        Some(o) if o.len() == 6 => o.to_vec(),
        _ => (0..6).collect(),
    };

    let col_widths = ["110px", "auto", "100px", "40px", "50px", "80px"];
    let center_cols = [true, false, true, true, true, false]; // Part Number, Location, UOM, Qty centered
    let text_class_cols = [true, true, true, false, false, true]; // Part Number, Description, Location, Tags use .text span

    // Colgroup: only for included columns, in display order
    for &i in &order {
        if columns_in_html.get(i).copied().unwrap_or(false) {
            let w = col_widths.get(i).copied().unwrap_or("auto");
            html.push_str("<col style=\"width: ");
            html.push_str(w);
            html.push_str("\">\n");
        }
    }

    html.push_str("</colgroup>\n<thead>\n<tr>\n");
    for &i in &order {
        if columns_in_html.get(i).copied().unwrap_or(false) {
            let cls = if center_cols.get(i).copied().unwrap_or(false) {
                " class=\"pr-center\""
            } else {
                ""
            };
            html.push_str("<th");
            html.push_str(cls);
            html.push_str(">");
            html.push_str(BOM_COL_NAMES.get(i).copied().unwrap_or(""));
            html.push_str("</th>\n");
        }
    }
    html.push_str("</tr>\n</thead>\n<tbody>\n");

    for (partno, desc, qty, uom, loc, tags) in parts {
        let loc_display = if loc.is_empty() { "N/A" } else { loc.as_str() };
        let uom_display = if uom.is_empty() { "EA" } else { uom.as_str() };
        let tags_display = tags.join(" ");
        let partno_esc = escape_html(partno);
        let desc_esc = escape_html(desc);
        let loc_esc = escape_html(loc_display);
        let uom_esc = escape_html(uom_display);
        let tags_esc = escape_html(&tags_display);

        let cell_values: [&str; 6] = [
            &partno_esc,
            &desc_esc,
            &loc_esc,
            &uom_esc,
            &qty.to_string(),
            &tags_esc,
        ];

        html.push_str("<tr>");
        for &i in &order {
            if columns_in_html.get(i).copied().unwrap_or(false) {
                let val = cell_values.get(i).copied().unwrap_or("");
                let use_text = text_class_cols.get(i).copied().unwrap_or(false);
                let center = center_cols.get(i).copied().unwrap_or(false);
                let mut cls = String::new();
                if use_text {
                    cls.push_str("text");
                }
                if center {
                    if !cls.is_empty() {
                        cls.push(' ');
                    }
                    cls.push_str("pr-center");
                }
                let cls_attr = if cls.is_empty() {
                    String::new()
                } else {
                    format!(" class=\"{}\"", cls)
                };
                html.push_str("<td");
                html.push_str(&cls_attr);
                html.push_str(" title=\"");
                html.push_str(val);
                html.push_str("\">");
                if use_text {
                    html.push_str("<span>");
                }
                html.push_str(val);
                if use_text {
                    html.push_str("</span>");
                }
                html.push_str("</td>");
            }
        }
        html.push_str("</tr>\n");
    }

    html.push_str(r#"</tbody>
</table>
</div>
</body>
</html>
"#);

    html
}
