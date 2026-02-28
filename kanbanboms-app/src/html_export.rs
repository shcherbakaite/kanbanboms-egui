//! HTML generation for print preview (ported from Django print_request template).

use crate::models::{get_aggregated_parts, Bom, Request, RequestEntry};
use std::collections::HashMap;
use uuid::Uuid;

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

/// Generates a self-contained HTML document for print preview.
/// Mirrors the Django print_request template structure (without accumatica, IDs, barcodes).
pub fn generate_html_print_preview(
    request: &Request,
    assemblies: &[RequestEntry],
    boms: &HashMap<Uuid, Bom>,
    bom_entries: &[crate::models::BomEntry],
    request_entries: &[RequestEntry],
) -> String {
    let date = chrono::Local::now().format("%m/%d/%Y").to_string();
    let parts = get_aggregated_parts(request.id, boms, bom_entries, request_entries);

    let mut html = String::new();
    html.push_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Kitting BOM</title>
<style>
.pr-wrapper { padding: 10px; margin: 0 auto; width: 8.5in; }
.pr-buttons { margin-bottom: 1rem; }
@media print { .pr-buttons { display: none; } .pr-wrapper { width: 95%; margin: auto; padding: auto; } }
.pr-table { border: 1px solid black; border-collapse: collapse; table-layout: fixed; width: 100%; }
.pr-table th, .pr-table td { border: 1px solid black; padding: 5px; }
.pr-table thead th:nth-child(1) { width: 130px; }
.pr-table thead th:nth-child(2) { width: auto; }
.pr-table thead th:nth-child(3) { width: 130px; }
.pr-table thead th:nth-child(4) { width: 75px; }
.pr-table tbody tr:nth-child(odd) { background-color: #e3e3e3; }
.pr-table tr th:nth-child(1), .pr-table tr td:nth-child(1),
.pr-table tr th:nth-child(3), .pr-table tr td:nth-child(3),
.pr-table tr th:nth-child(4), .pr-table tr td:nth-child(4) { text-align: center; }
.pr-table td.text span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: inline-block; max-width: 100%; }
.pr-assemblies-table { width: 100%; table-layout: fixed; }
.pr-assemblies-table tr td:nth-child(1) { width: 45px; }
.pr-assemblies-table tr td:nth-child(2) { width: 125px; }
.pr-assemblies-table td.text span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: inline-block; width: auto; max-width: 100%; }
.pr-2 { padding-right: 0.5rem; }
.mt-2 { margin-top: 0.5rem; }
.mb-4 { margin-bottom: 1rem; }
.btn { padding: 6px 12px; cursor: pointer; border: 1px solid #ccc; border-radius: 4px; background: #f5f5f5; font-size: 14px; }
.btn:hover { background: #e0e0e0; }
@media print { .printed tbody tr:nth-child(odd) td { background-color: #dee2e6 !important; -webkit-print-color-adjust: exact; } }
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
<tr><th class="pr-2" scope="col" style="width: auto; white-space: nowrap">Date:</th><td style="width: 100%">"#);
    html.push_str(&escape_html(&date));
    html.push_str("</td></tr>\n");

    if !request.requested_by.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap\">Requested By:</th><td>");
        html.push_str(&escape_html(&request.requested_by));
        html.push_str("</td></tr>\n");
    }
    if !request.machine_number.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap\">Machine #:</th><td>");
        html.push_str(&escape_html(&request.machine_number));
        html.push_str("</td></tr>\n");
    }
    if !request.notes.is_empty() {
        html.push_str("<tr><th class=\"pr-2\" style=\"width: auto; white-space: nowrap; vertical-align: top\">Notes:</th><td>");
        html.push_str(&escape_html(&request.notes));
        html.push_str("</td></tr>\n");
    }

    html.push_str(r#"</tbody>
</table>

<h2 class="mt-2">Assemblies</h2>
<table class="pr-assemblies-table">
<tbody>
"#);

    for ae in assemblies {
        if let Some(bom) = boms.get(&ae.part_id) {
            html.push_str("<tr><td>x");
            html.push_str(&ae.quantity.to_string());
            html.push_str("</td><td>");
            html.push_str(&escape_html(&bom.partno));
            html.push_str("</td><td class=\"text\"><span>");
            html.push_str(&escape_html(&bom.description));
            html.push_str("</span></td></tr>\n");
        }
    }

    html.push_str(r#"</tbody>
</table>

<h2 class="mt-2">Bill Of Materials</h2>
<table class="pr-table printed">
<thead>
<tr>
<th>Part Number</th>
<th>Description</th>
<th>Location</th>
<th>Quantity</th>
</tr>
</thead>
<tbody>
"#);

    for (partno, desc, qty, loc, _tags) in &parts {
        let loc_display = if loc.is_empty() { "N/A" } else { loc.as_str() };
        html.push_str("<tr><td class=\"text\"><span>");
        html.push_str(&escape_html(partno));
        html.push_str("</span></td><td class=\"text\"><span>");
        html.push_str(&escape_html(desc));
        html.push_str("</span></td><td class=\"text\"><span>");
        html.push_str(&escape_html(loc_display));
        html.push_str("</span></td><td>");
        html.push_str(&qty.to_string());
        html.push_str("</td></tr>\n");
    }

    html.push_str(r#"</tbody>
</table>
</div>
</body>
</html>
"#);

    html
}
