use crate::bom_edit::bom_edit_ui;
use crate::bom_preview::bom_preview_ui;
use crate::models::{get_aggregated_parts, Bom, BomEntry, Request, RequestEntry};
use crate::pdf_export::generate_pdf;
use crate::request_edit::request_edit_ui;
use crate::storage::{parse_csv_import, StoredData};
use egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Screen {
    RequestEdit,
    BomEdit,
    BomPreview,
}

impl Default for Screen {
    fn default() -> Self {
        Screen::RequestEdit
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct KanbanBomsApp {
    pub boms: Vec<Bom>,
    pub bom_entries: Vec<BomEntry>,
    pub requests: Vec<Request>,
    pub request_entries: Vec<RequestEntry>,
    pub current_request_id: Option<Uuid>,
    pub current_screen: Screen,
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
}

impl Default for KanbanBomsApp {
    fn default() -> Self {
        let mut app = Self {
            boms: Vec::new(),
            bom_entries: Vec::new(),
            requests: Vec::new(),
            request_entries: Vec::new(),
            current_request_id: None,
            current_screen: Screen::RequestEdit,
            bom_edit_search_modal: false,
            bom_edit_search_query: String::new(),
            selected_bom_for_edit: None,
            import_text: String::new(),
            import_modal_open: false,
            import_error: None,
            location_edit_modal: None,
            partno_add_input: String::new(),
            partno_add_error: None,
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

    fn seed_test_data(&mut self) {
        use crate::models::{Bom, BomEntry};
        let b1 = Uuid::new_v4();
        let b2 = Uuid::new_v4();
        let b3 = Uuid::new_v4();
        let b4 = Uuid::new_v4();
        let b5 = Uuid::new_v4();
        let b6 = Uuid::new_v4();
        self.boms = vec![
            Bom { id: b1, partno: "10001-AB-001".into(), description: "Main Assembly".into(), batch_quantity: 1, location: "A1".into() },
            Bom { id: b2, partno: "10002-CD-002".into(), description: "Sub Assembly".into(), batch_quantity: 1, location: "A2".into() },
            Bom { id: b3, partno: "20001-EF-001".into(), description: "Steel Bolt M8".into(), batch_quantity: 0, location: "B1".into() },
            Bom { id: b4, partno: "20002-GH-002".into(), description: "Hex Nut M8".into(), batch_quantity: 0, location: "B2".into() },
            Bom { id: b5, partno: "20003-IJ-003".into(), description: "Washer 8mm".into(), batch_quantity: 0, location: "B3".into() },
            Bom { id: b6, partno: "EL-1234".into(), description: "Electronic Module".into(), batch_quantity: 1, location: "C1".into() },
        ];
        self.bom_entries = vec![
            BomEntry { bom_id: b1, part_id: b2, quantity: 2, disabled: false },
            BomEntry { bom_id: b1, part_id: b3, quantity: 8, disabled: false },
            BomEntry { bom_id: b1, part_id: b4, quantity: 8, disabled: false },
            BomEntry { bom_id: b1, part_id: b5, quantity: 16, disabled: false },
            BomEntry { bom_id: b1, part_id: b6, quantity: 1, disabled: false },
            BomEntry { bom_id: b2, part_id: b3, quantity: 4, disabled: false },
            BomEntry { bom_id: b2, part_id: b4, quantity: 4, disabled: false },
            BomEntry { bom_id: b2, part_id: b5, quantity: 8, disabled: false },
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
        let parts = get_aggregated_parts(
            request_id,
            &boms_map,
            &self.bom_entries,
            &self.request_entries,
        );
        let mut w = Vec::new();
        {
            let mut writer = csv::Writer::from_writer(&mut w);
            let _ = writer.write_record(&["Part Number", "Description", "Quantity"]);
            for (partno, desc, qty, _) in &parts {
                let _ = writer.write_record(&[partno, desc, &qty.to_string()]);
            }
            let _ = writer.flush();
        }
        String::from_utf8_lossy(&w).to_string()
    }

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
        match generate_pdf(&request, &assemblies, &boms_map, &self.bom_entries, &self.request_entries) {
            Ok(bytes) => {
                #[cfg(target_arch = "wasm32")]
                download_bytes(&bytes, "kitting_bom.pdf");
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Err(e) = std::fs::write("kitting_bom.pdf", &bytes) {
                        log::error!("Failed to write PDF: {}", e);
                    }
                }
            }
            Err(e) => log::error!("PDF generation failed: {}", e),
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
                        disabled: be.disabled,
                    });
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn download_bytes(data: &[u8], filename: &str) {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let array = js_sys::Uint8Array::from(data);
    let blob = web_sys::Blob::new_with_u8_array_and_options(array.as_ref(), &web_sys::BlobPropertyBag::new().type_("application/pdf")).unwrap();
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
        };
        data.save_to_eframe(storage);
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::light());

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.current_screen, Screen::RequestEdit, "Requests");
                ui.selectable_value(&mut self.current_screen, Screen::BomEdit, "BOM Editor");
                ui.separator();
                if ui.button("Import CSV/JSON").clicked() {
                    self.import_modal_open = true;
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

        match self.current_screen {
            Screen::RequestEdit => {
                request_edit_ui(self, ctx);
            }
            Screen::BomEdit => {
                bom_edit_ui(self, ctx);
            }
            Screen::BomPreview => {
                bom_preview_ui(self, ctx);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn download_bytes_impl(data: &[u8], filename: &str) {
    use wasm_bindgen::JsCast;
    let window = web_sys::window().expect("No window");
    let document = window.document().expect("No document");
    let array = js_sys::Uint8Array::from(data);
    let mime = if filename.ends_with(".pdf") {
        "application/pdf"
    } else {
        "text/csv"
    };
    let blob = web_sys::Blob::new_with_u8_array_and_options(
        array.as_ref(),
        &web_sys::BlobPropertyBag::new().type_(mime),
    )
    .unwrap();
    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
    let a = document.create_element("a").unwrap();
    let _ = a.set_attribute("href", &url);
    let _ = a.set_attribute("download", filename);
    a.dyn_ref::<web_sys::HtmlElement>().unwrap().click();
    web_sys::Url::revoke_object_url(&url).ok();
}
