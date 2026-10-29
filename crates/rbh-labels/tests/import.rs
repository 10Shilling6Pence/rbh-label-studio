#![allow(clippy::unwrap_used, clippy::expect_used)]

use rbh_labels::import_batch;
use serde_json::json;

const EXAMPLE: &str = include_str!("../examples/example_batch.json");

#[test]
fn imports_graded_sample_and_custom() {
    let batch = import_batch(EXAMPLE).expect("example JSON should import");
    assert_eq!(batch.order_number, "RBH-000010");
    assert_eq!(batch.labels.len(), 3);
    assert_eq!(batch.labels[0].grade_display, "9");
    assert_eq!(batch.labels[0].grade_title, "MINT");
    assert_eq!(batch.labels[0].qr_url, "https://www.rbhgrading.com/cert/12345678");
    assert_eq!(batch.labels[0].subgrades.as_ref().unwrap().centering, 9.5);
    assert_eq!(batch.labels[1].grade_display, "S");
    assert_eq!(batch.labels[1].grade_title, "SAMPLE");
    assert!(batch.labels[1].subgrades.is_none());
    assert_eq!(batch.labels[2].grade_display, "C");
    assert_eq!(batch.labels[2].grade_title, "CUSTOM");
}

#[test]
fn imported_print_rows_preserve_exact_four_lines() {
    let mut data: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
    data["certifications"][0]["printRows"] = json!([
        "2026 RIFTBOUND UNL EN", "RARE", "#197 DIANA, SCORN OF THE", "MOON"
    ]);
    let batch = import_batch(&data.to_string()).unwrap();
    let rows = batch.labels[0].print_rows.as_ref().unwrap();
    assert_eq!(rows[0], "2026 RIFTBOUND UNL EN");
    assert_eq!(rows[2], "#197 DIANA, SCORN OF THE");
    assert_eq!(rows[3], "MOON");
    assert!(batch.labels[1].print_rows.is_none());
}

#[test]
fn rejects_print_rows_with_fewer_than_four_lines() {
    let mut data: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
    data["certifications"][0]["printRows"] = json!(["A", "B", "C"]);
    assert!(import_batch(&data.to_string()).unwrap_err().contains("invalid RBH label JSON"));
}

#[test]
fn rejects_print_row_exceeding_character_limit() {
    let mut data: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
    data["certifications"][0]["printRows"] = json!(["W".repeat(33), "B", "C", ""]);
    assert!(import_batch(&data.to_string()).unwrap_err().contains("printRows row 1"));
}

#[test]
fn rejects_duplicate_certification_numbers() {
    let duplicated = EXAMPLE.replace("12345679", "12345678");
    assert!(import_batch(&duplicated).unwrap_err().contains("duplicate"));
}

#[test]
fn rejects_grade_that_is_not_half_point_increment() {
    let invalid = EXAMPLE.replace("\"grade\": 9,", "\"grade\": 9.3,");
    assert!(import_batch(&invalid).unwrap_err().contains("invalid grade"));
}

#[test]
fn rejects_missing_required_subgrades() {
    let invalid = EXAMPLE.replace("\"centering\": 9.5,", "\"notCentering\": 9.5,");
    assert!(import_batch(&invalid).unwrap_err().contains("invalid RBH label JSON"));
}

#[test]
fn rejects_multiline_header() {
    let invalid = EXAMPLE.replace("2026 RIFTBOUND VEN EN", "2026 RIFTBOUND\\nVEN EN");
    assert!(import_batch(&invalid).unwrap_err().contains("line break"));
}

#[test]
fn rejects_numeric_grade_on_sample() {
    let invalid = EXAMPLE.replace("\"classification\": \"sample\",", "\"classification\": \"sample\", \"grade\": 9,");
    assert!(import_batch(&invalid).unwrap_err().contains("must not contain numerical grades"));
}
