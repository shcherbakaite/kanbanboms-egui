use crate::bom_edit::BomEditRow;
use crate::bom_preview::BomPreviewRow;
use crate::part_master::PartMasterRow;
use crate::dock::{show_dock_ui, DockTab, PartEditState, RequestTabState};
use crate::models::{get_aggregated_parts, Bom, BomEntry, BomRevision, Request, RequestEntry};
use crate::html_export::generate_html_print_preview;
#[cfg(not(target_arch = "wasm32"))]
use crate::pdf_export::generate_pdf;
use crate::storage::{parse_csv_import, StoredData};
use egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
    /// BOM Preview: data table (egui-data-table)
    #[serde(skip)]
    pub bom_preview_table: egui_data_table::DataTable<BomPreviewRow>,
    /// BOM Preview: last sync key (request_id, parts_len, filtered_len) to avoid replacing rows every frame
    #[serde(skip)]
    pub bom_preview_last_sync_key: Option<(Uuid, usize, usize)>,
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
    /// Part Master: last sync key (tab, filter, boms_len) to avoid replacing rows every frame
    #[serde(skip)]
    pub part_master_last_sync_key: Option<(usize, String, usize, String)>,
    /// BOM Edit: data table (egui-data-table)
    #[serde(skip)]
    pub bom_edit_table: egui_data_table::DataTable<BomEditRow>,
    /// BOM Edit: last sync key (bom_id, entries_count, viewing_revision) to avoid replacing rows every frame
    #[serde(skip)]
    pub bom_edit_last_sync_key: Option<(Uuid, usize, Option<u32>)>,
    /// BOM revisions per bom_id (newest first)
    #[serde(default)]
    pub bom_revisions: HashMap<Uuid, Vec<BomRevision>>,
    /// Next revision number per bom_id
    #[serde(default)]
    pub bom_revision_next: HashMap<Uuid, u32>,
    /// BOM Edit: which revision is being viewed (None = current/editable)
    #[serde(skip)]
    pub bom_edit_viewing_revision: Option<u32>,
    /// When Part Master "Edit BOM" is clicked, set this; dock will open a BOM Edit tab and clear it.
    #[serde(skip)]
    pub pending_open_bom: Option<Uuid>,
    /// Dock layout state (egui_dock).
    #[serde(skip)]
    pub dock_state: Option<egui_dock::DockState<DockTab>>,
    /// Per-request tab state (Edit/Preview mode).
    #[serde(skip)]
    pub request_states: std::collections::HashMap<Uuid, RequestTabState>,
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
            bom_preview_table: egui_data_table::DataTable::new(),
            bom_preview_last_sync_key: None,
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
            pending_open_bom: None,
            dock_state: None,
            request_states: std::collections::HashMap::new(),
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
        if app.boms.is_empty() {
            app.seed_test_data();
        }
        app
    }

    /// Add 1000 random parts per category (EA, ME, SD, TR, Others). Skips part numbers that already exist.
    pub fn add_seed_parts(&mut self) {
        use crate::models::Bom;
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let existing: std::collections::HashSet<String> = self.boms.iter().map(|b| b.partno.clone()).collect();
        const DESCRIPTIONS: &[&str] = &[
            "Assembly", "Bracket", "Bushing", "Cap", "Clip", "Cover", "Gasket", "Guide",
            "Handle", "Housing", "Insert", "Plate", "Plug", "Retainer", "Ring", "Seal",
            "Shim", "Sleeve", "Spacer", "Spring", "Stud", "Washer", "Block", "Rod",
        ];
        const PER_CATEGORY: usize = 1000;
        for (category, base_start) in [("EA", 10000u32), ("ME", 20000), ("SD", 30000), ("TR", 40000)] {
            for seq in 1..=PER_CATEGORY {
                let base = base_start + seq as u32;
                let partno = format!("{:05}-{}-{:03}", base, category, seq);
                if existing.contains(&partno) {
                    continue;
                }
                let desc_idx = rng.gen_range(0..DESCRIPTIONS.len());
                let description = format!("{} {}", DESCRIPTIONS[desc_idx], partno);
                let loc = format!("{}{}", category, (seq % 20) + 1);
                self.boms.push(Bom {
                    id: Uuid::new_v4(),
                    partno,
                    description,
                    batch_quantity: 0,
                    location: loc,
                    custom_fields: HashMap::new(),
                });
            }
        }
        for seq in 1..=PER_CATEGORY {
            let partno = format!("EL-{:04}", seq);
            if existing.contains(&partno) {
                continue;
            }
            let desc_idx = rng.gen_range(0..DESCRIPTIONS.len());
            let description = format!("{} {}", DESCRIPTIONS[desc_idx], partno);
            let loc = format!("EL{}", (seq % 20) + 1);
            self.boms.push(Bom {
                id: Uuid::new_v4(),
                partno,
                description,
                batch_quantity: 0,
                location: loc,
                custom_fields: HashMap::new(),
            });
        }
    }

    fn seed_test_data(&mut self) {
        use crate::models::{Bom, BomEntry};
        let mut rng = rand::thread_rng();
        let b1 = Uuid::new_v4();
        let b2 = Uuid::new_v4();
        let b3 = Uuid::new_v4();
        let b4 = Uuid::new_v4();
        let b5 = Uuid::new_v4();
        let b6 = Uuid::new_v4();
        let mut boms = vec![
            Bom { id: b1, partno: "10001-AB-001".into(), description: "Main Assembly".into(), batch_quantity: 1, location: "A1".into(), custom_fields: [("Supplier".into(), "Acme Corp".into()), ("Lead Time".into(), "2 weeks".into())].into_iter().collect() },
            Bom { id: b2, partno: "10002-CD-002".into(), description: "Sub Assembly".into(), batch_quantity: 1, location: "A2".into(), custom_fields: [("Supplier".into(), "Beta Inc".into())].into_iter().collect() },
            Bom { id: b3, partno: "20001-EF-001".into(), description: "Steel Bolt M8".into(), batch_quantity: 0, location: "B1".into(), custom_fields: HashMap::new() },
            Bom { id: b4, partno: "20002-GH-002".into(), description: "Hex Nut M8".into(), batch_quantity: 0, location: "B2".into(), custom_fields: [("Lead Time".into(), "1 week".into())].into_iter().collect() },
            Bom { id: b5, partno: "20003-IJ-003".into(), description: "Washer 8mm".into(), batch_quantity: 0, location: "B3".into(), custom_fields: HashMap::new() },
            Bom { id: b6, partno: "EL-1234".into(), description: "Electronic Module".into(), batch_quantity: 1, location: "C1".into(), custom_fields: HashMap::new() },
        ];
        const DESCRIPTIONS: &[&str] = &[
            "Assembly", "Bracket", "Bushing", "Cap", "Clip", "Cover", "Gasket", "Guide",
            "Handle", "Housing", "Insert", "Plate", "Plug", "Retainer", "Ring", "Seal",
            "Shim", "Sleeve", "Spacer", "Spring", "Stud", "Washer", "Block", "Rod",
        ];
        use rand::Rng;
        const PER_CATEGORY: usize = 1000;
        for (category, base_start) in [("EA", 10000u32), ("ME", 20000), ("SD", 30000), ("TR", 40000)] {
            for seq in 1..=PER_CATEGORY {
                let base = base_start + seq as u32;
                let partno = format!("{:05}-{}-{:03}", base, category, seq);
                let desc_idx = rng.gen_range(0..DESCRIPTIONS.len());
                let description = format!("{} {}", DESCRIPTIONS[desc_idx], partno);
                let loc = format!("{}{}", category, (seq % 20) + 1);
                boms.push(Bom {
                    id: Uuid::new_v4(),
                    partno,
                    description,
                    batch_quantity: 0,
                    location: loc,
                    custom_fields: HashMap::new(),
                });
            }
        }
        for seq in 1..=PER_CATEGORY {
            let partno = format!("EL-{:04}", seq);
            let desc_idx = rng.gen_range(0..DESCRIPTIONS.len());
            let description = format!("{} {}", DESCRIPTIONS[desc_idx], partno);
            let loc = format!("EL{}", (seq % 20) + 1);
            boms.push(Bom {
                id: Uuid::new_v4(),
                partno,
                description,
                batch_quantity: 0,
                location: loc,
                custom_fields: HashMap::new(),
            });
        }
        self.boms = boms;
        self.bom_entries = vec![
            BomEntry { bom_id: b1, part_id: b2, quantity: 2, disabled: false, tags: vec!["subassembly".into()] },
            BomEntry { bom_id: b1, part_id: b3, quantity: 8, disabled: false, tags: vec!["hardware".into()] },
            BomEntry { bom_id: b1, part_id: b4, quantity: 8, disabled: false, tags: vec!["hardware".into()] },
            BomEntry { bom_id: b1, part_id: b5, quantity: 16, disabled: false, tags: vec!["hardware".into()] },
            BomEntry { bom_id: b1, part_id: b6, quantity: 1, disabled: false, tags: vec!["electronic".into()] },
            BomEntry { bom_id: b2, part_id: b3, quantity: 4, disabled: false, tags: vec!["hardware".into()] },
            BomEntry { bom_id: b2, part_id: b4, quantity: 4, disabled: false, tags: vec!["hardware".into()] },
            BomEntry { bom_id: b2, part_id: b5, quantity: 8, disabled: false, tags: vec!["hardware".into()] },
        ];
        if let Some(rid) = self.current_request_id {
            self.request_entries.push(RequestEntry { request_id: rid, part_id: b1, quantity: 2 });
        }
    }

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
        }
    }

    pub(crate) fn boms_by_id(&self) -> HashMap<Uuid, Bom> {
        self.boms.iter().map(|b| (b.id, b.clone())).collect()
    }

    /// BOM entries for request aggregation: uses latest revision when available, else bom_entries.
    pub(crate) fn effective_bom_entries_for_request(&self, request_id: Uuid) -> Vec<BomEntry> {
        let mut result = Vec::new();
        let assembly_ids: std::collections::HashSet<Uuid> = self
            .request_entries
            .iter()
            .filter(|e| e.request_id == request_id)
            .map(|e| e.part_id)
            .collect();
        for bom_id in assembly_ids {
            let entries = if let Some(revs) = self.bom_revisions.get(&bom_id) {
                revs.first()
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
    }

    pub fn clear_request(&mut self) {
        if let Some(rid) = self.current_request_id {
            self.request_entries.retain(|e| e.request_id != rid);
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
            let _ = writer.write_record(&["Part Number", "Description", "Quantity"]);
            for (partno, desc, qty, _, _) in &parts {
                let _ = writer.write_record(&[partno, desc, &qty.to_string()]);
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
        match generate_pdf(&request, &assemblies, &boms_map, &bom_entries, &self.request_entries) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write("kitting_bom.pdf", &bytes) {
                    log::error!("Failed to write PDF: {}", e);
                }
            }
            Err(e) => log::error!("PDF generation failed: {}", e),
        }
    }

    pub fn trigger_print_preview(&self) {
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
        let html = generate_html_print_preview(
            &request,
            &assemblies,
            &boms_map,
            &bom_entries,
            &self.request_entries,
        );
        crate::app::open_html_in_browser(&html);
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
                        disabled: be.disabled,
                        tags: be.tags.clone(),
                    });
                }
            }
        }
    }
}

