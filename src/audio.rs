//! Audio file storage and naming for Deutschlandfunk article downloads.
//!
//! Files default to `<app_data>/audio/<slug>.mp3` but the directory is
//! user-configurable through `AppSettings::audio_dir`.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Default audio sub-directory under `app_data_dir`.
pub const DEFAULT_AUDIO_SUBDIR: &str = "audio";

/// Resolve the directory where audio files should be stored.
///
/// If `configured` is non-empty it is used directly (created if missing).
/// Otherwise we fall back to `<app_data>/audio/`.
pub fn resolve_audio_dir(configured: &str) -> Result<PathBuf> {
    let path = if configured.trim().is_empty() {
        crate::app_data_dir()?.join(DEFAULT_AUDIO_SUBDIR)
    } else {
        PathBuf::from(configured)
    };
    std::fs::create_dir_all(&path)
        .with_context(|| format!("failed to create audio directory {}", path.display()))?;
    Ok(path)
}

/// Build the destination path for an article's audio file.
/// Prefers a stable per-article id (sophora id) for the file name, falling
/// back to a slug derived from the article URL.
pub fn audio_file_path(audio_dir: &Path, article_url: &str, sophora_id: Option<&str>) -> PathBuf {
    let stem = sophora_id
        .map(|s| sanitize_filename(s))
        .unwrap_or_else(|| slug_from_url(article_url));
    audio_dir.join(format!("{stem}.mp3"))
}

fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_owned()
}

fn slug_from_url(url: &str) -> String {
    let last_segment = url.trim_end_matches('/').rsplit('/').next().unwrap_or(url);
    let stem = last_segment.trim_end_matches(".html");
    sanitize_filename(stem)
}

/// Format duration (seconds) as `MM:SS` or `HH:MM:SS`.
pub fn format_duration(seconds: i64) -> String {
    if seconds <= 0 {
        return "—".to_owned();
    }
    let h = seconds / 3600;
    let m = (seconds % 3600) / 60;
    let s = seconds % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Format file size in bytes as a short human-readable string.
pub fn format_size(bytes: i64) -> String {
    if bytes <= 0 {
        return "—".to_owned();
    }
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let b = bytes as f64;
    if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{b} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_url_strips_extension_and_path() {
        assert_eq!(
            slug_from_url("https://www.deutschlandfunk.de/foo-bar-100.html"),
            "foo-bar-100"
        );
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize_filename("foo:bar/baz?"), "foo_bar_baz");
    }

    #[test]
    fn audio_file_path_prefers_sophora_id() {
        let p = audio_file_path(
            Path::new("/tmp"),
            "https://www.deutschlandfunk.de/foo-100.html",
            Some("foo-100"),
        );
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "foo-100.mp3");
    }

    #[test]
    fn audio_file_path_falls_back_to_url_slug() {
        let p = audio_file_path(
            Path::new("/tmp"),
            "https://www.deutschlandfunk.de/foo-100.html",
            None,
        );
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "foo-100.mp3");
    }

    #[test]
    fn format_duration_short() {
        assert_eq!(format_duration(75), "1:15");
        assert_eq!(format_duration(0), "—");
    }

    #[test]
    fn format_duration_long() {
        assert_eq!(format_duration(3 * 3600 + 5 * 60 + 7), "3:05:07");
    }

    #[test]
    fn format_size_human() {
        assert_eq!(format_size(0), "—");
        assert_eq!(format_size(800), "800 B");
        assert_eq!(format_size(2_500), "2 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }
}
