use super::{
    AudioInfo,
    selectors::parsed,
    text::{clean_whitespace, strip_markup},
};
use scraper::Html;
use serde_json::Value;

pub(super) fn parse_client_queries(document: &Html) -> Vec<Value> {
    let mut out = Vec::new();
    for el in document.select(&parsed::CLIENT_QUERIES) {
        let Some(raw) = el.value().attr("data-json") else {
            continue;
        };
        match serde_json::from_str::<Value>(raw) {
            Ok(v) => out.push(v),
            Err(_) => {
                let decoded = decode_html_entities(raw);
                if let Ok(v) = serde_json::from_str::<Value>(&decoded) {
                    out.push(v);
                }
            }
        }
    }
    out
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
}

/// Extract the article copy text from the parsed `js-client-queries` JSON
/// blobs. Deutschlandfunk renders only the lede/teaser as visible HTML
/// `<p>` tags; the full body lives inside an Article entry's
/// `articleCopyText` array. For audio pieces the field is absent and the
/// caller falls back to visible HTML/audio metadata.
pub(super) fn extract_article_copy_text(scripts: &[Value]) -> Option<String> {
    for script in scripts {
        let value = script.get("value")?;
        let typename = value
            .get("__typename")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if typename != "Article" && typename != "AudioArticle" {
            continue;
        }
        let copy = value
            .get("articleCopyText")
            .or_else(|| value.get("audioCopyText"))
            .or_else(|| value.get("copyText"))
            .and_then(|v| v.as_array())?;

        let mut blocks = Vec::new();
        for block in copy {
            let Some(obj) = block.as_object() else {
                continue;
            };
            let kind = obj.get("__typename").and_then(|v| v.as_str()).unwrap_or("");
            let content = obj
                .get("content")
                .and_then(|v| v.as_str())
                .map(strip_markup)
                .map(|s| clean_whitespace(&s))
                .unwrap_or_default();
            match kind {
                "ParagraphText" if !content.is_empty() => blocks.push(content),
                "ParagraphHeading" | "ParagraphSubheading" if !content.is_empty() => {
                    blocks.push(format!("## {content}"));
                }
                "ParagraphList" | "ParagraphBulletList" | "ParagraphOrderedList" => {
                    if let Some(items) = obj.get("items").and_then(|v| v.as_array()) {
                        for item in items {
                            if let Some(t) = item.as_str() {
                                let cleaned = clean_whitespace(t);
                                if !cleaned.is_empty() {
                                    blocks.push(format!("- {cleaned}"));
                                }
                            }
                        }
                    } else if !content.is_empty() {
                        blocks.push(format!("- {content}"));
                    }
                }
                "ParagraphQuote" if !content.is_empty() => {
                    blocks.push(format!("\u{201E}{content}\u{201C}"));
                }
                _ => {}
            }
        }

        if !blocks.is_empty() {
            return Some(blocks.join("\n\n"));
        }
    }
    None
}

/// Pull a few free-floating fields off the Article entry that only exist in
/// the JSON (kicker, leader, news source, etc.).
pub(super) fn extract_article_meta_fields(
    scripts: &[Value],
) -> (Option<String>, Option<String>, Option<String>) {
    for script in scripts {
        let Some(value) = script.get("value") else {
            continue;
        };
        let typename = value
            .get("__typename")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if typename != "Article" && typename != "AudioArticle" {
            continue;
        }
        let s = |k: &str| {
            value
                .get(k)
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        return (
            s("articleKicker").or_else(|| s("kicker")),
            s("articleLeader").or_else(|| s("leader")),
            s("articleNewsSource"),
        );
    }
    (None, None, None)
}

pub(super) fn extract_audio(scripts: &[Value]) -> AudioInfo {
    for script in scripts {
        let value = match script.get("value") {
            Some(v) => v,
            None => continue,
        };
        if value.get("__typename").and_then(|x| x.as_str()) != Some("Audio") {
            continue;
        }
        let s = |key: &str| value.get(key).and_then(|v| v.as_str()).map(str::to_owned);
        let i = |key: &str| value.get(key).and_then(|v| v.as_i64());
        return AudioInfo {
            audio_url: s("audioUrl"),
            download_url: s("downloadUrl"),
            podcast_url: s("audioUrlPodcast"),
            duration_seconds: i("duration"),
            file_size_bytes: i("fileSize"),
            kicker: s("audioKicker"),
            leader: s("audioLeader"),
            show_notes: s("audioShowNotes"),
            author_text: s("authorText"),
            sophora_id: s("sophoraId"),
            dira_id: s("diraId"),
        };
    }
    AudioInfo::default()
}

pub(super) fn first_string_from_scripts(scripts: &[Value], keys: &[&str]) -> Option<String> {
    for script in scripts {
        let value = script.get("value")?;
        for key in keys {
            if let Some(v) = value.get(*key).and_then(|v| v.as_str()) {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_owned());
                }
            }
        }
    }
    None
}

pub(super) fn infer_section_from_scripts(scripts: &[Value]) -> Option<String> {
    first_string_from_scripts(scripts, &["pageType", "siteName"])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_extracts_audio_and_copy_text() {
        let html = include_str!("../../tests/fixtures/audio_article_client_queries.html");
        let document = Html::parse_document(html);
        let scripts = parse_client_queries(&document);

        let audio = extract_audio(&scripts);
        assert_eq!(
            audio.audio_url.as_deref(),
            Some("https://ondemand-mp3.dradio.de/x.mp3")
        );
        assert_eq!(
            audio.download_url.as_deref(),
            Some("https://download.deutschlandfunk.de/x.mp3")
        );
        assert_eq!(audio.duration_seconds, Some(123));

        let copy = extract_article_copy_text(&scripts).unwrap();
        assert!(copy.contains("Erster Absatz mit Text."));
        assert!(copy.contains("## Zwischenuberschrift"));
        assert!(copy.contains("- Ein Listenpunkt"));
    }
}
