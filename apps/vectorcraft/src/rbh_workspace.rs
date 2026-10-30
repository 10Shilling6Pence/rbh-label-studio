//! Local RBH label-production workspace. No website or Supabase connection.
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct RbhWorkspace {
    open: bool,
    batch_path: Option<PathBuf>,
    certifications: Vec<String>,
    status: String,
}

impl RbhWorkspace {
    pub(crate) fn new() -> Self {
        Self { open: true, status: "Select a local RBH Grading JSON export to begin.".into(), ..Self::default() }
    }

    fn load(&mut self, path: PathBuf) {
        let result = (|| -> Result<Vec<String>, String> {
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {e}"))?;
            let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}"))?;
            if value.get("schemaVersion").and_then(serde_json::Value::as_u64) != Some(1) {
                return Err("Expected RBH schemaVersion 1".into());
            }
            let entries = value.get("certifications").and_then(serde_json::Value::as_array)
                .ok_or("Missing certifications array")?;
            if entries.is_empty() || entries.len() > 20 {
                return Err(format!("Expected 1–20 certifications, found {}", entries.len()));
            }
            let mut seen = HashSet::new();
            let mut ids = Vec::with_capacity(entries.len());
            for entry in entries {
                let id = entry.get("certificationNumber").and_then(serde_json::Value::as_str)
                    .ok_or("A certificationNumber is missing")?;
                if id.len() != 8 || !id.bytes().all(|c| c.is_ascii_digit()) || !seen.insert(id.to_owned()) {
                    return Err(format!("Invalid or duplicate certification number: {id}"));
                }
                ids.push(id.to_owned());
            }
            Ok(ids)
        })();
        match result {
            Ok(ids) => {
                self.status = format!("Loaded {} unique certifications. No artwork has been generated.", ids.len());
                self.certifications = ids;
                self.batch_path = Some(path);
            }
            Err(e) => {
                self.status = format!("Import failed: {e}");
                self.certifications.clear();
                self.batch_path = None;
            }
        }
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("rbh_workspace_launcher"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -36.0])
            .show(ctx, |ui| {
                if ui.button("RBH Label Production").clicked() { self.open = true; }
            });
        let mut open = self.open;
        egui::Window::new("RBH Label Production")
            .id(egui::Id::new("rbh_label_production_window"))
            .open(&mut open)
            .default_width(480.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("RBH Label Studio");
                ui.label("Local label production | JSON import and batch review");
                ui.separator();
                if ui.button("Import RBH Grading JSON...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("RBH JSON", &["json"])
                        .pick_file() {
                        self.load(path);
                    }
                }
                if let Some(path) = &self.batch_path {
                    ui.label(format!("File: {}", path.display()));
                }
                ui.label(&self.status);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(format!("Certifications: {} / 20", self.certifications.len()));
                    if self.certifications.len() == 20 { ui.label("Full sheet"); }
                    else { ui.label("QA only — incomplete sheet"); }
                });
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for (i, cert) in self.certifications.iter().enumerate() {
                        ui.label(format!("{:02}    {}", i + 1, cert));
                    }
                });
                ui.separator();
                ui.label("EPS generation will be connected in the next phase. Existing CLI and timestamped export remain available.");
            });
        self.open = open;
    }
}
