#![allow(clippy::unwrap_used, clippy::expect_used)]

use rbh_labels::CardIdentity;
use rbh_labels::layout::{FitIssue, FrontRowLimits, TextMeasurer, layout_front_text};

// A deterministic fake font lets us test the rules without shipping a font.
// Production will supply measured Helvetica Neue Bold glyph advances.
struct FakeFont;
impl TextMeasurer for FakeFont {
    fn width_pt(&self, text: &str) -> Result<f64, String> {
        Ok(text.chars().map(|c| match c {
            'W' => 9.0,
            'I' => 2.0,
            ' ' => 2.0,
            _ => 5.0,
        }).sum())
    }
}

fn card(title: &str) -> CardIdentity {
    CardIdentity {
        year_manufacturer_language: "2026 RIFTBOUND VEN EN".into(),
        variant: "RARE".into(),
        set_number: "#149".into(),
        title: title.into(),
    }
}

fn limits() -> FrontRowLimits {
    FrontRowLimits { row_1_pt: 120.0, row_2_pt: 120.0, row_3_pt: 110.0, row_4_pt: 110.0 }
}

#[test]
fn single_line_title_does_not_create_a_fourth_row() {
    let result = layout_front_text(&card("VI"), limits(), &FakeFont).unwrap();
    assert!(result.fits());
    assert_eq!(result.row_3, "#149 VI");
    assert!(result.row_4.is_none());
}

#[test]
fn multiword_title_wraps_only_between_title_words() {
    let result = layout_front_text(&card("JAYCE, DEFENDER OF TOMORROW"), limits(), &FakeFont).unwrap();
    assert!(result.fits());
    assert_eq!(result.row_3, "#149 JAYCE, DEFENDER OF");
    assert_eq!(result.row_4.as_deref(), Some("TOMORROW"));
}

#[test]
fn header_overflow_is_flagged_not_wrapped() {
    let mut c = card("VI");
    c.year_manufacturer_language = "WWWWWWWWWWWWWW".into();
    let mut max = limits();
    max.row_1_pt = 40.0;
    let result = layout_front_text(&c, max, &FakeFont).unwrap();
    assert!(!result.fits());
    assert_eq!(result.row_1, "WWWWWWWWWWWWWW");
    assert!(result.issues.iter().any(|issue| matches!(issue, FitIssue::SingleLineOverflow { row: 1, .. })));
}

#[test]
fn variant_overflow_is_flagged_not_wrapped() {
    let mut c = card("VI");
    c.variant = "WWWWWWWWWWWWWW".into();
    let mut max = limits();
    max.row_2_pt = 40.0;
    let result = layout_front_text(&c, max, &FakeFont).unwrap();
    assert!(!result.fits());
    assert_eq!(result.row_2, "WWWWWWWWWWWWWW");
    assert!(result.issues.iter().any(|issue| matches!(issue, FitIssue::SingleLineOverflow { row: 2, .. })));
}

#[test]
fn a_single_very_wide_title_word_is_blocked_without_truncation() {
    let c = card("WWWWWWWWWWWWWWWWWWWW");
    let result = layout_front_text(&c, limits(), &FakeFont).unwrap();
    assert!(!result.fits());
    assert_eq!(result.row_3, "#149 WWWWWWWWWWWWWWWWWWWW");
    assert!(result.issues.contains(&FitIssue::TitleDoesNotFitTwoRows));
}

#[test]
fn a_second_row_that_cannot_fit_is_blocked() {
    let mut max = limits();
    max.row_4_pt = 12.0;
    let result = layout_front_text(&card("JAYCE DEFENDER OF TOMORROW"), max, &FakeFont).unwrap();
    assert!(!result.fits());
    assert!(result.issues.contains(&FitIssue::TitleDoesNotFitTwoRows));
}

#[test]
fn no_set_number_is_supported() {
    let mut c = card("VI");
    c.set_number.clear();
    let result = layout_front_text(&c, limits(), &FakeFont).unwrap();
    assert_eq!(result.row_3, "VI");
    assert!(result.fits());
}

#[test]
fn malformed_widths_cannot_approve_a_label() {
    let mut max = limits();
    max.row_3_pt = f64::NAN;
    assert!(layout_front_text(&card("VI"), max, &FakeFont).is_err());
}

#[test]
fn repeated_spaces_cannot_be_silently_reformatted() {
    let c = card("JAYCE  DEFENDER");
    assert!(layout_front_text(&c, limits(), &FakeFont).is_err());
}

#[test]
fn narrow_and_wide_characters_use_different_widths() {
    let mut max = limits();
    max.row_3_pt = 50.0;
    let narrow = layout_front_text(&card("IIIIIIIIII"), max, &FakeFont).unwrap();
    let wide = layout_front_text(&card("WWWWWWWWWW"), max, &FakeFont).unwrap();
    assert!(narrow.fits());
    assert!(!wide.fits());
}
