//! Produce a *QA-only* native VectorCraft front-sheet proof from the RBH master.
//!
//! Uses unmodified local masters and externally supplied calibration. Never
//! exports for print and never approves a label. No real card data is committed.
use rbh_labels::layout::TextMeasurer;
use rbh_labels::{Classification, PreparedBatch};
use serde_json::Value;
use vectorcraft_text::FontDb;

use crate::{ExactFontMeasurer, FrontLabelGeometry, LabelTypeface, fit_front_label};

const EXPECTED_WIDTH: f64 = 648.0;
const EXPECTED_HEIGHT: f64 = 864.0;
const DEFAULT_CLEARANCE_PT: f64 = 3.0; // Provisional; QA proof only.

fn get<'a>(obj: &'a Value, key: &str) -> Result<&'a Value, String> {
    obj.get(key).ok_or_else(|| format!("missing field {key}"))
}

fn number(obj: &Value, key: &str) -> Result<f64, String> {
    get(obj, key)?.as_f64().ok_or_else(|| format!("{key} must be a number"))
}

fn node_id(field: &Value) -> Result<Option<u64>, String> {
    if field.is_null() {
        return Ok(None);
    }
    get(field, "node_id")?.as_u64().map(Some).ok_or("field node_id must be a positive integer".into())
}

fn find_node(node: &Value, id: u64) -> Option<&Value> {
    if node.get("id").and_then(Value::as_u64) == Some(id) {
        return Some(node);
    }
    match node {
        Value::Array(a) => a.iter().find_map(|v| find_node(v, id)),
        Value::Object(m) => m.values().find_map(|v| find_node(v, id)),
        _ => None,
    }
}

fn find_node_mut(node: &mut Value, id: u64) -> Option<&mut Value> {
    if node.get("id").and_then(Value::as_u64) == Some(id) {
        return Some(node);
    }
    match node {
        Value::Array(a) => a.iter_mut().find_map(|v| find_node_mut(v, id)),
        Value::Object(m) => m.values_mut().find_map(|v| find_node_mut(v, id)),
        _ => None,
    }
}

fn text_obj(node: &Value) -> Result<&Value, String> {
    let kind = get(node, "kind")?;
    if get(kind, "type")?.as_str() != Some("text") {
        return Err("calibration node is not an editable text object".into());
    }
    let runs = get(kind, "runs")?.as_array().ok_or("text runs must be an array")?;
    if runs.len() != 1 {
        return Err("this proof requires a single text run per calibrated field".into());
    }
    Ok(kind)
}

fn text_and_x(node: &Value) -> Result<(String, f64), String> {
    let kind = text_obj(node)?;
    let txt = kind["runs"][0]["text"].as_str().ok_or("text run has no string")?.to_owned();
    let x = get(kind, "xf")?.as_array().and_then(|a| a.get(4)).and_then(Value::as_f64).ok_or("text transform lacks X origin")?;
    Ok((txt, x))
}

fn update_text(node: &mut Value, new_text: &str, new_x: Option<f64>) -> Result<(), String> {
    // Validate before mutation so no non-text object can be accidentally edited.
    text_obj(node)?;
    node["kind"]["runs"][0]["text"] = Value::String(new_text.to_owned());
    if let Some(x) = new_x {
        if !x.is_finite() {
            return Err("text origin must be finite".into());
        }
        let xf = node["kind"]["xf"].as_array_mut().ok_or("text transform is not an array")?;
        if xf.len() != 6 {
            return Err("text transform must have 6 components".into());
        }
        xf[4] = Value::from(x);
    }
    Ok(())
}

fn replace_at(master: &mut Value, id: u64, new_text: &str, new_x: Option<f64>) -> Result<(), String> {
    let node = find_node_mut(master, id).ok_or_else(|| format!("template node {id} not found"))?;
    update_text(node, new_text, new_x)
}

