use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bom {
    pub id: Uuid,
    pub partno: String,
    pub description: String,
    pub batch_quantity: i32,
    pub location: String,
    /// Arbitrary key-value fields associated with the part.
    #[serde(default)]
    pub custom_fields: HashMap<String, String>,
    /// Cached count of BOM entries (line items) for this assembly. Recomputed on load and when entries change.
    #[serde(default)]
    pub bom_entry_count: u32,
}

fn default_uom() -> String {
    "EA".to_string()
}

/// Unit of measure options for BOM entries.
pub const UOM_OPTIONS: &[&str] = &["EA", "IN", "M", "MM"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BomEntry {
    pub bom_id: Uuid,
    pub part_id: Uuid,
    pub quantity: i32,
    /// Unit of measure: EA (each), IN (inches), M (meters), MM (millimeters).
    #[serde(default = "default_uom")]
    pub uom: String,
    pub disabled: bool,
    /// When true and part_id is an assembly, its BOM contents are expanded into the request BOM.
    /// Only applies when uom is EA (each).
    #[serde(default)]
    pub expand: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Snapshot of BOM entries at a save point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BomRevision {
    pub revision: u32,
    pub entries: Vec<BomEntry>,
    /// User comment describing what changed in this revision.
    #[serde(default)]
    pub comment: String,
    /// When this revision was saved (ISO 8601).
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    pub id: Uuid,
    pub requested_by: String,
    pub machine_number: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestEntry {
    pub request_id: Uuid,
    pub part_id: Uuid,
    pub quantity: i32,
}

use once_cell::sync::Lazy;

static RE_5_2_3: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)\W*([0-9]{5})\s*-\s*([a-zA-Z]{2})\s*-\s*([0-9]{3})\s*").unwrap());
static RE_EL: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)\W*EL\s*-\s*([0-9]{4})\s*").unwrap());
/// Match any -XX- pattern (2 letters between dashes). Used for category extraction.
static RE_CATEGORY: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)-([a-zA-Z]{2})-").unwrap());

/// Recomputes bom_entry_count for all BOMs from bom_entries. Call after load and when entries change.
pub fn recompute_bom_entry_counts(boms: &mut [Bom], bom_entries: &[BomEntry]) {
    use std::collections::HashMap;
    let mut counts: HashMap<Uuid, u32> = HashMap::new();
    for e in bom_entries {
        *counts.entry(e.bom_id).or_insert(0) += 1;
    }
    for b in boms.iter_mut() {
        b.bom_entry_count = counts.get(&b.id).copied().unwrap_or(0);
    }
}

/// Part categories used in Part Master (worksheet-style tabs).
pub const PART_CATEGORIES: &[&str] = &["EA", "ME", "SD", "TR", "Others"];

/// Category from part number: extracts XX from any ..-XX-.. pattern. Returns the 2-letter code (e.g. SD, EA).
/// Parts that don't match (e.g. EL-1234) return "Others".
pub fn part_category(partno: &str) -> String {
    let upper = partno.to_uppercase();
    if let Some(caps) = RE_CATEGORY.captures(&upper) {
        return caps[1].to_string();
    }
    "Others".to_string()
}

/// Normalize part number per Django utils: 12345-XX-123 or EL-1234 format
pub fn normalize_partno(partno: &str) -> String {
    let upper = partno.to_uppercase();
    if let Some(caps) = RE_5_2_3.captures(&upper) {
        return format!("{}-{}-{}", &caps[1], &caps[2], &caps[3]);
    }
    if let Some(caps) = RE_EL.captures(&upper) {
        return format!("EL-{}", &caps[1]);
    }
    String::new()
}

/// Aggregated part for print/export: (partno, description, quantity, uom, location, tags)
pub type AggregatedPart = (String, String, i32, String, String, Vec<String>);

/// Recursively collect parts from an assembly's BOM. When expand=true, uom=EA, and the part is an assembly,
/// recurses into its BOM instead of adding the part itself.
fn collect_parts_recursive(
    assembly_id: Uuid,
    req_qty: i32,
    boms: &HashMap<Uuid, Bom>,
    bom_entries: &[BomEntry],
) -> Vec<(String, String, i32, String, String, Vec<String>)> {
    let mut result = Vec::new();
    let is_assembly = |part_id: Uuid| bom_entries.iter().any(|e| e.bom_id == part_id);

    for be in bom_entries.iter().filter(|e| e.bom_id == assembly_id && !e.disabled) {
        let component = match boms.get(&be.part_id) {
            Some(b) => b,
            None => continue,
        };
        let qty = req_qty * be.quantity;
        if qty <= 0 {
            continue;
        }

        let uom = if be.uom.is_empty() { "EA" } else { be.uom.as_str() };
        let can_expand = uom.eq_ignore_ascii_case("EA");
        if be.expand && can_expand && is_assembly(be.part_id) {
            let sub_parts = collect_parts_recursive(be.part_id, qty, boms, bom_entries);
            result.extend(sub_parts);
        } else {
            result.push((
                component.partno.clone(),
                component.description.clone(),
                qty,
                uom.to_string(),
                component.location.clone(),
                be.tags.clone(),
            ));
        }
    }
    result
}

pub fn get_aggregated_parts(
    request_id: Uuid,
    boms: &HashMap<Uuid, Bom>,
    bom_entries: &[BomEntry],
    request_entries: &[RequestEntry],
) -> Vec<AggregatedPart> {
    let mut parts: Vec<(String, String, i32, String, String, Vec<String>)> = Vec::new();

    for req_entry in request_entries.iter().filter(|e| e.request_id == request_id) {
        let assembly = match boms.get(&req_entry.part_id) {
            Some(b) => b,
            None => continue,
        };
        let req_qty = req_entry.quantity;
        let assembly_parts = collect_parts_recursive(assembly.id, req_qty, boms, bom_entries);
        parts.extend(assembly_parts);
    }

    // Group by (partno, uom): sum quantities and merge tags
    let mut groups: HashMap<(String, String), (String, String, i32, String, std::collections::HashSet<String>)> =
        HashMap::new();
    for (partno, desc, qty, uom, loc, tags) in parts {
        let key = (partno.clone(), uom.clone());
        groups
            .entry(key)
            .and_modify(|e| {
                e.2 += qty;
                e.4.extend(tags.iter().cloned());
            })
            .or_insert_with(|| {
                let mut tag_set = std::collections::HashSet::new();
                tag_set.extend(tags.into_iter());
                (partno, desc, qty, loc, tag_set)
            });
    }

    let mut result: Vec<AggregatedPart> = groups
        .into_iter()
        .map(|((partno, uom), (_, desc, qty, loc, tag_set))| {
            let mut tags: Vec<String> = tag_set.into_iter().collect();
            tags.sort();
            (partno, desc, qty, uom, loc, tags)
        })
        .collect();
    result.sort_by(|a, b| (a.4.as_str(), a.0.as_str()).cmp(&(b.4.as_str(), b.0.as_str())));
    result
}
