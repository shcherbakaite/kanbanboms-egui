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

        ui.strong("Bill Of Materials");
        ui.add_space(4.0);
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("bom_grid")
                .num_columns(4)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("Part Number");
                    ui.strong("Description");
                    ui.strong("Location");
                    ui.strong("Quantity");
                    ui.end_row();
                    for (partno, desc, qty, loc) in &parts {
                        let loc_display = if loc.is_empty() { "N/A" } else { loc.as_str() };
                        ui.label(partno);
                        ui.label(desc);
                        ui.label(loc_display);
                        ui.label(qty.to_string());
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