fn style_measurer<'a>(db: &'a FontDb, node: &Value) -> Result<ExactFontMeasurer<'a>, String> {
    let style = &text_obj(node)?["runs"][0]["style"];
    let family = get(style, "font_family")?.as_str().ok_or("font family missing")?;
    let weight = get(style, "font_style")?.as_str().ok_or("font style missing")?;
    if family != "Helvetica Neue" || weight != "Bold" {
        return Err(format!("unexpected production typeface {family} {weight}"));
    }
    let size_pt = number(style, "size")?;
    let tracking = style.get("tracking").and_then(Value::as_f64).unwrap_or(0.0);
    let horizontal_scale_percent = style.get("h_scale").and_then(Value::as_f64).unwrap_or(100.0);
    ExactFontMeasurer::new(db, LabelTypeface { family: family.to_owned(), style: weight.to_owned(), size_pt, tracking, horizontal_scale_percent })
}

fn replace_centered(master: &mut Value, id: u64, text: &str, db: &FontDb) -> Result<(), String> {
    let node = find_node(master, id).ok_or_else(|| format!("text node {id} is absent"))?;
    let (old_text, old_x) = text_and_x(node)?;
    let font = style_measurer(db, node)?;
    let original_width = font.width_pt(&old_text)?;
    let new_width = font.width_pt(text)?;
    replace_at(master, id, text, Some(old_x + (original_width - new_width) / 2.0))
}

fn id_for(fields: &Value, name: &str) -> Result<Option<u64>, String> {
    node_id(get(fields, name)?)
}

/// In a partial batch, remove the RBH logo from an *unfilled* label slot.
/// Only the monogram node identified by calibration is removed; the slot's
/// remaining structure, artwork in occupied slots and sheet crop marks stay.
fn remove_unused_monogram(master: &mut Value, slot: &Value) -> Result<(), String> {
    let group_id = get(slot, "group_node_id")?.as_u64().ok_or("slot group node ID missing")?;
    let group_name = get(slot, "group_name")?.as_str().ok_or("slot group name missing")?;
    let logo_id = get(get(slot, "monogram")?, "node_id")?.as_u64().ok_or("monogram node ID missing")?;
    let group = find_node_mut(master, group_id).ok_or_else(|| format!("slot {group_name} not found"))?;
    if group.get("name").and_then(Value::as_str) != Some(group_name) {
        return Err(format!("slot {group_name} group name does not match calibration"));
    }
    let children = group["kind"]["children"].as_array_mut().ok_or("slot has no children")?;
    let position = children
        .iter()
        .position(|node| node.get("id").and_then(Value::as_u64) == Some(logo_id))
        .ok_or_else(|| format!("slot {group_name} missing calibrated monogram {logo_id}"))?;
    let logo = children.get(position).ok_or("calibrated monogram index invalid")?;
    if logo["kind"]["type"].as_str() != Some("group")
        || !logo.get("name").and_then(Value::as_str).is_some_and(|name| name.starts_with("rbh-logo-local"))
    {
        return Err(format!("slot {group_name} calibrated monogram does not match the expected logo group"));
    }
    children.remove(position);
    Ok(())
}

/// Samples display the normal four subgrade headings with # placeholders,
/// making their labels useful as reference samples without assigning scores.
/// Custom/Alter labels reserve the entire subgrade area for future use.
fn subgrade_display(classification: Classification, numerical: Option<f64>) -> Result<Option<String>, String> {
    match classification {
        Classification::Graded => {
            let value = numerical.ok_or("graded certification has a missing subgrade")?;
            Ok(Some(value.to_string()))
        }
        Classification::Sample => Ok(Some("#".to_owned())),
        Classification::Custom => Ok(None),
    }
}

