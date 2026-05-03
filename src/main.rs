#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, anyhow};
use deutschlandfunk_lingq_tool::gui;
use log::info;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .init();

    info!("DLF LingQ Reader starting");
    gui::run().map_err(|err| anyhow!("failed to launch GUI: {err}"))
}
