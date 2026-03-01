use crate::app::KanbanBomsApp;
use crate::bom_search::search_boms;
use crate::models::{Bom, BomEntry, UOM_OPTIONS};
use egui::{Key, KeyboardShortcut, Modifiers};
use egui_data_table::viewer::{
    DecodeErrorBehavior, MoveDirection, RowCodec, UiActionContext,
};
use egui_data_table::{Renderer, RowViewer, UiAction};
use std::borrow::Cow;
use std::collections::HashSet;
use uuid::Uuid;

const CENTERED_MAX_WIDTH: f32 = 700.0;
const MAX_AUTOCOMPLETE_ITEMS: usize = 10;

/// Row type for the BOM Edit data table.
#[derive(Debug, Clone)]
pub struct BomEditRow {
    pub part_id: Uuid,
    pub partno: String,
    pub description: String,
    pub quantity: i32,
    pub uom: String,
    pub tags: String,
    pub disabled: bool,
    pub expand: bool,
}

/// Get bom_entries for the given bom_id, either from current state or from a specific revision.
fn bom_entries_for_edit(app: &KanbanBomsApp, bom_id: Uuid, viewing_revision: Option<u32>) -> Vec<BomEntry> {
    if let Some(rev) = viewing_revision {
        app.bom_revisions
            .get(&bom_id)
            .and_then(|revs| revs.iter().find(|r| r.revision == rev))
            .map(|r| r.entries.clone())
            .unwrap_or_default()
    } else {
        app.bom_entries
            .iter()
            .filter(|e| e.bom_id == bom_id)
            .cloned()
            .collect()
    }
}

/// Build rows from bom_entries for the given bom_id.
fn bom_edit_rows_for_table(app: &KanbanBomsApp, entries: &[BomEntry]) -> Vec<BomEditRow> {
    let boms_map = app.boms_by_id();
    entries
        .iter()
        .filter_map(|e| {
            boms_map.get(&e.part_id).map(|part| BomEditRow {
                part_id: part.id,
                partno: part.partno.clone(),
                description: part.description.clone(),
                quantity: e.quantity,
                uom: if e.uom.is_empty() { "EA".to_string() } else { e.uom.clone() },
                tags: e.tags.join(" "),
                disabled: e.disabled,
                expand: e.expand,
            })
        })
        .collect()
}

/// Sync key to avoid replacing table rows every frame.
fn bom_edit_sync_key(app: &KanbanBomsApp, bom_id: Uuid, entries_count: usize, viewing_revision: Option<u32>) -> (u64, Uuid, usize, Option<u32>) {
    (app.data_version, bom_id, entries_count, viewing_revision)
}

/// Apply table rows back to bom_entries. Removes entries not in table, updates existing, adds new.
/// Call this every frame when the BOM editor table has data so changes reflect in requests/preview.
pub fn sync_table_to_bom_entries(app: &mut KanbanBomsApp, bom_id: Uuid, rows: &[BomEditRow]) {
    let current_part_ids: HashSet<Uuid> = rows
        .iter()
        .filter(|r| r.part_id != Uuid::nil())
        .map(|r| r.part_id)
        .collect();

    // Remove entries for parts no longer in table
    app.bom_entries
        .retain(|e| e.bom_id != bom_id || current_part_ids.contains(&e.part_id));

    for row in rows {
        if row.part_id == Uuid::nil() {
            continue;
        }
        let tags: Vec<String> = row
            .tags
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let uom = if row.uom.is_empty() { "EA".to_string() } else { row.uom.clone() };
        if let Some(entry) = app
            .bom_entries
            .iter_mut()
            .find(|e| e.bom_id == bom_id && e.part_id == row.part_id)
        {
            entry.quantity = row.quantity;
            entry.uom = uom;
            entry.tags = tags;
            entry.disabled = row.disabled;
            entry.expand = row.expand;
        } else {
            app.bom_entries.push(BomEntry {
                bom_id,
                part_id: row.part_id,
                quantity: row.quantity,
                uom,
                disabled: row.disabled,
                expand: row.expand,
                tags,
            });
        }
    }
    crate::models::recompute_bom_entry_counts(&mut app.boms, &app.bom_entries);
}

/// Codec for copy-to-clipboard (TSV).
pub struct BomEditCodec;

impl RowCodec<BomEditRow> for BomEditCodec {
    type DeserializeError = ();