/// The master contains an empty `L04 title3` group, not a text node. Clone
/// `title2` into that existing group to create the missing editable baseline.
/// This is the only intentional structural addition to the proof artwork.
fn ensure_third_row(master: &mut Value, slot: &Value) -> Result<u64, String> {
    let fields = get(slot, "fields")?;
    if let Some(id) = id_for(fields, "title3")? {
        return Ok(id);
    }
    let group_name = get(slot, "group_name")?.as_str().ok_or("slot group_name missing")?;
    let row2_id = id_for(fields, "title2")?.ok_or("title2 must be present")?;
    let mut new_node = find_node(master, row2_id).cloned().ok_or("title2 node not found")?;
    text_obj(&new_node)?;
    let new_id = get(get(master, "document")?, "next_id")?.as_u64().ok_or("document next_id is missing")?;
    // Ensure the new ID is not already used anywhere in the document.
    if find_node(master, new_id).is_some() {
        return Err("document next_id collides with an existing node".into());
    }
    new_node["id"] = Value::from(new_id);
    let row2 = new_node["kind"]["xf"].as_array_mut().ok_or("title2 transform missing")?;
    let row2_y = row2.get(5).and_then(Value::as_f64).ok_or("title2 baseline missing")?;
    row2[5] = Value::from(row2_y + 8.5);
    new_node["kind"]["runs"][0]["style"]["leading"] = Value::from(8.4);
    update_text(&mut new_node, "", None)?;
    let group_id = get(slot, "group_node_id")?.as_u64().ok_or("slot group ID missing")?;
    let group = find_node_mut(master, group_id).ok_or("slot group missing")?;
    if group.get("name").and_then(Value::as_str) != Some(group_name) {
        return Err("slot group name does not match calibration".into());
    }
    let children = group["kind"]["children"].as_array_mut().ok_or("slot has no children")?;
    let title3_group_name = format!("{group_name} title3");
    let target = children
        .iter_mut()
        .find(|o| o.get("name").and_then(Value::as_str) == Some(title3_group_name.as_str()))
        .ok_or("empty title3 wrapper missing")?;
    let target_children = target["kind"]["children"].as_array_mut().ok_or("title3 wrapper has no children")?;
    if !target_children.is_empty() {
        return Err("title3 wrapper is not empty; calibration must be refreshed".into());
    }
    target_children.push(new_node);
    master["document"]["next_id"] = Value::from(new_id.checked_add(1).ok_or("node ID overflow")?);
    Ok(new_id)
}

