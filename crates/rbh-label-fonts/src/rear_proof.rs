//! QA-only rear-sheet generator. Uses the calibrated 20-position rear master,
//! preserving the sheet's marks and branding while regenerating QR symbols.
//! Physical duplex orientation and scan performance still require human QA.

use qrcodegen::{QrCode, QrCodeEcc};
use rbh_labels::PreparedBatch;
use serde_json::{Value, json};

const PAGE_WIDTH_PT: f64 = 648.0;
const PAGE_HEIGHT_PT: f64 = 864.0;
const QUIET_ZONE_MODULES: i32 = 4;

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value.get(key).ok_or_else(|| format!("missing {key}"))
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    field(value, key)?.as_f64().ok_or_else(|| format!("{key} must be numeric"))
}

fn find_node_mut(value: &mut Value, target: u64) -> Option<&mut Value> {
    if value.get("id").and_then(Value::as_u64) == Some(target) {
        return Some(value);
    }
    match value {
        Value::Array(items) => items.iter_mut().find_map(|item| find_node_mut(item, target)),
        Value::Object(fields) => fields.values_mut().find_map(|item| find_node_mut(item, target)),
        _ => None,
    }
}

fn find_node(value: &Value, target: u64) -> Option<&Value> {
    if value.get("id").and_then(Value::as_u64) == Some(target) {
        return Some(value);
    }
    match value {
        Value::Array(items) => items.iter().find_map(|item| find_node(item, target)),
        Value::Object(fields) => fields.values().find_map(|item| find_node(item, target)),
        _ => None,
    }
}

// The imported RBH rear EPS has 47 loose, white-filled square paths at the
// layer root. They are legacy QR backings, NOT part of the compound QR groups
// that Phase 3A replaces. Some overlap neighboring rows, and one is an
// accidental stray near the right crop marks. Retaining them causes the large
// white artifacts in the rear proof even when all 20 QR codes are populated.
//
// Only the unmodified, verified RBH rear master is supported here. Do not
// remove other white artwork or any object nested inside a logo or other group.
const EXPECTED_LEGACY_QR_BACKINGS: usize = 47;

fn is_legacy_qr_backing(node: &Value) -> bool {
    if node.get("name").is_some_and(|name| !name.is_null()) || node["kind"]["type"] != "path" {
        return false;
    }
    let Some(items) = node["appearance"]["items"].as_array() else { return false };
    if items.len() != 1 || items[0]["kind"] != "fill" || items[0]["paint"]["type"] != "solid" {
        return false;
    }
    let color = &items[0]["paint"]["color"];
    if color["model"] != "rgb" || ["r", "g", "b"].iter().any(|key| color[*key].as_f64() != Some(1.0)) {
        return false;
    }
    let Some(paths) = node["kind"]["path"]["subpaths"].as_array() else { return false };
    if paths.len() != 1 || paths[0]["closed"] != true {
        return false;
    }
    let Some(anchors) = paths[0]["anchors"].as_array() else { return false };
    if !(4..=5).contains(&anchors.len()) {
        return false;
    }
    let mut points = Vec::with_capacity(anchors.len());
    for anchor in anchors {
        let Some(x) = anchor["p"][0].as_f64() else { return false };
        let Some(y) = anchor["p"][1].as_f64() else { return false };
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        points.push((x, y));
    }
    let x0 = points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let x1 = points.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let y0 = points.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let y1 = points.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    if !(30.0..50.0).contains(&(x1 - x0)) || !(30.0..50.0).contains(&(y1 - y0)) {
        return false;
    }
    // All legacy backings are axis-aligned square/rectangular paths in the
    // QR area. Some have a fifth anchor duplicating the first corner.
    if x0 < 250.0 || x1 > 580.0 || y0 < 50.0 || y1 > 800.0 {
        return false;
    }
    points.iter().all(|&(x, y)| (x - x0).abs().min((x - x1).abs()) < 0.05 && (y - y0).abs().min((y - y1).abs()) < 0.05)
}

