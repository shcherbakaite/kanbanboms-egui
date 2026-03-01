use crate::bom_edit::BomEditRow;
use crate::bom_preview::BomPreviewRow;
use crate::part_master::{PartMasterRow, UsageReportRow};
use crate::dock::{show_dock_ui, DockTab, PartEditState, RequestTabState};
use crate::models::{get_aggregated_parts, recompute_bom_entry_counts, AggregatedPart, Bom, BomEntry, BomRevision, Request, RequestEntry};
use crate::html_export::generate_html_print_preview;
#[cfg(not(target_arch = "wasm32"))]
use crate::pdf_export::generate_pdf;
use crate::backend::{Backend, LocalBackend, PocketBaseBackend};
use crate::storage::{parse_csv_import, StoredData};
use chrono::Utc;
use egui::{Key, KeyboardShortcut, Modifiers};
use std::collections::HashMap;
use uuid::Uuid;

/// Clippy-style mascot ASCII art for bottom-right overlay
static BG_GRAPHIC: &str = r"
                                                  ▄████▄
                                                 ▐▌░░░░▐▌
                                              ▄▀▀█▀░░░░▐▌
                                              ▄░▐▄░░░░░▐▌▀▀▄
                                            ▐▀░▄▄░▀▌░▄▀▀░▀▄░▀
                                            ▐░▀██▀░▌▐░▄██▄░▌
                                             ▀▄░▄▄▀░▐░░▀▀░░▌
                                                █░░░░▀▄▄░▄▀
                                                █░█░░░░█░▐
                                                █░█░░░▐▌░█
                                                █░█░░░▐▌░█
                                                ▐▌▐▌░░░█░█
                                                ▐▌░█▄░▐▌░█
                                                 █░░▀▀▀░░▐▌
                                                 ▐▌░░░░░░█
                                                  █▄░░░░▄█
                                                   ▀████▀
";

static HELPFUL_ADVICE: &[&str] = &[
    "Add assemblies to your request by right-clicking them in Part Master",
    "Ctrl+Shift+I opens the Import dialog to paste CSV or JSON data. (Secret)",
    "Use the Backend button to configure PocketBase for cloud sync.",
    "Part Master shows all parts; filter by category tabs or search.",
    "Ctrl+Z undoes edits in text fields. Save BOM revisions to preserve history.",
    "Part numbers must be unique. Use Part Master to add new parts.",
    "Set quantity in the Request Edit tab. Aggregation multiplies by request quantities.",
    "Click Save in BOM Edit to create a revision with a comment.",
    "Data auto-saves to PocketBase when configured. Local storage saves on exit.",
    "Search by part number or description in Part Master or BOM Edit.",
    "Add assemblies to your request for kitting; the system aggregates all sub-parts.",
    "Preview shows aggregated parts from the current request. Use tags to filter.",
    "Expand a sub-assembly in BOM Edit to include its parts when building kitting lists.",
    "Quantity per assembly: multiply by how many assemblies you're requesting.",
    "Use the search bar in Part Master or BOM Edit to find parts quickly.",
    "Expand: when Yes, sub-assembly parts are listed instead of the assembly itself. Only applies when UOM is EA.",
    "Location codes help locate parts in physical storage. Edit in Part Master.",
    "View past revisions in BOM Edit to compare changes. Save creates new revisions.",
    "Batch quantity on assemblies: default requested amount when adding to a request.",
    "Add custom fields in Part Master for supplier, lead time, or other metadata.",
    "Tags: label BOM lines (e.g. hardware, electronic). Toggle tag visibility in Preview to filter the kitting list.",
    "Preview tab shows the aggregated kitting list. Export to CSV, HTML, or PDF.",
    "A part with BOM entries is an assembly. Add it to request to get all its parts.",
    "Ctrl+Shift+I: paste JSON or CSV. Data merges with existing parts by part number.",
    "Configure PocketBase URL in Backend to sync data across devices.",
    "Toggle Dark mode in the top bar for reduced eye strain and hair loss.",
    "Filter Part Master or BOM Edit by typing part number or description.",
    "Expand sub-assemblies in BOM Edit to show nested structure. Use for kitting.",
    "Part numbers identify parts uniquely. Use a consistent format.",
    "Disable BOM entries to exclude them from kitting without deleting.",
    "Requested quantities multiply through assembly hierarchies for total part counts.",
    "Preview tab: generate HTML for printing. Opens in browser or downloads.",
    "Export PDF from the Preview tab (native only). Same layout as print preview.",
    "Requested by and machine number: add in Request Edit for kitting labels.",
    "Machine number identifies the request. Use in Request Edit.",
    "Add parts to BOM: use Add Part in BOM Edit. Create new parts in Part Master.",
    "Part Master categories (EA, ME, SD, TR, Others) filter the parts list.",
    "You can create your own part master categories.",
    "Part descriptions appear in the kitting list. Edit in Part Master.",
    "UOM: EA (each), IN (inches), M (meters), MM (millimeters). Use EA for countable parts, others for length.",
];


