use crate::app::KanbanBomsApp;
use crate::models::get_aggregated_parts;
use chrono::Local;
use egui;

const CENTERED_MAX_WIDTH: f32 = 700.0;

pub fn bom_preview_ui(app: &mut KanbanBomsApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_rect_before_wrap();
        let width = avail.width().min(CENTERED_MAX_WIDTH);
        let left = avail.left() + (avail.width() - width) / 2.0;
        let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));
        ui.allocate_new_ui(egui::UiBuilder::default().max_rect(rect), |ui| {
        ui.heading("Kitting BOM - Preview");
        ui.add_space(8.0);

        let request_id = match app.current_request_id {
            Some(id) => id,
            None => {
                ui.label("No request selected");
                return;
            }
        };

        let assemblies: Vec<_> = app
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .cloned()
            .collect();

        if assemblies.is_empty() {
            ui.label("Add assemblies to the request to see the BOM preview.");
            ui.add_space(8.0);
            if ui.button("Back to Requests").clicked() {
                app.current_screen = crate::app::Screen::RequestEdit;
            }
            return;
        }

        let date = Local::now().format("%m/%d/%Y").to_string();
        ui.label(format!("Date: {}", date));
        ui.add_space(8.0);

        ui.strong("Assemblies");
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("assemblies_grid")
                .num_columns(3)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("Qty");
                    ui.strong("Part Number");
                    ui.strong("Description");
                    ui.end_row();
                    for ae in &assemblies {
                        if let Some(bom) = app.boms.iter().find(|b| b.id == ae.part_id) {
                            ui.label(ae.quantity.to_string());
                            ui.label(&bom.partno);
                            ui.label(&bom.description);
                            ui.end_row();
                        }
                    }
                });
        });
        ui.add_space(12.0);

        let boms_map = app.boms_by_id();
        let parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &app.bom_entries,
            &app.request_entries,
        );

        // Collect all unique tags and ensure they have visibility state (default: visible)
        let all_tags: std::collections::HashSet<String> = parts
            .iter()
            .flat_map(|(_, _, _, _, tags)| tags.iter().cloned())
            .collect();
        for tag in &all_tags {
            app.preview_tag_visible
                .entry(tag.clone())
                .or_insert(true);
        }

        if !all_tags.is_empty() {
            ui.strong("Filter by tag:");
            ui.horizontal_wrapped(|ui| {
                let mut tags_sorted: Vec<_> = all_tags.into_iter().collect();
                tags_sorted.sort();
                for tag in tags_sorted {
                    let visible = app.preview_tag_visible.get_mut(&tag).unwrap();
                    if ui.checkbox(visible, &tag).changed() {
                        // Toggled
                    }
                }
            });
            ui.add_space(8.0);
        }

        // Filter parts: show if part has no tags, or has at least one visible tag
        let mut filtered_parts: Vec<_> = parts
            .iter()
            .filter(|(_, _, _, _, tags)| {
                tags.is_empty()
                    || tags.iter().any(|t| app.preview_tag_visible.get(t).copied().unwrap_or(true))
            })
            .cloned()
            .collect();

        // Sort by selected column
        if let Some((col, asc)) = app.preview_bom_sort {
            filtered_parts.sort_by(|a, b| {
                let ord = match col {
                    0 => a.0.cmp(&b.0),
                    1 => a.1.cmp(&b.1),
                    2 => a.3.cmp(&b.3),
                    3 => a.2.cmp(&b.2),
                    4 => a.4.join(" ").cmp(&b.4.join(" ")),
                    _ => std::cmp::Ordering::Equal,
                };
                if asc {
                    ord
                } else {
                    ord.reverse()
                }
            });
        }

        ui.strong("Bill Of Materials");
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("bom_grid")
                .num_columns(5)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    let col_headers = ["Part Number", "Description", "Location", "Quantity", "Tags"];
                    for (col, &label) in col_headers.iter().enumerate() {
                        let (cur_col, asc) = app.preview_bom_sort.unwrap_or((99, true));
                        let marker = if cur_col == col {
                            if asc { " ▲" } else { " ▼" }
                        } else {
                            ""
                        };
                        if ui.link(format!("{}{}", label, marker)).clicked() {
                            app.preview_bom_sort = Some((
                                col,
                                if app.preview_bom_sort.map(|(c, _)| c == col).unwrap_or(false) {
                                    !asc
                                } else {
                                    true
                                },
                            ));
                        }
                    }
                    ui.end_row();
                    for (partno, desc, qty, loc, tags) in &filtered_parts {
                        let loc_display = if loc.is_empty() { "N/A" } else { loc.as_str() };
                        let tags_display = tags.join(" ");
                        ui.label(partno);
                        ui.label(desc);
                        ui.label(loc_display);
                        ui.label(qty.to_string());
                        ui.label(if tags_display.is_empty() { "—" } else { &tags_display });
                        ui.end_row();
                    }
                });
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Back to Requests").clicked() {
                app.current_screen = crate::app::Screen::RequestEdit;
            }
            if ui.button("Export PDF").clicked() {
                app.trigger_pdf_download();
            }
        });
        });
    });
}