fn remove_legacy_qr_backings(master: &mut Value) -> Result<(), String> {
    let layers = master["document"]["layers"].as_array_mut().ok_or("rear master layers missing")?;
    if layers.len() != 1 {
        return Err("rear master must have exactly one layer for QR cleanup".into());
    }
    let nodes = layers[0]["kind"]["children"].as_array_mut().ok_or("rear root layer has no children")?;
    let found = nodes.iter().filter(|node| is_legacy_qr_backing(node)).count();
    if found != EXPECTED_LEGACY_QR_BACKINGS {
        return Err(format!(
            "rear master has {found} loose QR backgrounds, expected {EXPECTED_LEGACY_QR_BACKINGS}; inspect/recalibrate before export"
        ));
    }
    nodes.retain(|node| !is_legacy_qr_backing(node));
    Ok(())
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({"anchors": [
        {"p": [x, y]}, {"p": [x + width, y]},
        {"p": [x + width, y + height]}, {"p": [x, y + height]}
    ], "closed": true})
}

fn fill(white: bool) -> Value {
    let c = if white { 1.0 } else { 0.0 };
    json!({"items": [{"kind": "fill", "paint": {
        "type": "solid", "color": {"model": "rgb", "r": c, "g": c, "b": c}
    }}]})
}

/// The original QR artwork measures 29 horizontal module pitches but only
/// 28 vertical centerline gaps. The horizontal width is the intended size.
fn original_qr_black_width_pt(size: (f64, f64)) -> Result<f64, String> {
    let (width, height_span) = size;
    if !width.is_finite() || !height_span.is_finite() || width <= 0.0 || height_span <= 0.0 {
        return Err("rear QR calibration has an invalid size".into());
    }
    if (height_span - width * 28.0 / 29.0).abs() > 0.15 {
        return Err("rear QR has unexpected module geometry; recalibrate from master".into());
    }
    Ok(width)
}

fn next_node_id(next: &mut u64) -> Result<u64, String> {
    let id = *next;
    *next = next.checked_add(1).ok_or("document node ID overflow")?;
    Ok(id)
}

/// Generate a real ISO QR symbol from the URL provided by RBH's validated
/// certification importer, with a four-module UNPRINTED quiet zone.
/// Only black vector paths are generated; no opaque backing is drawn.
fn qr_art(url: &str, center: (f64, f64), black_size_pt: f64, next: &mut u64) -> Result<Vec<Value>, String> {
    if !center.0.is_finite() || !center.1.is_finite() || !black_size_pt.is_finite() || !(15.0..=50.0).contains(&black_size_pt) {
        return Err("invalid calibrated QR size or center".into());
    }
    let qr = QrCode::encode_text(url, QrCodeEcc::Medium).map_err(|_| "certification URL could not be QR encoded")?;
    let side = qr.size();
    if side <= 0 || side > 177 {
        return Err("unexpected QR module count".into());
    }
    let module_pt = black_size_pt / f64::from(side);
    let x0 = center.0 - black_size_pt / 2.0;
    let y0 = center.1 - black_size_pt / 2.0;
    let quiet = f64::from(QUIET_ZONE_MODULES) * module_pt;
    if x0 - quiet < 0.0 || y0 - quiet < 0.0 || x0 + black_size_pt + quiet > PAGE_WIDTH_PT || y0 + black_size_pt + quiet > PAGE_HEIGHT_PT {
        return Err("QR symbol plus quiet zone exceeds sheet bounds".into());
    }
    let mut modules = Vec::with_capacity((side * side / 2) as usize);
    for y in 0..side {
        for x in 0..side {
            if qr.get_module(x, y) {
                modules.push(rect(x0 + f64::from(x) * module_pt, y0 + f64::from(y) * module_pt, module_pt, module_pt));
            }
        }
    }
    let black_id = next_node_id(next)?;
    Ok(vec![json!({"id": black_id, "name": "rbh-qr-modules", "appearance": fill(false), "kind": {
        "type": "path", "path": {"subpaths": modules}
    }})])
}

fn point(value: &Value, name: &str) -> Result<(f64, f64), String> {
    let pair = field(value, name)?.as_array().ok_or_else(|| format!("{name} must be a pair"))?;
    if pair.len() != 2 {
        return Err(format!("{name} must contain exactly two values"));
    }
    let x = pair.first().and_then(Value::as_f64).ok_or_else(|| format!("{name} X is not numeric"))?;
    let y = pair.get(1).and_then(Value::as_f64).ok_or_else(|| format!("{name} Y is not numeric"))?;
    if !x.is_finite() || !y.is_finite() {
        return Err(format!("{name} coordinates must be finite"));
    }
    Ok((x, y))
}

