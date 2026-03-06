//! BOM Diff Tool: compare two revisions of a BOM side-by-side.

use crate::app::KanbanBomsApp;
use crate::bom_edit::bom_entries_for_edit;
use crate::models::BomEntry;
use egui::{Key, KeyboardShortcut, Modifiers};
use std::collections::HashSet;
use egui_data_table::viewer::{RowCodec, TableColumnConfig, UiActionContext};
use egui_data_table::viewer::MoveDirection;
use egui_data_table::{Renderer, RowViewer, UiAction};
use std::borrow::Cow;
use std::collections::HashMap;
use uuid::Uuid;

const CENTERED_MAX_WIDTH: f32 = 700.0;
const DASH: &str = "—";

/// Row type for the BOM Diff data table (read-only).
#[derive(Debug, Clone)]
pub struct BomDiffRow {
    pub part_id: Uuid,
    pub partno: String,
    pub description: String,
    pub qty_a: Option<i32>,
    pub qty_b: Option<i32>,
    pub uom_a: String,
    pub uom_b: String,
    pub tags_a: String,
    pub tags_b: String,
    pub disabled_a: Option<bool>,
    pub disabled_b: Option<bool>,
    pub expand_a: Option<bool>,
    pub expand_b: Option<bool>,
}

impl BomDiffRow {
    /// Diff value: qty_b - qty_a when both present; -qty when only in A; +qty when only in B.
    pub fn diff_value(&self) -> Option<i32> {
        match (self.qty_a, self.qty_b) {
            (Some(a), Some(b)) => Some(b - a),
            (Some(a), None) => Some(-a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }
    }

    pub fn diff_display(&self) -> String {
        match self.diff_value() {
            None => DASH.to_string(),
            Some(d) => {
                if d > 0 {
                    format!("+{}", d)
                } else if d < 0 {
                    d.to_string()
                } else {
                    "0".to_string()
                }
            }
        }
    }
}

/// Build merged diff rows from entries of revision A and B.
fn bom_diff_rows(
    app: &KanbanBomsApp,
    entries_a: &[BomEntry],
    entries_b: &[BomEntry],
) -> Vec<BomDiffRow> {
    let boms_map = app.boms_by_id();
    let mut by_part: HashMap<Uuid, (Option<BomEntry>, Option<BomEntry>)> = HashMap::new();

    for e in entries_a {
        by_part
            .entry(e.part_id)
            .or_insert((None, None))
            .0 = Some(e.clone());
    }
    for e in entries_b {
        by_part
            .entry(e.part_id)
            .or_insert((None, None))
            .1 = Some(e.clone());
    }

    let mut rows: Vec<BomDiffRow> = by_part
        .into_iter()
        .filter_map(|(part_id, (opt_a, opt_b))| {
            let part = boms_map.get(&part_id)?;
            let (uom_a, uom_b) = (
                opt_a
                    .as_ref()
                    .map(|e| if e.uom.is_empty() { "EA" } else { e.uom.as_str() })
                    .unwrap_or(DASH)
                    .to_string(),
                opt_b
                    .as_ref()
                    .map(|e| if e.uom.is_empty() { "EA" } else { e.uom.as_str() })
                    .unwrap_or(DASH)
                    .to_string(),
            );
            let (tags_a, tags_b) = (
                opt_a
                    .as_ref()
                    .map(|e| e.tags.join(" "))
                    .unwrap_or_else(|| DASH.to_string()),
                opt_b
                    .as_ref()
                    .map(|e| e.tags.join(" "))
                    .unwrap_or_else(|| DASH.to_string()),
            );
            let (disabled_a, disabled_b) = (
                opt_a.as_ref().map(|e| e.disabled),
                opt_b.as_ref().map(|e| e.disabled),
            );
            let (expand_a, expand_b) = (
                opt_a.as_ref().map(|e| e.expand),
                opt_b.as_ref().map(|e| e.expand),
            );

            Some(BomDiffRow {
                part_id,
                partno: part.partno.clone(),
                description: part.description.clone(),
                qty_a: opt_a.as_ref().map(|e| e.quantity),
                qty_b: opt_b.as_ref().map(|e| e.quantity),
                uom_a,
                uom_b,
                tags_a,
                tags_b,
                disabled_a,
                disabled_b,
                expand_a,
                expand_b,
            })
        })
        .collect();

    rows.sort_by(|a, b| a.partno.cmp(&b.partno));
    rows
}

fn fmt_opt_i32(v: Option<i32>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| DASH.to_string())
}

