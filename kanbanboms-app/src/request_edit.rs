use crate::app::KanbanBomsApp;
use crate::bom_search::search_boms;
use crate::models::normalize_partno;
use egui;
use uuid::Uuid;

const CENTERED_MAX_WIDTH: f32 = 700.0;

pub fn request_edit_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui) {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
    let left = avail.left() + (avail.width() - width) / 2.0;
    let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.heading("Kanban BOMs - Request");
        ui.add_space(8.0);

        let entries: Vec<_> = app.request_entries.iter().cloned().collect();

        if entries.is_empty() {
            ui.label("Scan kanban card or enter part numbers manually");
        } else {
            let len_before = app.request_entries.len();
            let mut changed = false;
            let mut to_remove: Vec<Uuid> = Vec::new();
            egui::ScrollArea::vertical()
                .id_salt("request_assembly_scroll")
                .show(ui, |ui| {
                egui::Grid::new("request_assembly_grid")
                    .num_columns(6)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.strong("Part Number");
                        ui.strong("Description");
                        ui.strong("Quantity");
                        ui.strong("Revision");
                        ui.strong("");
                        ui.strong("");
                        ui.end_row();

                        // Cache display by (boms_version, part_ids) - quantity changes don't affect partno/description
                        let part_ids: Vec<Uuid> = entries.iter().map(|e| e.part_id).collect();
                        let cache_hit = app
                            .request_edit_display_cache
                            .as_ref()
                            .map(|(bv, pids, _)| (*bv, pids.as_slice()))
                            == Some((app.boms_version, part_ids.as_slice()));
                        let display: Vec<_> = if cache_hit {
                            app.request_edit_display_cache.as_ref().unwrap().2.clone()
                        } else {
                            let built: Vec<_> = {
                                let boms_map = app.boms_by_id_ref();
                                entries
                                    .iter()
                                    .filter_map(|e| {
                                        boms_map.get(&e.part_id).map(|b| {
                                            (e.part_id, b.partno.clone(), b.description.clone())
                                        })
                                    })
                                    .collect()
                            };
                            app.request_edit_display_cache =
                                Some((app.boms_version, part_ids, built.clone()));
                            built
                        };
                        for (part_id, partno, description) in &display {
                            if let Some(re) = app.request_entries.iter_mut().find(|e| e.part_id == *part_id) {
                                ui.label(partno);
                                ui.label(description);
                                let r = ui.add(egui::DragValue::new(&mut re.quantity).speed(0.5).range(0..=10000));
                                let rev_label = app
                                    .bom_revisions
                                    .get(part_id)
                                    .and_then(|revs| revs.iter().max_by_key(|r| r.revision))
                                    .map(|r| format!("Revision {}", r.revision))
                                    .unwrap_or_else(|| "—".to_string());
                                ui.label(rev_label);
                                if r.changed() {
                                    changed = true;
                                }
                                if ui.small_button("Edit BOM").clicked() {
                                    app.pending_open_bom = Some(*part_id);
                                }
                                if ui.small_button("Remove").clicked() {
                                    to_remove.push(*part_id);
                                }
                                ui.end_row();
                            }
                        }
                    });
            });
            for pid in to_remove {
                app.request_entries.retain(|e| e.part_id != pid);
            }
            if changed || app.request_entries.len() != len_before {
                app.mark_request_dirty();
            }
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Clear List").clicked() {
                app.clear_request();
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
                .map(|b| (b.id, b.partno.clone(), b.description.clone(), b.bom_entry_count))
                .collect();
            if results.is_empty() {
                ui.label("No matching BOMs");
            } else {
                let mut clicked_id = None;
                let available_height = ui.available_rect_before_wrap().height();
                egui::ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .max_height(available_height)
                    .show(ui, |ui| {
                        egui::Grid::new("search_results_header")
                            .num_columns(3)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.strong("Part Number");
                                ui.strong("Description");
                                ui.strong("BOM");
                                ui.end_row();
                            });

                        for (id, partno, description, bom_count) in &results {
                            let label = format!("{}  {}  📋 {}", partno, description, bom_count);
                            let response = ui.selectable_label(false, label);
                            if response.clicked() {
                                clicked_id = Some(*id);
                            }
                        }
                    });
                if let Some(id) = clicked_id {
                    app.add_assembly_to_request(id);
                    app.partno_add_input.clear();
                    app.partno_add_error = None;
                }
            }
        }
    });
}
