use crate::app::KanbanBomsApp;
use crate::models::get_aggregated_parts;
use chrono::Local;
use egui::{Key, KeyboardShortcut, Modifiers};
use egui_data_table::viewer::{RowCodec, TableColumnConfig, UiActionContext};
use egui_data_table::{Renderer, RowViewer, UiAction};
use egui_data_table::viewer::MoveDirection;
use std::borrow::Cow;
use std::collections::HashSet;

const CENTERED_MAX_WIDTH: f32 = 700.0;

/// Row type for the BOM Preview data table (Bill Of Materials).
#[derive(Debug, Clone)]
pub struct BomPreviewRow {
    pub partno: String,
    pub description: String,
    pub location: String,
    pub quantity: i32,
    pub tags: String,
}

/// Codec for copy-to-clipboard (TSV). Decode not used (read-only table).
struct BomPreviewCodec;

impl RowCodec<BomPreviewRow> for BomPreviewCodec {
    type DeserializeError = ();

    fn create_empty_decoded_row(&mut self) -> BomPreviewRow {
        BomPreviewRow {
            partno: String::new(),
            description: String::new(),
            location: String::new(),
            quantity: 0,
            tags: String::new(),
        }
    }

    fn encode_column(&mut self, src_row: &BomPreviewRow, column: usize, dst: &mut String) {
        match column {
            0 => dst.push_str(&src_row.partno),
            1 => dst.push_str(&src_row.description),
            2 => dst.push_str(&src_row.location),
            3 => dst.push_str(&src_row.quantity.to_string()),
            4 => dst.push_str(&src_row.tags),
            _ => {}
        }
    }

    fn decode_column(
        &mut self,
        _src_data: &str,
        _column: usize,
        _dst_row: &mut BomPreviewRow,
    ) -> Result<(), egui_data_table::viewer::DecodeErrorBehavior> {
        Ok(())
    }
}

struct BomPreviewViewer;

impl RowViewer<BomPreviewRow> for BomPreviewViewer {
    fn num_columns(&mut self) -> usize {
        5
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => Cow::Borrowed("Part Number"),
            1 => Cow::Borrowed("Description"),
            2 => Cow::Borrowed("Location"),
            3 => Cow::Borrowed("Quantity"),
            4 => Cow::Borrowed("Tags"),
            _ => Cow::Borrowed(""),
        }
    }

    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        match column {
            1 => TableColumnConfig::initial(200.0).resizable(true),
            _ => TableColumnConfig::auto().resizable(true),
        }
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        column < 5
    }

    fn compare_cell(&self, row_a: &BomPreviewRow, row_b: &BomPreviewRow, column: usize) -> std::cmp::Ordering {
        match column {
            0 => row_a.partno.cmp(&row_b.partno),
            1 => row_a.description.cmp(&row_b.description),
            2 => row_a.location.cmp(&row_b.location),
            3 => row_a.quantity.cmp(&row_b.quantity),
            4 => row_a.tags.cmp(&row_b.tags),
            _ => std::cmp::Ordering::Equal,
        }
    }

    fn try_create_codec(&mut self, is_encoding: bool) -> Option<impl RowCodec<BomPreviewRow>> {
        if is_encoding {
            Some(BomPreviewCodec)
        } else {
            None
        }
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &BomPreviewRow, column: usize) {
        match column {
            0 => {
                ui.label(&row.partno);
            }
            1 => {
                ui.label(&row.description);
            }
            2 => {
                let loc = if row.location.is_empty() { "N/A" } else { row.location.as_str() };
                ui.label(loc);
            }
            3 => {
                ui.label(row.quantity.to_string());
            }
            4 => {
                ui.label(if row.tags.is_empty() { "—" } else { &row.tags });
            }
            _ => {}
        }
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut BomPreviewRow,
        column: usize,
    ) -> Option<egui::Response> {
        match column {
            0 => Some(ui.label(&row.partno)),
            1 => Some(ui.label(&row.description)),
            2 => Some(ui.label(if row.location.is_empty() { "N/A" } else { &row.location })),
            3 => Some(ui.label(row.quantity.to_string())),
            4 => Some(ui.label(if row.tags.is_empty() { "—" } else { &row.tags })),
            _ => None,
        }
    }

    fn set_cell_value(&mut self, src: &BomPreviewRow, dst: &mut BomPreviewRow, column: usize) {
        match column {
            0 => dst.partno = src.partno.clone(),
            1 => dst.description = src.description.clone(),
            2 => dst.location = src.location.clone(),
            3 => dst.quantity = src.quantity,
            4 => dst.tags = src.tags.clone(),
            _ => {}
        }
    }

    fn new_empty_row(&mut self) -> BomPreviewRow {
        BomPreviewRow {
            partno: String::new(),
            description: String::new(),
            location: String::new(),
            quantity: 0,
            tags: String::new(),
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &BomPreviewRow) -> bool {
        false
    }

    fn hotkeys(&mut self, context: &UiActionContext) -> Vec<(KeyboardShortcut, UiAction)> {
        if context.cursor.is_editing() {
            return Vec::new();
        }
        let none = Modifiers::NONE;
        let ctrl = Modifiers::CTRL;
        type MD = MoveDirection;
        vec![
            (KeyboardShortcut::new(ctrl, Key::C), UiAction::CopySelection),
            (KeyboardShortcut::new(none, Key::ArrowUp), UiAction::MoveSelection(MD::Up)),
            (KeyboardShortcut::new(none, Key::ArrowDown), UiAction::MoveSelection(MD::Down)),
            (KeyboardShortcut::new(none, Key::ArrowLeft), UiAction::MoveSelection(MD::Left)),
            (KeyboardShortcut::new(none, Key::ArrowRight), UiAction::MoveSelection(MD::Right)),
            (KeyboardShortcut::new(ctrl, Key::A), UiAction::SelectAll),
            (KeyboardShortcut::new(none, Key::PageUp), UiAction::NavPageUp),
            (KeyboardShortcut::new(none, Key::PageDown), UiAction::NavPageDown),
            (KeyboardShortcut::new(none, Key::Home), UiAction::NavTop),
            (KeyboardShortcut::new(none, Key::End), UiAction::NavBottom),
        ]
    }

    fn trivial_config(&mut self) -> egui_data_table::viewer::TrivialConfig {
        egui_data_table::viewer::TrivialConfig {
            table_row_height: Some(22.0),
            max_undo_history: 0,
            max_scroll_height: None,
        }
    }

    fn allowed_context_menu_actions(&self) -> Option<HashSet<UiAction>> {
        Some([UiAction::CopySelection].into_iter().collect())
    }
}