/// A full, 20-label sheet is mandatory for production. The QA proof generator
/// deliberately allows fewer so tests can be reviewed without printing.
pub fn validate_full_sheet(count: usize) -> Result<(), String> {
    if count == 20 { Ok(()) } else { Err(format!("production sheet requires 20 approved labels; found {count}")) }
}

/// Rear slots follow the same position order as the front generator (L01..L20).
/// No horizontal mirroring is applied until duplex orientation has been verified.
/// The output is a draft, never an approved print-shop file.
pub fn generate_rear_proof(mut master: Value, calibration: &Value, batch: &PreparedBatch) -> Result<Value, String> {
    if batch.labels.is_empty() || batch.labels.len() > 20 {
        return Err("rear proof requires 1–20 imported certifications".into());
    }
    if master.get("format").and_then(Value::as_str) != Some("vectorcraft") {
        return Err("rear master is not a native VectorCraft document".into());
    }
    let document = field(&master, "document")?;
    if !field(document, "title")?.as_str().is_some_and(|s| s.starts_with("RBH_REAR_")) {
        return Err("expected an RBH rear-sheet master".into());
    }
    let artboards = field(document, "artboards")?.as_array().ok_or("artboards must be a list")?;
    if artboards.len() != 1 {
        return Err("rear master must have exactly one artboard".into());
    }
    let board = field(artboards.first().ok_or("rear artboard missing")?, "rect")?;
    if (number(board, "x1")? - number(board, "x0")? - PAGE_WIDTH_PT).abs() > 0.01
        || (number(board, "y1")? - number(board, "y0")? - PAGE_HEIGHT_PT).abs() > 0.01
    {
        return Err("rear master is not 648 × 864 pt".into());
    }
    let slots = field(field(calibration, "rear")?, "slots")?.as_array().ok_or("rear calibration slots missing")?;
    if slots.len() != 20 {
        return Err("rear calibration must have 20 slots".into());
    }
    let mut next_id = field(document, "next_id")?.as_u64().ok_or("master next_id missing")?;
    // Validate every planned replacement before mutating any QR group.
    let mut generated = Vec::with_capacity(20);
    for (i, slot) in slots.iter().enumerate() {
        if slot.get("position").and_then(Value::as_u64) != Some((i + 1) as u64) {
            return Err(format!("rear position {} is out of sequence", i + 1));
        }
        let id = field(slot, "qr_group_node_id")?.as_u64().ok_or("rear QR node ID missing")?;
        let original = find_node(&master, id).ok_or_else(|| format!("rear QR node {id} is absent"))?;
        if original["kind"]["type"].as_str() != Some("compound") {
            return Err(format!("rear QR node {id} is not the original compound artwork"));
        }
        let center = point(slot, "qr_center_pt")?;
        let size = point(slot, "qr_size_pt")?;
        let black_width = original_qr_black_width_pt(size)?;
        let children = if let Some(cert) = batch.labels.get(i) { qr_art(&cert.qr_url, center, black_width, &mut next_id)? } else { Vec::new() };
        generated.push((id, children));
    }
    // Clear old QR artwork. Each occupied position now receives a vector QR
    // whose dark modules match the corresponding certification URL.
    for (id, children) in generated {
        let qr_group = find_node_mut(&mut master, id).ok_or("calibrated QR vanished")?;
        qr_group.as_object_mut().ok_or("QR node must be an object")?.remove("appearance");
        qr_group["kind"] = json!({"type": "group", "clip": false, "children": children});
    }
    // The original white QR backings are separate top-level paths; replacing
    // their adjacent black compound artwork does not remove them. Clear the
    // 47 verified remnants (including the stray near the right crop marks).
    // Generated QR symbols contain no white-filled underlay.
    remove_legacy_qr_backings(&mut master)?;
    master["document"]["next_id"] = Value::from(next_id);
    Ok(master)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_legacy_unlinked_white_paths_not_new_qrs_or_branding() {
        let old_white = rect(274.5, 65.1, 38.7, 38.7);
        let old_path = |id| {
            json!({
                "id": id,
                "appearance": fill(true),
                "kind": {"type": "path", "path": {"subpaths": [old_white.clone()]}}
            })
        };
        let mut children: Vec<Value> = (100..147).map(old_path).collect();
        let qr_group = json!({"id": 250, "name": "generated QR", "kind": {
            "type": "group", "children": [{"id": 251, "name": "rbh-qr-modules",
                "appearance": fill(false), "kind": {"type": "path", "path": {"subpaths": [rect(270.0, 70.0, 1.0, 1.0)]}}}]
        }});
        let crop_marks = json!({"id": 300, "name": "crop-marks", "kind": {"type": "group", "children": []}});
        children.push(qr_group);
        children.push(crop_marks);
        let mut doc = json!({"document": {"layers": [{"kind": {"children": children}}]}});
        remove_legacy_qr_backings(&mut doc).unwrap();
        assert_eq!(doc["document"]["layers"][0]["kind"]["children"].as_array().unwrap().len(), 2);
        assert!(find_node(&doc, 251).is_some(), "generated black QR modules must survive");
        assert!(find_node(&doc, 300).is_some(), "registration marks must survive");
        assert!(find_node(&doc, 100).is_none(), "old loose white rectangle must be removed");
    }

    #[test]
    fn changed_master_blocks_cleanup_instead_of_removing_unrelated_artwork() {
        let mut doc = json!({"document": {"layers": [{"kind": {"children": [
            {"id": 1, "name": "important-white-art", "appearance": fill(true),
             "kind": {"type": "path", "path": {"subpaths": [rect(274.0, 65.0, 38.0, 38.0)]}}}
        ]}}]}});
        assert!(remove_legacy_qr_backings(&mut doc).is_err());
        assert!(find_node(&doc, 1).is_some());
    }

    #[test]
    fn distinct_certifications_generate_distinct_vector_qrs() {
        let mut ids = 30;
        let first = qr_art("https://www.rbhgrading.com/cert/12345678", (250.0, 200.0), 30.0, &mut ids).unwrap();
        let second = qr_art("https://www.rbhgrading.com/cert/12345679", (250.0, 200.0), 30.0, &mut ids).unwrap();
        assert_eq!(first.len(), 1, "only black QR modules may be emitted");
        assert_eq!(second.len(), 1, "only black QR modules may be emitted");
        assert_ne!(first[0]["kind"]["path"], second[0]["kind"]["path"]);
        assert_eq!(first[0]["name"], "rbh-qr-modules");
        assert_eq!(first[0]["appearance"], fill(false));
    }

    #[test]
    fn original_master_module_pitch_is_preserved() {
        let size = original_qr_black_width_pt((30.6852, 29.6270)).unwrap();
        let qr = QrCode::encode_text("https://www.rbhgrading.com/cert/12345678", QrCodeEcc::Medium).unwrap();
        assert_eq!(qr.size(), 29);
        assert!((size / f64::from(qr.size()) - 1.05811).abs() < 0.001);
        assert!(original_qr_black_width_pt((30.6852, 25.0)).is_err());
    }

    #[test]
    fn qr_symbols_contain_no_white_paths_or_underlays() {
        let mut ids = 1;
        let shapes = qr_art("https://www.rbhgrading.com/cert/12345678", (295.0, 80.0), 30.6852, &mut ids).unwrap();
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0]["name"], "rbh-qr-modules");
        assert_eq!(shapes[0]["appearance"], fill(false));
        assert!(!shapes[0].to_string().contains("rbh-qr-quiet-zone"));
    }

    #[test]
    fn bad_qr_coordinates_are_rejected() {
        let mut ids = 10;
        assert!(qr_art("https://www.rbhgrading.com/cert/12345678", (0.0, 200.0), 30.0, &mut ids).is_err());
        assert!(qr_art("https://www.rbhgrading.com/cert/12345678", (200.0, 200.0), f64::NAN, &mut ids).is_err());
    }

    #[test]
    fn production_requires_twenty_labels() {
        assert!(validate_full_sheet(3).is_err());
        assert!(validate_full_sheet(19).is_err());
        assert!(validate_full_sheet(20).is_ok());
        assert!(validate_full_sheet(21).is_err());
    }
}
