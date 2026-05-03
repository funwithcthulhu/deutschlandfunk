pub mod audio;
pub mod database;
pub mod deutschlandfunk;
pub mod gui;
pub mod lingq;
pub mod services;
pub mod settings;
pub mod transcribe;

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

/// Returns the app data directory (`<local_app_data>/deutschlandfunk_lingq_tool/`), creating it if needed.
pub fn app_data_dir() -> Result<PathBuf> {
    let mut base = dirs::data_local_dir()
        .context("could not determine local app data directory for this OS/user")?;
    base.push("deutschlandfunk_lingq_tool");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("failed to create {}", base.display()))?;
    Ok(base)
}

pub fn timestamped_backup_path() -> Result<PathBuf> {
    let backups_dir = app_data_dir()?.join("backups");
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let base_name = format!("deutschlandfunk_lingq_tool-{stamp}");

    for suffix in std::iter::once(String::new()).chain((2..=999).map(|n| format!("-{n}"))) {
        let path = backups_dir.join(format!("{base_name}{suffix}.db"));
        if !path.exists() {
            return Ok(path);
        }
    }

    bail!(
        "could not find an available backup filename in {}",
        backups_dir.display()
    )
}