fn verify_master(master: &Value, calibration: &Value) -> Result<(), String> {
    if master.get("format").and_then(Value::as_str) != Some("vectorcraft") {
        return Err("expected a native .vectorcraft master".into());
    }
    let doc = get(master, "document")?;
    // The calibration source names the converted .vectorcraft file, while
    // the native document retains its original EPS title. Confirm its type
    // and, below, every calibrated node ID and position.
    if !doc.get("title").and_then(Value::as_str).is_some_and(|s| s.starts_with("RBH_FRONT_")) {
        return Err("this is not an RBH front-sheet master".into());
    }
    let artboards = get(doc, "artboards")?.as_array().ok_or("master artboards missing")?;
    let rect = &artboards.first().ok_or("master has no artboard")?["rect"];
    let w = number(rect, "x1")? - number(rect, "x0")?;
    let h = number(rect, "y1")? - number(rect, "y0")?;
    if (w - EXPECTED_WIDTH).abs() > 0.01 || (h - EXPECTED_HEIGHT).abs() > 0.01 {
        return Err("master artboard is not 648 x 864 pt".into());
    }
    let slots = get(get(calibration, "front")?, "slots")?.as_array().ok_or("calibration slots missing")?;
    if slots.len() != 20 {
        return Err("expected exactly 20 calibrated front slots".into());
    }
    for (i, slot) in slots.iter().enumerate() {
        if slot.get("position").and_then(Value::as_u64) != Some((i + 1) as u64) {
            return Err(format!("slot {} is missing or out of sequence", i + 1));
        }
        let group_id = get(slot, "group_node_id")?.as_u64().ok_or("slot group ID missing")?;
        let group_name = get(slot, "group_name")?.as_str().ok_or("slot name missing")?;
        if find_node(master, group_id).and_then(|o| o.get("name")).and_then(Value::as_str) != Some(group_name) {
            return Err(format!("slot {group_name} was not found in this master"));
        }
        let fields = get(slot, "fields")?.as_object().ok_or("slot fields missing")?;
        for (name, field) in fields {
            if let Some(id) = node_id(field)? {
                let text = find_node(master, id).ok_or_else(|| format!("missing {group_name} {name} node {id}"))?;
                text_obj(text)?;
                // Spot-check the imported coordinate against calibration.
                if let Some(x) = field.get("origin_pt").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_f64) {
                    let (_, actual) = text_and_x(text)?;
                    if (x - actual).abs() > 0.2 {
                        return Err(format!("{group_name} {name} X coordinate changed; recalibrate"));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Populate a 20-position front sheet with up to 20 certification records.
/// Remaining slots have all text and their individual monograms removed; sheet
/// registration marks remain. Output is always an **unapproved QA draft**.
/// Errors return without modifying the original master.
pub fn generate_front_proof(mut master: Value, calibration: &Value, batch: &PreparedBatch, db: &FontDb) -> Result<Value, String> {
    verify_master(&master, calibration)?;
    if batch.labels.is_empty() || batch.labels.len() > 20 {
        return Err("front proof requires 1 to 20 imported certifications".into());
    }
    let slots = get(get(calibration, "front")?, "slots")?.as_array().ok_or("front calibration missing slots")?;
    let font = ExactFontMeasurer::new(db, LabelTypeface::helvetica_neue_bold(7.0, 0.0, 100.0))?;
    let mut layouts = Vec::with_capacity(batch.labels.len());
    // Preflight all occupied slots BEFORE changing the output document.
    for (i, label) in batch.labels.iter().enumerate() {
        let slot = slots.get(i).ok_or("certification has no corresponding slot")?;
        let fields = get(slot, "fields")?;
        let left = get(get(fields, "title1")?, "origin_pt")?.as_array().and_then(|a| a.first()).and_then(Value::as_f64).ok_or("missing text X")?;
        let bounds = get(get(slot, "monogram")?, "control_point_envelope_pt")?.as_array().ok_or("missing monogram boundary")?;
        let monogram = bounds.first().and_then(Value::as_f64).ok_or("missing monogram X")?;
        let geometry = FrontLabelGeometry { text_left_pt: [left; 4], monogram_left_pt: monogram, clearance_pt: DEFAULT_CLEARANCE_PT };
        let layout = fit_front_label(&label.card, geometry, &font)?;
        if !layout.fits() {
            return Err(format!("cert {}: card identification exceeds calibrated rows: {:?}", label.certification_number, layout.issues));
        }
        layouts.push(layout);
    }

    for (i, slot) in slots.iter().enumerate() {
        let fields = get(slot, "fields")?;
        if let Some(label) = batch.labels.get(i) {
            let layout = layouts.get(i).ok_or("internal layout count mismatch")?;
            for (name, content) in [("title1", layout.row_1.as_str()), ("title2", layout.row_2.as_str())] {
                let id = id_for(fields, name)?.ok_or_else(|| format!("required {name} field missing"))?;
                replace_at(&mut master, id, content, None)?;
            }
            let id = ensure_third_row(&mut master, slot)?;
            let title = match &layout.row_4 {
                Some(row4) => format!("{}\n{row4}", layout.row_3),
                None => layout.row_3.clone(),
            };
            replace_at(&mut master, id, &title, None)?;
            for (name, content) in
                [("grade", label.grade_display.as_str()), ("subtitle", label.grade_title.as_str()), ("cert", label.certification_number.as_str())]
            {
                let field_id = id_for(fields, name)?.ok_or_else(|| format!("required {name} field missing"))?;
                replace_centered(&mut master, field_id, content, db)?;
            }
            let subgrade_values = label.subgrades.as_ref();
            // These are fixed 2x2 positions. For Samples, preserve all four
            // headings and show # as an explicitly ungraded placeholder.
            // Custom/Alter certifications leave the complete section blank.
            for (label_name, value_name, heading, value) in [
                ("centering label", "centering value", "CENTERING", subgrade_values.map(|s| s.centering)),
                ("corners label", "corners value", "CORNERS", subgrade_values.map(|s| s.corners)),
                ("edges label", "edges value", "EDGES", subgrade_values.map(|s| s.edges)),
                ("surface label", "surface value", "SURFACE", subgrade_values.map(|s| s.surface)),
            ] {
                let lab_id = id_for(fields, label_name)?.ok_or("subgrade label node missing")?;
                let val_id = id_for(fields, value_name)?.ok_or("subgrade value node missing")?;
                match subgrade_display(label.classification, value)? {
                    Some(display) => {
                        replace_at(&mut master, lab_id, heading, None)?;
                        replace_at(&mut master, val_id, &display, None)?;
                    }
                    None => {
                        replace_at(&mut master, lab_id, "", None)?;
                        replace_at(&mut master, val_id, "", None)?;
                    }
                }
            }
        } else {
            // Absolutely no historical customer text survives in unused slots.
            let field_map = fields.as_object().ok_or("fields must be an object")?;
            for field in field_map.values() {
                if let Some(id) = node_id(field)? {
                    replace_at(&mut master, id, "", None)?;
                }
            }
            // Unfilled positions must not print a standalone RBH monogram.
            // The crop marks are separate sheet-level groups and stay intact.
            remove_unused_monogram(&mut master, slot)?;
        }
    }
    Ok(master)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_mutation_does_not_change_style_or_transform() {
        let mut n = json!({"id":4,"kind":{"type":"text","xf":[1,0,0,1,10,20],"runs":[{"text":"OLD","style":{"size":7}}]}});
        update_text(&mut n, "NEW", None).unwrap();
        assert_eq!(n["kind"]["runs"][0]["text"], "NEW");
        assert_eq!(n["kind"]["xf"][4], 10);
        assert_eq!(n["kind"]["runs"][0]["style"]["size"], 7);
    }

    #[test]
    fn lookup_is_by_stable_id_not_visible_text() {
        let tree = json!({"children":[{"id":3,"kind":{"type":"group"}},{"id":11,"kind":{"type":"text","xf":[1,0,0,1,5,2],"runs":[{"text":"#"}]}}]});
        assert_eq!(find_node(&tree, 11).and_then(|v| v.get("id")).and_then(Value::as_u64), Some(11));
        assert!(find_node(&tree, 99).is_none());
    }

    #[test]
    fn cannot_change_non_text_nodes() {
        let mut n = json!({"id":7,"kind":{"type":"path"}});
        assert!(update_text(&mut n, "9", None).is_err());
    }

    #[test]
    fn sample_subgrade_grid_is_present_but_not_numerically_graded() {
        assert_eq!(subgrade_display(Classification::Sample, None).unwrap().as_deref(), Some("#"));
        assert_eq!(subgrade_display(Classification::Graded, Some(9.5)).unwrap().as_deref(), Some("9.5"));
        assert!(subgrade_display(Classification::Graded, None).is_err());
    }

    #[test]
    fn unused_slot_removes_only_its_calibrated_monogram() {
        let mut master = json!({"id": 1, "kind": {"type": "group", "children": [
            {"id": 2, "name": "L04", "kind": {"type": "group", "children": [
                {"id": 3, "name": "rbh-logo-local-4", "kind": {"type": "group", "children": []}},
                {"id": 4, "name": "L04 title1", "kind": {"type": "group", "children": []}}
            ]}},
            {"id": 5, "name": "crop-marks", "kind": {"type": "group", "children": []}}
        ]}});
        let slot = json!({"group_node_id": 2, "group_name": "L04", "monogram": {"node_id": 3}});
        remove_unused_monogram(&mut master, &slot).unwrap();
        assert!(find_node(&master, 3).is_none());
        assert!(find_node(&master, 4).is_some());
        assert!(find_node(&master, 5).is_some());
    }

    #[test]
    fn unexpected_logo_id_is_not_removed() {
        let mut master = json!({"id": 1, "name": "L04", "kind": {"type": "group", "children": [
            {"id": 2, "name": "L04 title1", "kind": {"type": "group", "children": []}}
        ]}});
        let slot = json!({"group_node_id": 1, "group_name": "L04", "monogram": {"node_id": 2}});
        assert!(remove_unused_monogram(&mut master, &slot).is_err());
        assert!(find_node(&master, 2).is_some());
    }

    #[test]
    fn custom_subgrade_grid_is_reserved_and_blank() {
        assert_eq!(subgrade_display(Classification::Custom, None).unwrap(), None);
    }
}
