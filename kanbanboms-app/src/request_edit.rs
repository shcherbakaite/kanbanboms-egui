use crate::app::KanbanBomsApp;
use crate::bom_search::search_boms;
use crate::models::normalize_partno;
use egui;

const CENTERED_MAX_WIDTH: f32 = 700.0;

pub fn request_edit_ui(app: &mut KanbanBomsApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_rect_before_wrap();
        let width = avail.width().min(CENTERED_MAX_WIDTH);
        let left = avail.left() + (avail.width() - width) / 2.0;
        let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));
        ui.allocate_new_ui(egui::UiBuilder::default().max_rect(rect), |ui| {
        ui.heading("Kanban BOMs - Request");
        ui.add_space(8.0);

        let request_id = match app.current_request_id {
            Some(id) => id,
            None => {
                ui.label("No request selected");
                return;
            }
        };

        let entries: Vec<_> = app
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .cloned()
            .collect();

        if entries.is_empty() {
            ui.label("Scan kanban card or enter part numbers manually");
        } else {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("request_assembly_grid")
                    .num_columns(3)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("Part Number");
                        ui.strong("Description");
                        ui.strong("Quantity");
                        ui.end_row();

                        for entry in &entries {
                            if let Some(bom) = app.boms.iter().find(|b| b.id == entry.part_id) {
                                if let Some(re) = app.request_entries.iter_mut().find(|e| e.request_id == request_id && e.part_id == entry.part_id) {
                                    ui.label(&bom.partno);
                                    ui.label(&bom.description);
                                    ui.add(egui::DragValue::new(&mut re.quantity).speed(0.5).range(0..=10000));
                                    ui.end_row();
                                }
                            }
                        }
                    });
            });
            app.request_entries.retain(|e| e.quantity > 0);
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Clear List").clicked() {
                app.clear_request();
            }
            if ui.button("Preview").clicked() {
                app.current_screen = crate::app::Screen::BomPreview;
            }
            if ui.button("Export PDF").clicked() {
                app.trigger_pdf_download();
            }
            let csv = app.export_csv();
            if !csv.is_empty() && ui.button("Export CSV").clicked() {
                #[cfg(target_arch = "wasm32")]
                crate::app::download_bytes(csv.as_bytes(), "kitting_bom.csv");
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Err(e) = std::fs::write("kitting_bom.csv", &csv) {
                        log::error!("Failed to write CSV: {}", e);
                    }
                }
            }
        });

        ui.add_space(8.0);
        ui.label("Add by part number:");
        ui.horizontal(|ui| {
            let response = ui.add(egui::TextEdit::singleline(&mut app.partno_add_input).desired_width(200.0));
            let add_clicked = ui.button("Add").clicked();
            let enter_pressed = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if add_clicked || enter_pressed {
                app.partno_add_error = None;
                let normalized = normalize_partno(&app.partno_add_input);
                if !normalized.is_empty() {
                    if let Some(bom) = app.boms.iter().find(|b| b.partno.eq_ignore_ascii_case(&normalized)) {
                        app.add_assembly_to_request(bom.id);
                        app.partno_add_input.clear();
                    } else {
                        app.partno_add_error = Some("Part number does not exist".to_string());
                    }
                }
            }
            if let Some(ref err) = app.partno_add_error {
                ui.colored_label(egui::Color32::RED, err);
            }
        });
        if !app.partno_add_input.is_empty() {
            let results: Vec<_> = search_boms(&app.boms, &app.partno_add_input)
                .into_iter()
                .map(|b| (b.id, b.partno.clone(), b.description.clone()))
                .collect();
            if results.is_empty() {
                ui.label("No matching BOMs");
            } else {
                let mut clicked_id = None;
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    egui::Grid::new("search_results_grid")
                        .num_columns(3)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.strong("Part Number");
                            ui.strong("Description");
                            ui.strong("");
                            ui.end_row();

                            for (id, partno, description) in &results {
                                ui.label(partno);
                                ui.label(description);
                                if ui.small_button("Add").clicked() {
                                    clicked_id = Some(*id);
                                }
                                ui.end_row();
                            }
                        });
                });
                if let Some(id) = clicked_id {
                    app.add_assembly_to_request(id);
                    app.partno_add_input.clear();
                    app.partno_add_error = None;
                }
            }
        }
        });
    });
}
