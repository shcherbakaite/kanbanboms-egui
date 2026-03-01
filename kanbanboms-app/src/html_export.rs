//! HTML generation for print preview (ported from Django print_request template).

use crate::models::{Bom, Request, RequestEntry};
use std::collections::HashMap;
use std::cmp::Ordering;
use uuid::Uuid;

/// Part tuple: (partno, description, quantity, uom, custom_fields, tags)
pub type PartTuple = (String, String, i32, String, HashMap<String, String>, Vec<String>);

fn compare_part_by_column(
    a: &PartTuple,
    b: &PartTuple,
    col_name: &str,
    asc: bool,
) -> Ordering {
    let ord = match col_name {
        "Part Number" => a.0.cmp(&b.0),
        "Description" => a.1.cmp(&b.1),
        "UOM" => a.3.cmp(&b.3),
        "Qty" => a.2.cmp(&b.2),
        "Tags" => a.5.join(" ").cmp(&b.5.join(" ")),
        meta => {
            let va = a.4.get(meta).map(|s| s.as_str()).unwrap_or("");
            let vb = b.4.get(meta).map(|s| s.as_str()).unwrap_or("");
            va.cmp(vb)
        }
    };
    if asc {
        ord
    } else {
        ord.reverse()
    }
}

/// Sorts parts by the given sort state (column_name, ascending). Primary sort is last.
pub fn sort_parts_by_state(
    parts: &mut [PartTuple],
    sort_state: &[(String, bool)],
) {
    for (col_name, asc) in sort_state.iter().rev() {
        parts.sort_by(|a, b| compare_part_by_column(a, b, col_name, *asc));
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

fn get_cell_value(part: &PartTuple, col_name: &str) -> String {
    match col_name {
        "Part Number" => part.0.clone(),
        "Description" => part.1.clone(),
        "UOM" => if part.3.is_empty() { "EA".to_string() } else { part.3.clone() },
        "Qty" => part.2.to_string(),
        "Tags" => part.5.join(" "),
        meta => part.4.get(meta).cloned().unwrap_or_default(),
    }
}

fn is_meta_field(col_name: &str) -> bool {
    !matches!(col_name, "Part Number" | "Description" | "UOM" | "Qty" | "Tags")
}

/// Generates a self-contained HTML document for print preview.
/// Mirrors the Django print_request template structure (without accumatica, IDs, barcodes).
/// `parts` should be in the desired display order (e.g. matching preview data grid sort).
/// `column_specs`: (column_name, include_in_html) in display order.
pub fn generate_html_print_preview(
    request: &Request,
    assemblies: &[RequestEntry],
    boms: &HashMap<Uuid, Bom>,
    parts: &[PartTuple],
    column_specs: &[(String, bool)],
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

    // Columns to emit (in display order, only include when true)
    let included: Vec<&(String, bool)> = column_specs
        .iter()
        .filter(|(_, inc)| *inc)
        .collect();

    let col_width = |name: &str| -> &'static str {
        match name {
            "Part Number" => "110px",
            "Description" => "auto",
            "UOM" => "40px",
            "Qty" => "50px",
            "Tags" => "80px",
            _ => "100px", // meta fields
        }
    };
    let center_col = |name: &str| -> bool {
        matches!(name, "Part Number" | "UOM" | "Qty") || is_meta_field(name)
    };
    let is_text_col = |name: &str| -> bool {
        matches!(name, "Part Number" | "Description" | "Tags") || is_meta_field(name)
    };

    for (name, _) in &included {
        html.push_str("<col style=\"width: ");
        html.push_str(col_width(name));
        html.push_str("\">\n");
    }

    html.push_str("</colgroup>\n<thead>\n<tr>\n");
    for (name, _) in &included {
        let cls = if center_col(name) {
            " class=\"pr-center\""
        } else {
            ""
        };
        html.push_str("<th");
        html.push_str(cls);
        html.push_str(">");
        html.push_str(&escape_html(name));
        html.push_str("</th>\n");
    }
    html.push_str("</tr>\n</thead>\n<tbody>\n");

    for part in parts {
        html.push_str("<tr>");
        for (name, _) in &included {
            let val = get_cell_value(part, name);
            let display = if val.is_empty() && is_meta_field(name) {
                "N/A".to_string()
            } else {
                val
            };
            let val_esc = escape_html(&display);
            let use_text = is_text_col(name);
            let center = center_col(name);
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
            html.push_str(&val_esc);
            html.push_str("\">");
            if use_text {
                html.push_str("<span>");
            }
            html.push_str(&val_esc);
            if use_text {
                html.push_str("</span>");
            }
            html.push_str("</td>");
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
