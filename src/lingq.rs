use anyhow::{Context, Result, bail};
use log::info;
use reqwest::{Client, multipart};
use serde::Deserialize;
use std::path::PathBuf;

const LINGQ_BASE: &str = "https://www.lingq.com/api/v3";
const LINGQ_AUTH: &str = "https://www.lingq.com/api/v2/api-token-auth/";

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: i64,
    pub title: String,
    pub lessons_count: i64,
}

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub api_key: String,
    pub language_code: String,
    pub collection_id: Option<i64>,
    pub title: String,
    pub text: String,
    pub original_url: Option<String>,
    /// Optional path to a local MP3 to attach to the lesson via multipart.
    pub audio_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UploadResponse {
    pub lesson_id: i64,
    pub lesson_url: String,
}

#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub token: String,
}

#[derive(Deserialize, Debug)]
struct LingqLessonResponse {
    id: Option<i64>,
    #[serde(rename = "lessonId")]
    lesson_id_camel: Option<i64>,
    #[serde(rename = "lesson_id")]
    lesson_id_snake: Option<i64>,
    pk: Option<i64>,
    url: Option<String>,
    #[serde(rename = "lessonUrl")]
    lesson_url: Option<String>,
    /// Some LingQ deployments return the URL of the attached audio after a
    /// multipart upload; if it's null/missing despite us sending an audio
    /// part, the upload silently dropped the file. We surface a warning.
    audio: Option<String>,
}

impl LingqLessonResponse {
    fn lesson_id(&self) -> Option<i64> {
        self.id
            .or(self.lesson_id_camel)
            .or(self.lesson_id_snake)
            .or(self.pk)
            .or_else(|| self.lesson_url().and_then(lesson_id_from_url))
    }

    fn lesson_url(&self) -> Option<&str> {
        self.lesson_url
            .as_deref()
            .filter(|url| !url.trim().is_empty())
            .or_else(|| self.url.as_deref().filter(|url| !url.trim().is_empty()))
    }
}

#[derive(Deserialize)]
struct LingqTokenResponse {
    token: Option<String>,
}

