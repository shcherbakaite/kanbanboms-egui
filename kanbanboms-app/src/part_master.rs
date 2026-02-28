use crate::app::KanbanBomsApp;
use crate::models::{part_category, Bom};
use egui::{Key, KeyboardShortcut, Modifiers};
use egui_data_table::viewer::{
    DecodeErrorBehavior, MoveDirection, RowCodec, TableColumnConfig, UiActionContext,
};
use egui_data_table::{ColumnHeaderMenuAction, Renderer, RowViewer, UiAction};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Row type for the Part Master data table.
#[derive(Debug, Clone)]
pub struct PartMasterRow {
    pub part_id: Uuid,
    pub partno: String,
    pub description: String,
    pub has_bom: bool,
    /// Custom field values for this part (key -> value).
    pub custom_fields: HashMap<String, String>,
}

/// Index into visible categories for the selected worksheet tab.
fn selected_tab_index(app: &KanbanBomsApp) -> usize {
    let len = app.part_master_visible_categories.len();
    app.part_master_tab.min(len.saturating_sub(1))
}

fn parts_in_category<'a>(boms: &'a [Bom], category: &str) -> Vec<&'a Bom> {
    let mut out: Vec<&Bom> = if category == "All" {
        boms.iter().collect()
    } else {
        boms
            .iter()
            .filter(|b| part_category(&b.partno) == category)
            .collect()
    };
    out.sort_by(|a, b| a.partno.cmp(&b.partno));
    out
}

fn part_has_bom(app: &KanbanBomsApp, part_id: Uuid) -> bool {
    app.bom_entries.iter().any(|e| e.bom_id == part_id)
}

const CENTERED_MAX_WIDTH: f32 = 700.0;
const TAB_BAR_HEIGHT: f32 = 36.0;

fn matches_filter(part: &Bom, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let f = filter.to_lowercase();
    part.partno.to_lowercase().contains(&f) || part.description.to_lowercase().contains(&f)
}

