pub mod audio;
pub mod database;
pub mod deutschlandfunk;
pub mod gui;
pub mod lingq;
pub mod services;
pub mod settings;
pub mod transcribe;

use anyhow::{Context, Result};
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