#[derive(Deserialize)]
struct LingqCollectionsResponse {
    results: Vec<LingqCollectionRow>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct LingqCollectionRow {
    id: i64,
    title: String,
    #[serde(rename = "lessonsCount")]
    lessons_count: Option<i64>,
    #[serde(rename = "lessons_count")]
    lessons_count_alt: Option<i64>,
}

#[derive(Clone)]
pub struct LingqClient {
    client: Client,
}

impl LingqClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(format!(
                "deutschlandfunk_lingq_tool/{}",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .context("failed to build LingQ HTTP client")?;

        Ok(Self { client })
    }

    pub async fn login(&self, username: &str, password: &str) -> Result<LoginResponse> {
        info!("LingQ login attempt for user: {username}");
        let params = [("username", username), ("password", password)];
        let response = self
            .client
            .post(LINGQ_AUTH)
            .form(&params)
            .send()
            .await
            .context("LingQ login request failed")?;

        let response = response
            .error_for_status()
            .context("LingQ rejected the username/password login")?;
        let payload: LingqTokenResponse = response
            .json()
            .await
            .context("failed to parse LingQ login response")?;

        let token = payload
            .token
            .filter(|token| !token.trim().is_empty())
            .context("LingQ login succeeded but no token was returned")?;

        Ok(LoginResponse { token })
    }

    pub async fn get_collections(
        &self,
        api_key: &str,
        language_code: &str,
    ) -> Result<Vec<Collection>> {
        let mut all_collections = Vec::new();
        let mut url = Some(format!("{}/{}/collections/my/", LINGQ_BASE, language_code));
        let max_pages = 20;
        let mut page = 0;

        while let Some(current_url) = url.take() {
            page += 1;
            if page > max_pages {
                break;
            }

            let mut auth = reqwest::header::HeaderValue::from_str(&format!("Token {api_key}"))
                .context("invalid API key characters")?;
            auth.set_sensitive(true);
            let response = self
                .client
                .get(&current_url)
                .header("Authorization", auth)
                .send()
                .await
                .context("LingQ collections request failed")?;

            let response = response
                .error_for_status()
                .context("LingQ rejected the API key or collections request")?;
            let page_data: LingqCollectionsResponse = response
                .json()
                .await
                .context("failed to parse LingQ collections response")?;

            all_collections.extend(page_data.results.into_iter().map(|row| Collection {
                id: row.id,
                title: row.title,
                lessons_count: row.lessons_count.or(row.lessons_count_alt).unwrap_or(0),
            }));

            url = page_data.next;
        }

        Ok(all_collections)
    }

    pub async fn upload_lesson(&self, request: &UploadRequest) -> Result<UploadResponse> {
        info!("Uploading lesson to LingQ: {}", request.title);
        let normalized_text = normalize_text(&request.text);
        if normalized_text.trim().is_empty() {
            bail!("lesson text is empty");
        }

        let mut auth =
            reqwest::header::HeaderValue::from_str(&format!("Token {}", request.api_key))
                .context("invalid API key characters")?;
        auth.set_sensitive(true);

        let url = format!("{}/{}/lessons/", LINGQ_BASE, request.language_code);

        let response = if let Some(audio_path) = audio_path_for_upload(request) {
            let form = build_lesson_multipart(request, &normalized_text, &audio_path).await?;
            self.client
                .post(&url)
                .header("Authorization", auth)
                .multipart(form)
                .send()
                .await
                .context("LingQ multipart upload request failed")?
        } else {
            let mut payload = serde_json::json!({
                "title": request.title,
                "text": normalized_text,
                "status": "private",
            });
            if let Some(collection_id) = request.collection_id {
                payload["collection"] = serde_json::json!(collection_id);
            }
            if let Some(original_url) = &request.original_url {
                payload["original_url"] = serde_json::json!(original_url);
            }
            self.client
                .post(&url)
                .header("Authorization", auth)
                .json(&payload)
                .send()
                .await
                .context("LingQ upload request failed")?
        };

        let response = response
            .error_for_status()
            .context("LingQ rejected the lesson upload")?;
        let body = response
            .text()
            .await
            .context("failed to read LingQ upload response")?;
        let (upload, lesson) =
            parse_lesson_response(&body, &request.language_code, None, "upload")?;

        if let Some(lesson) = lesson {
            warn_if_audio_dropped(request, upload.lesson_id, lesson.audio.as_deref());
        }
        Ok(upload)
    }
    /// Update an existing lesson on LingQ (PATCH). Useful when article text
    /// has been re-fetched with better content or the article was previously
    /// truncated and is now available.
    pub async fn update_lesson(
        &self,
        request: &UploadRequest,
        lesson_id: i64,
    ) -> Result<UploadResponse> {
        info!("Updating LingQ lesson {}: {}", lesson_id, request.title);
        let normalized_text = normalize_text(&request.text);
        if normalized_text.trim().is_empty() {
            bail!("lesson text is empty");
        }

        let mut auth =
            reqwest::header::HeaderValue::from_str(&format!("Token {}", request.api_key))
                .context("invalid API key characters")?;
        auth.set_sensitive(true);

        let url = format!(
            "{}/{}/lessons/{}/",
            LINGQ_BASE, request.language_code, lesson_id
        );

        let response = if let Some(audio_path) = audio_path_for_upload(request) {
            let form = build_lesson_multipart(request, &normalized_text, &audio_path).await?;
            self.client
                .patch(&url)
                .header("Authorization", auth)
                .multipart(form)
                .send()
                .await
                .context("LingQ multipart update request failed")?
        } else {
            let mut payload = serde_json::json!({
                "title": request.title,
                "text": normalized_text,
            });
            if let Some(original_url) = &request.original_url {
                payload["original_url"] = serde_json::json!(original_url);
            }
            self.client
                .patch(&url)
                .header("Authorization", auth)
                .json(&payload)
                .send()
                .await
                .context("LingQ update request failed")?
        };

        let response = response
            .error_for_status()
            .context("LingQ rejected the lesson update")?;
        let body = response
            .text()
            .await
            .context("failed to read LingQ update response")?;
        let (upload, lesson) =
            parse_lesson_response(&body, &request.language_code, Some(lesson_id), "update")?;

        if let Some(lesson) = lesson {
            warn_if_audio_dropped(request, upload.lesson_id, lesson.audio.as_deref());
        }
        Ok(upload)
    }
}

fn parse_lesson_response(
    body: &str,
    language_code: &str,
    fallback_lesson_id: Option<i64>,
    operation: &str,
) -> Result<(UploadResponse, Option<LingqLessonResponse>)> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        let lesson_id = fallback_lesson_id.with_context(|| {
            format!("LingQ {operation} response was empty and no lesson id was available")
        })?;
        return Ok((
            UploadResponse {
                lesson_id,
                lesson_url: lesson_url(language_code, lesson_id),
            },
            None,
        ));
    }

    let lesson: LingqLessonResponse = match serde_json::from_str(trimmed) {
        Ok(lesson) => lesson,
        Err(error) => {
            if let Some(lesson_id) = fallback_lesson_id {
                log::warn!(
                    "LingQ {operation} response for lesson {lesson_id} was not JSON; \
                     treating the successful HTTP status as success: {}",
                    summarize_response_body(trimmed)
                );
                return Ok((
                    UploadResponse {
                        lesson_id,
                        lesson_url: lesson_url(language_code, lesson_id),
                    },
                    None,
                ));
            }

            return Err(error).with_context(|| {
                format!(
                    "failed to parse LingQ {operation} response: {}",
                    summarize_response_body(trimmed)
                )
            });
        }
    };

    let lesson_id = lesson
        .lesson_id()
        .or(fallback_lesson_id)
        .with_context(|| format!("LingQ {operation} response did not include a lesson id"))?;
    let upload = UploadResponse {
        lesson_id,
        lesson_url: lesson
            .lesson_url()
            .map(str::to_owned)
            .unwrap_or_else(|| lesson_url(language_code, lesson_id)),
    };

    Ok((upload, Some(lesson)))
}

