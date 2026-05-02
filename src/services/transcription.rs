use crate::{
    database::Database,
    transcribe::{self, WhisperConfig},
};
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TranscriptionOutcome {
    pub article_id: i64,
    pub audio_path: PathBuf,
    pub chars: usize,
    pub source: String,
}

pub async fn transcribe_saved_article(
    db: &Database,
    article_id: i64,
    config: &WhisperConfig,
) -> Result<TranscriptionOutcome> {
    let article = db
        .get_article(article_id)?
        .ok_or_else(|| anyhow!("article #{article_id} not found"))?;
    if article.audio_local_path.trim().is_empty() {
        anyhow::bail!("article #{article_id} has no local audio");
    }

    let audio_path = PathBuf::from(&article.audio_local_path);
    if !audio_path.is_file() {
        anyhow::bail!("audio file missing: {}", audio_path.display());
    }

    let text = transcribe::transcribe_audio(config, Path::new(&audio_path)).await?;
    let source = config.source_tag();
    db.set_transcript(article.id, &text, &source)?;

    Ok(TranscriptionOutcome {
        article_id: article.id,
        audio_path,
        chars: text.len(),
        source,
    })
}
