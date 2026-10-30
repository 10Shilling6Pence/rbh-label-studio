//! Local RBH label-production workspace. No website or Supabase connection.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver};

struct Generated {
    front_png: PathBuf,
    rear_png: PathBuf,
    front_eps: PathBuf,
    rear_eps: PathBuf,
}

#[derive(Default)]
pub(crate) struct RbhWorkspace {
    open: bool,
    batch_path: Option<PathBuf>,
    certifications: Vec<String>,
    front_master: Option<PathBuf>,
    rear_master: Option<PathBuf>,
    calibration: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    status: String,
    progress: Option<Receiver<Result<Generated, String>>>,
    generated: Option<Generated>,
}

fn tool_path(name: &str) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate RBH Label Studio: {e}"))?;
    let folder = exe.parent().ok_or("Cannot locate executable directory")?;
    let name = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };
    let path = folder.join(name);
    if !path.is_file() {
        return Err(format!("Missing local tool: {}. Build rbh-label-fonts binaries and vectorcraft-cli alongside the desktop EXE.", path.display()));
    }
    Ok(path)
}

fn run_tool(name: &str, args: &[&Path]) -> Result<(), String> {
    let tool = tool_path(name)?;
    let output = Command::new(&tool).args(args).output()
        .map_err(|e| format!("Could not run {}: {e}", tool.display()))?;
    if output.status.success() { return Ok(()); }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!("{name} failed: {} {}", stderr.trim(), stdout.trim()))
}

fn run_convert(source: &Path, destination: &Path, png: bool) -> Result<(), String> {
    let tool = tool_path("vectorcraft-cli")?;
    let mut command = Command::new(tool);
    command.arg("convert").arg(source).arg(destination);
    if png { command.args(["--scale", "2"]); }
    else { command.args(["--artboard", "0"]); }
    let result = command.output().map_err(|e| format!("Conversion failed to start: {e}"))?;
    if result.status.success() { Ok(()) }
    else { Err(format!("EPS/preview conversion failed: {}", String::from_utf8_lossy(&result.stderr))) }
}

fn generate(batch: PathBuf, front: PathBuf, rear: PathBuf, calibration: PathBuf, directory: PathBuf) -> Result<Generated, String> {
    if !directory.is_dir() { return Err("Select an existing output folder".into()); }
    // One timestamp for the complete pair. The file creators refuse to overwrite anything.
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| format!("Cannot generate timestamp: {e}"))?.as_nanos().to_string();
    let front_doc = directory.join(format!("RBH-FRONT-{stamp}.vectorcraft"));
    let rear_doc = directory.join(format!("RBH-REAR-{stamp}.vectorcraft"));
    let result = Generated {
        front_png: directory.join(format!("RBH-FRONT-{stamp}.png")),
        rear_png: directory.join(format!("RBH-REAR-{stamp}.png")),
        front_eps: directory.join(format!("RBH-FRONT-{stamp}.eps")),
        rear_eps: directory.join(format!("RBH-REAR-{stamp}.eps")),
    };
    for file in [&front_doc, &rear_doc, &result.front_png, &result.rear_png, &result.front_eps, &result.rear_eps] {
        if file.exists() { return Err(format!("Refusing to overwrite {}", file.display())); }
    }
    run_tool("rbh-front-proof", &[&front, &calibration, &batch, &front_doc])?;
    run_tool("rbh-rear-proof", &[&rear, &calibration, &batch, &rear_doc])?;
    run_convert(&front_doc, &result.front_png, true)?;
    run_convert(&rear_doc, &result.rear_png, true)?;
    run_convert(&front_doc, &result.front_eps, false)?;
    run_convert(&rear_doc, &result.rear_eps, false)?;
    Ok(result)
}

#[cfg(windows)]
fn open_preview(path: &Path) -> Result<(), String> {
    // File is selected by the user or created by this process, never a remote URL.
    Command::new("explorer.exe").arg(path).spawn()
        .map_err(|e| format!("Cannot open preview: {e}"))?;
    Ok(())
}

#[cfg(not(windows))]
fn open_preview(path: &Path) -> Result<(), String> {
    Command::new("xdg-open").arg(path).spawn()
        .map_err(|e| format!("Cannot open preview: {e}"))?;
    Ok(())
}

impl RbhWorkspace {
    pub(crate) fn new() -> Self {
        Self { open: true, status: "Select a local RBH Grading JSON export to begin.".into(), ..Self::default() }
    }