#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct KanbanBomsApp {
    pub boms: Vec<Bom>,
    pub bom_entries: Vec<BomEntry>,
    pub requests: Vec<Request>,
    pub request_entries: Vec<RequestEntry>,
    pub current_request_id: Option<Uuid>,
    #[serde(skip)]
    pub bom_edit_search_modal: bool,
    #[serde(skip)]
    pub bom_edit_search_query: String,
    #[serde(skip)]
    pub selected_bom_for_edit: Option<Uuid>,
    #[serde(skip)]
    pub import_text: String,
    #[serde(skip)]
    pub import_modal_open: bool,
    #[serde(skip)]
    pub import_error: Option<String>,
    #[serde(skip)]
    pub location_edit_modal: Option<(Uuid, String)>,
    #[serde(skip)]
    pub partno_add_input: String,
    #[serde(skip)]
    pub partno_add_error: Option<String>,
    /// Tag visibility in preview: true = show parts with this tag
    #[serde(skip)]
    pub preview_tag_visible: HashMap<String, bool>,
    /// In-progress tags edit: (bom_id, part_id) -> current text (allows spaces while typing)
    #[serde(skip)]
    pub bom_edit_tags_buffer: HashMap<(Uuid, Uuid), String>,
    /// BOM Preview: which columns to include in HTML print (Part Number, Description, Location, UOM, Qty, Tags). PDF ignores this.
    #[serde(skip, default = "default_bom_preview_columns_in_html")]
    pub bom_preview_columns_in_html: [bool; 6],
    /// BOM Preview: data table (egui-data-table)
    #[serde(skip)]
    pub bom_preview_table: egui_data_table::DataTable<BomPreviewRow>,
    /// BOM Preview: last sync key (data_version, request_id, parts_len, filtered_len, total_qty) to avoid replacing rows every frame
    #[serde(skip)]
    pub bom_preview_last_sync_key: Option<(u64, Uuid, usize, usize, i32)>,
    /// Cached (bom_entries, aggregated parts) for BOM preview; only valid when Preview tab is active. Cleared when switching to Edit.
    #[serde(skip)]
    pub bom_preview_parts_cache: Option<(u64, Uuid, Vec<BomEntry>, Vec<AggregatedPart>)>,
    /// When switching to Preview with cache miss: defer heavy BOM build to next frame so the mode switch paints immediately.
    #[serde(skip)]
    pub bom_preview_deferred_build: Option<Uuid>,
    /// Request Edit: cached (partno, description) per assembly; invalid when assemblies or boms change. Avoids boms_by_id_ref() every frame when editing quantity.
    #[serde(skip)]
    pub request_edit_display_cache: Option<(Uuid, u64, Vec<Uuid>, Vec<(Uuid, String, String)>)>,
    /// Part Master: selected worksheet tab index
    #[serde(skip)]
    pub part_master_tab: usize,
    /// Part Master: visible category tabs (user can add/remove via right-click). "All" is always present.
    #[serde(default = "default_visible_categories")]
    pub part_master_visible_categories: Vec<String>,
    /// Part Master: Add Category popup open and input text
    #[serde(skip)]
    pub part_master_add_category_open: bool,
    #[serde(skip)]
    pub part_master_add_category_input: String,
    /// Part Master: Edit Part window. None = new part, Some(id) = edit existing. Opens new tab without closing others.
    #[serde(skip)]
    pub part_master_edit_part: Option<Option<Uuid>>,
    /// Per-tab state for Part Edit (tab_id -> state).
    #[serde(skip)]
    pub part_edit_states: HashMap<Uuid, PartEditState>,
    /// When a new-part tab is saved, maps tab_id -> created bom_id (for title and Edit BOM).
    #[serde(skip)]
    pub part_edit_new_to_bom: HashMap<Uuid, Uuid>,
    /// Part Edit tab(s) to close. Some(id) = close that tab, None = close none.
    #[serde(skip)]
    pub part_master_close_edit_tab: Option<Uuid>,
    /// Part Master: filter text to search parts (part number / description)
    #[serde(skip)]
    pub part_master_filter: String,
    /// Part Master: data table (egui-data-table)
    #[serde(skip)]
    pub part_master_table: egui_data_table::DataTable<PartMasterRow>,
    /// Part Master: last sync key (boms_version, tab, filter, boms_len, custom_sig, categories_len) to avoid replacing rows every frame
    #[serde(skip)]
    pub part_master_last_sync_key: Option<(u64, usize, String, usize, String, usize)>,
    /// BOM Edit: data table (egui-data-table)
    #[serde(skip)]
    pub bom_edit_table: egui_data_table::DataTable<BomEditRow>,
    /// BOM Edit: last sync key (data_version, bom_id, entries_count, viewing_revision) to avoid replacing rows every frame
    #[serde(skip)]
    pub bom_edit_last_sync_key: Option<(u64, Uuid, usize, Option<u32>)>,
    /// BOM revisions per bom_id (newest first)
    #[serde(default)]
    pub bom_revisions: HashMap<Uuid, Vec<BomRevision>>,
    /// Next revision number per bom_id
    #[serde(default)]
    pub bom_revision_next: HashMap<Uuid, u32>,
    /// BOM Edit: which revision is being viewed (None = current/editable)
    #[serde(skip)]
    pub bom_edit_viewing_revision: Option<u32>,
    /// BOM Save: modal for revision comment. Some(bom_id, comment) = modal open.
    #[serde(skip)]
    pub bom_save_revision_modal: Option<(Uuid, String)>,
    /// BOM Revert: modal for confirmation. Some(bom_id, revision) = modal open.
    #[serde(skip)]
    pub bom_revert_revision_modal: Option<(Uuid, u32)>,
    /// When Part Master "Edit BOM" is clicked, set this; dock will open a BOM Edit tab and clear it.
    #[serde(skip)]
    pub pending_open_bom: Option<Uuid>,
    /// When Part Master "Usage report" is clicked, set this; dock will open a Usage Report tab.
    #[serde(skip)]
    pub part_master_usage_report: Option<Uuid>,
    /// Usage Report: data table (egui-data-table)
    #[serde(skip)]
    pub usage_report_table: egui_data_table::DataTable<UsageReportRow>,
    /// Usage Report: last sync key (data_version, part_id, usages_count)
    #[serde(skip)]
    pub usage_report_last_sync_key: Option<(u64, Uuid, usize)>,
    /// Dock layout state (egui_dock).
    #[serde(skip)]
    pub dock_state: Option<egui_dock::DockState<DockTab>>,
    /// Per-request tab state (Edit/Preview mode).
    #[serde(skip)]
    pub request_states: std::collections::HashMap<Uuid, RequestTabState>,
    /// PocketBase API base URL (e.g. http://localhost:8090/api). Empty = local storage only.
    #[serde(default)]
    pub api_base_url: String,
    /// Backend settings modal open
    #[serde(skip)]
    pub backend_modal_open: bool,
    /// Last backend error message
    #[serde(skip)]
    pub backend_error: Option<String>,
    /// Status flash: (message, is_error). Green for success, red for error. Auto-clears after ~5s.
    #[serde(skip)]
    pub status_flash: Option<(String, bool)>,
    #[serde(skip)]
    pub status_flash_at: Option<f64>,
    /// About window open
    #[serde(skip)]
    pub about_modal_open: bool,
    /// Dark mode (persisted)
    #[serde(default)]
    pub dark_mode: bool,
    /// WASM: pending load result (channel receiver)
    #[serde(skip)]
    #[cfg(target_arch = "wasm32")]
    pub backend_load_rx: Option<std::sync::mpsc::Receiver<Result<StoredData, String>>>,
    /// WASM: pending save result (channel receiver)
    #[serde(skip)]
    #[cfg(target_arch = "wasm32")]
    pub backend_save_rx: Option<std::sync::mpsc::Receiver<Result<(), String>>>,
    /// Data changed since last save; triggers auto-save when backend is configured
    #[serde(skip)]
    pub data_dirty: bool,
    /// When data_dirty was first set (for debounce). None = reset timer on next mark_dirty.
    #[serde(skip)]
    pub data_dirty_debounce_at: Option<f64>,
    /// Incremented on every data change; included in view sync keys so all views refresh when any view saves
    #[serde(skip, default)]
    pub data_version: u64,
    /// Incremented only when boms/bom_entries change; Part Master and Usage Report use this to avoid rebuilds on request_entries changes
    #[serde(skip, default)]
    pub boms_version: u64,
    /// Mascot overlay: time when first shown. Hidden after ~20s.
    #[serde(skip)]
    pub mascot_shown_at: Option<f64>,
    /// Mascot overlay: random advice index (picked when first shown).
    #[serde(skip)]
    pub mascot_advice_index: Option<usize>,
}

