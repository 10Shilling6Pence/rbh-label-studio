//! RBH Label Studio's certification import and standardized display model.
//!
//! This crate intentionally has no database, graphics, or network dependency.
//! It does not modify the grades assigned by RBH Grading.

pub mod layout;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
const MAX_CERTIFICATIONS: usize = 1000;
const MAX_IMPORT_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 512;
const QR_BASE: &str = "https://www.rbhgrading.com/cert/";

/// Official RBH grade designations, as agreed for Label Studio.
/// Integers represent half-point steps (e.g. 19 means 9.5).
const GRADE_TITLES: &[(i32, &str)] = &[
    (20, "GEM MINT"),
    (19, "MINT+"),
    (18, "MINT"),
    (17, "NM-MT+"),
    (16, "NM-MT"),
    (15, "NM+"),
    (14, "NM"),
    (13, "EX-MT+"),
    (12, "EX-MT"),
    (11, "EX+"),
    (10, "EX"),
    (9, "VG-EX+"),
    (8, "VG-EX"),
    (7, "VG+"),
    (6, "VG"),
    (5, "GOOD+"),
    (4, "GOOD"),
    (3, "FR+"),
    (2, "FR"),
];

fn half_steps(value: f64) -> Option<i32> {
    if !value.is_finite() || !(1.0..=10.0).contains(&value) {
        return None;
    }
    let steps = value * 2.0;
    if steps.fract() != 0.0 {
        return None;
    }
    Some(steps as i32)
}

/// The official title for an RBH numerical grade. No guessing or rounding.
pub fn grade_title(grade: f64) -> Option<&'static str> {
    let steps = half_steps(grade)?;
    GRADE_TITLES.iter().find(|(n, _)| *n == steps).map(|(_, title)| *title)
}

fn grade_text(grade: f64) -> Option<String> {
    let steps = half_steps(grade)?;
    Some(if steps % 2 == 0 {
        format!("{}", steps / 2)
    } else {
        format!("{}.5", steps / 2)
    })
}

/// Input contract for an export from RBH Grading. No customer PII is needed.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportBatch {
    pub schema_version: u32,
    pub order_number: String,
    pub certifications: Vec<ImportedCertification>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedCertification {
    pub certification_number: String,
    pub classification: Classification,
    #[serde(default)]
    pub grade: Option<f64>,
    #[serde(default)]
    pub subgrades: Option<Subgrades>,
    pub card: CardIdentity,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Classification {
    Graded,
    Sample,
    Custom,
}

/// These values are imported from the grading website and are never edited here.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Subgrades {
    pub centering: f64,
    pub corners: f64,
    pub edges: f64,
    pub surface: f64,
}

/// Rows 1 and 2 cannot wrap. Row 3 combines set number and title; the title
/// may continue onto row 4. Row 5 and 6 are reserved for fixed 2x2 subgrades.
/// Exact glyph widths and monogram clearance belong to the renderer, not here.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CardIdentity {
    pub year_manufacturer_language: String,
    pub variant: String,
    #[serde(default)]
    pub set_number: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedBatch {
    pub schema_version: u32,
    pub order_number: String,
    pub labels: Vec<PreparedLabel>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedLabel {
    pub certification_number: String,
    pub classification: Classification,
    pub grade_display: String,
    pub grade_title: String,
    pub subgrades: Option<Subgrades>,
    pub qr_url: String,
    pub card: CardIdentity,
}

fn check_line(value: &str, field: &str, may_be_empty: bool) -> Result<(), String> {
    if !may_be_empty && value.trim().is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if value.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("{field} exceeds {MAX_TEXT_CHARS} characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} contains a control character or line break"));
    }
    Ok(())
}

fn validate_card(card: &CardIdentity) -> Result<(), String> {
    check_line(&card.year_manufacturer_language, "card.yearManufacturerLanguage", false)?;
    check_line(&card.variant, "card.variant", false)?;
    check_line(&card.set_number, "card.setNumber", true)?;
    check_line(&card.title, "card.title", false)?;
    Ok(())
}