    fn create_empty_decoded_row(&mut self) -> BomEditRow {
        BomEditRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            quantity: 1,
            uom: "EA".to_string(),
            tags: String::new(),
            disabled: false,
            expand: false,
        }
    }

    fn encode_column(&mut self, src_row: &BomEditRow, column: usize, dst: &mut String) {
        match column {
            0 => dst.push_str(&src_row.partno),
            1 => dst.push_str(&src_row.description),
            2 => dst.push_str(&src_row.quantity.to_string()),
            3 => dst.push_str(&src_row.uom),
            4 => dst.push_str(&src_row.tags),
            5 => dst.push_str(if src_row.disabled { "Yes" } else { "No" }),
            6 => dst.push_str(if src_row.expand { "Yes" } else { "No" }),
            _ => {}
        }
    }

    fn decode_column(
        &mut self,
        src_data: &str,
        column: usize,
        dst_row: &mut BomEditRow,
    ) -> Result<(), DecodeErrorBehavior> {
        match column {
            0 => dst_row.partno = src_data.to_string(),
            1 => dst_row.description = src_data.to_string(),
            2 => dst_row.quantity = src_data.parse().unwrap_or(1),
            3 => {
                dst_row.uom = UOM_OPTIONS
                    .iter()
                    .find(|&&o| o.eq_ignore_ascii_case(src_data))
                    .map(|s| (*s).to_string())
                    .unwrap_or_else(|| "EA".to_string());
            }
            4 => dst_row.tags = src_data.to_string(),
            5 => dst_row.disabled = src_data.eq_ignore_ascii_case("yes"),
            6 => dst_row.expand = src_data.eq_ignore_ascii_case("yes"),
            _ => {}
        }
        Ok(())
    }
}

/// Viewer for BOM Edit table with autocomplete on Part Number and Description.
pub struct BomEditViewer {
    pub boms: Vec<Bom>,
    /// Part IDs already in this BOM (to avoid duplicate suggestions when adding)
    pub existing_part_ids: HashSet<Uuid>,
    /// System clipboard content for paste (e.g. from arboard)
    pub system_clipboard: Option<String>,
    /// When user picks a custom action, set by the grid; consumed after draw.
    pub pending_custom_action: Option<(usize, String)>,
}

impl BomEditViewer {
    /// Resolve part_id from partno when pasting from clipboard (part_id is nil but partno is set).
    fn resolve_part_id_from_partno(&self, row: &mut BomEditRow) {
        if row.part_id != Uuid::nil() || row.partno.trim().is_empty() {
            return;
        }
        let partno = row.partno.trim();
        let matches: Vec<_> = self
            .boms
            .iter()
            .filter(|b| b.partno == partno)
            .collect();
        if matches.len() == 1 {
            row.part_id = matches[0].id;
            row.description = matches[0].description.clone();
        }
    }

    fn search_parts(&self, query: &str, exclude_part_ids: &HashSet<Uuid>) -> Vec<&Bom> {
        if query.trim().is_empty() {
            return Vec::new();
        }
        let mut results = search_boms(&self.boms, query);
        results.retain(|b| !exclude_part_ids.contains(&b.id) && !self.existing_part_ids.contains(&b.id));
        results.truncate(MAX_AUTOCOMPLETE_ITEMS);
        results
    }
}

