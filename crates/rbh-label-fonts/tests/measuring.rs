#![allow(clippy::unwrap_used, clippy::expect_used)]

use rbh_label_fonts::{ExactFontMeasurer, FrontLabelGeometry, LabelTypeface};
use rbh_labels::CardIdentity;
use rbh_labels::layout::{FrontRowLimits, TextMeasurer, layout_front_text};
use vectorcraft_text::FontDb;

// Source Sans 3 is bundled by VectorCraft and is ONLY a fixture for unit tests;
// production uses the customer's installed, exact Helvetica Neue Bold face.
fn test_font() -> ExactFontMeasurer<'static> {
    ExactFontMeasurer::new(
        FontDb::global(),
        LabelTypeface {
            family: "Source Sans 3".to_owned(),
            style: "Regular".to_owned(),
            size_pt: 10.0,
            tracking: 0.0,
            horizontal_scale_percent: 100.0,
        },
    )
    .unwrap()
}

#[test]
fn shaped_wide_and_narrow_letters_measure_differently() {
    let font = test_font();
    let wide = font.width_pt("WWWWWWWWWW").unwrap();
    let narrow = font.width_pt("IIIIIIIIII").unwrap();
    assert!(wide > narrow, "W={wide}, I={narrow}");
    assert!(narrow > 0.0);
}

#[test]
fn same_text_is_wider_at_larger_font_size() {
    let small = test_font().width_pt("RIFTBOUND").unwrap();
    let large = ExactFontMeasurer::new(
        FontDb::global(),
        LabelTypeface { family: "Source Sans 3".into(), style: "Regular".into(), size_pt: 20.0, tracking: 0.0, horizontal_scale_percent: 100.0 },
    )
    .unwrap()
    .width_pt("RIFTBOUND")
    .unwrap();
    assert!(large > small * 1.8, "small={small}, large={large}");
}

#[test]
fn missing_font_blocks_production_instead_of_substituting() {
    let db = FontDb::global();
    let bad = ExactFontMeasurer::new(
        db,
        LabelTypeface {
            family: "RBH nonexistent typeface 999999".into(),
            style: "Bold".into(),
            size_pt: 9.0,
            tracking: 0.0,
            horizontal_scale_percent: 100.0,
        },
    );
    assert!(bad.is_err());
}

#[test]
fn bad_font_settings_are_rejected() {
    let bad = ExactFontMeasurer::new(FontDb::global(), LabelTypeface::helvetica_neue_bold(f64::NAN, 0.0, 100.0));
    assert!(bad.is_err());
}

#[test]
fn monogram_clearance_is_subtracted_from_measured_width() {
    let geometry = FrontLabelGeometry { text_left_pt: [100.0, 100.0, 100.0, 100.0], monogram_left_pt: 220.0, clearance_pt: 4.0 };
    let limits = geometry.row_limits().unwrap();
    assert_eq!(limits.row_1_pt, 116.0);
    assert_eq!(limits.row_4_pt, 116.0);
}

#[test]
fn invalid_or_overlapping_coordinates_are_blocked() {
    let bad = FrontLabelGeometry { text_left_pt: [100.0, 200.0, 100.0, 100.0], monogram_left_pt: 200.0, clearance_pt: 3.0 };
    assert!(bad.row_limits().is_err());
}

#[test]
fn actual_font_widths_drive_the_phase_2a_title_wrap() {
    let font = test_font();
    let card = CardIdentity {
        year_manufacturer_language: "2026 RIFTBOUND VEN EN".into(),
        variant: "RARE".into(),
        set_number: "#149".into(),
        title: "JAYCE DEFENDER OF TOMORROW".into(),
    };
    let third = font.width_pt("#149 JAYCE DEFENDER").unwrap();
    let fourth = font.width_pt("OF TOMORROW").unwrap();
    let whole = font.width_pt("#149 JAYCE DEFENDER OF TOMORROW").unwrap();
    assert!(third < whole);
    let result =
        layout_front_text(&card, FrontRowLimits { row_1_pt: 648.0, row_2_pt: 648.0, row_3_pt: third + 0.01, row_4_pt: fourth + 0.01 }, &font)
            .unwrap();
    assert!(result.fits());
    assert_eq!(result.row_3, "#149 JAYCE DEFENDER");
    assert_eq!(result.row_4.as_deref(), Some("OF TOMORROW"));
}