fn fmt_opt_bool(v: Option<bool>) -> String {
    v.map(|b| if b { "Yes".to_string() } else { "No".to_string() })
        .unwrap_or_else(|| DASH.to_string())
}

/// Codec for copy-to-clipboard (TSV). Decode not used (read-only table).
struct BomDiffCodec;

impl RowCodec<BomDiffRow> for BomDiffCodec {
    type DeserializeError = ();

    fn create_empty_decoded_row(&mut self) -> BomDiffRow {
        BomDiffRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            qty_a: None,
            qty_b: None,
            uom_a: String::new(),
            uom_b: String::new(),
            tags_a: String::new(),
            tags_b: String::new(),
            disabled_a: None,
            disabled_b: None,
            expand_a: None,
            expand_b: None,
        }
    }

    fn encode_column(&mut self, src_row: &BomDiffRow, column: usize, dst: &mut String) {
        match column {
            0 => dst.push_str(&src_row.partno),
            1 => dst.push_str(&src_row.description),
            2 => dst.push_str(&format!(
                "{} / {}",
                fmt_opt_i32(src_row.qty_a),
                fmt_opt_i32(src_row.qty_b)
            )),
            3 => dst.push_str(&src_row.diff_display()),
            4 => dst.push_str(&format!("{} / {}", src_row.uom_a, src_row.uom_b)),
            5 => dst.push_str(&format!("{} / {}", src_row.tags_a, src_row.tags_b)),
            6 => dst.push_str(&format!(
                "{} / {}",
                fmt_opt_bool(src_row.disabled_a),
                fmt_opt_bool(src_row.disabled_b)
            )),
            7 => dst.push_str(&format!(
                "{} / {}",
                fmt_opt_bool(src_row.expand_a),
                fmt_opt_bool(src_row.expand_b)
            )),
            _ => {}
        }
    }

    fn decode_column(
        &mut self,
        _src_data: &str,
        _column: usize,
        _dst_row: &mut BomDiffRow,
    ) -> Result<(), egui_data_table::viewer::DecodeErrorBehavior> {
        Ok(())
    }
}

/// Viewer for BOM Diff table (read-only).
struct BomDiffViewer;

impl RowViewer<BomDiffRow> for BomDiffViewer {
    fn num_columns(&mut self) -> usize {
        8
    }

