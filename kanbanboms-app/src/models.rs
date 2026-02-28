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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BomEntry {
    pub bom_id: Uuid,
    pub part_id: Uuid,
    pub quantity: i32,
    pub disabled: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Snapshot of BOM entries at a save point.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BomRevision {
    pub revision: u32,
    pub entries: Vec<BomEntry>,
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

/// Part categories used in Part Master (worksheet-style tabs).
pub const PART_CATEGORIES: &[&str] = &["EA", "ME", "SD", "TR", "Others"];

/// Category from part number suffix: 12345-XX-123 -> XX. Returns the actual 2-letter code (e.g. GH, EA).
/// Parts that don't match the format (e.g. EL-1234) return "Others".
pub fn part_category(partno: &str) -> String {
    let upper = partno.to_uppercase();
    if let Some(caps) = RE_5_2_3.captures(&upper) {
        return caps[2].to_string();
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

/// Aggregated part for print/export: (partno, description, quantity, location, tags)
pub type AggregatedPart = (String, String, i32, String, Vec<String>);

pub fn get_aggregated_parts(
    request_id: Uuid,
    boms: &HashMap<Uuid, Bom>,
    bom_entries: &[BomEntry],
    request_entries: &[RequestEntry],
) -> Vec<AggregatedPart> {
    let mut parts: Vec<(String, String, i32, String, Vec<String>)> = Vec::new();

    for req_entry in request_entries.iter().filter(|e| e.request_id == request_id) {
        let assembly = match boms.get(&req_entry.part_id) {
            Some(b) => b,
            None => continue,
        };
        let req_qty = req_entry.quantity;

        for be in bom_entries.iter().filter(|e| e.bom_id == assembly.id && !e.disabled) {
            let component = match boms.get(&be.part_id) {
                Some(b) => b,
                None => continue,
            };
            let qty = req_qty * be.quantity;
            if qty > 0 {
                parts.push((
                    component.partno.clone(),
                    component.description.clone(),
                    qty,
                    component.location.clone(),
                    be.tags.clone(),
                ));
            }
        }
    }

    // Group by partno: sum quantities and merge tags
    let mut groups: HashMap<String, (String, String, i32, String, std::collections::HashSet<String>)> =
        HashMap::new();
    for (partno, desc, qty, loc, tags) in parts {
        groups
            .entry(partno.clone())
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
        .map(|(_, (partno, desc, qty, loc, tag_set))| {
            let mut tags: Vec<String> = tag_set.into_iter().collect();
            tags.sort();
            (partno, desc, qty, loc, tags)
        })
        .collect();
    result.sort_by(|a, b| (a.3.as_str(), a.0.as_str()).cmp(&(b.3.as_str(), b.0.as_str())));
    result
}
