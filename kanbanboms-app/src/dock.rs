//! egui_dock integration: Part Master (fixed), BOM Editor (per-part), Request (per-request with Edit/Preview mode).

use crate::app::KanbanBomsApp;
use crate::bom_edit::bom_edit_ui;
use crate::bom_preview::bom_preview_ui;
use crate::part_master::part_master_ui;
use crate::request_edit::request_edit_ui;
use egui::WidgetText;
use egui_dock::{DockArea, DockState, Style, TabViewer};
use uuid::Uuid;

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
    /// Request editor/preview for a specific request.
    Request(Uuid),
    /// Part edit/create. Uuid = tab id (part id for existing, generated for new).
    PartEdit(Uuid),
}

impl DockTab {
    pub fn title(&self, app: &KanbanBomsApp) -> String {
        match self {
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
            DockTab::Request(req_id) => app
                .requests
                .iter()
                .find(|r| r.id == *req_id)
                .map(|r| {
                    let machine = if r.machine_number.is_empty() {
                        "Request"
                    } else {
                        &r.machine_number
                    };
                    format!("{}", machine)
                })
                .unwrap_or_else(|| "Request".to_string()),
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
                let prev = self.app.selected_bom_for_edit;
                self.app.selected_bom_for_edit = Some(*bom_id);
                bom_edit_ui(self.app, ui);
                self.app.selected_bom_for_edit = prev;
            }
            DockTab::Request(req_id) => {
                let prev_req = self.app.current_request_id;
                self.app.current_request_id = Some(*req_id);

                let state = self
                    .app
                    .request_states
                    .entry(*req_id)
                    .or_default();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.preview_mode, false, "Edit");
                    ui.selectable_value(&mut state.preview_mode, true, "Preview");
                });
                ui.add_space(4.0);

                if state.preview_mode {
                    bom_preview_ui(self.app, ui);
                } else {
                    request_edit_ui(self.app, ui);
                }

                self.app.current_request_id = prev_req;
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

/// Build initial dock layout: Part Master and current request as tabs.
pub fn make_initial_dock_state(app: &KanbanBomsApp) -> DockState<DockTab> {
    let mut tabs = vec![DockTab::PartMaster];
    if let Some(req_id) = app.current_request_id {
        tabs.push(DockTab::Request(req_id));
    }
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

    let has_part_master = surface.tabs().any(|t| matches!(t, DockTab::PartMaster));
    let has_current_request = app.current_request_id.map_or(false, |rid| {
        surface.tabs().any(|t| matches!(t, DockTab::Request(id) if *id == rid))
    });

    if !has_part_master {
        surface.push_to_first_leaf(DockTab::PartMaster);
    }
    if let Some(rid) = app.current_request_id {
        if !has_current_request {
            surface.push_to_first_leaf(DockTab::Request(rid));
        }
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
