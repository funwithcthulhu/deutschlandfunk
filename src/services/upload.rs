use crate::{
    database::Database,
    ids::ArticleId,
    lingq::{LingqClient, UploadRequest},
};
use anyhow::{Context, Result, anyhow, bail};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct UploadArticleOptions {
    pub api_key: String,
    pub language_code: String,
    pub collection_id: Option<i64>,
    pub attach_audio: bool,
}

#[derive(Debug, Clone)]
pub struct UploadArticleOutcome {
    pub article_id: i64,
    pub title: String,
    pub lesson_id: i64,
    pub lesson_url: String,
    pub updated_existing: bool,
}

pub async fn upload_article_to_lingq(
    lingq: &LingqClient,
    db: &Database,
    article_id: i64,
    options: &UploadArticleOptions,
) -> Result<UploadArticleOutcome> {
    let typed_article_id =
        ArticleId::new(article_id).ok_or_else(|| anyhow!("invalid article id {article_id}"))?;
    let article = db
        .get_article_by_id(typed_article_id)?
        .ok_or_else(|| anyhow!("article #{article_id} not found"))?;
    db.set_upload_status(typed_article_id, "pending", None)?;
    let audio_path = if options.attach_audio && !article.audio_local_path.trim().is_empty() {
        match validate_audio_path(&article.audio_local_path) {
            Ok(path) => Some(path),
            Err(err) => {
                let message = format!("{err:#}");
                db.set_upload_status(typed_article_id, "failed", Some(&message))?;
                return Err(err);
            }
        }
    } else {
        None
    };

    let request = UploadRequest {
        api_key: options.api_key.clone(),
        language_code: options.language_code.clone(),
        collection_id: options.collection_id,
        title: article.title.clone(),
        text: article.upload_text().to_owned(),
        original_url: Some(article.url.clone()),
        audio_path,
    };

    let updated_existing = article.lingq_lesson_id.is_some();
    db.set_upload_status(typed_article_id, "uploading", None)?;
    let response = if let Some(existing_id) = article.lingq_lesson_id {
        match lingq.update_lesson(&request, existing_id).await {
            Ok(response) => response,
            Err(err) => {
                let message = format!("{err:#}");
                db.set_upload_status(typed_article_id, "failed", Some(&message))?;
                return Err(err);
            }
        }
    } else {
        match lingq.upload_lesson(&request).await {
            Ok(response) => response,
            Err(err) => {
                let message = format!("{err:#}");
                db.set_upload_status(typed_article_id, "failed", Some(&message))?;
                return Err(err);
            }
        }
    };

    db.mark_uploaded_by_id(typed_article_id, response.lesson_id, &response.lesson_url)?;

    Ok(UploadArticleOutcome {
        article_id: article.id,
        title: article.title,
        lesson_id: response.lesson_id,
        lesson_url: response.lesson_url,
        updated_existing,
    })
}

fn validate_audio_path(raw_path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw_path);
    if !path.is_file() {
        bail!("audio file is missing: {}", path.display());
    }
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("mp3"))
    {
        bail!("audio file is not an MP3: {}", path.display());
    }
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("failed to read audio metadata for {}", path.display()))?;
    if metadata.len() == 0 {
        bail!("audio file is empty: {}", path.display());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_audio_path_rejects_empty_mp3() {
        let path = temp_path("empty.mp3");
        std::fs::write(&path, []).unwrap();

        let err = validate_audio_path(path.to_str().unwrap()).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert!(format!("{err:#}").contains("audio file is empty"));
    }

    #[test]
    fn validate_audio_path_accepts_nonempty_mp3() {
        let path = temp_path("sound.mp3");
        std::fs::write(&path, [0x49, 0x44, 0x33]).unwrap();

        let validated = validate_audio_path(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(validated, path);
    }

    #[test]
    fn validate_audio_path_rejects_non_mp3_extension() {
        let path = temp_path("sound.wav");
        std::fs::write(&path, [1, 2, 3]).unwrap();

        let err = validate_audio_path(path.to_str().unwrap()).unwrap_err();
        let _ = std::fs::remove_file(&path);

        assert!(format!("{err:#}").contains("audio file is not an MP3"));
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("dlf_upload_test_{}_{}", std::process::id(), name))
    }
}