/// Collects all unique custom field names from all BOMs, sorted.
fn all_custom_field_names(boms: &[Bom]) -> Vec<String> {
    let mut names: HashSet<String> = HashSet::new();
    for b in boms {
        names.extend(b.custom_fields.keys().cloned());
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Builds the current list of part rows for the selected category and filter.
fn part_rows_for_table(app: &KanbanBomsApp) -> Vec<PartMasterRow> {
    let categories = &app.part_master_visible_categories;
    let category = categories
        .get(selected_tab_index(app))
        .map(|s| s.as_str())
        .unwrap_or("All");
    parts_in_category(&app.boms, category)
        .into_iter()
        .filter(|p| matches_filter(p, &app.part_master_filter))
        .map(|p| PartMasterRow {
            part_id: p.id,
            partno: p.partno.clone(),
            description: p.description.clone(),
            has_bom: part_has_bom(app, p.id),
            custom_fields: p.custom_fields.clone(),
        })
        .collect()
}

/// Sync key to avoid replacing table rows every frame (preserves selection/scroll).
fn part_master_sync_key(app: &KanbanBomsApp) -> (usize, String, usize, String) {
    let custom_sig = all_custom_field_names(&app.boms).join("|");
    (
        app.part_master_tab,
        app.part_master_filter.clone(),
        app.boms.len(),
        custom_sig,
    )
}

/// Codec for copy-to-clipboard (TSV). Decode not used (read-only table).
pub struct PartMasterCodec {
    pub custom_field_names: Vec<String>,
}

impl RowCodec<PartMasterRow> for PartMasterCodec {
    type DeserializeError = ();

    fn create_empty_decoded_row(&mut self) -> PartMasterRow {
        PartMasterRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            has_bom: false,
            custom_fields: HashMap::new(),
        }
    }

    fn encode_column(&mut self, src_row: &PartMasterRow, column: usize, dst: &mut String) {
        match column {
            0 => dst.push_str(&src_row.partno),
            1 => dst.push_str(&src_row.description),
            2 => dst.push_str(if src_row.has_bom { "Yes" } else { "No" }),
            i if i >= 3 && i - 3 < self.custom_field_names.len() => {
                let name = &self.custom_field_names[i - 3];
                dst.push_str(src_row.custom_fields.get(name).map(|s| s.as_str()).unwrap_or(""));
            }
            _ => {}
        }
    }

    fn decode_column(
        &mut self,
        _src_data: &str,
        _column: usize,
        _dst_row: &mut PartMasterRow,
    ) -> Result<(), DecodeErrorBehavior> {
        Ok(())
    }
}

pub struct PartMasterViewer {
    /// When user picks a custom action (e.g. "Edit BOM"), set by the grid; consumed after draw.
    pub pending_custom_action: Option<(usize, String)>,
    /// Ordered list of custom field names (columns 3, 4, 5, ...).
    pub custom_field_names: Vec<String>,
}

impl PartMasterViewer {
    pub fn new(custom_field_names: Vec<String>) -> Self {
        Self {
            pending_custom_action: None,
            custom_field_names,
        }
    }
}

impl Default for PartMasterViewer {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl RowViewer<PartMasterRow> for PartMasterViewer {
    fn num_columns(&mut self) -> usize {
        3 + self.custom_field_names.len()
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => Cow::Borrowed("Part Number"),
            1 => Cow::Borrowed("Description"),
            2 => Cow::Borrowed("BOM"),
            i if i >= 3 && i - 3 < self.custom_field_names.len() => {
                Cow::Owned(self.custom_field_names[i - 3].clone())
            }
            _ => Cow::Borrowed(""),
        }
    }

    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        match column {
            1 => TableColumnConfig::initial(280.0).resizable(true),
            _ => TableColumnConfig::auto().resizable(true),
        }
    }

    fn try_create_codec(&mut self, is_encoding: bool) -> Option<impl RowCodec<PartMasterRow>> {
        if is_encoding {
            Some(PartMasterCodec {
                custom_field_names: self.custom_field_names.clone(),
            })
        } else {
            None
        }
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &PartMasterRow, column: usize) {
        match column {
            0 => {
                ui.label(&row.partno);
            }
            1 => {
                ui.label(&row.description);
            }
            2 => {
                if row.has_bom {
                    ui.label(egui::RichText::new("📋").size(16.0));
                } else {
                    ui.label(egui::RichText::new("—").color(egui::Color32::GRAY));
                }
            }
            i if i >= 3 && i - 3 < self.custom_field_names.len() => {
                let name = &self.custom_field_names[i - 3];
                let val = row.custom_fields.get(name).map(|s| s.as_str()).unwrap_or("");
                ui.label(val);
            }
            _ => {}
        }
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut PartMasterRow,
        column: usize,
    ) -> Option<egui::Response> {
        // Read-only: show same as view
        match column {
            0 => Some(ui.label(&row.partno)),
            1 => Some(ui.label(&row.description)),
            2 => Some(ui.label(
                if row.has_bom {
                    egui::RichText::new("📋").size(16.0)
                } else {
                    egui::RichText::new("—").color(egui::Color32::GRAY)
                },
            )),
            i if i >= 3 && i - 3 < self.custom_field_names.len() => {
                let name = &self.custom_field_names[i - 3];
                let val = row.custom_fields.get(name).map(|s| s.as_str()).unwrap_or("");
                Some(ui.label(val))
            }
            _ => None,
        }
    }

    fn set_cell_value(&mut self, src: &PartMasterRow, dst: &mut PartMasterRow, column: usize) {
        match column {
            0 => dst.partno = src.partno.clone(),
            1 => dst.description = src.description.clone(),
            2 => dst.has_bom = src.has_bom,
            i if i >= 3 && i - 3 < self.custom_field_names.len() => {
                let name = &self.custom_field_names[i - 3];
                let val = src.custom_fields.get(name).cloned().unwrap_or_default();
                dst.custom_fields.insert(name.clone(), val);
            }
            _ => {}
        }
    }

    fn new_empty_row(&mut self) -> PartMasterRow {
        PartMasterRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            has_bom: false,
            custom_fields: HashMap::new(),
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &PartMasterRow) -> bool {
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

    /// Part Master is read-only: only Copy is allowed; hide and disable Cut, Delete, Duplicate, Clear.
    fn allowed_context_menu_actions(&self) -> Option<HashSet<UiAction>> {
        Some([UiAction::CopySelection].into_iter().collect())
    }

    fn custom_context_menu_items(&self) -> Vec<(Cow<'_, str>, String)> {
        vec![
            (Cow::Borrowed("Add to request"), "add_to_request".to_string()),
            (Cow::Borrowed("Edit part"), "edit_part".to_string()),
            (Cow::Borrowed("Edit BOM"), "edit_bom".to_string()),
        ]
    }

    fn custom_action_sink(&mut self) -> Option<&mut Option<(usize, String)>> {
        Some(&mut self.pending_custom_action)
    }

    fn column_header_context_menu_extras(
        &mut self,
        ui: &mut egui::Ui,
        num_columns: usize,
        vis_col_indices: &[usize],
        insert_at: usize,
        push: &mut dyn FnMut(ColumnHeaderMenuAction),
    ) {
        if self.custom_field_names.is_empty() {
            return;
        }
        ui.separator();
        ui.label("Custom fields");
        for (i, name) in self.custom_field_names.iter().enumerate() {
            let col = 3 + i;
            if col >= num_columns {
                break;
            }
            let is_visible = vis_col_indices.contains(&col);
            let mut checked = is_visible;
            if ui.checkbox(&mut checked, name).changed() {
                if checked {
                    push(ColumnHeaderMenuAction::ShowColumn {
                        column: col,
                        at: insert_at,
                    });
                } else {
                    push(ColumnHeaderMenuAction::HideColumn(col));
                }
                ui.close();
            }
        }
    }
}

/// UI for the docked Part Edit tab (New part / Edit part).
pub fn part_edit_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui, tab_id: Uuid) {
    use crate::dock::PartEditState;

    // Resolve bom_id: existing part = tab_id, new part after save = part_edit_new_to_bom[tab_id]
    let bom_id = if app.boms.iter().any(|b| b.id == tab_id) {
        tab_id
    } else {
        app.part_edit_new_to_bom.get(&tab_id).copied().unwrap_or(tab_id)
    };
    let is_new = !app.boms.iter().any(|b| b.id == tab_id)
        && !app.part_edit_new_to_bom.contains_key(&tab_id);

    // Get or create per-tab state
    let state = app.part_edit_states.entry(tab_id).or_insert_with(|| {
        if let Some(bom) = app.boms.iter().find(|b| b.id == bom_id) {
            PartEditState {
                partno: bom.partno.clone(),
                description: bom.description.clone(),
                custom_fields: bom
                    .custom_fields
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                error: None,
            }
        } else {
            PartEditState::default()
        }
    });

    if let Some(err) = &state.error {
        ui.colored_label(egui::Color32::RED, err);
    }
    ui.label("Part number:");
    ui.add(
        egui::TextEdit::singleline(&mut state.partno)
            .desired_width(280.0)
            .hint_text("e.g. 20002-GH-002"),
    );
    ui.add_space(4.0);
    ui.label("Description:");
    ui.add(
        egui::TextEdit::singleline(&mut state.description)
            .desired_width(280.0)
            .hint_text("Part description"),
    );
    ui.add_space(8.0);

    // Edit BOM button (only when editing existing part)
    if app.boms.iter().any(|b| b.id == bom_id) {
        if ui.button("Edit BOM").clicked() {
            app.pending_open_bom = Some(bom_id);
        }
        ui.add_space(8.0);
    }

    // Custom fields
    ui.strong("Custom fields");
    ui.add_space(4.0);
    let mut to_remove = None;
    for (i, (k, v)) in state.custom_fields.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(k)
                    .desired_width(120.0)
                    .hint_text("Field name"),
            );
            ui.add(
                egui::TextEdit::singleline(v)
                    .desired_width(180.0)
                    .hint_text("Value"),
            );
            if ui.small_button("✕").clicked() {
                to_remove = Some(i);
            }
        });
    }
    if let Some(i) = to_remove {
        state.custom_fields.remove(i);
    }
    if ui.button("+ Add field").clicked() {
        state.custom_fields.push((String::new(), String::new()));
    }
    ui.add_space(12.0);

    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            let partno = state.partno.trim().to_string();
            let description = state.description.trim().to_string();
            let custom_fields: std::collections::HashMap<String, String> = state
                .custom_fields
                .iter()
                .filter(|(k, _)| !k.trim().is_empty())
                .map(|(k, v)| (k.trim().to_string(), v.clone()))
                .collect();
            if partno.is_empty() {
                state.error = Some("Part number is required".to_string());
            } else if is_new && app.boms.iter().any(|b| b.partno == partno) {
                state.error = Some("Part number already exists".to_string());
            } else if app.boms.iter().any(|b| b.id == bom_id) {
                let other_has = app.boms.iter().any(|b| b.id != bom_id && b.partno == partno);
                if other_has {
                    state.error = Some("Part number already exists".to_string());
                } else if let Some(bom) = app.boms.iter_mut().find(|b| b.id == bom_id) {
                    bom.partno = partno;
                    bom.description = description;
                    bom.custom_fields = custom_fields;
                    state.error = None;
                    app.part_master_last_sync_key = None;
                }
            } else {
                let partno_norm = crate::models::normalize_partno(&partno);
                let final_partno = if partno_norm.is_empty() {
                    partno
                } else {
                    partno_norm
                };
                let new_id = Uuid::new_v4();
                app.boms.push(crate::models::Bom {
                    id: new_id,
                    partno: final_partno,
                    description,
                    batch_quantity: 0,
                    location: String::new(),
                    custom_fields,
                });
                app.part_edit_new_to_bom.insert(tab_id, new_id);
                state.error = None;
                app.part_master_last_sync_key = None;
            }
        }
        if ui.button("Cancel").clicked() {
            app.part_master_close_edit_tab = Some(tab_id);
        }
    });
}