    fn column_name(&mut self, column: usize) -> Cow<'static, str> {
        match column {
            0 => Cow::Borrowed("Part Number"),
            1 => Cow::Borrowed("Description"),
            2 => Cow::Borrowed("Qty/Qty"),
            3 => Cow::Borrowed("Diff"),
            4 => Cow::Borrowed("UOM/UOM"),
            5 => Cow::Borrowed("Tags/Tags"),
            6 => Cow::Borrowed("Disabled/Disabled"),
            7 => Cow::Borrowed("Expand/Expand"),
            _ => Cow::Borrowed(""),
        }
    }

    fn column_render_config(&mut self, column: usize) -> TableColumnConfig {
        if column == 1 {
            TableColumnConfig::initial(200.0).resizable(true)
        } else {
            TableColumnConfig::auto().resizable(true)
        }
    }

    fn is_sortable_column(&mut self, column: usize) -> bool {
        column < 8
    }

    fn compare_cell(
        &self,
        row_a: &BomDiffRow,
        row_b: &BomDiffRow,
        column: usize,
    ) -> std::cmp::Ordering {
        let va = self.get_cell_value(row_a, column);
        let vb = self.get_cell_value(row_b, column);
        va.cmp(&vb)
    }

    fn try_create_codec(&mut self, is_encoding: bool) -> Option<impl RowCodec<BomDiffRow>> {
        if is_encoding {
            Some(BomDiffCodec)
        } else {
            None
        }
    }

    fn show_cell_view(&mut self, ui: &mut egui::Ui, row: &BomDiffRow, column: usize) {
        let (text, color) = match column {
            0 => (row.partno.clone(), None),
            1 => (row.description.clone(), None),
            2 => (
                format!("{} / {}", fmt_opt_i32(row.qty_a), fmt_opt_i32(row.qty_b)),
                None,
            ),
            3 => {
                let text = row.diff_display();
                let color = row.diff_value().map(|d| {
                    if d > 0 {
                        egui::Color32::DARK_GREEN
                    } else if d < 0 {
                        egui::Color32::DARK_RED
                    } else {
                        egui::Color32::GRAY
                    }
                });
                (text, color)
            }
            4 => (format!("{} / {}", row.uom_a, row.uom_b), None),
            5 => (format!("{} / {}", row.tags_a, row.tags_b), None),
            6 => (
                format!(
                    "{} / {}",
                    fmt_opt_bool(row.disabled_a),
                    fmt_opt_bool(row.disabled_b)
                ),
                None,
            ),
            7 => (
                format!(
                    "{} / {}",
                    fmt_opt_bool(row.expand_a),
                    fmt_opt_bool(row.expand_b)
                ),
                None,
            ),
            _ => (String::new(), None),
        };

        if let Some(c) = color {
            ui.colored_label(c, text);
        } else {
            ui.label(text);
        }
    }

    fn show_cell_editor(
        &mut self,
        ui: &mut egui::Ui,
        row: &mut BomDiffRow,
        column: usize,
    ) -> Option<egui::Response> {
        let (text, color) = match column {
            0 => (row.partno.clone(), None),
            1 => (row.description.clone(), None),
            2 => (
                format!("{} / {}", fmt_opt_i32(row.qty_a), fmt_opt_i32(row.qty_b)),
                None,
            ),
            3 => {
                let text = row.diff_display();
                let color = row.diff_value().map(|d| {
                    if d > 0 {
                        egui::Color32::GREEN
                    } else if d < 0 {
                        egui::Color32::RED
                    } else {
                        egui::Color32::GRAY
                    }
                });
                (text, color)
            }
            4 => (format!("{} / {}", row.uom_a, row.uom_b), None),
            5 => (format!("{} / {}", row.tags_a, row.tags_b), None),
            6 => (
                format!(
                    "{} / {}",
                    fmt_opt_bool(row.disabled_a),
                    fmt_opt_bool(row.disabled_b)
                ),
                None,
            ),
            7 => (
                format!(
                    "{} / {}",
                    fmt_opt_bool(row.expand_a),
                    fmt_opt_bool(row.expand_b)
                ),
                None,
            ),
            _ => (String::new(), None),
        };

        let response = if let Some(c) = color {
            ui.colored_label(c, text)
        } else {
            ui.label(text)
        };
        Some(response)
    }

    fn set_cell_value(&mut self, _src: &BomDiffRow, _dst: &mut BomDiffRow, _column: usize) {}

    fn clone_row_as_copied_base(&mut self, row: &BomDiffRow) -> BomDiffRow {
        row.clone()
    }

    fn new_empty_row(&mut self) -> BomDiffRow {
        BomDiffRow {
            part_id: Uuid::nil(),
            partno: String::new(),
            description: String::new(),
            qty_a: None,
            qty_b: None,
            uom_a: String::new(),
            uom_b: String::new(),
            tags_a: String::new(),
            tags_b: String::new(),
            disabled_a: None,
            disabled_b: None,
            expand_a: None,
            expand_b: None,
        }
    }

    fn confirm_row_deletion_by_ui(&mut self, _row: &BomDiffRow) -> bool {
        false
    }

    fn allowed_context_menu_actions(&self) -> Option<HashSet<UiAction>> {
        Some([UiAction::CopySelection].into_iter().collect())
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

    fn allow_cell_edit(&mut self, _row: &BomDiffRow, _column: usize) -> bool {
        false
    }

    fn trivial_config(&mut self) -> egui_data_table::viewer::TrivialConfig {
        egui_data_table::viewer::TrivialConfig {
            table_row_height: Some(22.0),
            max_undo_history: 0,
            max_scroll_height: None,
        }
    }
}

impl BomDiffViewer {
    fn get_cell_value(&self, row: &BomDiffRow, column: usize) -> String {
        match column {
            0 => row.partno.clone(),
            1 => row.description.clone(),
            2 => format!("{} / {}", fmt_opt_i32(row.qty_a), fmt_opt_i32(row.qty_b)),
            3 => row.diff_display(),
            4 => format!("{} / {}", row.uom_a, row.uom_b),
            5 => format!("{} / {}", row.tags_a, row.tags_b),
            6 => format!(
                "{} / {}",
                fmt_opt_bool(row.disabled_a),
                fmt_opt_bool(row.disabled_b)
            ),
            7 => format!(
                "{} / {}",
                fmt_opt_bool(row.expand_a),
                fmt_opt_bool(row.expand_b)
            ),
            _ => String::new(),
        }
    }
}