fn default_bom_preview_columns_in_html() -> [bool; 6] {
    [true; 6]
}

fn default_visible_categories() -> Vec<String> {
    let mut cats = vec!["All".to_string()];
    cats.extend(crate::models::PART_CATEGORIES.iter().map(|s| s.to_string()));
    cats
}

impl Default for KanbanBomsApp {
    fn default() -> Self {
        let mut app = Self {
            boms: Vec::new(),
            bom_entries: Vec::new(),
            requests: Vec::new(),
            request_entries: Vec::new(),
            current_request_id: None,
            bom_edit_search_modal: false,
            bom_edit_search_query: String::new(),
            selected_bom_for_edit: None,
            import_text: String::new(),
            import_modal_open: false,
            import_error: None,
            location_edit_modal: None,
            partno_add_input: String::new(),
            partno_add_error: None,
            preview_tag_visible: HashMap::new(),
            bom_edit_tags_buffer: HashMap::new(),
            bom_preview_columns_in_html: [true; 6],
            bom_preview_table: egui_data_table::DataTable::new(),
            bom_preview_last_sync_key: None,
            bom_preview_parts_cache: None,
            bom_preview_deferred_build: None,
            request_edit_display_cache: None,
            part_master_tab: 0,
            part_master_visible_categories: default_visible_categories(),
            part_master_add_category_open: false,
            part_master_add_category_input: String::new(),
            part_master_edit_part: None,
            part_edit_states: HashMap::new(),
            part_edit_new_to_bom: HashMap::new(),
            part_master_close_edit_tab: None,
            part_master_filter: String::new(),
            part_master_table: egui_data_table::DataTable::new(),
            part_master_last_sync_key: None,
            bom_edit_table: egui_data_table::DataTable::new(),
            bom_edit_last_sync_key: None,
            bom_revisions: HashMap::new(),
            bom_revision_next: HashMap::new(),
            bom_edit_viewing_revision: None,
            bom_save_revision_modal: None,
            bom_revert_revision_modal: None,
            pending_open_bom: None,
            part_master_usage_report: None,
            usage_report_table: egui_data_table::DataTable::new(),
            usage_report_last_sync_key: None,
            dock_state: None,
            request_states: std::collections::HashMap::new(),
            api_base_url: String::new(),
            backend_modal_open: false,
            backend_error: None,
            status_flash: None,
            status_flash_at: None,
            about_modal_open: false,
            dark_mode: false,
            #[cfg(target_arch = "wasm32")]
            backend_load_rx: None,
            #[cfg(target_arch = "wasm32")]
            backend_save_rx: None,
            data_dirty: false,
            data_dirty_debounce_at: None,
            data_version: 0,
            boms_version: 0,
            mascot_shown_at: None,
            mascot_advice_index: None,
        };
        app.ensure_request();
        app
    }
}

