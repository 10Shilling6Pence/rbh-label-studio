//! Local QA draft generator for RBH rear sheets. Never print without review.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use rbh_label_fonts::rear_proof::generate_rear_proof;
use rbh_labels::import_batch;

fn read_bounded(path: &PathBuf, max_bytes: u64) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > max_bytes {
        return Err(format!("{} exceeds the input size limit", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn run() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [master_file, calibration_file, batch_file, output_file] = <[PathBuf; 4]>::try_from(args)
        .map_err(|_| "usage: rbh-rear-proof REAR_MASTER.vectorcraft CALIBRATION.json BATCH.json NEW_OUTPUT.vectorcraft".to_owned())?;
    if output_file.extension().and_then(|s| s.to_str()) != Some("vectorcraft") {
        return Err("output must use .vectorcraft extension".into());
    }
    let master: serde_json::Value =
        serde_json::from_str(&read_bounded(&master_file, 16 * 1024 * 1024)?).map_err(|e| format!("invalid rear master: {e}"))?;
    let calibration: serde_json::Value =
        serde_json::from_str(&read_bounded(&calibration_file, 4 * 1024 * 1024)?).map_err(|e| format!("invalid calibration: {e}"))?;
    let batch = import_batch(&read_bounded(&batch_file, 8 * 1024 * 1024)?)?;
    let proof = generate_rear_proof(master, &calibration, &batch)?;
    let output = serde_json::to_vec_pretty(&proof).map_err(|e| format!("failed to serialize rear proof: {e}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_file)
        .map_err(|e| format!("cannot create {}: {e} (existing files are never overwritten)", output_file.display()))?;
    file.write_all(&output).map_err(|e| format!("failed to write rear proof: {e}"))?;
    println!("QA DRAFT ONLY: {} rear QR positions regenerated in {}", batch.labels.len(), output_file.display());
    for (i, label) in batch.labels.iter().enumerate() {
        println!("Position {:02}: {} -> {}", i + 1, label.certification_number, label.qr_url);
    }
    println!("NOT PRINT APPROVED: QR scanability, front/rear orientation and full-sheet approvals remain unverified.");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rbh-rear-proof: {error}");
        std::process::exit(1);
    }
}
