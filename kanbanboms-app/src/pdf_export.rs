use crate::models::{get_aggregated_parts, Bom, Request, RequestEntry};
use genpdf::elements::{FrameCellDecorator, Paragraph, TableLayout, Text};
use genpdf::Element;
use genpdf::{elements, fonts, style, Document, SimplePageDecorator};
use std::collections::HashMap;
use std::io::Cursor;
use uuid::Uuid;

const FONT: &[u8] = include_bytes!("../assets/fonts/LiberationSans-Regular.ttf");

fn load_font() -> fonts::FontFamily<fonts::FontData> {
    let font_data = fonts::FontData::new(FONT.to_vec(), None).expect("Failed to load font");
    fonts::FontFamily {
        regular: font_data.clone(),
        bold: font_data.clone(),
        italic: font_data.clone(),
        bold_italic: font_data.clone(),
    }
}

pub fn generate_pdf(
    request: &Request,
    assemblies: &[RequestEntry],
    boms: &HashMap<Uuid, Bom>,
    bom_entries: &[crate::models::BomEntry],
    request_entries: &[RequestEntry],
) -> Result<Vec<u8>, String> {
    let font = load_font();
    let mut doc = Document::new(font);
    doc.set_title("Kitting BOM");
    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(10);
    doc.set_page_decorator(decorator);

    let date = chrono::Local::now().format("%m/%d/%Y").to_string();

    doc.push(Paragraph::new("Kitting BOM").styled(style::Style::new().with_font_size(18)));
    doc.push(elements::Break::new(1.0));

    doc.push(Paragraph::new(format!("Date: {}", date)));
    doc.push(elements::Break::new(1.0));

    doc.push(Paragraph::new("Assemblies").styled(style::Style::new().with_font_size(14)));
    doc.push(elements::Break::new(0.5));

    let mut asm_table = TableLayout::new(vec![1, 2, 3]);
    asm_table.set_cell_decorator(FrameCellDecorator::new(true, true, false));
    asm_table
        .row()
        .element(Text::new("Qty"))
        .element(Text::new("Part Number"))
        .element(Text::new("Description"))
        .push()
        .map_err(|e| e.to_string())?;
    for ae in assemblies {
        if let Some(bom) = boms.get(&ae.part_id) {
            asm_table
                .row()
                .element(Text::new(ae.quantity.to_string()))
                .element(Text::new(bom.partno.clone()))
                .element(Text::new(bom.description.clone()))
                .push()
                .map_err(|e| e.to_string())?;
        }
    }
    doc.push(asm_table);
    doc.push(elements::Break::new(1.0));

    let parts = get_aggregated_parts(
        request.id,
        boms,
        bom_entries,
        request_entries,
    );
    doc.push(Paragraph::new("Bill Of Materials").styled(style::Style::new().with_font_size(14)));
    doc.push(elements::Break::new(0.5));

    let mut part_table = TableLayout::new(vec![1, 2, 1, 1]);
    part_table.set_cell_decorator(FrameCellDecorator::new(true, true, false));
    part_table
        .row()
        .element(Text::new("Part Number"))
        .element(Text::new("Description"))
        .element(Text::new("Location"))
        .element(Text::new("Quantity"))
        .push()
        .map_err(|e| e.to_string())?;
    for (partno, desc, qty, loc) in &parts {
        let loc_display = if loc.is_empty() { "N/A" } else { loc.as_str() };
        part_table
            .row()
            .element(Text::new(partno.clone()))
            .element(Text::new(desc.clone()))
            .element(Text::new(loc_display.to_string()))
            .element(Text::new(qty.to_string()))
            .push()
            .map_err(|e| e.to_string())?;
    }
    doc.push(part_table);

    let mut buf = Cursor::new(Vec::new());
    doc.render(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}
