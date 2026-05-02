//! Optional local transcription via whisper.cpp.
//!
//! We don't bundle a Rust Whisper crate (those depend on a model file plus
//! native compilation). Instead we shell out to a user-installed whisper.cpp
//! binary (`main` or `whisper-cli`). That keeps the surface small and lets
//! users pick whatever model size + GPU build their machine can handle.
//!
//! Typical install on Windows:
//!   1. Build whisper.cpp from source or download a release binary.
//!   2. Download a model: `ggml-large-v3.bin` (~3 GB) or `ggml-medium.bin`.
//!   3. In Settings, set `whisper_cli_path` and `whisper_model_path`.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Stdio;

/// Configuration for a single transcription run.
#[derive(Debug, Clone)]
pub struct WhisperConfig {
    pub cli_path: PathBuf,
    pub model_path: PathBuf,
    pub language: String,
}

impl WhisperConfig {
    /// Pull a config out of `AppSettings` if it's complete enough to attempt
    /// a run. Returns Err with a user-facing reason otherwise.
    pub fn from_settings(s: &crate::settings::AppSettings) -> Result<Self> {
        let cli = s.whisper_cli_path.trim();
        let model = s.whisper_model_path.trim();
        if cli.is_empty() {
            bail!("Whisper CLI path is not configured. Set it in Settings.");
        }
        if model.is_empty() {
            bail!("Whisper model path is not configured. Set it in Settings.");
        }
        let cli_path = PathBuf::from(cli);
        if !cli_path.is_file() {
            bail!("Whisper CLI not found at {}", cli_path.display());
        }
        let model_path = PathBuf::from(model);
        if !model_path.is_file() {
            bail!("Whisper model not found at {}", model_path.display());
        }
        let language = if s.whisper_language.trim().is_empty() {
            "de".to_owned()
        } else {
            s.whisper_language.trim().to_owned()
        };
        Ok(Self {
            cli_path,
            model_path,
            language,
        })
    }

    /// Provenance tag we record alongside the transcript so different model
    /// versions show up distinctly in the UI / DB.
    pub fn source_tag(&self) -> String {
        let stem = self
            .model_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "whisper".to_owned());
        format!("whisper:{stem}")
    }
}

/// Run whisper.cpp on `audio_path` and return the transcript text.
///
/// Async wrapper around `tokio::process::Command` so it cooperates with the
/// existing tokio runtime in the GUI without blocking.
///
/// Whisper writes a `<audio>.txt` next to the audio file when invoked with
/// `--output-txt`; we read that file back and clean it up afterwards.
pub async fn transcribe_audio(config: &WhisperConfig, audio_path: &Path) -> Result<String> {
    use tokio::process::Command;
    if !audio_path.is_file() {
        bail!("Audio file not found: {}", audio_path.display());
    }

    // whisper.cpp writes outputs next to the input by default. We isolate
    // the run in a temp dir so concurrent runs and stale files don't
    // collide.
    let tmp = tempdir_for_audio(audio_path)?;
    let audio_in_tmp = tmp.join(
        audio_path
            .file_name()
            .context("audio path has no filename")?,
    );
    tokio::fs::copy(audio_path, &audio_in_tmp)
        .await
        .with_context(|| format!("failed to stage {}", audio_path.display()))?;

    let mut cmd = Command::new(&config.cli_path);
    cmd.arg("-m")
        .arg(&config.model_path)
        .arg("-l")
        .arg(&config.language)
        .arg("-otxt")
        .arg("-f")
        .arg(&audio_in_tmp)
        .current_dir(&tmp)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd
        .output()
        .await
        .with_context(|| format!("failed to launch whisper CLI {}", config.cli_path.display()))?;
    if !output.status.success() {
        // Surface stderr (truncated) so the user can see why it failed.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let truncated: String = stderr.chars().take(800).collect();
        bail!(
            "whisper exited with status {}: {}",
            output.status,
            truncated
        );
    }

    let txt_path = audio_in_tmp.with_extension({
        let ext = audio_in_tmp
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if ext.is_empty() {
            "txt".to_owned()
        } else {
            format!("{ext}.txt")
        }
    });
    let txt_path = if txt_path.is_file() {
        txt_path
    } else {
        // Some whisper builds write `<input>.txt` (no double-extension).
        audio_in_tmp.with_extension("txt")
    };
    if !txt_path.is_file() {
        bail!(
            "whisper completed but no .txt output was found at {}",
            txt_path.display()
        );
    }
    let transcript = tokio::fs::read_to_string(&txt_path)
        .await
        .with_context(|| format!("failed to read transcript {}", txt_path.display()))?;

    // Best-effort cleanup of the staged dir.
    let _ = tokio::fs::remove_dir_all(&tmp).await;

    Ok(post_process_transcript(&transcript))
}

/// Whisper inserts timestamp prefixes on each line by default; this helper
/// strips those if present and collapses double-newlines.
fn post_process_transcript(raw: &str) -> String {
    let stripped: Vec<String> = raw
        .lines()
        .map(|line| {
            // Lines look like "[00:00:00.000 --> 00:00:04.000] Hallo Welt."
            // when -otxt is run with timestamps; with -nt or default behaviour
            // they're plain text. Strip a leading bracket span if present.
            let trimmed = line.trim();
            if trimmed.starts_with('[')
                && let Some(close) = trimmed.find(']')
            {
                return trimmed[close + 1..].trim().to_owned();
            }
            trimmed.to_owned()
        })
        .filter(|l| !l.is_empty())
        .collect();
    stripped.join("\n")
}

fn tempdir_for_audio(audio_path: &Path) -> Result<PathBuf> {
    let stem = audio_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "transcribe".to_owned());
    let mut dir = std::env::temp_dir();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dir.push(format!("dlf_whisper_{stem}_{now}"));
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create temp dir {}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_process_strips_timestamp_prefix() {
        let raw = "[00:00:00.000 --> 00:00:02.000]  Hallo Welt.\n[00:00:02.000 --> 00:00:04.000]  Wie geht's?";
        let cleaned = post_process_transcript(raw);
        assert_eq!(cleaned, "Hallo Welt.\nWie geht's?");
    }

    #[test]
    fn post_process_preserves_plain_text() {
        let raw = "Hallo Welt.\n\nWie geht's?\n";
        let cleaned = post_process_transcript(raw);
        assert_eq!(cleaned, "Hallo Welt.\nWie geht's?");
    }

    #[test]
    fn config_rejects_missing_files() {
        let s = crate::settings::AppSettings {
            whisper_cli_path: "/nope/does-not-exist".into(),
            whisper_model_path: "/nope/also.bin".into(),
            whisper_language: "de".into(),
            ..Default::default()
        };
        assert!(WhisperConfig::from_settings(&s).is_err());
    }
}
