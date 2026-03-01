//! egui_dock integration: Part Master (fixed), BOM Editor (per-part), Request (per-request with Edit/Preview mode).

use crate::app::KanbanBomsApp;
use crate::bom_edit::bom_edit_ui;
use crate::bom_preview::bom_preview_ui;
use crate::part_master::part_master_ui;
use crate::request_edit::request_edit_ui;
use egui::WidgetText;
use egui_dock::{DockArea, DockState, Style, TabViewer};
use uuid::Uuid;

/// Max width for centered request content (Edit/Preview). Matches request_edit and bom_preview.
const REQUEST_CENTERED_MAX_WIDTH: f32 = 700.0;

/// Per-tab state for Part Edit form.
#[derive(Debug, Clone, Default)]
pub struct PartEditState {
    pub partno: String,
    pub description: String,
    pub custom_fields: Vec<(String, String)>,
    pub error: Option<String>,
}

/// Tab types for the dock layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DockTab {
    /// Fixed Part Master pane (cannot be closed).
    PartMaster,
    /// BOM Editor for a specific part (bom_id).
    BomEdit(Uuid),
    /// Request editor/preview (single request).
    Request,
    /// Part edit/create. Uuid = tab id (part id for existing, generated for new).
    PartEdit(Uuid),
    /// Usage report: lists all BOMs where a part (part_id) is used.
    UsageReport(Uuid),
}

impl DockTab {
    /// Returns true if the tab has unsaved changes.
    pub fn is_dirty(&self, app: &KanbanBomsApp) -> bool {
        match self {
            DockTab::PartMaster => false,
            DockTab::PartEdit(tab_id) => {
                let state = match app.part_edit_states.get(tab_id) {
                    Some(s) => s,
                    None => return false,
                };
                let bom_id = if app.boms.iter().any(|b| b.id == *tab_id) {
                    *tab_id
                } else {
                    app.part_edit_new_to_bom.get(tab_id).copied().unwrap_or(*tab_id)
                };
                let is_new = !app.boms.iter().any(|b| b.id == *tab_id)
                    && !app.part_edit_new_to_bom.contains_key(tab_id);
                if is_new {
                    !state.partno.trim().is_empty()
                        || !state.description.trim().is_empty()
                        || state
                            .custom_fields
                            .iter()
                            .any(|(k, v)| !k.trim().is_empty() || !v.trim().is_empty())
                } else if let Some(bom) = app.boms.iter().find(|b| b.id == bom_id) {
                    let current_cf: std::collections::HashMap<String, String> = state
                        .custom_fields
                        .iter()
                        .filter(|(k, _)| !k.trim().is_empty())
                        .map(|(k, v)| (k.trim().to_string(), v.clone()))
                        .collect();
                    state.partno.trim() != bom.partno
                        || state.description.trim() != bom.description
                        || current_cf != bom.custom_fields
                } else {
                    false
                }
            }
            DockTab::BomEdit(bom_id) => {
                if app.bom_edit_viewing_revisions.get(bom_id).copied().flatten().is_some() {
                    return false;
                }
                app.bom_edit_tables
                    .get(bom_id)
                    .map(|t| t.is_dirty())
                    .unwrap_or(false)
            }
            DockTab::Request | DockTab::UsageReport(_) => false,
        }
    }

    pub fn title(&self, app: &KanbanBomsApp) -> String {
        let base = match self {
            DockTab::PartMaster => "Part Master".to_string(),
            DockTab::PartEdit(tab_id) => {
                let bom_id = if app.boms.iter().any(|b| b.id == *tab_id) {
                    *tab_id
                } else {
                    app.part_edit_new_to_bom.get(tab_id).copied().unwrap_or(*tab_id)
                };
                app.boms
                    .iter()
                    .find(|b| b.id == bom_id)
                    .map(|b| format!("Edit: {}", b.partno))
                    .unwrap_or_else(|| "New Part".to_string())
            }
            DockTab::BomEdit(bom_id) => app
                .boms
                .iter()
                .find(|b| b.id == *bom_id)
                .map(|b| format!("BOM: {}", b.partno))
                .unwrap_or_else(|| "BOM Editor".to_string()),
            DockTab::Request => {
                let machine = if app.request.machine_number.is_empty() {
                    "Request"
                } else {
                    &app.request.machine_number
                };
                format!("{}", machine)
            }
            DockTab::UsageReport(part_id) => app
                .boms
                .iter()
                .find(|b| b.id == *part_id)
                .map(|b| format!("Usage: {}", b.partno))
                .unwrap_or_else(|| "Usage Report".to_string()),
        };
        if self.is_dirty(app) {
            format!("{} *", base)
        } else {
            base
        }
    }
}

/// Per-request tab state: Edit vs Preview mode.
#[derive(Debug, Clone, Default)]
pub struct RequestTabState {
    pub preview_mode: bool,
}

