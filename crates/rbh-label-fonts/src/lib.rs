//! Exact-font text measurement for RBH Label Studio.
//!
//! Uses VectorCraft's own HarfRust text shaper and glyph geometry. The
//! RBH certification model remains independent of graphics and system fonts.
//! Font files are NEVER included in this crate or in the public repository.

use rbh_labels::CardIdentity;
use rbh_labels::layout::{FrontRowLimits, FrontTextLayout, TextMeasurer, layout_front_text};
use vectorcraft_doc::{CharStyle, TextObject};
use vectorcraft_geom::Point;
use vectorcraft_text::{FontDb, FontMatch, layout};

/// Match the RBH master label's type settings (in physical PostScript points).
/// No guessed font-size or tracking defaults are suitable for print production.
#[derive(Clone, Debug)]
pub struct LabelTypeface {
    pub family: String,
    pub style: String,
    pub size_pt: f64,
    /// Tracking, in 1/1000 em, like VectorCraft's Character panel.
    pub tracking: f64,
    /// Horizontal scale, in percent (100 = no scaling).
    pub horizontal_scale_percent: f64,
}

impl LabelTypeface {
    pub fn helvetica_neue_bold(size_pt: f64, tracking: f64, horizontal_scale_percent: f64) -> Self {
        Self { family: "Helvetica Neue".into(), style: "Bold".into(), size_pt, tracking, horizontal_scale_percent }
    }

    fn validate(&self) -> Result<(), String> {
        if self.family.trim().is_empty() || self.style.trim().is_empty() {
            return Err("font family and style must be specified".into());
        }
        if !self.size_pt.is_finite() || self.size_pt <= 0.0 || self.size_pt > 200.0 {
            return Err("font size must be finite and between 0 and 200 pt".into());
        }
        if !self.tracking.is_finite() || self.tracking.abs() > 1000.0 {
            return Err("font tracking must be finite and within +/-1000 thousandths of an em".into());
        }
        if !self.horizontal_scale_percent.is_finite() || !(1.0..=400.0).contains(&self.horizontal_scale_percent) {
            return Err("horizontal font scale must be finite and between 1% and 400%".into());
        }
        Ok(())
    }
}

/// Strict font measurer: never substitutes another family or weight, even when
/// VectorCraft would normally be able to render with a fallback face.
pub struct ExactFontMeasurer<'a> {
    db: &'a FontDb,
    typeface: LabelTypeface,
}

impl<'a> ExactFontMeasurer<'a> {
    pub fn new(db: &'a FontDb, typeface: LabelTypeface) -> Result<Self, String> {
        typeface.validate()?;
        let Some((_, quality)) = db.resolve(&typeface.family, &typeface.style) else {
            return Err(format!("required font {} {} is missing; install it before producing labels", typeface.family, typeface.style));
        };
        if quality != FontMatch::Exact {
            return Err(format!("required font {} {} was substituted; production export is blocked", typeface.family, typeface.style));
        }
        Ok(Self { db, typeface })
    }

    pub fn typeface(&self) -> &LabelTypeface {
        &self.typeface
    }
}

impl TextMeasurer for ExactFontMeasurer<'_> {
    fn width_pt(&self, text: &str) -> Result<f64, String> {
        if text.is_empty() {
            return Ok(0.0);
        }
        // Do not let the layout engine's per-glyph fallback mask an absent
        // character from the exact typeface; that would change label widths.
        let (face, quality) = self
            .db
            .resolve(&self.typeface.family, &self.typeface.style)
            .ok_or_else(|| format!("required font {} {} is no longer available", self.typeface.family, self.typeface.style))?;
        if quality != FontMatch::Exact {
            return Err("required font is no longer an exact match".into());
        }
        if let Some(missing) = text.chars().find(|&ch| !face.covers(ch)) {
            return Err(format!("required font cannot render U+{:04X}; manual QA required", u32::from(missing)));
        }
        let style = CharStyle {
            font_family: self.typeface.family.clone(),
            font_style: self.typeface.style.clone(),
            size: self.typeface.size_pt,
            tracking: self.typeface.tracking,
            h_scale: self.typeface.horizontal_scale_percent,
            ..CharStyle::default()
        };
        let t = TextObject::point(Point::ZERO, text, style);
        let shaped = layout(self.db, &t);
        if shaped.overflow || shaped.lines.len() != 1 {
            return Err("font shaper could not produce exactly one line".into());
        }
        let line = shaped.lines.first().ok_or("font shaper returned no line")?;
        // Both advance and ink right-hand extents matter. Ink can overhang
        // the final glyph's advance and physically touch the RBH monogram.
        let advance = (line.x1 - line.x0).max(0.0);
        let ink_right = shaped.bounds.x1.max(0.0);
        let width = advance.max(ink_right);
        if !width.is_finite() || width < 0.0 {
            return Err("font shaper returned invalid physical dimensions".into());
        }
        Ok(width)
    }
}

/// Calibrated template geometry, measured from the RBH production master in
/// points. Positions are measured from the 9x12 inch page's left edge.
/// The monogram boundary includes any space it occupies to the left.
#[derive(Clone, Copy, Debug)]
pub struct FrontLabelGeometry {
    /// The x position of the card-description origin for each of rows 1-4.
    pub text_left_pt: [f64; 4],
    /// The leftmost point that any identification text must not touch.
    pub monogram_left_pt: f64,
    /// Clearance maintained between the text's visible ink and monogram.
    pub clearance_pt: f64,
}

impl FrontLabelGeometry {
    pub const SHEET_WIDTH_PT: f64 = 648.0;
    pub const SHEET_HEIGHT_PT: f64 = 864.0;

    pub fn row_limits(self) -> Result<FrontRowLimits, String> {
        let b = self.monogram_left_pt;
        let gap = self.clearance_pt;
        if !b.is_finite() || !(0.0..=Self::SHEET_WIDTH_PT).contains(&b) || !gap.is_finite() || gap < 0.0 {
            return Err("monogram boundary and clearance must be valid points on the sheet".into());
        }
        let mut widths = [0.0; 4];
        for (i, left) in self.text_left_pt.into_iter().enumerate() {
            if !left.is_finite() || left < 0.0 || left >= b - gap {
                return Err(format!("row {} starts outside the available text area", i + 1));
            }
            widths[i] = b - gap - left;
        }
        Ok(FrontRowLimits { row_1_pt: widths[0], row_2_pt: widths[1], row_3_pt: widths[2], row_4_pt: widths[3] })
    }
}

/// Validate text and wrap only the title, using genuine font shaping and a
/// calibrated monogram boundary. Human approval is still required afterwards.
pub fn fit_front_label(card: &CardIdentity, geometry: FrontLabelGeometry, measurer: &ExactFontMeasurer<'_>) -> Result<FrontTextLayout, String> {
    layout_front_text(card, geometry.row_limits()?, measurer)
}