/// Opens HTML content in the default browser (native) or a new tab (WASM).
pub fn open_html_in_browser(html: &str) {
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
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::env::temp_dir().join("kitting_bom_preview.html");
        if let Err(e) = std::fs::write(&path, html) {
            log::error!("Failed to write HTML preview: {}", e);
            return;
        }
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&path).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", path.as_os_str()])
            .spawn();
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        log::warn!("Unsupported OS for opening HTML preview");
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
        let data = StoredData {
            boms: self.boms.clone(),
            bom_entries: self.bom_entries.clone(),
            requests: self.requests.clone(),
            request_entries: self.request_entries.clone(),
            bom_revisions: self.bom_revisions.clone(),
            bom_revision_next: self.bom_revision_next.clone(),
        };
        data.save_to_eframe(storage);
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::light());

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Import CSV/JSON").clicked() {
                    self.import_modal_open = true;
                }
                if ui.button("Seed 1000 parts per category").clicked() {
                    self.add_seed_parts();
                }
            });
        });

        if self.import_modal_open {
            egui::Window::new("Import Data")
                .collapsible(false)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("Paste CSV or JSON data:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import_text)
                            .desired_width(400.0)
                            .desired_rows(10),
                    );
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

        let mut ds = self.dock_state.take();
        if ds.is_none() {
            ds = Some(crate::dock::make_initial_dock_state(self));
        }
        if let Some(ref mut dock_state) = ds {
            show_dock_ui(dock_state, self, ctx);
        }
        self.dock_state = ds;
    }
}

