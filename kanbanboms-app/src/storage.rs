use crate::models::{Bom, BomEntry, BomRevision};
use serde::{Deserialize, Serialize};

#[cfg(target_arch = "wasm32")]
use gloo_storage::Storage;
use std::collections::HashMap;
use uuid::Uuid;

fn default_part_master_categories() -> Vec<String> {
    let mut cats = vec!["All".to_string()];
    cats.extend(crate::models::PART_CATEGORIES.iter().map(|s| s.to_string()));
    cats
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredData {
    pub boms: Vec<Bom>,
    pub bom_entries: Vec<BomEntry>,
    /// Revisions per BOM: bom_id -> list of revisions (newest first)
    #[serde(default)]
    pub bom_revisions: HashMap<Uuid, Vec<BomRevision>>,
    /// Next revision number per BOM
    #[serde(default)]
    pub bom_revision_next: HashMap<Uuid, u32>,
    /// Part Master visible category tabs (shared between users when using PocketBase)
    #[serde(default = "default_part_master_categories")]
    pub part_master_categories: Vec<String>,
}

impl StoredData {
    pub const STORAGE_KEY: &'static str = "kanbanboms_data";

    pub fn save_to_eframe(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(self) {
            storage.set_string(Self::STORAGE_KEY, json);
        }
    }

    pub fn load_from_eframe(storage: &dyn eframe::Storage) -> Option<Self> {
        storage.get_string(Self::STORAGE_KEY).and_then(|json| {
            serde_json::from_str(&json).ok()
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save_to_local_storage(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            let _ = gloo_storage::LocalStorage::set(Self::STORAGE_KEY, json);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load_from_local_storage() -> Option<Self> {
        gloo_storage::LocalStorage::get(Self::STORAGE_KEY).ok()
    }
}

/// Parse CSV in populate_db format: row[1]=assembly partno, row[2]=desc, row[6]=qty, row[8]=disabled, row[9]=component partno, row[10]=component desc
pub fn parse_csv_import(csv_text: &str) -> Result<StoredData, String> {
    use crate::models::normalize_partno;
    use std::collections::HashMap;

    let mut reader = csv::Reader::from_reader(csv_text.as_bytes());
    let mut boms: HashMap<String, (String, Vec<(String, String, i32, bool)>)> = HashMap::new();

    for result in reader.records() {
        let record = result.map_err(|e| e.to_string())?;
        if record.len() < 11 {
            continue;
        }
        let assembly_partno = normalize_partno(&record[1]);
        if assembly_partno.is_empty() {
            continue;
        }
        let assembly_desc = record[2].to_string();
        let component_partno = normalize_partno(&record[9]);
        if component_partno.is_empty() {
            continue;
        }
        let component_desc = record.get(10).map(|s| s.to_string()).unwrap_or_default();
        let quantity: i32 = record.get(6).and_then(|s| s.parse().ok()).unwrap_or(1);
        let disabled = record
            .get(8)
            .map(|s| s.to_lowercase().replace(' ', "") == "yes")
            .unwrap_or(false);

        boms.entry(assembly_partno.clone())
            .or_insert_with(|| (assembly_desc, Vec::new()))
            .1
            .push((component_partno, component_desc, quantity, disabled));
    }

    let mut stored = StoredData {
        part_master_categories: default_part_master_categories(),
        ..Default::default()
    };
    let mut partno_to_id: HashMap<String, Uuid> = HashMap::new();

    for (asm_partno, (asm_desc, parts)) in boms {
        let asm_id = *partno_to_id
            .entry(asm_partno.clone())
            .or_insert_with(|| {
                let id = Uuid::new_v4();
                stored.boms.push(Bom {
                    id,
                    partno: asm_partno.clone(),
                    description: asm_desc,
                    batch_quantity: 1,
                    location: String::new(),
                    custom_fields: HashMap::new(),
                    bom_entry_count: 0,
                });
                id
            });

        for (comp_partno, comp_desc, qty, disabled) in parts {
            let comp_id = *partno_to_id.entry(comp_partno.clone()).or_insert_with(|| {
                let id = Uuid::new_v4();
                stored.boms.push(Bom {
                    id,
                    partno: comp_partno.clone(),
                    description: comp_desc,
                    batch_quantity: 0,
                    location: String::new(),
                    custom_fields: HashMap::new(),
                    bom_entry_count: 0,
                });
                id
            });
            stored.bom_entries.push(BomEntry {
                bom_id: asm_id,
                part_id: comp_id,
                quantity: qty,
                uom: "EA".to_string(),
                disabled,
                expand: false,
                tags: Vec::new(),
            });
        }
    }

    Ok(stored)
}
