//! Backend abstraction for KanbanBOMs data storage.
//! - LocalBackend: JSON via eframe storage (offline/fallback)
//! - PocketBaseBackend: REST API to PocketBase (shared state)

use crate::models::{Bom, BomEntry, BomRevision};
use crate::storage::StoredData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub enum BackendError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Parse(String),
    Storage(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Http(e) => write!(f, "HTTP: {}", e),
            BackendError::Json(e) => write!(f, "JSON: {}", e),
            BackendError::Parse(s) => write!(f, "Parse: {}", s),
            BackendError::Storage(s) => write!(f, "Storage: {}", s),
        }
    }
}

impl std::error::Error for BackendError {}

/// PocketBase list response: { items: [...], page, perPage, totalItems, totalPages }
#[derive(Debug, Deserialize)]
struct PbListResponse<T> {
    items: Vec<T>,
}

/// PocketBase record DTOs (field names match collection schema)
#[derive(Debug, Serialize, Deserialize)]
struct PbBom {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    uuid: String,
    partno: String,
    description: String,
    batch_quantity: i32,
    location: String,
    #[serde(default)]
    custom_fields: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PbBomEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    bom_uuid: String,
    part_uuid: String,
    quantity: i32,
    disabled: bool,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PbBomRevision {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    bom_uuid: String,
    revision: u32,
    entries: Vec<PbBomEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PbPartMasterCategory {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    name: String,
    sort_order: i32,
}

pub enum Backend {
    Local(LocalBackend),
    PocketBase(PocketBaseBackend),
}

pub struct LocalBackend;

pub struct PocketBaseBackend {
    base_url: String,
    #[cfg(not(target_arch = "wasm32"))]
    client: reqwest::blocking::Client,
    #[cfg(target_arch = "wasm32")]
    client: reqwest::Client,
}

impl PocketBaseBackend {
    pub fn new(base_url: &str) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        #[cfg(not(target_arch = "wasm32"))]
        let client = reqwest::blocking::Client::builder()
            .build()
            .expect("reqwest blocking client");
        #[cfg(target_arch = "wasm32")]
        let client = reqwest::Client::builder()
            .build()
            .expect("reqwest client");
        Self { base_url, client }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn get_records_blocking<T: for<'de> Deserialize<'de>>(
        &self,
        collection: &str,
    ) -> Result<Vec<T>, BackendError> {
        let url = self.url(&format!(
            "collections/{}/records?perPage=500",
            collection
        ));
        let resp = self.client.get(&url).send().map_err(BackendError::Http)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(BackendError::Parse(format!("{}: {}", status, text)));
        }
        let body: PbListResponse<T> = resp.json().map_err(BackendError::Http)?;
        Ok(body.items)
    }

    #[cfg(target_arch = "wasm32")]
    async fn get_records<T: for<'de> Deserialize<'de>>(
        &self,
        collection: &str,
    ) -> Result<Vec<T>, BackendError> {
        let url = self.url(&format!(
            "collections/{}/records?perPage=500",
            collection
        ));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(BackendError::Http)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::Parse(format!("{}: {}", status, text)));
        }
        let body: PbListResponse<T> = resp.json().await.map_err(BackendError::Http)?;
        Ok(body.items)
    }

    #[cfg(target_arch = "wasm32")]
    async fn delete_all(&self, collection: &str) -> Result<(), BackendError> {
        let items: Vec<serde_json::Value> = self.get_records(collection).await?;
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let url = self.url(&format!("collections/{}/records/{}", collection, id));
                let _ = self.client.delete(&url).send().await.map_err(BackendError::Http)?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn delete_all_blocking(&self, collection: &str) -> Result<(), BackendError> {
        let items: Vec<serde_json::Value> = self.get_records_blocking(collection)?;
        for item in items {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let url = self.url(&format!("collections/{}/records/{}", collection, id));
                let _ = self.client.delete(&url).send().map_err(BackendError::Http)?;
            }
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    async fn create_record<T: Serialize>(
        &self,
        collection: &str,
        body: &T,
    ) -> Result<(), BackendError> {
        let url = self.url(&format!("collections/{}/records", collection));
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(BackendError::Http)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::Parse(format!("{}: {}", status, text)));
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn create_record_blocking<T: Serialize>(
        &self,
        collection: &str,
        body: &T,
    ) -> Result<(), BackendError> {
        let url = self.url(&format!("collections/{}/records", collection));
        let resp = self.client.post(&url).json(body).send().map_err(BackendError::Http)?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(BackendError::Parse(format!("{}: {}", status, text)));
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_blocking(&self) -> Result<StoredData, BackendError> {
        let pb_boms: Vec<PbBom> = self.get_records_blocking("boms")?;
        let pb_entries: Vec<PbBomEntry> = self.get_records_blocking("bom_entries")?;
        let pb_revisions: Vec<PbBomRevision> = self.get_records_blocking("bom_revisions")?;
        let pb_categories: Vec<PbPartMasterCategory> =
            self.get_records_blocking("part_master_categories")?;
        Self::parse_loaded_data(pb_boms, pb_entries, pb_revisions, pb_categories)
    }

    fn parse_loaded_data(
        pb_boms: Vec<PbBom>,
        pb_entries: Vec<PbBomEntry>,
        pb_revisions: Vec<PbBomRevision>,
        pb_categories: Vec<PbPartMasterCategory>,
    ) -> Result<StoredData, BackendError> {
        let boms: Vec<Bom> = pb_boms
            .into_iter()
            .map(|p| {
                let id = Uuid::parse_str(&p.uuid).map_err(|e| {
                    BackendError::Parse(format!("Invalid BOM uuid {}: {}", p.uuid, e))
                })?;
                Ok(Bom {
                    id,
                    partno: p.partno,
                    description: p.description,
                    batch_quantity: p.batch_quantity,
                    location: p.location,
                    custom_fields: p.custom_fields,
                    bom_entry_count: 0, // recomputed in apply_loaded_data
                })
            })
            .collect::<Result<_, BackendError>>()?;

        let uuid_to_bom: HashMap<Uuid, Uuid> = boms.iter().map(|b| (b.id, b.id)).collect();

        let bom_entries: Vec<BomEntry> = pb_entries
            .into_iter()
            .filter_map(|p| {
                let bom_id = Uuid::parse_str(&p.bom_uuid).ok()?;
                let part_id = Uuid::parse_str(&p.part_uuid).ok()?;
                if uuid_to_bom.contains_key(&bom_id) && uuid_to_bom.contains_key(&part_id) {
                    Some(BomEntry {
                        bom_id,
                        part_id,
                        quantity: p.quantity,
                        disabled: p.disabled,
                        tags: p.tags,
                    })
                } else {
                    None
                }
            })
            .collect();

        let mut bom_revisions: HashMap<Uuid, Vec<BomRevision>> = HashMap::new();
        let mut bom_revision_next: HashMap<Uuid, u32> = HashMap::new();
        for pr in pb_revisions {
            let bom_id = match Uuid::parse_str(&pr.bom_uuid) {
                Ok(id) => id,
                Err(_) => continue,
            };
            let entries: Vec<BomEntry> = pr
                .entries
                .into_iter()
                .filter_map(|e| {
                    let bom_u = Uuid::parse_str(&e.bom_uuid).ok()?;
                    let part_u = Uuid::parse_str(&e.part_uuid).ok()?;
                    if uuid_to_bom.contains_key(&bom_u) && uuid_to_bom.contains_key(&part_u) {
                        Some(BomEntry {
                            bom_id: bom_u,
                            part_id: part_u,
                            quantity: e.quantity,
                            disabled: e.disabled,
                            tags: e.tags,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            let rev = BomRevision {
                revision: pr.revision,
                entries,
            };
            bom_revisions
                .entry(bom_id)
                .or_default()
                .insert(0, rev);
        }
        for (bom_id, revs) in &bom_revisions {
            if let Some(max) = revs.iter().map(|r| r.revision).max() {
                bom_revision_next.insert(*bom_id, max + 1);
            }
        }

        let mut cats: Vec<PbPartMasterCategory> = pb_categories;
        cats.sort_by_key(|p| p.sort_order);
        let mut part_master_categories: Vec<String> = cats.into_iter().map(|p| p.name).collect();
        if !part_master_categories.iter().any(|c| c == "All") {
            part_master_categories.insert(0, "All".to_string());
        }

        Ok(StoredData {
            boms,
            bom_entries,
            bom_revisions,
            bom_revision_next,
            part_master_categories,
        })
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn load(&self) -> Result<StoredData, BackendError> {
        let pb_boms: Vec<PbBom> = self.get_records("boms").await?;
        let pb_entries: Vec<PbBomEntry> = self.get_records("bom_entries").await?;
        let pb_revisions: Vec<PbBomRevision> = self.get_records("bom_revisions").await?;
        let pb_categories: Vec<PbPartMasterCategory> =
            self.get_records("part_master_categories").await?;
        Self::parse_loaded_data(pb_boms, pb_entries, pb_revisions, pb_categories)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_blocking(&self, data: &StoredData) -> Result<(), BackendError> {
        self.delete_all_blocking("part_master_categories")?;
        self.delete_all_blocking("bom_revisions")?;
        self.delete_all_blocking("request_entries")?;
        self.delete_all_blocking("bom_entries")?;
        self.delete_all_blocking("requests")?;
        self.delete_all_blocking("boms")?;

        for b in &data.boms {
            let pb = PbBom {
                id: None,
                uuid: b.id.to_string(),
                partno: b.partno.clone(),
                description: b.description.clone(),
                batch_quantity: b.batch_quantity,
                location: b.location.clone(),
                custom_fields: b.custom_fields.clone(),
            };
            self.create_record_blocking("boms", &pb)?;
        }

        for e in &data.bom_entries {
            let pb = PbBomEntry {
                id: None,
                bom_uuid: e.bom_id.to_string(),
                part_uuid: e.part_id.to_string(),
                quantity: e.quantity,
                disabled: e.disabled,
                tags: e.tags.clone(),
            };
            self.create_record_blocking("bom_entries", &pb)?;
        }

        for (bom_id, revs) in &data.bom_revisions {
            for r in revs {
                let entries: Vec<PbBomEntry> = r
                    .entries
                    .iter()
                    .map(|e| PbBomEntry {
                        id: None,
                        bom_uuid: e.bom_id.to_string(),
                        part_uuid: e.part_id.to_string(),
                        quantity: e.quantity,
                        disabled: e.disabled,
                        tags: e.tags.clone(),
                    })
                    .collect();
                let pb = PbBomRevision {
                    id: None,
                    bom_uuid: bom_id.to_string(),
                    revision: r.revision,
                    entries,
                };
                self.create_record_blocking("bom_revisions", &pb)?;
            }
        }

        for (i, name) in data.part_master_categories.iter().enumerate() {
            let pb = PbPartMasterCategory {
                id: None,
                name: name.clone(),
                sort_order: (i + 1) as i32, // 1-based: PocketBase treats 0 as missing for required number
            };
            self.create_record_blocking("part_master_categories", &pb)?;
        }

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn save(&self, data: &StoredData) -> Result<(), BackendError> {
        // Full replace: delete all, then create
        self.delete_all("part_master_categories").await?;
        self.delete_all("bom_revisions").await?;
        self.delete_all("request_entries").await?;
        self.delete_all("bom_entries").await?;
        self.delete_all("requests").await?;
        self.delete_all("boms").await?;

        for b in &data.boms {
            let pb = PbBom {
                id: None,
                uuid: b.id.to_string(),
                partno: b.partno.clone(),
                description: b.description.clone(),
                batch_quantity: b.batch_quantity,
                location: b.location.clone(),
                custom_fields: b.custom_fields.clone(),
            };
            self.create_record("boms", &pb).await?;
        }

        for e in &data.bom_entries {
            let pb = PbBomEntry {
                id: None,
                bom_uuid: e.bom_id.to_string(),
                part_uuid: e.part_id.to_string(),
                quantity: e.quantity,
                disabled: e.disabled,
                tags: e.tags.clone(),
            };
            self.create_record("bom_entries", &pb).await?;
        }

        for (bom_id, revs) in &data.bom_revisions {
            for r in revs {
                let entries: Vec<PbBomEntry> = r
                    .entries
                    .iter()
                    .map(|e| PbBomEntry {
                        id: None,
                        bom_uuid: e.bom_id.to_string(),
                        part_uuid: e.part_id.to_string(),
                        quantity: e.quantity,
                        disabled: e.disabled,
                        tags: e.tags.clone(),
                    })
                    .collect();
                let pb = PbBomRevision {
                    id: None,
                    bom_uuid: bom_id.to_string(),
                    revision: r.revision,
                    entries,
                };
                self.create_record("bom_revisions", &pb).await?;
            }
        }

        for (i, name) in data.part_master_categories.iter().enumerate() {
            let pb = PbPartMasterCategory {
                id: None,
                name: name.clone(),
                sort_order: (i + 1) as i32, // 1-based: PocketBase treats 0 as missing for required number
            };
            self.create_record("part_master_categories", &pb).await?;
        }

        Ok(())
    }
}

impl Backend {
    pub fn load_sync(
        &self,
        storage: Option<&dyn eframe::Storage>,
    ) -> Result<StoredData, BackendError> {
        match self {
            Backend::Local(_) => {
                let storage = storage.ok_or_else(|| {
                    BackendError::Storage("No storage available".to_string())
                })?;
                StoredData::load_from_eframe(storage).ok_or_else(|| {
                    BackendError::Storage("Failed to load from storage".to_string())
                })
            }
            Backend::PocketBase(pb) => {
                #[cfg(not(target_arch = "wasm32"))]
                return pb.load_blocking();
                #[cfg(target_arch = "wasm32")]
                return Err(BackendError::Storage(
                    "PocketBase load on WASM must use load_async".to_string(),
                ));
            }
        }
    }

    pub async fn load_async(&self) -> Result<StoredData, BackendError> {
        match self {
            Backend::Local(_) => Err(BackendError::Storage(
                "Local backend requires sync load with storage".to_string(),
            )),
            Backend::PocketBase(pb) => {
                #[cfg(target_arch = "wasm32")]
                return pb.load().await;
                #[cfg(not(target_arch = "wasm32"))]
                return Err(BackendError::Storage(
                    "PocketBase load on native must use load_sync".to_string(),
                ));
            }
        }
    }

    pub fn save_sync(
        &self,
        data: &StoredData,
        storage: Option<&mut dyn eframe::Storage>,
    ) -> Result<(), BackendError> {
        match self {
            Backend::Local(_) => {
                if let Some(s) = storage {
                    data.save_to_eframe(s);
                    Ok(())
                } else {
                    Err(BackendError::Storage("No storage available".to_string()))
                }
            }
            Backend::PocketBase(pb) => {
                #[cfg(not(target_arch = "wasm32"))]
                return pb.save_blocking(data);
                #[cfg(target_arch = "wasm32")]
                return Err(BackendError::Storage(
                    "PocketBase save on WASM must use save_async".to_string(),
                ));
            }
        }
    }

    pub async fn save_async(&self, data: &StoredData) -> Result<(), BackendError> {
        match self {
            Backend::Local(_) => Err(BackendError::Storage(
                "Local backend requires sync save with storage".to_string(),
            )),
            Backend::PocketBase(pb) => {
                #[cfg(target_arch = "wasm32")]
                return pb.save(data).await;
                #[cfg(not(target_arch = "wasm32"))]
                return Err(BackendError::Storage(
                    "PocketBase save on native must use save_sync".to_string(),
                ));
            }
        }
    }
}
