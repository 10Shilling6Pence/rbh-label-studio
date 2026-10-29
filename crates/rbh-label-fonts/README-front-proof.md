# RBH front-sheet proof — Phase 2C

This is a **QA-only prototype**, not a production exporter. It imports a validated
`schemaVersion: 1` certification batch, uses the exact installed Helvetica Neue
Bold face to check text widths, and writes a new native VectorCraft front sheet.
**It never changes the original master and never overwrites an existing proof.**

The calibration file and master stay **outside the public Git repository**.
The provided calibration is a *candidate*, and its 3 pt monogram clearance is
provisional. The generated document is not authorized for printing.

From the repository root in PowerShell:

```powershell
cargo run -p rbh-label-fonts --bin rbh-front-proof -j 1 -- "..\rbh-label-test-assets\output\front-master.vectorcraft" "..\rbh-label-test-assets\calibration\rbh-label-studio-calibration-candidate.json" "crates\rbh-labels\examples\example_batch.json" "..\rbh-label-test-assets\output\front-proof.vectorcraft"
```

Then render a *preview only*:

```powershell
.\target\debug\vectorcraft-cli.exe convert "..\rbh-label-test-assets\output\front-proof.vectorcraft" "..\rbh-label-test-assets\output\front-proof.png" --scale 2
```

This prototype copies the original artwork, replaces the grade, designation,
certification number, and identification text, and populates four subgrade values
for numerical grades. Samples retain the four fixed subgrade headings with
`#` (ungraded) placeholders; Custom/Alter labels leave that area empty. Unused slots are
cleared of previous text **and their individual RBH monograms** so partial
batches have no stray logos. Populated-slot logos and sheet registration marks
are preserved. The empty `L04 title3` wrapper receives
an editable text object when slot 4 is populated.

**Limits:** front side only; no QR code, rear-sheet pairing, approvals, print
exports or automated email. Grade-block centering is inherited from the
original text origins, adjusted by exact glyph widths, and requires visual QA.
A 3 pt safety boundary is a provisional design choice. The software blocks
an overflowing description rather than shrinking text. The source artboard
and all 20 calibrated field locations are checked before output.

The sample batch contains fictional certification numbers. For a second proof
use a different output filename: existing outputs will not be overwritten.