impl RowViewer<BomEditRow> for BomEditViewer {
    fn num_columns(&mut self) -> usize {
        7
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => Cow::Borrowed("Part Number"),
            1 => Cow::Borrowed("Description"),
            2 => Cow::Borrowed("Qty"),
            3 => Cow::Borrowed("UOM"),
            4 => Cow::Borrowed("Tags"),
            5 => Cow::Borrowed("Disabled"),
            6 => Cow::Borrowed("Expand"),
            _ => Cow::Borrowed(""),
        }
    }

    fn try_create_codec(&mut self, _is_encoding: bool) -> Option<impl RowCodec<BomEditRow>> {
        Some(BomEditCodec)
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &BomEditRow, column: usize) {
        match column {
            0 => { ui.label(&row.partno); }
            1 => { ui.label(&row.description); }
            2 => { ui.label(row.quantity.to_string()); }
            3 => { ui.label(if row.uom.is_empty() { "EA" } else { &row.uom }); }
            4 => { ui.label(&row.tags); }
            5 => { ui.label(if row.disabled { "Yes" } else { "No" }); }
            6 => { ui.label(if row.expand { "Yes" } else { "No" }); }
            _ => { ui.label(""); }
        }
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut BomEditRow,
        column: usize,
    ) -> Option<egui::Response> {
        match column {
            0 | 1 => {
                let popup_id = ui.auto_id_with("bom_autocomplete").with(column);
                let query = if column == 0 {
                    &mut row.partno
                } else {
                    &mut row.description
                };

                let response = ui.add(
                    egui::TextEdit::singleline(query)
                        .desired_width(120.0)
                        .id(ui.auto_id_with("bom_edit").with(column)),
                );

                // Show autocomplete when focused and has text. Defer until at least 2 chars to avoid
                // popup stealing focus on first keystroke (common egui focus-loss bug).
                if (response.has_focus() || response.gained_focus())
                    && query.trim().len() >= 2
                {
                    ui.memory_mut(|m| m.open_popup(popup_id));
                }

                let exclude: HashSet<Uuid> = std::iter::once(row.part_id)
                    .filter(|id| *id != Uuid::nil())
                    .collect();
                let matches: Vec<(Uuid, String, String)> = self
                    .search_parts(query, &exclude)
                    .into_iter()
                    .map(|b| (b.id, b.partno.clone(), b.description.clone()))
                    .collect();

                let mut selected: Option<(Uuid, String, String)> = None;
                egui::popup_below_widget(
                    ui,
                    popup_id,
                    &response,
                    egui::PopupCloseBehavior::CloseOnClick,
                    |ui: &mut egui::Ui| {
                        ui.set_min_width(280.0);
                        ui.set_max_height(200.0);
                        if matches.is_empty() {
                            ui.label("No matching parts");
                            return;
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (part_id, partno, description) in &matches {
                                let text = format!("{} - {}", partno, description);
                                if ui.selectable_label(false, &text).clicked() {
                                    selected = Some((*part_id, partno.clone(), description.clone()));
                                }
                            }
                        });
                    },
                );

                // Apply selected autocomplete (set part_id, partno, description)
                if let Some((part_id, partno, description)) = selected {
                    row.part_id = part_id;
                    row.partno = partno;
                    row.description = description;
                }

                Some(response)
            }
            2 => Some(ui.add(
                egui::DragValue::new(&mut row.quantity)
                    .speed(0.5)
                    .range(1..=10000),
            )),
            3 => {
                let selected_text = if row.uom.is_empty() { "EA" } else { row.uom.as_str() };
                let response = egui::ComboBox::from_id_salt(("bom_uom", row.part_id))
                    .selected_text(selected_text)
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for &opt in UOM_OPTIONS {
                            if ui.selectable_label(row.uom.eq_ignore_ascii_case(opt), opt).clicked() {
                                row.uom = opt.to_string();
                            }
                        }
                    })
                    .response;
                Some(response)
            }
            4 => Some(ui.add(
                egui::TextEdit::singleline(&mut row.tags)
                    .desired_width(120.0)
                    .id(ui.auto_id_with("bom_tags").with(row.part_id)),
            )),
            5 => Some(ui.checkbox(&mut row.disabled, "")),
            6 => {
                let is_ea = row.uom.is_empty() || row.uom.eq_ignore_ascii_case("EA");
                let mut expand = row.expand;
                let response = ui.add_enabled(is_ea, egui::Checkbox::new(&mut expand, ""));
                if response.changed() {
                    row.expand = expand;
                }
                Some(response)
            }
            _ => None,
        }
    }

    fn set_cell_value(&mut self, src: &BomEditRow, dst: &mut BomEditRow, column: usize) {
        match column {
            0 => {
                dst.partno = src.partno.clone();
                dst.part_id = src.part_id;
                dst.description = src.description.clone();
                self.resolve_part_id_from_partno(dst);
            }
            1 => {
                dst.description = src.description.clone();
                dst.partno = src.partno.clone();
                dst.part_id = src.part_id;
                self.resolve_part_id_from_partno(dst);
            }
            2 => dst.quantity = src.quantity,
            3 => dst.uom = src.uom.clone(),
            4 => dst.tags = src.tags.clone(),
            5 => dst.disabled = src.disabled,
            6 => dst.expand = src.expand,
            _ => {}
        }
    }

    fn new_empty_row(&mut self) -> BomEditRow {
        BomEditRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            quantity: 1,
            uom: "EA".to_string(),
            tags: String::new(),
            disabled: false,
            expand: false,
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &BomEditRow) -> bool {
        true
    }

    fn hotkeys(&mut self, context: &UiActionContext) -> Vec<(KeyboardShortcut, UiAction)> {
        let none = Modifiers::NONE;
        let ctrl = Modifiers::CTRL;
        let shift = Modifiers::SHIFT;
        let ctrl_shift = Modifiers::CTRL | Modifiers::SHIFT;
        type MD = MoveDirection;

        if context.cursor.is_editing() {
            // Editing: Enter=confirm, Esc=cancel, arrows/Tab=confirm and move to adjacent cell
            return vec![
                (KeyboardShortcut::new(none, Key::Enter), UiAction::CommitEdition),
                (KeyboardShortcut::new(none, Key::Escape), UiAction::CancelEdition),
                (KeyboardShortcut::new(none, Key::ArrowUp), UiAction::CommitEditionAndMove(MD::Up)),
                (KeyboardShortcut::new(none, Key::ArrowDown), UiAction::CommitEditionAndMove(MD::Down)),
                (KeyboardShortcut::new(none, Key::ArrowLeft), UiAction::CommitEditionAndMove(MD::Left)),
                (KeyboardShortcut::new(none, Key::ArrowRight), UiAction::CommitEditionAndMove(MD::Right)),
                (KeyboardShortcut::new(none, Key::Tab), UiAction::CommitEditionAndMove(MD::Right)),
                (KeyboardShortcut::new(shift, Key::Tab), UiAction::CommitEditionAndMove(MD::Left)),
            ];
        }

        vec![
            (KeyboardShortcut::new(none, Key::Enter), UiAction::SelectionStartEditing),
            (KeyboardShortcut::new(ctrl, Key::C), UiAction::CopySelection),
            (KeyboardShortcut::new(ctrl, Key::X), UiAction::CutSelection),
            (KeyboardShortcut::new(ctrl_shift, Key::V), UiAction::PasteInsert),
            (KeyboardShortcut::new(ctrl, Key::V), UiAction::PasteInPlace),
            (KeyboardShortcut::new(ctrl, Key::V), UiAction::PasteInsert),
            (KeyboardShortcut::new(ctrl, Key::Z), UiAction::Undo),
            (KeyboardShortcut::new(ctrl, Key::Y), UiAction::Redo),
            (KeyboardShortcut::new(ctrl, Key::D), UiAction::DuplicateRow),
            (KeyboardShortcut::new(none, Key::Delete), UiAction::DeleteSelection),
            (KeyboardShortcut::new(ctrl, Key::Delete), UiAction::DeleteRow),
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
            max_undo_history: 50,
            max_scroll_height: None,
        }
    }

    /// Allow all edit actions (Cut, Copy, Paste, Delete, Duplicate, etc.)
    fn allowed_context_menu_actions(&self) -> Option<HashSet<UiAction>> {
        None
    }

    fn get_system_clipboard_for_paste(&mut self) -> Option<String> {
        self.system_clipboard.clone()
    }

    fn custom_context_menu_items(&self, row: &BomEditRow) -> Vec<(Cow<'_, str>, String)> {
        if row.part_id == Uuid::nil() {
            return vec![];
        }
        let pid = row.part_id.to_string();
        vec![
            (Cow::Borrowed("Edit part"), format!("edit_part:{}", pid)),
            (Cow::Borrowed("Edit BOM"), format!("edit_bom:{}", pid)),
            (Cow::Borrowed("Usage report"), format!("usage_report:{}", pid)),
        ]
    }

    fn custom_action_sink(&mut self) -> Option<&mut Option<(usize, String)>> {
        Some(&mut self.pending_custom_action)
    }
}