fn validate_subgrades(subgrades: &Subgrades) -> Result<(), String> {
    for (name, value) in [
        ("centering", subgrades.centering),
        ("corners", subgrades.corners),
        ("edges", subgrades.edges),
        ("surface", subgrades.surface),
    ] {
        if grade_title(value).is_none() {
            return Err(format!("invalid {name} subgrade {value}: expected 1–10 in half-point steps"));
        }
    }
    Ok(())
}

/// Parse and validate a batch. Failed imports produce no prepared labels.
/// Only approved grading values are accepted; card text fitting is a later QA stage.
pub fn import_batch(input: &str) -> Result<PreparedBatch, String> {
    if input.len() > MAX_IMPORT_BYTES {
        return Err("JSON import exceeds 8 MiB".into());
    }
    let batch: ImportBatch = serde_json::from_str(input).map_err(|e| format!("invalid RBH label JSON: {e}"))?;
    if batch.schema_version != SCHEMA_VERSION {
        return Err(format!("unsupported schemaVersion {} (expected {SCHEMA_VERSION})", batch.schema_version));
    }
    check_line(&batch.order_number, "orderNumber", false)?;
    if batch.certifications.is_empty() || batch.certifications.len() > MAX_CERTIFICATIONS {
        return Err(format!("expected 1–{MAX_CERTIFICATIONS} certifications"));
    }

    let mut seen = HashSet::new();
    let mut labels = Vec::with_capacity(batch.certifications.len());
    for cert in batch.certifications {
        let id = cert.certification_number.as_str();
        if id.len() != 8 || !id.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("invalid certification number {id:?}: expected exactly eight ASCII digits"));
        }
        if !seen.insert(cert.certification_number.clone()) {
            return Err(format!("duplicate certification number {id}"));
        }
        validate_card(&cert.card).map_err(|e| format!("cert {id}: {e}"))?;
        let (grade_display, title, subgrades) = match cert.classification {
            Classification::Graded => {
                let grade = cert.grade.ok_or_else(|| format!("cert {id}: missing grade"))?;
                let name = grade_title(grade).ok_or_else(|| format!("cert {id}: invalid grade {grade}"))?;
                let display = grade_text(grade).ok_or_else(|| format!("cert {id}: invalid grade {grade}"))?;
                let subgrades = cert.subgrades.ok_or_else(|| format!("cert {id}: missing subgrades"))?;
                validate_subgrades(&subgrades).map_err(|e| format!("cert {id}: {e}"))?;
                (display, name.to_owned(), Some(subgrades))
            }
            Classification::Sample | Classification::Custom => {
                if cert.grade.is_some() || cert.subgrades.is_some() {
                    return Err(format!("cert {id}: sample/custom certifications must not contain numerical grades or subgrades"));
                }
                match cert.classification {
                    Classification::Sample => ("S".into(), "SAMPLE".into(), None),
                    Classification::Custom => ("C".into(), "CUSTOM".into(), None),
                    Classification::Graded => return Err(format!("cert {id}: inconsistent classification")),
                }
            }
        };
        labels.push(PreparedLabel {
            certification_number: cert.certification_number.clone(),
            classification: cert.classification,
            grade_display,
            grade_title: title,
            subgrades,
            qr_url: format!("{QR_BASE}{}", cert.certification_number),
            card: cert.card,
        });
    }
    Ok(PreparedBatch { schema_version: SCHEMA_VERSION, order_number: batch.order_number, labels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_nineteen_designations_are_exact() {
        assert_eq!(GRADE_TITLES.len(), 19);
        for (steps, title) in GRADE_TITLES {
            assert_eq!(grade_title(f64::from(*steps) / 2.0), Some(*title));
        }
        assert_eq!(grade_title(9.0), Some("MINT"));
        assert_eq!(grade_title(1.5), Some("FR+"));
        assert_eq!(grade_title(1.0), Some("FR"));
        assert_eq!(grade_title(9.4), None);
        assert_eq!(grade_title(f64::NAN), None);
        assert_eq!(grade_title(0.0), None);
        assert_eq!(grade_title(10.5), None);
    }

    #[test]
    fn displays_whole_grades_without_decimal_suffix() {
        assert_eq!(grade_text(9.0).as_deref(), Some("9"));
        assert_eq!(grade_text(9.5).as_deref(), Some("9.5"));
        assert_eq!(grade_text(1.0).as_deref(), Some("1"));
    }
}
