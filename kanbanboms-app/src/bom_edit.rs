use crate::app::KanbanBomsApp;
use crate::bom_search::bom_search_ui;
use egui;

const CENTERED_MAX_WIDTH: f32 = 700.0;

pub fn bom_edit_ui(app: &mut KanbanBomsApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let avail = ui.available_rect_before_wrap();
        let width = avail.width().min(CENTERED_MAX_WIDTH);
        let left = avail.left() + (avail.width() - width) / 2.0;
        let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));
        ui.allocate_new_ui(egui::UiBuilder::default().max_rect(rect), |ui| {
        ui.heading("BOM Editor");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Select BOM to edit:");
            let boms: Vec<_> = app.boms.iter().collect();
            let selected_partno = app
                .selected_bom_for_edit
                .and_then(|id| app.boms.iter().find(|b| b.id == id).map(|b| b.partno.clone()))
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("bom_select")
                .selected_text(if selected_partno.is_empty() {
                    "Select...".to_string()
                } else {
                    selected_partno.clone()
                })
                .show_ui(ui, |ui| {
                    for bom in &boms {
                        let text = format!("{} - {}", bom.partno, bom.description);
                        let is_selected = app.selected_bom_for_edit == Some(bom.id);
                        if ui.selectable_label(is_selected, &text).clicked() {
                            app.selected_bom_for_edit = Some(bom.id);
                            app.bom_edit_search_modal = false;
                        }
                    }
                });
        });

        if let Some(bom_id) = app.selected_bom_for_edit {
            let bom = match app.boms.iter().find(|b| b.id == bom_id) {
                Some(b) => b.clone(),
                None => return,
            };
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Assembly:");
                ui.label(&bom.partno);
                ui.label("-");
                ui.label(&bom.description);
            });
            ui.add_space(4.0);

            let entries: Vec<_> = app
                .bom_entries
                .iter()
                .filter(|e| e.bom_id == bom_id)
                .cloned()
                .collect();

            let mut to_remove = None;
            egui::Grid::new("bom_edit_grid")
                .num_columns(5)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("Part");
                    ui.strong("Description");
                    ui.strong("Qty");
                    ui.strong("Disabled");
                    ui.strong("");
                    ui.end_row();

                    for entry in &entries {
                        if let Some(part) = app.boms.iter().find(|b| b.id == entry.part_id) {
                            if let Some(be) = app.bom_entries.iter_mut().find(|e| e.bom_id == bom_id && e.part_id == entry.part_id) {
                                ui.label(&part.partno);
                                ui.label(&part.description);
                                ui.add(egui::DragValue::new(&mut be.quantity).speed(0.5).range(1..=10000));
                                ui.checkbox(&mut be.disabled, "");
                                if ui.small_button("✕").clicked() {
                                    to_remove = Some((bom_id, entry.part_id));
                                }
                                ui.end_row();
                            }
                        }
                    }
                });
            if let Some((bid, pid)) = to_remove {
                app.bom_entries.retain(|e| !(e.bom_id == bid && e.part_id == pid));
            }

            ui.add_space(8.0);
            if ui.button("Add Component").clicked() {
                app.bom_edit_search_modal = true;
            }
        }
        });
    });

    if app.bom_edit_search_modal {
        let mut selected = None;
        egui::Window::new("Add Component to BOM")
            .collapsible(false)
            .resizable(true)
            .show(ctx, |ui| {
                bom_search_ui(ui, &mut app.bom_edit_search_query, &app.boms, &mut selected);
                if let Some(part_id) = selected {
                    if let Some(bom_id) = app.selected_bom_for_edit {
                        if !app.bom_entries.iter().any(|e| e.bom_id == bom_id && e.part_id == part_id) {
                            app.bom_entries.push(crate::models::BomEntry {
                                bom_id,
                                part_id,
                                quantity: 1,
                                disabled: false,
                            });
                        }
                        app.bom_edit_search_modal = false;
                    }
                }
            });
    }
}
