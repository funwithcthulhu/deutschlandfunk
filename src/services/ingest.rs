use crate::{
    audio,
    database::Database,
    deutschlandfunk::{Article, DeutschlandfunkClient},
};
use anyhow::{Result, anyhow};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFailureMode {
    BestEffort,
    Strict,
}

#[derive(Debug, Clone)]
pub struct SaveArticleOptions {
    pub download_audio: bool,
    pub audio_dir: String,
    pub audio_failure_mode: AudioFailureMode,
}

#[derive(Debug, Clone)]
pub struct AudioDownloadOutcome {
    pub path: PathBuf,
    pub bytes: u64,
    pub reused_existing: bool,
}

#[derive(Debug, Clone)]
pub struct SaveArticleOutcome {
    pub article_id: i64,
    pub audio: Option<AudioDownloadOutcome>,
    pub audio_error: Option<String>,
}

pub async fn save_article_with_optional_audio(
    scraper: &DeutschlandfunkClient,
    db: &Database,
    article: &Article,
    options: &SaveArticleOptions,
) -> Result<SaveArticleOutcome> {
    let article_id = db.save_article(article)?;
    let mut audio_outcome = None;
    let mut audio_error = None;

    if options.download_audio {
        match download_article_audio(scraper, article, &options.audio_dir, |_, _| {}).await {
            Ok(Some(audio)) => {
                db.set_audio_local_path(article_id, &audio.path.to_string_lossy())?;
                audio_outcome = Some(audio);
            }
            Ok(None) => {}
            Err(err) if options.audio_failure_mode == AudioFailureMode::BestEffort => {
                log::warn!("audio download failed for {}: {err:#}", article.url);
                audio_error = Some(format!("{err:#}"));
            }
            Err(err) => return Err(err),
        }
    }

    Ok(SaveArticleOutcome {
        article_id,
        audio: audio_outcome,
        audio_error,
    })
}

pub async fn download_article_audio<F>(
    scraper: &DeutschlandfunkClient,
    article: &Article,
    audio_dir_setting: &str,
    progress: F,
) -> Result<Option<AudioDownloadOutcome>>
where
    F: FnMut(u64, u64),
{
    let Some(audio_url) = article.audio.best_download_url() else {
        return Ok(None);
    };
    let audio_dir = audio::resolve_audio_dir(audio_dir_setting)?;
    let dest = audio::audio_file_path(
        &audio_dir,
        &article.url,
        article.audio.sophora_id.as_deref(),
    );

    if dest.is_file() {
        let bytes = dest
            .metadata()
            .map(|m| m.len())
            .map_err(|err| anyhow!("failed to inspect existing audio {}: {err}", dest.display()))?;
        return Ok(Some(AudioDownloadOutcome {
            path: dest,
            bytes,
            reused_existing: true,
        }));
    }

    let bytes = scraper.download_audio(audio_url, &dest, progress).await?;
    Ok(Some(AudioDownloadOutcome {
        path: dest,
        bytes,
        reused_existing: false,
    }))
}