/// TabViewer that renders each dock tab.
pub struct AppTabViewer<'a> {
    pub app: &'a mut KanbanBomsApp,
}

impl TabViewer for AppTabViewer<'_> {
    type Tab = DockTab;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.title(self.app).into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            DockTab::PartMaster => {
                part_master_ui(self.app, ui);
            }
            DockTab::PartEdit(tab_id) => {
                crate::part_master::part_edit_ui(self.app, ui, *tab_id);
            }
            DockTab::BomEdit(bom_id) => {
                bom_edit_ui(self.app, ui, *bom_id);
            }
            DockTab::Request => {
                let state = &mut self.app.request_tab_state;

                // Align Edit/Preview buttons with the centered content below (same as request_edit/bom_preview)
                let avail = ui.available_rect_before_wrap();
                let width = avail.width().min(REQUEST_CENTERED_MAX_WIDTH);
                let left = avail.left() + (avail.width() - width) / 2.0;
                let button_rect = egui::Rect::from_min_size(
                    egui::pos2(left, avail.top()),
                    egui::vec2(width, 28.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(button_rect), |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut state.preview_mode, false, "Edit");
                        ui.selectable_value(&mut state.preview_mode, true, "Preview");
                    });
                });
                ui.add_space(4.0);

                if state.preview_mode {
                    bom_preview_ui(self.app, ui);
                } else {
                    // Clear BOM preview cache and deferred build when switching to Edit so we don't hold stale data
                    self.app.bom_preview_parts_cache = None;
                    self.app.bom_preview_deferred_build = None;
                    request_edit_ui(self.app, ui);
                }
            }
            DockTab::UsageReport(part_id) => {
                crate::part_master::usage_report_ui(self.app, ui, *part_id);
            }
        }
    }

    fn is_closeable(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, DockTab::PartMaster)
    }

    /// Disable Eject to avoid crash when main surface becomes empty (egui_dock panics on empty tree).
    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }
}

/// Build initial dock layout: Part Master and Request as tabs.
pub fn make_initial_dock_state(_app: &KanbanBomsApp) -> DockState<DockTab> {
    let tabs = vec![DockTab::PartMaster, DockTab::Request];
    DockState::new(tabs)
}

/// Ensure dock state has Part Master, current Request, and any pending BOM/PartEdit tabs. Call each frame to sync.
pub fn ensure_dock_tabs(dock_state: &mut DockState<DockTab>, app: &mut KanbanBomsApp) {
    // Open Part Edit tab when "New part" or "Edit part" is clicked (add without closing others)
    if let Some(edit_id) = app.part_master_edit_part.take() {
        let tab_id = edit_id.unwrap_or_else(Uuid::new_v4);
        let already_open = dock_state.main_surface().tabs().any(|t| {
            matches!(t, DockTab::PartEdit(id) if *id == tab_id)
        });
        if !already_open {
            dock_state.main_surface_mut().push_to_first_leaf(DockTab::PartEdit(tab_id));
        }
    }

    // Close specific Part Edit tab when Cancel is clicked
    if let Some(tab_to_close) = app.part_master_close_edit_tab.take() {
        dock_state.retain_tabs(|tab| !matches!(tab, DockTab::PartEdit(id) if *id == tab_to_close));
        app.part_edit_states.remove(&tab_to_close);
        app.part_edit_new_to_bom.remove(&tab_to_close);
    }

    let surface = dock_state.main_surface_mut();

    // Open BOM Edit tab when Part Master "Edit BOM" is clicked
    if let Some(bom_id) = app.pending_open_bom.take() {
        let has_bom = surface.tabs().any(|t| matches!(t, DockTab::BomEdit(id) if *id == bom_id));
        if !has_bom {
            surface.push_to_first_leaf(DockTab::BomEdit(bom_id));
        }
    }

    // Open Usage Report tab when Part Master "Usage report" is clicked
    if let Some(part_id) = app.part_master_usage_report.take() {
        let has_report = surface.tabs().any(|t| matches!(t, DockTab::UsageReport(id) if *id == part_id));
        if !has_report {
            surface.push_to_first_leaf(DockTab::UsageReport(part_id));
        }
    }

    let has_part_master = surface.tabs().any(|t| matches!(t, DockTab::PartMaster));
    let has_request = surface.tabs().any(|t| matches!(t, DockTab::Request));

    if !has_part_master {
        surface.push_to_first_leaf(DockTab::PartMaster);
    }
    if !has_request {
        surface.push_to_first_leaf(DockTab::Request);
    }
}

/// Show the dock area.
pub fn show_dock_ui(
    dock_state: &mut DockState<DockTab>,
    app: &mut KanbanBomsApp,
    ctx: &egui::Context,
) {
    ensure_dock_tabs(dock_state, app);

    egui::CentralPanel::default().show(ctx, |ui| {
        let mut tab_viewer = AppTabViewer { app };
        DockArea::new(dock_state)
            .style(Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut tab_viewer);
    });
}
