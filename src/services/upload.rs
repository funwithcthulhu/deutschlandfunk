use crate::{
    database::Database,
    lingq::{LingqClient, UploadRequest},
};
use anyhow::{Result, anyhow};
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
    let article = db
        .get_article(article_id)?
        .ok_or_else(|| anyhow!("article #{article_id} not found"))?;
    let audio_path = if options.attach_audio && !article.audio_local_path.trim().is_empty() {
        let path = PathBuf::from(&article.audio_local_path);
        path.is_file().then_some(path)
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
    let response = if let Some(existing_id) = article.lingq_lesson_id {
        lingq.update_lesson(&request, existing_id).await?
    } else {
        lingq.upload_lesson(&request).await?
    };

    db.mark_uploaded(article.id, response.lesson_id, &response.lesson_url)?;

    Ok(UploadArticleOutcome {
        article_id: article.id,
        title: article.title,
        lesson_id: response.lesson_id,
        lesson_url: response.lesson_url,
        updated_existing,
    })
}
