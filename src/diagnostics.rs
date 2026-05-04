use crate::{
    app_data_dir,
    database::{Database, DatabaseHealth, LibraryStats},
    settings::AppSettings,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};

pub fn health_report(db: &Database, settings: &AppSettings, token_present: bool) -> String {
    let app_data = app_data_dir().ok();
    let db_path = app_data
        .as_ref()
        .map(|path| path.join("deutschlandfunk_lingq_tool.db"));
    let backups_dir = app_data.as_ref().map(|path| path.join("backups"));
    let stats = db.get_stats().ok();
    let health = db
        .get_health(db_path.as_deref(), backups_dir.as_deref())
        .ok();

    let mut lines = Vec::new();
    if let Some(path) = app_data.as_ref() {
        lines.push(format!("App data: {}", path.display()));
    }
    if let Some(health) = health.as_ref() {
        lines.push(format!(
            "Database: schema v{}, {} migration(s), {}, FK {}, integrity {}",
            health.schema_version,
            health.migration_count,
            health.journal_mode,
            if health.foreign_keys_enabled {
                "on"
            } else {
                "off"
            },
            health.integrity_check
        ));
        lines.push(format!(
            "Storage: {}, {} page(s), {} free page(s)",
            health
                .database_size_bytes
                .map(format_bytes)
                .unwrap_or_else(|| "unknown size".to_owned()),
            health.page_count,
            health.freelist_count
        ));
        lines.push(format!(
            "Latest backup: {}",
            health.latest_backup.as_deref().unwrap_or("(none found)")
        ));
    }
    if let Some(stats) = stats.as_ref() {
        lines.push(format!(
            "Library: {} article(s), {} uploaded, avg {} words",
            stats.total_articles, stats.uploaded_articles, stats.average_word_count
        ));
    }
    lines.push(format!(
        "LingQ token: {}",
        if token_present {
            "present"
        } else {
            "not saved"
        }
    ));
    lines.push(format!(
        "Audio folder: {}",
        resolved_audio_dir(app_data.as_deref(), &settings.audio_dir)
    ));
    lines.push(format!(
        "Whisper: CLI {}, model {}",
        configured(&settings.whisper_cli_path),
        configured(&settings.whisper_model_path)
    ));

    lines.join("\n")
}

pub fn export_diagnostics_bundle(
    db: &Database,
    settings: &AppSettings,
    token_present: bool,
) -> Result<PathBuf> {
    let app_data = app_data_dir()?;
    let db_path = app_data.join("deutschlandfunk_lingq_tool.db");
    let backups_dir = app_data.join("backups");
    let diagnostics_dir = app_data.join("diagnostics");
    std::fs::create_dir_all(&diagnostics_dir).with_context(|| {
        format!(
            "failed to create diagnostics directory {}",
            diagnostics_dir.display()
        )
    })?;

    let stats = db.get_stats().ok();
    let health = db.get_health(Some(&db_path), Some(&backups_dir)).ok();
    let report = json!({
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "app": {
            "name": "DLF LingQ Reader",
            "version": env!("CARGO_PKG_VERSION"),
            "app_data_dir": app_data.display().to_string(),
            "database_path": db_path.display().to_string()
        },
        "database": health.as_ref().map(health_json),
        "library": stats.as_ref().map(stats_json),
        "settings": {
            "browse_section": settings.browse_section,
            "download_audio_on_fetch": settings.download_audio_on_fetch,
            "upload_audio_to_lingq": settings.upload_audio_to_lingq,
            "audio_dir": resolved_audio_dir(Some(&app_data), &settings.audio_dir),
            "lingq_language": settings.lingq_language,
            "lingq_collection_id": settings.lingq_collection_id,
            "lingq_token_present": token_present,
            "whisper_cli_configured": !settings.whisper_cli_path.trim().is_empty(),
            "whisper_model_configured": !settings.whisper_model_path.trim().is_empty(),
            "whisper_language": settings.whisper_language
        },
        "logs": {
            "recent_lines": [],
            "note": "File logging is not configured yet; runtime logs go through env_logger."
        }
    });

    let path = diagnostics_dir.join(format!(
        "dlf-lingq-reader-diagnostics-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::write(&path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write diagnostics file {}", path.display()))?;
    Ok(path)
}

fn health_json(health: &DatabaseHealth) -> serde_json::Value {
    json!({
        "schema_version": health.schema_version,
        "migration_count": health.migration_count,
        "journal_mode": health.journal_mode,
        "foreign_keys_enabled": health.foreign_keys_enabled,
        "integrity_check": health.integrity_check,
        "page_count": health.page_count,
        "freelist_count": health.freelist_count,
        "database_size_bytes": health.database_size_bytes,
        "latest_backup": health.latest_backup
    })
}

fn stats_json(stats: &LibraryStats) -> serde_json::Value {
    json!({
        "total_articles": stats.total_articles,
        "uploaded_articles": stats.uploaded_articles,
        "average_word_count": stats.average_word_count,
        "sections": stats.sections.iter().map(|section| json!({
            "section": section.section,
            "count": section.count
        })).collect::<Vec<_>>()
    })
}

fn resolved_audio_dir(app_data: Option<&Path>, configured: &str) -> String {
    if configured.trim().is_empty() {
        app_data
            .map(|path| path.join("audio").display().to_string())
            .unwrap_or_else(|| "(default app data audio folder)".to_owned())
    } else {
        configured.to_owned()
    }
}

fn configured(value: &str) -> &'static str {
    if value.trim().is_empty() {
        "not configured"
    } else {
        "configured"
    }
}

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1024 * 1024;
    const KB: u64 = 1024;
    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SettingsStore;

    #[test]
    fn health_report_does_not_include_token_value() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let mut settings = SettingsStore::in_memory_default();
        settings.data_mut().lingq_api_key = "secret-token".to_owned();

        let report = health_report(&db, settings.data(), true);

        assert!(report.contains("LingQ token: present"));
        assert!(!report.contains("secret-token"));
    }
}