    fn load(&mut self, path: PathBuf) {
        let result = (|| -> Result<Vec<String>, String> {
            let meta = std::fs::metadata(&path).map_err(|e| format!("Cannot read file: {e}"))?;
            if meta.len() > 8 * 1024 * 1024 { return Err("JSON file is too large".into()); }
            let content = std::fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {e}"))?;
            let batch = rbh_labels::import_batch(&content)?;
            let mut seen = HashSet::new();
            let ids: Vec<String> = batch.labels.iter().map(|l| l.certification_number.clone()).collect();
            if ids.is_empty() || ids.len() > 20 { return Err("Expected 1â€“20 certifications".into()); }
            if !ids.iter().all(|id| seen.insert(id.clone())) { return Err("Duplicate certification number".into()); }
            Ok(ids)
        })();
        self.generated = None;
        match result {
            Ok(ids) => {
                self.status = format!("Loaded {} validated certifications.", ids.len());
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

    fn start_generation(&mut self) {
        let (Some(batch), Some(front), Some(rear), Some(calibration), Some(directory)) = (
            self.batch_path.clone(), self.front_master.clone(), self.rear_master.clone(),
            self.calibration.clone(), self.output_dir.clone()
        ) else { self.status = "Choose JSON, both masters, calibration, and output folder.".into(); return; };
        if self.certifications.len() != 20 { self.status = "A print sheet requires exactly 20 certifications.".into(); return; }
        let (sender, receiver) = mpsc::channel();
        self.progress = Some(receiver);
        self.generated = None;
        self.status = "Generating both proofs, previews, and timestamped EPS files locally...".into();
        std::thread::spawn(move || { let _ = sender.send(generate(batch, front, rear, calibration, directory)); });
    }

    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.progress {
            match receiver.try_recv() {
                Ok(Ok(generated)) => {
                    self.status = format!("Created EPS pair:\n{}\n{}", generated.front_eps.display(), generated.rear_eps.display());
                    self.generated = Some(generated);
                    self.progress = None;
                }
                Ok(Err(e)) => { self.status = e; self.progress = None; }
                Err(mpsc::TryRecvError::Disconnected) => { self.status = "Export worker ended unexpectedly".into(); self.progress = None; }
                Err(mpsc::TryRecvError::Empty) => { ctx.request_repaint_after(std::time::Duration::from_millis(150)); }
            }
        }
        egui::Area::new(egui::Id::unique("rbh_workspace_launcher"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -36.0])
            .show(ctx, |ui| {
                if ui.button("RBH Label Production").clicked() { self.open = true; }
            });
        let mut open = self.open;
        egui::Window::new("RBH Label Production")
            .id(egui::Id::unique("rbh_label_production_window"))
            .open(&mut open)
            .default_width(540.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("RBH Label Studio");
                ui.label("Local JSON import, calibrated sheet production, and EPS export");
                ui.separator();
                if ui.button("Import RBH Grading JSON...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("RBH JSON", &["json"]).pick_file() { self.load(path); }
                }
                if let Some(path) = &self.batch_path { ui.label(format!("JSON: {}", path.display())); }
                ui.horizontal(|ui| {
                    ui.label(format!("Certifications: {} / 20", self.certifications.len()));
                    if self.certifications.len() == 20 { ui.label("Full sheet"); }
                    else { ui.label("QA only â€” incomplete sheet"); }
                });
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    for (i, cert) in self.certifications.iter().enumerate() { ui.label(format!("{:02}  {cert}", i + 1)); }
                });
                ui.separator();
                ui.label("Production files (stored locally; selection lasts for this session)");
                if ui.button("Choose FRONT master...").clicked() { self.front_master = rfd::FileDialog::new().add_filter("VectorCraft", &["vectorcraft"]).pick_file(); }
                if let Some(p) = &self.front_master { ui.small(format!("Front: {}", p.display())); }
                if ui.button("Choose REAR master...").clicked() { self.rear_master = rfd::FileDialog::new().add_filter("VectorCraft", &["vectorcraft"]).pick_file(); }
                if let Some(p) = &self.rear_master { ui.small(format!("Rear: {}", p.display())); }
                if ui.button("Choose calibration JSON...").clicked() { self.calibration = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file(); }
                if let Some(p) = &self.calibration { ui.small(format!("Calibration: {}", p.display())); }
                if ui.button("Choose output folder...").clicked() { self.output_dir = rfd::FileDialog::new().pick_folder(); }
                if let Some(p) = &self.output_dir { ui.small(format!("Output: {}", p.display())); }
                ui.separator();
                let ready = self.certifications.len() == 20 && self.batch_path.is_some() && self.front_master.is_some()
                    && self.rear_master.is_some() && self.calibration.is_some() && self.output_dir.is_some() && self.progress.is_none();
                if ui.add_enabled(ready, egui::Button::new("Generate Front + Mirrored Rear and Export EPS Pair")).clicked() { self.start_generation(); }
                if let Some(done) = &self.generated {
                    ui.horizontal(|ui| {
                        if ui.button("View Front PNG").clicked() { if let Err(e) = open_preview(&done.front_png) { self.status = e; } }
                        if ui.button("View Rear PNG").clicked() { if let Err(e) = open_preview(&done.rear_png) { self.status = e; } }
                    });
                }
                ui.label(&self.status);
                ui.small("QA proof only. Verify 9 x 12 in geometry, duplex registration, and QR scans before authorizing printing.");
            });
        self.open = open;
    }
}