pub fn bom_preview_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui) {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
    let left = avail.left() + (avail.width() - width) / 2.0;
    let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
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
            return;
        }

        let date = Local::now().format("%m/%d/%Y").to_string();
        ui.label(format!("Date: {}", date));
        ui.add_space(8.0);

        ui.strong("Assemblies");
        ui.add_space(4.0);
        let request_id = app.current_request_id.unwrap_or_default();
        egui::ScrollArea::horizontal()
            .id_salt(("assemblies_scroll", request_id))
            .show(ui, |ui| {
            egui::Grid::new(("assemblies_grid", request_id))
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
        let bom_entries = app.effective_bom_entries_for_request(request_id);
        let parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &bom_entries,
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
        let filtered_parts: Vec<_> = parts
            .iter()
            .filter(|(_, _, _, _, tags)| {
                tags.is_empty()
                    || tags.iter().any(|t| app.preview_tag_visible.get(t).copied().unwrap_or(true))
            })
            .cloned()
            .collect();

        // Sync table rows when data changes
        let sync_key = (
            request_id,
            parts.len(),
            filtered_parts.len(),
        );
        if app.bom_preview_last_sync_key != Some(sync_key) {
            let rows: Vec<BomPreviewRow> = filtered_parts
                .iter()
                .map(|(partno, desc, qty, loc, tags)| BomPreviewRow {
                    partno: partno.clone(),
                    description: desc.clone(),
                    location: loc.clone(),
                    quantity: *qty,
                    tags: tags.join(" "),
                })
                .collect();
            app.bom_preview_table.replace(rows);
            app.bom_preview_last_sync_key = Some(sync_key);
        }

        ui.strong("Bill Of Materials");
        ui.add_space(4.0);
        let table_area_height = ui.available_rect_before_wrap().height();
        let mut viewer = BomPreviewViewer;
        ui.add(
            Renderer::new(&mut app.bom_preview_table, &mut viewer)
                .with_table_row_height(22.0)
                .with_max_scroll_height(table_area_height.max(100.0)),
        );

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Print Preview").clicked() {
                app.trigger_print_preview();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button("Export PDF").clicked() {
                app.trigger_pdf_download();
            }
        });
    });
}
