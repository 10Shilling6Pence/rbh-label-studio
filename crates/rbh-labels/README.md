# RBH Labels — Phase 1: import and grading rules

This is the first, **headless** component of RBH Label Studio. It does not yet add a UI,
modify Illustrator artwork, or export production labels. It has no database or network
connection and does not include customer personal information.

## Contract

- `schemaVersion`: must be `1`.
- `orderNumber`: nonempty reference to the RBH submission.
- `certifications`: 1–1,000 cards, each with a unique, eight-digit `certificationNumber`.
- `classification`: `graded`, `sample`, or `custom`.
- For `graded`: `grade` and all four `subgrades` are mandatory, on the approved 1–10
  half-point scale. The title is derived automatically from the numerical grade.
- For `sample` and `custom`: numerical grades and subgrades must be absent.
  These use `S` / `SAMPLE` and `C` / `CUSTOM` respectively.
- `card.yearManufacturerLanguage`: **row 1, never wraps**.
- `card.variant`: **row 2, never wraps**.
- `card.setNumber` and `card.title`: **row 3; title can continue on row 4 only**.
- **Rows 5 and 6** are reserved for the fixed 2×2 subgrades, with the values imported
  from RBH. Their positions are controlled by the renderer, not by card text.
- QR destination is derived from the certification number as
  `https://www.rbhgrading.com/cert/########`.

**Deliberately not implemented:** font-metric text fitting, QR image drawing, actual
artwork placement, sheet layout, approvals, and emailing. These require their own tests.
No guessed grades, automatic font shrinking, silent text truncation, or rewriting of a
classification is allowed.

## Test locally

From the repository root:

```powershell
cargo test -p rbh-labels -j 1
```

The first release of this component uses `crates/rbh-labels/examples/example_batch.json`
with fictional certification numbers. The RBH website will later emit this schema via
a staff-only export; this crate does not change the website.
