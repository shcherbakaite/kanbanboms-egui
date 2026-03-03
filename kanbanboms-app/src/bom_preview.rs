use crate::app::KanbanBomsApp;
use crate::models::{get_aggregated_parts, AggregatedPart};
use chrono::Local;
use egui::{Key, KeyboardShortcut, Modifiers};
use egui_data_table::viewer::{RowCodec, TableColumnConfig, UiActionContext};
use egui_data_table::{Renderer, RowViewer, UiAction};
use egui_data_table::viewer::MoveDirection;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

const CENTERED_MAX_WIDTH: f32 = 700.0;

/// Default meta fields to show when none exist in data.
const DEFAULT_META_FIELDS: &[&str] = &["Location", "MPN", "Lead time"];

/// Collects meta field names from aggregated parts, merged with defaults, sorted.
pub fn meta_field_names_from_parts(parts: &[AggregatedPart]) -> Vec<String> {
    let mut names: HashSet<String> = DEFAULT_META_FIELDS.iter().map(|s| (*s).to_string()).collect();
    for (_, _, _, _, custom_fields, _) in parts {
        names.extend(custom_fields.keys().cloned());
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Column names in table order: Part Number, Description, [meta...], UOM, Qty, Tags.
pub fn bom_preview_column_names(meta_field_names: &[String]) -> Vec<String> {
    let mut names = vec!["Part Number".to_string(), "Description".to_string()];
    names.extend(meta_field_names.iter().cloned());
    names.push("UOM".to_string());
    names.push("Qty".to_string());
    names.push("Tags".to_string());
    names
}

/// Row type for the BOM Preview data table (Bill Of Materials).
#[derive(Debug, Clone)]
pub struct BomPreviewRow {
    pub partno: String,
    pub description: String,
    pub custom_fields: HashMap<String, String>,
    pub uom: String,
    pub quantity: i32,
    pub tags: String,
}

/// Codec for copy-to-clipboard (TSV). Decode not used (read-only table).
struct BomPreviewCodec {
    meta_field_names: Vec<String>,
}

impl RowCodec<BomPreviewRow> for BomPreviewCodec {
    type DeserializeError = ();

    fn create_empty_decoded_row(&mut self) -> BomPreviewRow {
        BomPreviewRow {
            partno: String::new(),
            description: String::new(),
            custom_fields: HashMap::new(),
            uom: String::new(),
            quantity: 0,
            tags: String::new(),
        }
    }

    fn encode_column(&mut self, src_row: &BomPreviewRow, column: usize, dst: &mut String) {
        let n_meta = self.meta_field_names.len();
        match column {
            0 => dst.push_str(&src_row.partno),
            1 => dst.push_str(&src_row.description),
            i if i >= 2 && i < 2 + n_meta => {
                let name = &self.meta_field_names[i - 2];
                dst.push_str(src_row.custom_fields.get(name).map(|s| s.as_str()).unwrap_or(""));
            }
            i if i == 2 + n_meta => dst.push_str(if src_row.uom.is_empty() { "EA" } else { &src_row.uom }),
            i if i == 3 + n_meta => dst.push_str(&src_row.quantity.to_string()),
            i if i == 4 + n_meta => dst.push_str(&src_row.tags),
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

pub struct BomPreviewViewer {
    pub meta_field_names: Vec<String>,
    /// partno -> part_id for context menu actions
    pub part_ids_by_partno: HashMap<String, Uuid>,
    /// When user picks a custom action, set by the grid; consumed after draw.
    pub pending_custom_action: Option<(usize, String)>,
}

impl BomPreviewViewer {
    fn num_cols(&self) -> usize {
        5 + self.meta_field_names.len()
    }

    fn column_name_at(&self, column: usize) -> Cow<'static, str> {
        let n_meta = self.meta_field_names.len();
        match column {
            0 => Cow::Borrowed("Part Number"),
            1 => Cow::Borrowed("Description"),
            i if i >= 2 && i < 2 + n_meta => {
                Cow::Owned(self.meta_field_names.get(i - 2).cloned().unwrap_or_default())
            }
            i if i == 2 + n_meta => Cow::Borrowed("UOM"),
            i if i == 3 + n_meta => Cow::Borrowed("Qty"),
            i if i == 4 + n_meta => Cow::Borrowed("Tags"),
            _ => Cow::Borrowed(""),
        }
    }

    fn get_cell_value(&self, row: &BomPreviewRow, column: usize) -> String {
        let n_meta = self.meta_field_names.len();
        match column {
            0 => row.partno.clone(),
            1 => row.description.clone(),
            i if i >= 2 && i < 2 + n_meta => {
                let name = self.meta_field_names.get(i - 2).map(|s| s.as_str()).unwrap_or("");
                row.custom_fields.get(name).cloned().unwrap_or_default()
            }
            i if i == 2 + n_meta => if row.uom.is_empty() { "EA".to_string() } else { row.uom.clone() },
            i if i == 3 + n_meta => row.quantity.to_string(),
            i if i == 4 + n_meta => row.tags.clone(),
            _ => String::new(),
        }
    }
}

impl RowViewer<BomPreviewRow> for BomPreviewViewer {
    fn num_columns(&mut self) -> usize {
        self.num_cols()
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        self.column_name_at(column)
    }

    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        if column == 1 {
            TableColumnConfig::initial(200.0).resizable(true)
        } else {
            TableColumnConfig::auto().resizable(true)
        }
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        column < self.num_cols()
    }

    fn compare_cell(&self, row_a: &BomPreviewRow, row_b: &BomPreviewRow, column: usize) -> std::cmp::Ordering {
        let va = self.get_cell_value(row_a, column);
        let vb = self.get_cell_value(row_b, column);
        va.cmp(&vb)
    }

    fn try_create_codec(&mut self, is_encoding: bool) -> Option<impl RowCodec<BomPreviewRow>> {
        if is_encoding {
            Some(BomPreviewCodec {
                meta_field_names: self.meta_field_names.clone(),
            })
        } else {
            None
        }
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &BomPreviewRow, column: usize) {
        let val = self.get_cell_value(row, column);
        let n_meta = self.meta_field_names.len();
        let display = if column >= 2 && column < 2 + n_meta && val.is_empty() {
            "N/A"
        } else if column == 4 + n_meta && val.is_empty() {
            "—"
        } else {
            val.as_str()
        };
        ui.label(display);
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut BomPreviewRow,
        column: usize,
    ) -> Option<egui::Response> {
        Some(ui.label(self.get_cell_value(row, column).as_str()))
    }

    fn set_cell_value(&mut self, src: &BomPreviewRow, dst: &mut BomPreviewRow, column: usize) {
        let n_meta = self.meta_field_names.len();
        match column {
            0 => dst.partno = src.partno.clone(),
            1 => dst.description = src.description.clone(),
            i if i >= 2 && i < 2 + n_meta => {
                if let Some(name) = self.meta_field_names.get(i - 2) {
                    dst.custom_fields.insert(name.clone(), src.custom_fields.get(name).cloned().unwrap_or_default());
                }
            }
            i if i == 2 + n_meta => dst.uom = src.uom.clone(),
            i if i == 3 + n_meta => dst.quantity = src.quantity,
            i if i == 4 + n_meta => dst.tags = src.tags.clone(),
            _ => {}
        }
    }

    fn new_empty_row(&mut self) -> BomPreviewRow {
        BomPreviewRow {
            partno: String::new(),
            description: String::new(),
            custom_fields: HashMap::new(),
            uom: String::new(),
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
            read_only: false,
        }
    }

    fn persist_ui_state(&self) -> bool {
        true
    }

    fn allowed_context_menu_actions(&self) -> Option<HashSet<UiAction>> {
        Some([UiAction::CopySelection].into_iter().collect())
    }

    fn custom_context_menu_items(&self, row: &BomPreviewRow) -> Vec<(Cow<'_, str>, String)> {
        if let Some(&pid) = self.part_ids_by_partno.get(&row.partno) {
            let pid_str = pid.to_string();
            vec![
                (Cow::Borrowed("Edit part"), format!("edit_part:{}", pid_str)),
                (Cow::Borrowed("Edit BOM"), format!("edit_bom:{}", pid_str)),
                (Cow::Borrowed("Usage report"), format!("usage_report:{}", pid_str)),
            ]
        } else {
            vec![]
        }
    }

    fn custom_action_sink(&mut self) -> Option<&mut Option<(usize, String)>> {
        Some(&mut self.pending_custom_action)
    }
}

pub fn bom_preview_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui) {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
    let left = avail.left() + (avail.width() - width) / 2.0;
    let rect = egui::Rect::from_min_size(egui::pos2(left, avail.top()), egui::vec2(width, avail.height()));

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let inner = ui.available_rect_before_wrap();
        egui::ScrollArea::vertical()
            .max_height((inner.height() - 52.0).max(100.0))
            .id_salt("bom_preview_scroll")
            .show(ui, |ui| {
        ui.heading("Kitting BOM - Preview");
        ui.add_space(8.0);

        let assemblies: Vec<_> = app.request_entries.iter().cloned().collect();

        if assemblies.is_empty() {
            ui.label("Add assemblies to the request to see the BOM preview.");
            ui.add_space(8.0);
            return;
        }

        // Only rebuild BOM when Preview is active; use cache when data unchanged.
        // Defer heavy build by one frame so the Preview mode switch paints immediately (avoids missed clicks).
        let cache_key = app.data_version;
        let cache_hit = app.bom_preview_parts_cache.as_ref().map(|(k0, _, _)| *k0) == Some(cache_key);
        let deferred = app.bom_preview_deferred_build.is_some();

        if !cache_hit && !deferred {
            // First frame after switching to Preview with cache miss: show loading, defer build to next frame
            app.bom_preview_deferred_build = Some(());
            ui.label("Building BOM…");
            ui.ctx().request_repaint();
            return;
        }

        let (bom_entries, parts): (Vec<_>, Vec<_>) = if cache_hit {
            let cached = app.bom_preview_parts_cache.as_ref().unwrap();
            (cached.1.clone(), cached.2.clone())
        } else {
            app.bom_preview_deferred_build = None;
            let bom_entries = app.effective_bom_entries_for_request();
            let boms_map = app.boms_by_id();
            let p = get_aggregated_parts(&boms_map, &bom_entries, &app.request_entries);
            app.bom_preview_parts_cache = Some((cache_key, bom_entries.clone(), p.clone()));
            (bom_entries, p)
        };

        let date = Local::now().format("%m/%d/%Y").to_string();
        ui.label(format!("Date: {}", date));
        ui.add_space(8.0);

        ui.strong("Assemblies");
        ui.add_space(4.0);
        egui::ScrollArea::horizontal()
            .id_salt("assemblies_scroll")
            .show(ui, |ui| {
            egui::Grid::new("assemblies_grid")
                .num_columns(4)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.strong("Qty");
                    ui.strong("Part Number");
                    ui.strong("Description");
                    ui.strong("Tags");
                    ui.end_row();
                    let boms_map = app.boms_by_id_ref();
                    for ae in &assemblies {
                        if let Some(bom) = boms_map.get(&ae.part_id) {
                            let assembly_tags: std::collections::HashSet<String> = bom_entries
                                .iter()
                                .filter(|be| be.bom_id == ae.part_id)
                                .flat_map(|be| be.tags.iter().cloned())
                                .collect();
                            let tags_str: String = {
                                let mut v: Vec<_> = assembly_tags.into_iter().collect();
                                v.sort();
                                v.join(", ")
                            };
                            ui.label(ae.quantity.to_string());
                            ui.label(&bom.partno);
                            ui.label(&bom.description);
                            ui.label(if tags_str.is_empty() { "—" } else { &tags_str });
                            ui.end_row();
                        }
                    }
                });
        });
        ui.add_space(12.0);

        // Meta field names for table columns (Location, MPN, Lead time, etc.)
        let meta_field_names = meta_field_names_from_parts(&parts);
        let column_names = bom_preview_column_names(&meta_field_names);

        // Tag filters after assemblies (always show section)
        let all_tags: std::collections::HashSet<String> = parts
            .iter()
            .flat_map(|(_, _, _, _, _, tags)| tags.iter().cloned())
            .collect();
        for tag in &all_tags {
            app.preview_tag_visible
                .entry(tag.clone())
                .or_insert(true);
        }
        ui.strong("Filter by tags:");
        if all_tags.is_empty() {
            ui.label("No tags in this BOM");
        } else {
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
        }
        ui.add_space(8.0);

        // Filter parts: show if part has no tags, or has at least one visible tag
        let filtered_parts: Vec<_> = parts
            .iter()
            .filter(|(_, _, _, _, _, tags)| {
                tags.is_empty()
                    || tags.iter().any(|t| app.preview_tag_visible.get(t).copied().unwrap_or(true))
            })
            .cloned()
            .collect();

        // Sync table rows when data changes (data_version ensures other-view saves trigger refresh).
        // Include meta_field_names in key so we rebuild when meta fields change (column count changes).
        let meta_sig = meta_field_names.join("|");
        let total_qty: i32 = filtered_parts.iter().map(|(_, _, q, _, _, _)| *q).sum();
        let sync_key = (
            app.data_version,
            parts.len(),
            filtered_parts.len(),
            total_qty,
            meta_sig.clone(),
        );
        if app.bom_preview_last_sync_key.as_ref() != Some(&sync_key) {
            let rows: Vec<BomPreviewRow> = filtered_parts
                .iter()
                .map(|(partno, desc, qty, uom, custom_fields, tags)| BomPreviewRow {
                    partno: partno.clone(),
                    description: desc.clone(),
                    custom_fields: custom_fields.clone(),
                    uom: uom.clone(),
                    quantity: *qty,
                    tags: tags.join(" "),
                })
                .collect();
            app.bom_preview_table.replace_keeping_ui(rows);
            app.bom_preview_last_sync_key = Some(sync_key);
        }

        ui.strong("Include in print (HTML):");
        ui.horizontal_wrapped(|ui| {
            for name in &column_names {
                let v = app.bom_preview_columns_in_html.entry(name.clone()).or_insert(true);
                if ui.checkbox(v, name).changed() {
                    // Toggled
                }
            }
        });
        ui.add_space(4.0);

        ui.strong("Bill Of Materials");
        ui.label("Drag column headers to reorder; order is reflected in HTML export.");
        ui.add_space(4.0);
        let table_area_height = ui.available_rect_before_wrap().height();
        let part_ids_by_partno: HashMap<String, Uuid> = app
            .boms
            .iter()
            .map(|b| (b.partno.clone(), b.id))
            .collect();
        let mut viewer = BomPreviewViewer {
            meta_field_names: meta_field_names.clone(),
            part_ids_by_partno,
            pending_custom_action: None,
        };
        ui.add(
            Renderer::new(&mut app.bom_preview_table, &mut viewer)
                .with_table_row_height(22.0)
                .with_max_scroll_height(table_area_height.max(100.0)),
        );

        if let Some((_row_idx, id)) = viewer.pending_custom_action.take() {
            let (action, uuid_str) = id.split_once(':').unwrap_or((id.as_str(), ""));
            if let Ok(pid) = Uuid::parse_str(uuid_str) {
                match action {
                    "edit_part" => app.part_master_edit_part = Some(Some(pid)),
                    "edit_bom" => app.pending_open_bom = Some(pid),
                    "usage_report" => app.part_master_usage_report = Some(pid),
                    _ => {}
                }
            }
        }
        });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Print Preview").clicked() {
                app.trigger_print_preview();
            }
        });
    });
}
