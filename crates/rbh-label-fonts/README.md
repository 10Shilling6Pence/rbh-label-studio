# RBH Label Studio — Phase 2B: exact font measurement

This crate bridges the independent `rbh-labels` data/layout model to the **same
font shaping engine VectorCraft uses to render text**. It requires a precise
font-family and style match, validates that every character is supported, and
measures full strings in physical points. It neither estimates widths by
character count nor substitutes an alternate font.

## Current status

- Implemented: actual text measurement via `vectorcraft-text::layout`, including
  kerning and tracking, fail-closed missing-face and missing-glyph validation.
- Implemented: 648 × 864 pt sheet coordinate boundaries and configurable
  monogram clearance, connected to the Phase 2A two-row title wrapper.
- Not yet implemented: identifying the *actual* text origins, monogram boundary,
  glyph-size/tracking/horizontal-scale values from the chosen RBH master. **No
  guessed production coordinates are defined.**
- Not yet implemented: editable artwork placement, QR drawing, QA UI, printing.

## Local verification

```powershell
cargo test -p rbh-label-fonts -j 1
cargo check -p vectorcraft -j 1
```

With Helvetica Neue Bold installed, the `font_probe` example verifies the exact
face and measures strings. Supply the actual point size from the chosen master:

```powershell
cargo run -p rbh-label-fonts --example font_probe -j 1 -- <POINT_SIZE>
```

This example assumes tracking 0 and horizontal scale 100% **for its measurements
only**. Those values must be read from the actual production master rather than
assumed for export.

## Asset / privacy requirements

Do not add proprietary Helvetica Neue font binaries, customer certification
records, or RBH's original Illustrator masters to the public source repository.
Use locally installed licensed fonts and keep production art in local test assets.
