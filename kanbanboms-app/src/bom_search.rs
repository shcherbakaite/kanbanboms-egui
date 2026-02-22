use crate::models::Bom;
use egui::{RichText, Sense};
use uuid::Uuid;

/// Filter BOMs where partno or description contains all keywords (case-insensitive AND)
pub fn search_boms<'a>(boms: &'a [Bom], keywords: &str) -> Vec<&'a Bom> {
    let kws: Vec<&str> = keywords.split_whitespace().filter(|s| !s.is_empty()).collect();
    if kws.is_empty() {
        return Vec::new();
    }
    boms.iter()
        .filter(|b| {
            let text = format!("{}{}", b.partno, b.description).to_lowercase();
            kws.iter().all(|kw| text.contains(&kw.to_lowercase()))
        })
        .collect()
}

/// Returns Some(bom_id) when user selects a BOM
pub fn bom_search_ui(
    ui: &mut egui::Ui,
    search_query: &mut String,
    boms: &[Bom],
    selected: &mut Option<Uuid>,
) {
    *selected = None;
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(search_query);
    });
    if search_query.is_empty() {
        ui.label("Type in a query...");
        return;
    }
    let results = search_boms(boms, search_query);
    if results.is_empty() {
        ui.label("No results...");
        return;
    }
    egui::ScrollArea::vertical().show_rows(ui, 24.0, results.len(), |ui, row_range| {
        for i in row_range {
            if let Some(bom) = results.get(i) {
                let text = format!("{} - {}", bom.partno, bom.description);
                let r = ui.add(
                    egui::Label::new(RichText::new(text).color(ui.visuals().hyperlink_color))
                        .sense(Sense::click()),
                );
                if r.clicked() {
                    *selected = Some(bom.id);
                }
            }
        }
    });
}