/// Ensure "All" is always present in visible categories.
fn ensure_all_category(app: &mut KanbanBomsApp) {
    if !app.part_master_visible_categories.iter().any(|c| c == "All") {
        app.part_master_visible_categories.insert(0, "All".to_string());
    }
}

pub fn part_master_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui) {
    ensure_all_category(app);
    let ctx = ui.ctx();
    if app.part_master_add_category_open {
        let mut open = true;
        egui::Window::new("Add Category")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Category (e.g. TR, EA, SD):");
                ui.add(
                    egui::TextEdit::singleline(&mut app.part_master_add_category_input)
                        .desired_width(180.0)
                        .hint_text("TR"),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        let mut cat = app.part_master_add_category_input.trim().to_uppercase();
                        if cat == "ALL" {
                            cat = "All".to_string();
                        }
                        if !cat.is_empty()
                            && !app.part_master_visible_categories.iter().any(|c| c == &cat)
                        {
                            app.part_master_visible_categories.push(cat);
                        }
                        app.part_master_add_category_open = false;
                        app.part_master_add_category_input.clear();
                    }
                    if ui.button("Cancel").clicked() {
                        app.part_master_add_category_open = false;
                        app.part_master_add_category_input.clear();
                    }
                });
            });
        if !open {
            app.part_master_add_category_open = false;
            app.part_master_add_category_input.clear();
        }
    }
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
        let left = avail.left() + (avail.width() - width) / 2.0;
        let content_height = (avail.bottom() - TAB_BAR_HEIGHT - avail.top()).max(0.0);
        let main_rect = egui::Rect::from_min_size(
            egui::pos2(left, avail.top()),
            egui::vec2(width, content_height),
        );
        let tab_rect = egui::Rect::from_min_size(
            egui::pos2(avail.left(), avail.bottom() - TAB_BAR_HEIGHT),
            egui::vec2(avail.width(), TAB_BAR_HEIGHT),
        );

        ui.scope_builder(egui::UiBuilder::new().max_rect(main_rect), |ui| {
            ui.heading("Part Master");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button("New part").clicked() {
                    app.part_master_edit_part = Some(None);
                }
                ui.separator();
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut app.part_master_filter)
                        .desired_width(200.0)
                        .hint_text("part number or description…"),
                );
                if !app.part_master_filter.is_empty() && ui.button("✕").clicked() {
                    app.part_master_filter.clear();
                }
            });
            ui.add_space(8.0);

            let category = app
                .part_master_visible_categories
                .get(selected_tab_index(app))
                .map(|s| s.as_str())
                .unwrap_or("Others");
            let sync_key = part_master_sync_key(app);
            if app.part_master_last_sync_key != Some(sync_key.clone()) {
                let rows = part_rows_for_table(app);
                app.part_master_table.replace(rows);
                app.part_master_last_sync_key = Some(sync_key);
            }

            ui.strong("Parts");
            ui.add_space(4.0);

            let table_area_height = ui.available_rect_before_wrap().height();

            if app.part_master_table.is_empty() {
                ui.label(format!("No parts in category \"{}\".", category));
            } else {
                let custom_field_names = all_custom_field_names(&app.boms);
                let mut viewer = PartMasterViewer::new(custom_field_names);
                ui.add(
                    Renderer::new(&mut app.part_master_table, &mut viewer)
                        .with_table_row_height(22.0)
                        .with_max_scroll_height(table_area_height.max(100.0)),
                );
                if let Some((row_idx, id)) = viewer.pending_custom_action.take() {
                    let part_id = app.part_master_table.iter().nth(row_idx).map(|r| r.part_id);
                    if let Some(pid) = part_id {
                        match id.as_str() {
                            "add_to_request" => app.add_assembly_to_request(pid),
                            "edit_part" => {
                                app.part_master_edit_part = Some(Some(pid));
                            }
                            "edit_bom" => {
                                app.pending_open_bom = Some(pid);
                            }
                            _ => {}
                        }
                    }
                }
            }
    });

    ui.scope_builder(egui::UiBuilder::new().max_rect(tab_rect), |ui| {
            egui::Frame::group(ui.style()).inner_margin(6.0).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let categories: Vec<String> =
                        app.part_master_visible_categories.iter().cloned().collect();
                    for (i, name) in categories.iter().enumerate() {
                        let can_remove = categories.len() > 1 && name != "All";
                        let count = parts_in_category(&app.boms, name)
                            .into_iter()
                            .filter(|p| matches_filter(p, &app.part_master_filter))
                            .count();
                        let tab_label = format!("{} ({})", name, count);
                        let selected = app.part_master_tab == i;
                        if selected {
                            ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
                        }
                        let response = ui.selectable_label(selected, tab_label);
                        if response.clicked() {
                            app.part_master_tab = i;
                        }
                        response.context_menu(|ui| {
                            if ui.button("Add Category").clicked() {
                                app.part_master_add_category_open = true;
                                app.part_master_add_category_input.clear();
                                ui.close();
                            }
                            ui.separator();
                            if can_remove {
                                if ui.button("Remove tab").clicked() {
                                    app.part_master_visible_categories.remove(i);
                                    if app.part_master_tab >= app.part_master_visible_categories.len() {
                                        app.part_master_tab =
                                            app.part_master_visible_categories.len().saturating_sub(1);
                                    } else if i < app.part_master_tab {
                                        app.part_master_tab -= 1;
                                    }
                                    ui.close();
                                }
                            } else {
                                ui.label("Cannot remove the last tab");
                            }
                        });
                        if selected {
                            let r = response.rect;
                            ui.painter().line_segment(
                                [r.left_top(), r.right_top()],
                                (2.0, egui::Color32::from_gray(120)),
                            );
                        }
                        ui.visuals_mut().override_text_color = None;
                    }
                });
            });
    });
}
