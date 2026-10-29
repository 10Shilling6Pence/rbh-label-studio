//! RBH's six-row front-label text layout contract.
//!
//! Rows 1 and 2 are single lines, rows 3 and 4 hold the set number and
//! (optionally wrapped) title, and rows 5 and 6 are a separate, fixed 2×2
//! subgrade grid. This module never measures by character count and never
//! changes grades, point size, or the card text to make a label fit.

use crate::CardIdentity;

/// Widths in PostScript points, measured from each row's text origin to its
/// calibrated safe boundary before the front monogram.
///
/// No production defaults: these values must come from the RBH master artwork.
#[derive(Clone, Copy, Debug)]
pub struct FrontRowLimits {
    pub row_1_pt: f64,
    pub row_2_pt: f64,
    pub row_3_pt: f64,
    pub row_4_pt: f64,
}

impl FrontRowLimits {
    fn validate(self) -> Result<(), String> {
        for (row, value) in [(1, self.row_1_pt), (2, self.row_2_pt), (3, self.row_3_pt), (4, self.row_4_pt)] {
            if !value.is_finite() || value <= 0.0 || value > 648.0 {
                return Err(format!("row {row} text limit must be finite and between 0 and 648 pt"));
            }
        }
        Ok(())
    }
}

/// Supplies shaped text widths in points using the font, font size, and
/// tracking of the original RBH template. A production implementation must
/// measure actual glyph advances; multiplying by character counts is wrong.
pub trait TextMeasurer {
    fn width_pt(&self, text: &str) -> Result<f64, String>;
}

/// An issue that prevents QA approval and production export.
#[derive(Clone, Debug, PartialEq)]
pub enum FitIssue {
    SingleLineOverflow { row: u8, measured_pt: f64, allowed_pt: f64 },
    TitleDoesNotFitTwoRows,
}

/// Rows 1–4 only. The grade block, monogram, and 2×2 subgrade grid are never
/// reflowed by the card information.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontTextLayout {
    pub row_1: String,
    pub row_2: String,
    pub row_3: String,
    pub row_4: Option<String>,
    pub issues: Vec<FitIssue>,
}

impl FrontTextLayout {
    /// An unresolved fit issue blocks approval. A later UI must still require
    /// human QA even when this returns true.
    pub fn fits(&self) -> bool {
        self.issues.is_empty()
    }
}

fn measured<M: TextMeasurer>(measurer: &M, text: &str) -> Result<f64, String> {
    let width = measurer.width_pt(text)?;
    if !width.is_finite() || width < 0.0 {
        return Err("font measurement returned a negative or non-finite width".into());
    }
    Ok(width)
}

fn third_row(number: &str, title: &str) -> String {
    if number.is_empty() { title.to_owned() } else { format!("{number} {title}") }
}

/// Determine the four card-identification lines and report any collisions.
///
/// Rows 1 and 2 never wrap or shrink. The set number stays on row 3; only
/// spaces between *title words* are permitted wrap points. Row 4 is unused
/// when the whole title fits on row 3. Nothing is clipped or silently omitted.
/// If wrapping cannot fit in two rows, the full original text is returned on
/// row 3 with a blocking QA issue, rather than losing any words.
pub fn layout_front_text<M: TextMeasurer>(card: &CardIdentity, limits: FrontRowLimits, measurer: &M) -> Result<FrontTextLayout, String> {
    limits.validate()?;
    if card.year_manufacturer_language.trim().is_empty()
        || card.variant.trim().is_empty()
        || card.title.trim().is_empty()
        || card.set_number != card.set_number.trim()
        || card.title != card.title.trim()
        || card.title.split(' ').any(str::is_empty)
        || card.title.chars().any(|c| c.is_whitespace() && c != ' ')
    {
        return Err("set number and title must use canonical spacing; check the source card data".into());
    }
    let mut issues = Vec::new();
    for (row, value, allowed) in [(1_u8, card.year_manufacturer_language.as_str(), limits.row_1_pt), (2_u8, card.variant.as_str(), limits.row_2_pt)] {
        let width = measured(measurer, value)?;
        if width > allowed {
            issues.push(FitIssue::SingleLineOverflow { row, measured_pt: width, allowed_pt: allowed });
        }
    }

    let all = third_row(&card.set_number, &card.title);
    if measured(measurer, &all)? <= limits.row_3_pt {
        return Ok(FrontTextLayout { row_1: card.year_manufacturer_language.clone(), row_2: card.variant.clone(), row_3: all, row_4: None, issues });
    }

    let words: Vec<&str> = card.title.split(' ').collect();
    // Prefer the longest possible third row while preserving whole words.
    for split in (1..words.len()).rev() {
        let third = third_row(&card.set_number, &words[..split].join(" "));
        let fourth = words[split..].join(" ");
        if measured(measurer, &third)? <= limits.row_3_pt && measured(measurer, &fourth)? <= limits.row_4_pt {
            return Ok(FrontTextLayout {
                row_1: card.year_manufacturer_language.clone(),
                row_2: card.variant.clone(),
                row_3: third,
                row_4: Some(fourth),
                issues,
            });
        }
    }

    issues.push(FitIssue::TitleDoesNotFitTwoRows);
    Ok(FrontTextLayout { row_1: card.year_manufacturer_language.clone(), row_2: card.variant.clone(), row_3: all, row_4: None, issues })
}