fn lesson_url(language_code: &str, lesson_id: i64) -> String {
    format!("https://www.lingq.com/{language_code}/learn/lesson/{lesson_id}/")
}

fn lesson_id_from_url(url: &str) -> Option<i64> {
    let trimmed = url.trim().trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .and_then(|value| value.parse().ok())
}

fn summarize_response_body(body: &str) -> String {
    const MAX_CHARS: usize = 240;
    let single_line = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= MAX_CHARS {
        single_line
    } else {
        format!(
            "{}...",
            single_line.chars().take(MAX_CHARS).collect::<String>()
        )
    }
}

/// Returns Some(path) only if the request has an audio file that exists on disk.
fn audio_path_for_upload(request: &UploadRequest) -> Option<PathBuf> {
    request.audio_path.as_ref().filter(|p| p.is_file()).cloned()
}

/// If we sent audio but the LingQ response came back without an `audio`
/// field, the server likely silently dropped it (wrong content-type, file
/// too large, lesson type mismatch, etc.). We can't surface a hard error
/// because the lesson itself was created — but we leave a log line so the
/// user can debug from the GUI status bar / cargo log output.
fn warn_if_audio_dropped(request: &UploadRequest, lesson_id: i64, audio_url: Option<&str>) {
    if audio_path_for_upload(request).is_some() && audio_url.map(str::is_empty).unwrap_or(true) {
        log::warn!(
            "LingQ accepted lesson {} but did not store the attached audio file — \
             the upload may have been silently dropped (check file size / mime type).",
            lesson_id
        );
    }
}

async fn build_lesson_multipart(
    request: &UploadRequest,
    normalized_text: &str,
    audio_path: &std::path::Path,
) -> Result<multipart::Form> {
    let bytes = tokio::fs::read(audio_path)
        .await
        .with_context(|| format!("failed to read audio file {}", audio_path.display()))?;
    let file_name = audio_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "lesson.mp3".to_owned());

    let audio_part = multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("audio/mpeg")
        .context("invalid audio mime type")?;

    let mut form = multipart::Form::new()
        .text("title", request.title.clone())
        .text("text", normalized_text.to_owned())
        .text("status", "private")
        .part("audio", audio_part);

    if let Some(collection_id) = request.collection_id {
        form = form.text("collection", collection_id.to_string());
    }
    if let Some(original_url) = &request.original_url {
        form = form.text("original_url", original_url.clone());
    }
    Ok(form)
}

fn normalize_text(text: &str) -> String {
    text.split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_collapses_whitespace_within_paragraphs() {
        assert_eq!(normalize_text("hello   world"), "hello world");
    }

    #[test]
    fn normalize_text_preserves_paragraph_breaks() {
        assert_eq!(
            normalize_text("para one\n\npara two"),
            "para one\n\npara two"
        );
    }

    #[test]
    fn normalize_text_strips_empty_paragraphs() {
        assert_eq!(normalize_text("hello\n\n\n\nworld"), "hello\n\nworld");
    }

    #[test]
    fn normalize_text_empty_input() {
        assert_eq!(normalize_text(""), "");
    }

    #[test]
    fn normalize_text_only_whitespace() {
        assert_eq!(normalize_text("   \n\n   \n\n   "), "");
    }

    #[test]
    fn parse_upload_response_requires_lesson_id() {
        let result = parse_lesson_response("{}", "de", None, "upload");

        assert!(result.is_err());
    }

    #[test]
    fn parse_upload_response_accepts_lesson_url() {
        let (upload, _) = parse_lesson_response(
            r#"{"url":"https://www.lingq.com/de/learn/lesson/44319423/"}"#,
            "de",
            None,
            "upload",
        )
        .expect("url-only response should produce an upload response");

        assert_eq!(upload.lesson_id, 44319423);
        assert_eq!(
            upload.lesson_url,
            "https://www.lingq.com/de/learn/lesson/44319423/"
        );
    }

    #[test]
    fn parse_update_response_accepts_empty_body_with_fallback() {
        let (upload, lesson) = parse_lesson_response("", "de", Some(44319423), "update")
            .expect("empty update response should use known lesson id");

        assert_eq!(upload.lesson_id, 44319423);
        assert_eq!(
            upload.lesson_url,
            "https://www.lingq.com/de/learn/lesson/44319423/"
        );
        assert!(lesson.is_none());
    }

    #[test]
    fn parse_update_response_accepts_missing_id_with_fallback() {
        let (upload, lesson) = parse_lesson_response(
            r#"{"title":"Updated lesson"}"#,
            "de",
            Some(44319423),
            "update",
        )
        .expect("partial update response should use known lesson id");

        assert_eq!(upload.lesson_id, 44319423);
        assert!(lesson.is_some());
    }

    #[test]
    fn parse_update_response_accepts_non_json_with_fallback() {
        let (upload, lesson) = parse_lesson_response("OK", "de", Some(44319423), "update")
            .expect("successful update with non-JSON body should use known lesson id");

        assert_eq!(upload.lesson_id, 44319423);
        assert!(lesson.is_none());
    }
}