pub fn bom_edit_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui, bom_id: Uuid) {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
    let left = avail.left() + (avail.width() - width) / 2.0;
    let rect = egui::Rect::from_min_size(
        egui::pos2(left, avail.top()),
        egui::vec2(width, avail.height()),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.heading("BOM Editor");
            ui.add_space(8.0);

            let viewing_revision = *app.bom_edit_viewing_revisions.entry(bom_id).or_insert(None);

            if app.boms.iter().any(|b| b.id == bom_id) {
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

                // Batch quantity editor (assembly-level)
                let mut batch_quantity = bom.batch_quantity;
                ui.horizontal(|ui| {
                    ui.label("Batch quantity:");
                    let r = ui.add_enabled(
                        viewing_revision.is_none(),
                        egui::DragValue::new(&mut batch_quantity)
                            .range(0..=10000)
                            .speed(0.5),
                    );
                    if r.changed() {
                        if let Some(b) = app.boms.iter_mut().find(|x| x.id == bom_id) {
                            b.batch_quantity = batch_quantity;
                        }
                    }
                });
                ui.add_space(4.0);

                // Revision dropdown
                let revisions: Vec<u32> = app
                    .bom_revisions
                    .get(&bom_id)
                    .map(|revs| revs.iter().map(|r| r.revision).collect())
                    .unwrap_or_default();
                let viewing = viewing_revision;
                ui.horizontal(|ui| {
                    ui.label("Revision:");
                    let current_label = "Current (editable)";
                    let selected_text = match viewing {
                        None => current_label.to_string(),
                        Some(r) => format!("Revision {}", r),
                    };
                    egui::ComboBox::from_id_salt(("bom_revision_select", bom_id))
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(viewing.is_none(), current_label).clicked() {
                                app.bom_edit_viewing_revisions.insert(bom_id, None);
                            }
                            for &rev in &revisions {
                                let is_selected = viewing == Some(rev);
                                if ui.selectable_label(is_selected, format!("Revision {}", rev)).clicked() {
                                    app.bom_edit_viewing_revisions.insert(bom_id, Some(rev));
                                }
                            }
                        });
                });
                ui.add_space(4.0);

                let entries = bom_entries_for_edit(app, bom_id, viewing_revision);
                let sync_key = bom_edit_sync_key(app, bom_id, entries.len(), viewing_revision);
                if app.bom_edit_last_sync_keys.get(&bom_id) != Some(&sync_key) {
                    let rows = bom_edit_rows_for_table(app, &entries);
                    app.bom_edit_tables
                        .entry(bom_id)
                        .or_insert_with(egui_data_table::DataTable::new)
                        .replace(rows);
                    app.bom_edit_last_sync_keys.insert(bom_id, sync_key);
                }

                let is_viewing_revision = viewing_revision.is_some();
                if is_viewing_revision {
                    ui.colored_label(egui::Color32::GRAY, "Viewing old revision (read-only)");
                    if let Some(rev) = viewing_revision {
                        if let Some(revision) = app
                            .bom_revisions
                            .get(&bom_id)
                            .and_then(|revs| revs.iter().find(|r| r.revision == rev))
                        {
                            if let Some(ref created_at) = revision.created_at {
                                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(created_at) {
                                    let formatted =
                                        dt.format("%Y-%m-%d %H:%M").to_string();
                                    ui.label(format!("Date: {}", formatted));
                                } else {
                                    ui.label(format!("Date: {}", created_at));
                                }
                            }
                            if !revision.comment.is_empty() {
                                ui.label(format!("Comment: {}", revision.comment));
                            }
                        }
                        if ui.button("Revert to this revision").clicked() {
                            app.bom_revert_revision_modal = Some((bom_id, rev));
                        }
                    }
                }
                ui.strong("Components");
                ui.add_space(4.0);

                let table_area_height = ui.available_rect_before_wrap().height();

                let table = app.bom_edit_tables.get_mut(&bom_id).unwrap();
                let boms = app.boms.clone();
                let existing_part_ids: HashSet<Uuid> = table
                    .iter()
                    .filter(|r| r.part_id != Uuid::nil())
                    .map(|r| r.part_id)
                    .collect();
                #[cfg(not(target_arch = "wasm32"))]
                let system_clipboard = arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut c| c.get_text().ok())
                    .filter(|s| !s.trim().is_empty());
                #[cfg(target_arch = "wasm32")]
                let system_clipboard: Option<String> = None;
                let mut viewer = BomEditViewer {
                    boms,
                    existing_part_ids,
                    system_clipboard,
                    pending_custom_action: None,
                };
                ui.add(
                    Renderer::new(table, &mut viewer)
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

                ui.add_space(4.0);
                let add_clicked = ui.horizontal(|ui| {
                    let add_enabled = !is_viewing_revision;
                    let add_btn = ui.add_enabled(add_enabled, egui::Button::new("Add Component"));
                    let save_btn = ui.add_enabled(!is_viewing_revision, egui::Button::new("Save"));
                    (add_btn.clicked(), save_btn.clicked())
                });
                if add_clicked.inner.0 {
                    app.bom_edit_tables
                        .get_mut(&bom_id)
                        .unwrap()
                        .extend(std::iter::once(BomEditRow {
                            part_id: Uuid::nil(),
                            partno: String::new(),
                            description: String::new(),
                            quantity: 1,
                            uom: "EA".to_string(),
                            tags: String::new(),
                            disabled: false,
                            expand: false,
                        }));
                }
                if add_clicked.inner.1 {
                    let rows: Vec<BomEditRow> = app
                        .bom_edit_tables
                        .get(&bom_id)
                        .unwrap()
                        .iter()
                        .cloned()
                        .collect();
                    sync_table_to_bom_entries(app, bom_id, &rows);
                    app.bom_save_revision_modal = Some((bom_id, String::new()));
                }
            }
    });
}
