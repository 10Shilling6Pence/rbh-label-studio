//! Read-only font calibration check. Run with the master artwork's point size:
//! cargo run -p rbh-label-fonts --example font_probe -- 6.0

use std::process::ExitCode;

use rbh_label_fonts::{ExactFontMeasurer, LabelTypeface};
use rbh_labels::layout::TextMeasurer;
use vectorcraft_text::FontDb;

fn run() -> Result<(), String> {
    let size = std::env::args().nth(1).ok_or("supply the exact font size in points from the RBH master (for example: -- 6.0)")?;
    let size: f64 = size.parse().map_err(|_| "font size must be numeric")?;
    let font = ExactFontMeasurer::new(FontDb::global(), LabelTypeface::helvetica_neue_bold(size, 0.0, 100.0))?;
    println!("Exact Helvetica Neue Bold resolved at {size} pt. These are measured widths, not character-count estimates:");
    for text in ["WWWWWWWWWW", "IIIIIIIIII", "2026 RIFTBOUND VEN EN", "#149 JAYCE, DEFENDER OF TOMORROW"] {
        println!("{:.3} pt | {text}", font.width_pt(text)?);
    }
    println!("Tracking and horizontal scale are 0 and 100% in this probe. Set those from the master before production calibration.");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("RBH font probe: {e}");
            ExitCode::FAILURE
        }
    }
}