/// Sync key for BOM Diff table.
fn bom_diff_sync_key(
    app: &KanbanBomsApp,
    bom_id: Uuid,
    rev_a: Option<u32>,
    rev_b: Option<u32>,
    rows_len: usize,
) -> (u64, Uuid, Option<u32>, Option<u32>, usize) {
    (app.data_version, bom_id, rev_a, rev_b, rows_len)
}

pub fn bom_diff_ui(app: &mut KanbanBomsApp, ui: &mut egui::Ui, bom_id: Uuid) {
    let avail = ui.available_rect_before_wrap();
    let width = avail.width().min(CENTERED_MAX_WIDTH);
    let left = avail.left() + (avail.width() - width) / 2.0;
    let rect = egui::Rect::from_min_size(
        egui::pos2(left, avail.top()),
        egui::vec2(width, avail.height()),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.heading("BOM Diff");
        ui.add_space(8.0);

        if !app.boms.iter().any(|b| b.id == bom_id) {
            ui.label("BOM not found.");
            return;
        }

        let bom = app
            .boms
            .iter()
            .find(|b| b.id == bom_id)
            .cloned()
            .unwrap();
        ui.horizontal(|ui| {
            ui.label("Assembly:");
            ui.label(&bom.partno);
            ui.label("-");
            ui.label(&bom.description);
        });
        ui.add_space(4.0);

        let revisions: Vec<u32> = app
            .bom_revisions
            .get(&bom_id)
            .map(|revs| revs.iter().map(|r| r.revision).collect())
            .unwrap_or_default();

        let rev_a = *app
            .bom_diff_revision_a
            .entry(bom_id)
            .or_insert(None);
        let rev_b = *app
            .bom_diff_revision_b
            .entry(bom_id)
            .or_insert(None);

        ui.horizontal(|ui| {
            ui.label("Revision A:");
            let current_label = "Current";
            let selected_a = match rev_a {
                None => current_label.to_string(),
                Some(r) => format!("Revision {}", r),
            };
            egui::ComboBox::from_id_salt(("bom_diff_rev_a", bom_id))
                .selected_text(selected_a)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(rev_a.is_none(), current_label).clicked() {
                        app.bom_diff_revision_a.insert(bom_id, None);
                    }
                    for &r in &revisions {
                        if ui
                            .selectable_label(rev_a == Some(r), format!("Revision {}", r))
                            .clicked()
                        {
                            app.bom_diff_revision_a.insert(bom_id, Some(r));
                        }
                    }
                });
            ui.add_space(16.0);
            ui.label("Revision B:");
            let selected_b = match rev_b {
                None => current_label.to_string(),
                Some(r) => format!("Revision {}", r),
            };
            egui::ComboBox::from_id_salt(("bom_diff_rev_b", bom_id))
                .selected_text(selected_b)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(rev_b.is_none(), current_label).clicked() {
                        app.bom_diff_revision_b.insert(bom_id, None);
                    }
                    for &r in &revisions {
                        if ui
                            .selectable_label(rev_b == Some(r), format!("Revision {}", r))
                            .clicked()
                        {
                            app.bom_diff_revision_b.insert(bom_id, Some(r));
                        }
                    }
                });
        });
        ui.add_space(4.0);

        let entries_a = bom_entries_for_edit(app, bom_id, rev_a);
        let entries_b = bom_entries_for_edit(app, bom_id, rev_b);
        let rows = bom_diff_rows(app, &entries_a, &entries_b);
        let sync_key = bom_diff_sync_key(app, bom_id, rev_a, rev_b, rows.len());

        if rows.is_empty() {
            ui.strong("Components");
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::GRAY, "No data to compare.");
            return;
        }

        if app.bom_diff_last_sync_keys.get(&bom_id) != Some(&sync_key) {
            app.bom_diff_tables
                .entry(bom_id)
                .or_insert_with(egui_data_table::DataTable::new)
                .replace_keeping_ui(rows);
            app.bom_diff_last_sync_keys.insert(bom_id, sync_key);
        }

        ui.strong("Components");
        ui.add_space(4.0);

        let table_area_height = ui.available_rect_before_wrap().height();
        let table = app.bom_diff_tables.get_mut(&bom_id).unwrap();
        let mut viewer = BomDiffViewer;
        ui.add(
            Renderer::new(table, &mut viewer)
                .with_table_row_height(22.0)
                .with_max_scroll_height(table_area_height.max(100.0)),
        );
    });
}
