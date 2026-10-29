//! Local-only QA draft generator: no customer data leaves this machine.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use rbh_label_fonts::front_proof::generate_front_proof;
use rbh_labels::import_batch;
use vectorcraft_text::FontDb;

const MAX_MASTER_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CAL_BYTES: u64 = 4 * 1024 * 1024;

fn bounded_read(path: &PathBuf, max: u64) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if meta.len() > max {
        return Err(format!("{} is too large", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn run() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [master_file, calibration_file, batch_file, output_file] = <[PathBuf; 4]>::try_from(args)
        .map_err(|_| "usage: rbh-front-proof FRONT_MASTER.vectorcraft CALIBRATION.json BATCH.json NEW_OUTPUT.vectorcraft".to_owned())?;
    if output_file.extension().and_then(|x| x.to_str()) != Some("vectorcraft") {
        return Err("output must have the .vectorcraft extension".into());
    }
    let master: serde_json::Value =
        serde_json::from_str(&bounded_read(&master_file, MAX_MASTER_BYTES)?).map_err(|e| format!("master document JSON invalid: {e}"))?;
    let calibration: serde_json::Value =
        serde_json::from_str(&bounded_read(&calibration_file, MAX_CAL_BYTES)?).map_err(|e| format!("calibration JSON invalid: {e}"))?;
    let batch = import_batch(&bounded_read(&batch_file, 8 * 1024 * 1024)?)?;
    let proof = generate_front_proof(master, &calibration, &batch, FontDb::global())?;
    let bytes = serde_json::to_vec_pretty(&proof).map_err(|e| format!("could not encode proof: {e}"))?;
    // Never overwrite an original master, previous proof, or anything else.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output_file)
        .map_err(|e| format!("cannot create {} (never overwrite existing files): {e}", output_file.display()))?;
    file.write_all(&bytes).map_err(|e| format!("could not save proof: {e}"))?;
    println!("QA DRAFT ONLY: {} labels populated in {} (20 positions total)", batch.labels.len(), output_file.display());
    println!("NOT PRINT APPROVED: monogram clearance is provisional; rear QRs and staff QA are not implemented.");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("rbh-front-proof: {error}");
        std::process::exit(1);
    }
}