impl KanbanBomsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };
        app.ensure_request();
        // If PocketBase URL configured, always load from backend on start
        #[cfg(not(target_arch = "wasm32"))]
        if !app.api_base_url.trim().is_empty() {
            let backend = Backend::PocketBase(PocketBaseBackend::new(&app.api_base_url));
            match backend.load_sync(cc.storage) {
                Ok(data) => {
                    app.apply_loaded_data(data);
                }
                Err(e) => {
                    app.backend_error = Some(e.to_string());
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        if !app.api_base_url.trim().is_empty() {
            app.load_from_backend(cc.storage);
        }
        recompute_bom_entry_counts(&mut app.boms, &app.bom_entries);
        app
    }

    fn backend(&self) -> Backend {
        if self.api_base_url.trim().is_empty() {
            Backend::Local(LocalBackend)
        } else {
            Backend::PocketBase(PocketBaseBackend::new(&self.api_base_url))
        }
    }

    fn load_from_backend(&mut self, storage: Option<&dyn eframe::Storage>) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let backend = self.backend();
            match backend.load_sync(storage) {
                Ok(data) => {
                    self.apply_loaded_data(data);
                    self.set_status_flash("Loaded from server", false);
                }
                Err(e) => {
                    let msg = e.to_string();
                    self.backend_error = Some(msg.clone());
                    self.set_status_flash(&msg, true);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let url = self.api_base_url.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.backend_load_rx = Some(rx);
            wasm_bindgen_futures::spawn_local(async move {
                let pb = PocketBaseBackend::new(&url);
                let result = pb.load().await.map_err(|e| e.to_string());
                let _ = tx.send(result);
            });
        }
    }

    fn apply_loaded_data(&mut self, data: StoredData) {
        self.boms = data.boms;
        self.bom_entries = data.bom_entries;
        recompute_bom_entry_counts(&mut self.boms, &self.bom_entries);
        // requests/request_entries are local state only, not stored in database
        self.bom_revisions = data.bom_revisions;
        self.bom_revision_next = data.bom_revision_next;
        self.part_master_visible_categories = data.part_master_categories;
        self.ensure_request();
        self.backend_error = None;
        self.data_dirty = false; // loaded from server, not dirty
    }

    fn set_status_flash(&mut self, msg: &str, is_error: bool) {
        self.status_flash = Some((msg.to_string(), is_error));
        self.status_flash_at = None; // set in update when we have ctx time
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.data_dirty = true;
        self.data_dirty_debounce_at = None; // reset debounce timer
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Call when only request_entries/requests change (local state, not persisted to server).
    /// Increments data_version for BOM preview cache invalidation, but does NOT trigger server save.
    pub(crate) fn mark_request_dirty(&mut self) {
        self.data_version = self.data_version.wrapping_add(1);
    }

    /// Call when boms or bom_entries change; increments both data_version and boms_version.
    /// Part Master and Usage Report use boms_version to avoid rebuilds when only request_entries change.
    pub(crate) fn mark_boms_dirty(&mut self) {
        self.mark_dirty();
        self.boms_version = self.boms_version.wrapping_add(1);
    }

    fn save_to_backend(&mut self, storage: Option<&mut dyn eframe::Storage>) {
        recompute_bom_entry_counts(&mut self.boms, &self.bom_entries);
        let data = StoredData {
            boms: self.boms.clone(),
            bom_entries: self.bom_entries.clone(),
            bom_revisions: self.bom_revisions.clone(),
            bom_revision_next: self.bom_revision_next.clone(),
            part_master_categories: self.part_master_visible_categories.clone(),
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let backend = self.backend();
            match backend.save_sync(&data, storage) {
                Ok(()) => {
                    self.backend_error = None;
                    self.set_status_flash("Saved to server", false);
                }
                Err(e) => {
                    let msg = e.to_string();
                    self.backend_error = Some(msg.clone());
                    self.set_status_flash(&msg, true);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let url = self.api_base_url.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            self.backend_save_rx = Some(rx);
            wasm_bindgen_futures::spawn_local(async move {
                let pb = PocketBaseBackend::new(&url);
                let result = pb.save(&data).await.map_err(|e| e.to_string());
                let _ = tx.send(result);
            });
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_backend_pending(&mut self) {
        use std::sync::mpsc::TryRecvError;
        if let Some(rx) = self.backend_load_rx.take() {
            match rx.try_recv() {
                Ok(Ok(data)) => {
                    self.apply_loaded_data(data);
                    self.set_status_flash("Loaded from server", false);
                }
                Ok(Err(e)) => {
                    self.backend_error = Some(e.clone());
                    self.set_status_flash(&e, true);
                }
                Err(TryRecvError::Empty) => self.backend_load_rx = Some(rx),
                Err(TryRecvError::Disconnected) => {}
            }
        }
        if let Some(rx) = self.backend_save_rx.take() {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    self.backend_error = None;
                    self.set_status_flash("Saved to server", false);
                }
                Ok(Err(e)) => {
                    self.backend_error = Some(e.clone());
                    self.set_status_flash(&e, true);
                }
                Err(TryRecvError::Empty) => self.backend_save_rx = Some(rx),
                Err(TryRecvError::Disconnected) => {}
            }
        }
    }
}

impl KanbanBomsApp {
    fn ensure_request(&mut self) {
        if self.current_request_id.is_none() || !self.requests.iter().any(|r| r.id == self.current_request_id.unwrap()) {
            let req = Request {
                id: Uuid::new_v4(),
                requested_by: String::new(),
                machine_number: String::new(),
                notes: String::new(),
            };
            self.current_request_id = Some(req.id);
            self.requests.push(req);
            self.mark_request_dirty();
        }
    }

    pub(crate) fn boms_by_id(&self) -> HashMap<Uuid, Bom> {
        self.boms.iter().map(|b| (b.id, b.clone())).collect()
    }

    /// O(1) lookup by id without cloning; use when only reading partno/description.
    pub(crate) fn boms_by_id_ref(&self) -> HashMap<Uuid, &Bom> {
        self.boms.iter().map(|b| (b.id, b)).collect()
    }

    /// BOM entries for request aggregation: uses latest revision when available, else bom_entries.
    /// Recursively includes entries for sub-assemblies when expand=true, so get_aggregated_parts
    /// can flatten expanded BOMs in the preview.
    pub(crate) fn effective_bom_entries_for_request(&self, request_id: Uuid) -> Vec<BomEntry> {
        let mut result = Vec::new();
        let mut to_process: Vec<Uuid> = self
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .map(|e| e.part_id)
            .collect();
        let mut processed = std::collections::HashSet::new();
        let is_assembly = |part_id: Uuid| {
            self.bom_entries.iter().any(|e| e.bom_id == part_id)
                || self.bom_revisions.contains_key(&part_id)
        };
        while let Some(bom_id) = to_process.pop() {
            if processed.contains(&bom_id) {
                continue;
            }
            processed.insert(bom_id);
            let entries = if let Some(revs) = self.bom_revisions.get(&bom_id) {
                revs.iter()
                    .max_by_key(|r| r.revision)
                    .map(|r| r.entries.clone())
                    .unwrap_or_else(|| {
                        self.bom_entries
                            .iter()
                            .filter(|e| e.bom_id == bom_id)
                            .cloned()
                            .collect()
                    })
            } else {
                self.bom_entries
                    .iter()
                    .filter(|e| e.bom_id == bom_id)
                    .cloned()
                    .collect()
            };
            for e in &entries {
                let uom_ea = e.uom.is_empty() || e.uom.eq_ignore_ascii_case("EA");
                if e.expand && uom_ea && is_assembly(e.part_id) {
                    to_process.push(e.part_id);
                }
            }
            result.extend(entries);
        }
        result
    }

    pub fn add_assembly_to_request(&mut self, bom_id: Uuid) {
        let request_id = match self.current_request_id {
            Some(id) => id,
            None => return,
        };
        let bom = match self.boms.iter().find(|b| b.id == bom_id) {
            Some(b) => b.clone(),
            None => return,
        };
        if let Some(re) = self.request_entries.iter_mut().find(|e| e.request_id == request_id && e.part_id == bom_id) {
            re.quantity += bom.batch_quantity.max(1);
        } else {
            self.request_entries.push(RequestEntry {
                request_id,
                part_id: bom_id,
                quantity: bom.batch_quantity.max(1),
            });
        }
        self.mark_request_dirty();
    }

    pub fn clear_request(&mut self) {
        if let Some(rid) = self.current_request_id {
            self.request_entries.retain(|e| e.request_id != rid);
            self.mark_request_dirty();
        }
    }

    pub fn export_csv(&self) -> String {
        let request_id = match self.current_request_id {
            Some(id) => id,
            None => return String::new(),
        };
        let boms_map = self.boms_by_id();
        let bom_entries = self.effective_bom_entries_for_request(request_id);
        let parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &bom_entries,
            &self.request_entries,
        );
        let mut w = Vec::new();
        {
            let mut writer = csv::Writer::from_writer(&mut w);
            let _ = writer.write_record(&["Part Number", "Description", "UOM", "Quantity"]);
            for (partno, desc, qty, uom, _, _) in &parts {
                let uom_display = if uom.is_empty() { "EA" } else { uom.as_str() };
                let _ = writer.write_record(&[partno, desc, uom_display, &qty.to_string()]);
            }
            let _ = writer.flush();
        }
        String::from_utf8_lossy(&w).to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn trigger_pdf_download(&self) {
        let request_id = match self.current_request_id {
            Some(id) => id,
            None => return,
        };
        let request = match self.requests.iter().find(|r| r.id == request_id) {
            Some(r) => r.clone(),
            None => return,
        };
        let assemblies: Vec<RequestEntry> = self
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .cloned()
            .collect();
        let boms_map = self.boms_by_id();
        let bom_entries = self.effective_bom_entries_for_request(request_id);
        let mut parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &bom_entries,
            &self.request_entries,
        );
        // Filter by tag visibility (same as preview)
        parts.retain(|(_, _, _, _, _, tags)| {
            tags.is_empty()
                || tags
                    .iter()
                    .any(|t| self.preview_tag_visible.get(t).copied().unwrap_or(true))
        });
        let sort_state = self.bom_preview_table.sort_state();
        crate::html_export::sort_parts_by_state(&mut parts, &sort_state);
        match generate_pdf(&request, &assemblies, &boms_map, &parts) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write("kitting_bom.pdf", &bytes) {
                    log::error!("Failed to write PDF: {}", e);
                }
            }
            Err(e) => log::error!("PDF generation failed: {}", e),
        }
    }

    pub fn trigger_print_preview(&mut self) {
        let request_id = match self.current_request_id {
            Some(id) => id,
            None => return,
        };
        let request = match self.requests.iter().find(|r| r.id == request_id) {
            Some(r) => r.clone(),
            None => return,
        };
        let assemblies: Vec<RequestEntry> = self
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .cloned()
            .collect();
        let boms_map = self.boms_by_id();
        let bom_entries = self.effective_bom_entries_for_request(request_id);
        let mut parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &bom_entries,
            &self.request_entries,
        );
        // Filter by tag visibility (same as preview)
        parts.retain(|(_, _, _, _, _, tags)| {
            tags.is_empty()
                || tags
                    .iter()
                    .any(|t| self.preview_tag_visible.get(t).copied().unwrap_or(true))
        });
        // Apply preview data grid sort order
        let sort_state = self.bom_preview_table.sort_state();
        crate::html_export::sort_parts_by_state(&mut parts, &sort_state);
        let column_order = self.bom_preview_table.column_order();
        let html = generate_html_print_preview(
            &request,
            &assemblies,
            &boms_map,
            &parts,
            &self.bom_preview_columns_in_html,
            column_order.as_deref(),
        );
        if let Some(path) = crate::app::open_html_in_browser(&html) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let path_str = path.canonicalize().unwrap_or(path.clone()).display().to_string();
                if arboard::Clipboard::new().and_then(|mut c| c.set_text(&path_str)).is_ok() {
                    self.set_status_flash(
                        "Preview saved to Downloads. Path copied to clipboard. If Firefox blocks it, use File > Open File and paste the path.",
                        false,
                    );
                } else {
                    self.set_status_flash(&format!("Preview saved to: {}", path_str), false);
                }
            }
        }
    }

    pub fn merge_import(&mut self, data: StoredData) {
        let mut id_map: HashMap<Uuid, Uuid> = HashMap::new();
        for bom in data.boms {
            if let Some(existing) = self.boms.iter().find(|b| b.partno == bom.partno) {
                id_map.insert(bom.id, existing.id);
            } else {
                let new_id = Uuid::new_v4();
                id_map.insert(bom.id, new_id);
                let mut new_bom = bom;
                new_bom.id = new_id;
                self.boms.push(new_bom);
            }
        }
        for be in data.bom_entries {
            if let (Some(&new_bom_id), Some(&new_part_id)) = (id_map.get(&be.bom_id), id_map.get(&be.part_id)) {
                if !self.bom_entries.iter().any(|e| e.bom_id == new_bom_id && e.part_id == new_part_id) {
                    self.bom_entries.push(BomEntry {
                        bom_id: new_bom_id,
                        part_id: new_part_id,
                        quantity: be.quantity,
                        uom: be.uom.clone(),
                        disabled: be.disabled,
                        expand: be.expand,
                        tags: be.tags.clone(),
                    });
                }
            }
        }
        for cat in &data.part_master_categories {
            if !self.part_master_visible_categories.iter().any(|c| c == cat) {
                self.part_master_visible_categories.push(cat.clone());
            }
        }
        recompute_bom_entry_counts(&mut self.boms, &self.bom_entries);
        self.mark_dirty();
    }
}

/// Opens HTML content in the default browser (native) or a new tab (WASM).
/// Returns the file path on native (for status message); None on WASM or on error.
pub fn open_html_in_browser(html: &str) -> Option<std::path::PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window().expect("No window");
        let array = js_sys::Uint8Array::from(html.as_bytes());
        let blob_parts = js_sys::Array::new();
        blob_parts.push(&array);
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type("text/html");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &opts)
            .expect("Failed to create blob");
        let url = web_sys::Url::create_object_url_with_blob(&blob).expect("Failed to create URL");
        let _ = window.open_with_url(&url);
        // Don't revoke URL immediately - the new window needs it to load
        // The URL will be garbage-collected when the window is closed
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // Save to ~/Downloads so user can open manually (Firefox blocks file:// from external apps)
        let path = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()
            .and_then(|h| {
                let dir = std::path::Path::new(&h).join("Downloads");
                let _ = std::fs::create_dir_all(&dir);
                Some(dir.join("kitting_bom_preview.html"))
            })
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "kitting_bom_preview_{}.html",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                ))
            });
        if let Err(e) = std::fs::write(&path, html) {
            log::error!("Failed to write HTML preview: {}", e);
            return None;
        }
        #[cfg(unix)]
        let _ = std::fs::metadata(&path).and_then(|m| {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = m.permissions();
            perms.set_mode(0o644);
            std::fs::set_permissions(&path, perms)
        });
        let _ = opener::open(&path); // Works for Chrome; Firefox blocks - path is copied for manual open
        Some(path)
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn download_bytes(data: &[u8], filename: &str) {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let array = js_sys::Uint8Array::from(data);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);
    let mime = if filename.ends_with(".pdf") {
        "application/pdf"
    } else {
        "text/csv"
    };
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &opts).unwrap();
    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
    let a = document.create_element("a").unwrap();
    let _ = a.set_attribute("href", &url);
    let _ = a.set_attribute("download", filename);
    a.dyn_ref::<web_sys::HtmlElement>().unwrap().click();
    web_sys::Url::revoke_object_url(&url).ok();
}

impl eframe::App for KanbanBomsApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        recompute_bom_entry_counts(&mut self.boms, &self.bom_entries);
        let data = StoredData {
            boms: self.boms.clone(),
            bom_entries: self.bom_entries.clone(),
            bom_revisions: self.bom_revisions.clone(),
            bom_revision_next: self.bom_revision_next.clone(),
            part_master_categories: self.part_master_visible_categories.clone(),
        };
        data.save_to_eframe(storage);
        // PocketBase sync is NOT done here: eframe calls save() periodically (auto_save_interval)
        // and on shutdown. A full sync would spam N DELETE + N POST requests per collection.
        // PocketBase sync happens only via: "Save to server" button, or debounced auto-save in update().
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });

        #[cfg(target_arch = "wasm32")]
        self.poll_backend_pending();

        // Ctrl+Shift+I: open JSON/CSV import modal
        let import_shortcut = KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::I);
        if ctx.input_mut(|i| i.consume_shortcut(&import_shortcut)) {
            self.import_modal_open = true;
            self.import_error = None;
        }

        let now = ctx.input(|i| i.time);
        if self.status_flash.is_some() {
            if self.status_flash_at.is_none() {
                self.status_flash_at = Some(now);
            }
            if let Some(at) = self.status_flash_at {
                if now - at > 5.0 {
                    self.status_flash = None;
                    self.status_flash_at = None;
                }
            }
        }

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.dark_mode, "Dark mode");
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some((ref msg, is_error)) = self.status_flash {
                    let color = if is_error {
                        egui::Color32::from_rgb(0xc0, 0x30, 0x30)
                    } else {
                        egui::Color32::from_rgb(0x28, 0x88, 0x48)
                    };
                    ui.colored_label(color, msg);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("About").clicked() {
                        self.about_modal_open = true;
                    }
                    if ui.button("Backend").clicked() {
                        self.backend_modal_open = true;
                    }
                });
            });
        });

        if self.about_modal_open {
            egui::Window::new("About Kanban BOMs v2.0")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading("Kanban BOMs v2.0");
                    ui.add_space(8.0);
                    ui.label("A BOM manager and a partmaster killer from the depths of production hell.");
                    ui.add_space(8.0);
                    ui.label("Features:");
                    ui.label("• Part master with categories, locations, and custom fields");
                    ui.label("• BOM editing with hierarchical assemblies, tags, and revision history");
                    ui.label("• Kitting requests with aggregated part lists");
                    ui.label("• CSV/JSON import, HTML print preview, PDF export (native)");
                    ui.label("• Optional PocketBase backend for cloud sync");
                    ui.add_space(8.0);
                    ui.label("Built with:");
                    ui.label("• egui / eframe — immediate-mode GUI");
                    ui.label("• egui_dock — tabbed dock layout");
                    ui.label("• egui-data-table — sortable data tables");
                    ui.label("• genpdf — PDF generation (native)");
                    ui.label("• PocketBase — optional backend");
                    ui.add_space(8.0);
                    ui.label("Rust • egui • MIT-style licenses");
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        self.about_modal_open = false;
                    }
                });
        }

        if self.backend_modal_open {
            egui::Window::new("Backend (PocketBase)")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("PocketBase API URL (e.g. http://localhost:8090/api):");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.api_base_url)
                            .desired_width(400.0),
                    );
                    if let Some(ref err) = self.backend_error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    ui.horizontal(|ui| {
                        let has_url = !self.api_base_url.trim().is_empty();
                        if ui
                            .add_enabled(has_url, egui::Button::new("Load from server"))
                            .clicked()
                        {
                            self.load_from_backend(None);
                        }
                        if ui
                            .add_enabled(has_url, egui::Button::new("Save to server"))
                            .clicked()
                        {
                            self.save_to_backend(None);
                        }
                        if ui.button("Close").clicked() {
                            self.backend_modal_open = false;
                            self.backend_error = None;
                        }
                    });
                });
        }

        if self.import_modal_open {
            egui::Window::new("Import Data")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 450.0])
                .show(ctx, |ui| {
                    ui.label("Paste CSV or JSON data:");
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.import_text)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(10),
                            );
                        });
                    if let Some(ref err) = self.import_error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Import").clicked() {
                            self.import_error = None;
                            if self.import_text.trim().starts_with('{') {
                                match serde_json::from_str::<StoredData>(&self.import_text) {
                                    Ok(data) => {
                                        self.merge_import(data);
                                        self.import_modal_open = false;
                                        self.import_text.clear();
                                    }
                                    Err(e) => self.import_error = Some(e.to_string()),
                                }
                            } else {
                                match parse_csv_import(&self.import_text) {
                                    Ok(data) => {
                                        self.merge_import(data);
                                        self.import_modal_open = false;
                                        self.import_text.clear();
                                    }
                                    Err(e) => self.import_error = Some(e),
                                }
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.import_modal_open = false;
                            self.import_error = None;
                        }
                    });
                });
        }

        let mut bom_save_confirm = false;
        let mut bom_save_cancel = false;
        if let Some((_bom_id, comment)) = self.bom_save_revision_modal.as_mut() {
            egui::Window::new("Save BOM Revision")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("What changed in this revision?");
                    ui.add(
                        egui::TextEdit::multiline(comment)
                            .desired_width(400.0)
                            .desired_rows(4),
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            bom_save_confirm = true;
                        }
                        if ui.button("Cancel").clicked() {
                            bom_save_cancel = true;
                        }
                    });
                });
        }
        if bom_save_confirm {
            if let Some((bom_id, comment)) = self.bom_save_revision_modal.take() {
                let entries: Vec<BomEntry> = self
                    .bom_entries
                    .iter()
                    .filter(|e| e.bom_id == bom_id)
                    .cloned()
                    .collect();
                let rev = *self.bom_revision_next.entry(bom_id).or_insert(1);
                self.bom_revisions
                    .entry(bom_id)
                    .or_default()
                    .insert(
                        0,
                        BomRevision {
                            revision: rev,
                            entries,
                            comment: comment.trim().to_string(),
                            created_at: Some(Utc::now().to_rfc3339()),
                        },
                    );
                *self.bom_revision_next.get_mut(&bom_id).unwrap() = rev + 1;
                self.bom_edit_viewing_revision = None;
                self.mark_boms_dirty();
            }
        }
        if bom_save_cancel {
            self.bom_save_revision_modal = None;
        }

        let mut bom_revert_confirm = false;
        let mut bom_revert_cancel = false;
        if let Some((_bom_id, rev)) = self.bom_revert_revision_modal.as_ref() {
            egui::Window::new("Revert BOM Revision")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Revert to revision {}? This will create a new revision with the same contents and make it the current (editable) state.",
                        rev
                    ));
                    ui.horizontal(|ui| {
                        if ui.button("Revert").clicked() {
                            bom_revert_confirm = true;
                        }
                        if ui.button("Cancel").clicked() {
                            bom_revert_cancel = true;
                        }
                    });
                });
        }
        if bom_revert_confirm {
            if let Some((bom_id, rev)) = self.bom_revert_revision_modal.take() {
                let entries: Vec<BomEntry> = self
                    .bom_revisions
                    .get(&bom_id)
                    .and_then(|revs| revs.iter().find(|r| r.revision == rev))
                    .map(|r| r.entries.clone())
                    .unwrap_or_default();
                self.bom_entries.retain(|e| e.bom_id != bom_id);
                self.bom_entries.extend(entries);
                let new_rev = *self.bom_revision_next.entry(bom_id).or_insert(1);
                self.bom_revisions
                    .entry(bom_id)
                    .or_default()
                    .insert(
                        0,
                        BomRevision {
                            revision: new_rev,
                            entries: self
                                .bom_entries
                                .iter()
                                .filter(|e| e.bom_id == bom_id)
                                .cloned()
                                .collect(),
                            comment: format!("Reverted to revision {}", rev),
                            created_at: Some(Utc::now().to_rfc3339()),
                        },
                    );
                *self.bom_revision_next.get_mut(&bom_id).unwrap() = new_rev + 1;
                self.bom_edit_viewing_revision = None;
                self.bom_edit_last_sync_key = None;
                crate::models::recompute_bom_entry_counts(&mut self.boms, &self.bom_entries);
                self.mark_boms_dirty();
            }
        }
        if bom_revert_cancel {
            self.bom_revert_revision_modal = None;
        }

        let mut ds = self.dock_state.take();
        if ds.is_none() {
            ds = Some(crate::dock::make_initial_dock_state(self));
        }
        if let Some(ref mut dock_state) = ds {
            show_dock_ui(dock_state, self, ctx);
        }
        self.dock_state = ds;

        // Auto-save when PocketBase is configured. Debounce to avoid freezing UI on add/remove.
        // (Local backend persists via eframe::App::save on exit/timer)
        if self.data_dirty && !self.api_base_url.trim().is_empty() {
            let now = ctx.input(|i| i.time);
            if self.data_dirty_debounce_at.is_none() {
                self.data_dirty_debounce_at = Some(now);
            }
            if let Some(t) = self.data_dirty_debounce_at {
                if now - t >= 1.5 {
                    self.save_to_backend(None);
                    self.data_dirty = false;
                    self.data_dirty_debounce_at = None;
                }
            }
        }

        // Mascot overlay: bottom-right, shows random advice for ~20s then disappears
        let now = ctx.input(|i| i.time);
        if self.mascot_shown_at.is_none() {
            self.mascot_shown_at = Some(now);
            self.mascot_advice_index = Some(rand::Rng::gen_range(&mut rand::thread_rng(), 0..HELPFUL_ADVICE.len()));
        }
        let show_duration = 20.0;
        if let (Some(at), Some(idx)) = (self.mascot_shown_at, self.mascot_advice_index) {
            if now - at < show_duration {
                let advice = HELPFUL_ADVICE[idx];
                egui::Area::new("mascot_overlay".into())
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::Vec2::new(-32.0, -32.0))
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(advice)
                                    .size(14.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new(BG_GRAPHIC)
                                    .monospace()
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100)),
                            );
                        });
                    });
            }
        }
    }
}

