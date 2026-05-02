//! Scraper for `deutschlandfunk.de`.
//!
//! Deutschlandfunk pages are server-rendered Sophora pages whose data is
//! duplicated into a large pile of `<script class="js-client-queries"
//! data-json="...">` tags (HTML-encoded JSON). For full coverage of audio
//! metadata we parse those scripts; for plain article body text we parse the
//! visible `<article class="b-article">` markup.

use anyhow::{Context, Result, bail};
use log::{debug, info, warn};
use regex::Regex;
use reqwest::{Client, StatusCode};
use scraper::{ElementRef, Html, Selector};
use serde_json::Value;
use std::{
    collections::{HashSet, VecDeque},
    sync::LazyLock,
    time::Duration,
};

const BASE_URL: &str = "https://www.deutschlandfunk.de";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36";

mod parsed {
    use super::*;
    pub static LINKS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::selectors::LINKS).unwrap());
    pub static ARTICLE: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::selectors::ARTICLE).unwrap());
    pub static BODY_ELEMENTS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::selectors::BODY_ELEMENTS).unwrap());
    pub static BODY_FALLBACK: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::selectors::BODY_FALLBACK).unwrap());
    pub static HEADLINE: LazyLock<Vec<Selector>> = LazyLock::new(|| {
        super::selectors::HEADLINE
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect()
    });
    pub static CLIENT_QUERIES: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("script.js-client-queries[data-json]").unwrap());
}

mod selectors {
    pub const TITLE: &[&str] = &["meta[property=\"og:title\"]", "h1", "title"];
    pub const SUBTITLE: &[&str] = &["meta[name=\"description\"]"];
    pub const AUTHOR_FALLBACK: &[&str] = &["[rel=\"author\"]", ".author", "[class*=\"author\"]"];
    pub const SECTION: &[&str] = &["meta[property=\"article:section\"]"];
    pub const DATE_TIME: &[&str] = &["time[datetime]"];
    pub const DATE_META: &[&str] = &[
        "meta[property=\"article:published_time\"]",
        "meta[name=\"date\"]",
    ];

    /// The visible article container. Deutschlandfunk wraps article content in
    /// `<article class="b-article">` (sometimes additional modifier classes).
    pub const ARTICLE: &str = "article.b-article, article[class*=\"b-article\"], article";
    pub const BODY_ELEMENTS: &str = "p, h2, h3, li";
    pub const BODY_FALLBACK: &str = "main p";

    pub const LINKS: &str = "a[href]";

    pub const HEADLINE: &[&str] = &[".headline", "h1", "h2", "h3", ".b-teaser-headline"];

    /// Boilerplate / navigation strings that creep into the body text.
    pub const BOILERPLATE_MARKERS: &[&str] = &[
        "Audio herunterladen",
        "Mehr zum Thema",
        "Diesen Beitrag teilen",
        "Newsletter abonnieren",
        "Das könnte Sie auch interessieren",
        "Beitrag teilen",
        "Datenschutz",
        "Impressum",
        "Deutschlandfunk App",
        "Folgen Sie uns auf",
        "Akzeptieren",
        "Cookie",
    ];
}

#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
}

/// Builtin Deutschlandfunk section landing pages. URLs verified against
/// deutschlandfunk.de — every entry returns a 200 from the site as of the
/// April 2026 audit. If a future redesign breaks one, run `cargo run --
/// browse-url --url <…>` to confirm and update here.
pub const SECTIONS: &[Section] = &[
    Section {
        id: "startseite",
        label: "Startseite",
        url: "https://www.deutschlandfunk.de/",
    },
    Section {
        id: "nachrichten",
        label: "Nachrichten",
        url: "https://www.deutschlandfunk.de/nachrichten-100.html",
    },
    Section {
        id: "hintergrund",
        label: "Hintergrund",
        url: "https://www.deutschlandfunk.de/hintergrund-100.html",
    },
    Section {
        id: "interview",
        label: "Interview",
        url: "https://www.deutschlandfunk.de/interview-100.html",
    },
    Section {
        id: "kommentar",
        label: "Kommentar",
        url: "https://www.deutschlandfunk.de/kommentare-und-themen-der-woche-100.html",
    },
    Section {
        id: "europa",
        label: "Europa heute",
        url: "https://www.deutschlandfunk.de/europa-heute-100.html",
    },
    Section {
        id: "wissenschaft",
        label: "Forschung aktuell",
        url: "https://www.deutschlandfunk.de/forschung-aktuell-100.html",
    },
    Section {
        id: "umwelt",
        label: "Umwelt und Verbraucher",
        url: "https://www.deutschlandfunk.de/umwelt-und-verbraucher-100.html",
    },
    Section {
        id: "kultur",
        label: "Kultur heute",
        url: "https://www.deutschlandfunk.de/kultur-heute-100.html",
    },
    Section {
        id: "buecher",
        label: "Büchermarkt",
        url: "https://www.deutschlandfunk.de/buechermarkt-100.html",
    },
    Section {
        id: "wirtschaft",
        label: "Wirtschaft am Mittag",
        url: "https://www.deutschlandfunk.de/wirtschaft-am-mittag-100.html",
    },
    Section {
        id: "sport",
        label: "Sport am Wochenende",
        url: "https://www.deutschlandfunk.de/sport-am-wochenende-100.html",
    },
    Section {
        id: "andruck",
        label: "Andruck (Sachbuch)",
        url: "https://www.deutschlandfunk.de/andruck-100.html",
    },
    Section {
        id: "essay",
        label: "Essay und Diskurs",
        url: "https://www.deutschlandfunk.de/essay-und-diskurs-100.html",
    },
    Section {
        id: "feature",
        label: "Das Feature",
        url: "https://www.deutschlandfunk.de/das-feature-100.html",
    },
    Section {
        id: "hoerspiel",
        label: "Hörspiel",
        url: "https://www.deutschlandfunk.de/hoerspiel-100.html",
    },
    Section {
        id: "presseschau",
        label: "Presseschau (DE)",
        url: "https://www.deutschlandfunk.de/presseschau-100.html",
    },
    Section {
        id: "presseschau-int",
        label: "Presseschau (intl.)",
        url: "https://www.deutschlandfunk.de/internationale-presseschau-100.html",
    },
    Section {
        id: "magazin",
        label: "Dlf-Magazin",
        url: "https://www.deutschlandfunk.de/dlf-magazin-102.html",
    },
];

#[derive(Debug, Clone)]
pub struct ArticleSummary {
    pub url: String,
    pub title: String,
    pub teaser: String,
    pub section: String,
    pub source_kind: DiscoverySourceKind,
    pub source_label: String,
    /// True if the listing teaser indicates an audio attachment (best-effort).
    pub has_audio_hint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySourceKind {
    Section,
    Subsection,
    Topic,
    Search,
}

impl DiscoverySourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Subsection => "subsection",
            Self::Topic => "topic",
            Self::Search => "search",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiscoveryReport {
    pub source_pages_visited: usize,
    pub section_pages_visited: usize,
    pub subsection_pages_visited: usize,
    pub topic_pages_visited: usize,
    pub section_articles: usize,
    pub subsection_articles: usize,
    pub topic_articles: usize,
    pub deduped_articles: usize,
}

impl DiscoveryReport {
    fn record_source_visit(&mut self, source_kind: DiscoverySourceKind) {
        self.source_pages_visited += 1;
        match source_kind {
            DiscoverySourceKind::Section => self.section_pages_visited += 1,
            DiscoverySourceKind::Subsection => self.subsection_pages_visited += 1,
            DiscoverySourceKind::Topic => self.topic_pages_visited += 1,
            DiscoverySourceKind::Search => {}
        }
    }
    fn record_article(&mut self, source_kind: DiscoverySourceKind) {
        match source_kind {
            DiscoverySourceKind::Section => self.section_articles += 1,
            DiscoverySourceKind::Subsection => self.subsection_articles += 1,
            DiscoverySourceKind::Topic => self.topic_articles += 1,
            DiscoverySourceKind::Search => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct BrowseSectionResult {
    pub articles: Vec<ArticleSummary>,
    pub report: DiscoveryReport,
}

/// Audio metadata extracted from a Deutschlandfunk article. All URL fields are
/// optional because some articles have only a download URL or only the
/// streaming URL available.
#[derive(Debug, Clone, Default)]
pub struct AudioInfo {
    pub audio_url: Option<String>,
    pub download_url: Option<String>,
    pub podcast_url: Option<String>,
    pub duration_seconds: Option<i64>,
    pub file_size_bytes: Option<i64>,
    pub kicker: Option<String>,
    pub leader: Option<String>,
    pub show_notes: Option<String>,
    pub author_text: Option<String>,
    pub sophora_id: Option<String>,
    pub dira_id: Option<String>,
}

impl AudioInfo {
    pub fn is_empty(&self) -> bool {
        self.audio_url.is_none() && self.download_url.is_none() && self.podcast_url.is_none()
    }

    /// Pick the URL most suitable for downloading the MP3 — prefer the
    /// `downloadUrl` (direct CDN) over the streaming `audioUrl`.
    pub fn best_download_url(&self) -> Option<&str> {
        self.download_url
            .as_deref()
            .or(self.audio_url.as_deref())
            .or(self.podcast_url.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct Article {
    pub url: String,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub date: String,
    pub section: String,
    pub body_text: String,
    pub clean_text: String,
    pub word_count: usize,
    pub difficulty: i64,
    pub fetched_at: String,
    /// True when site-style indicators (or absent body text) suggest the page
    /// is primarily an audio piece without a full transcript.
    pub paywalled: bool,
    pub audio: AudioInfo,
}

#[derive(Debug, Clone)]
pub struct ArticleMetadata {
    pub url: String,
    pub title: String,
    pub date: String,
    pub section: String,
}

#[derive(Clone)]
pub struct DeutschlandfunkClient {
    client: Client,
    article_url_re: Regex,
}

impl DeutschlandfunkClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client")?;
        // Article URLs follow the pattern <slug>-NNN.html
        let article_url_re =
            Regex::new(r"^https://www\.deutschlandfunk\.de/[a-z0-9öäüß][\w%./\-]*-\d+\.html$")
                .context("bad article regex")?;

        Ok(Self {
            client,
            article_url_re,
        })
    }

    pub fn sections(&self) -> &'static [Section] {
        SECTIONS
    }

    pub fn section_by_id(&self, id: &str) -> Option<&'static Section> {
        SECTIONS.iter().find(|section| section.id == id)
    }

    /// Scan the homepage for nav entries that
    /// aren't in the SECTIONS list. Returns (url, label) tuples; never errors
    /// fatally because the site's nav is decorative for our flow.
    pub async fn discover_new_sections(&self) -> Result<Vec<(String, String)>> {
        let html = self.client.get(BASE_URL).send().await?.text().await?;
        let document = Html::parse_document(&html);
        let nav_sel =
            Selector::parse("nav a[href]").unwrap_or_else(|_| Selector::parse("a").unwrap());
        let known: HashSet<&str> = SECTIONS.iter().map(|s| s.url).collect();
        let mut discovered = Vec::new();
        for el in document.select(&nav_sel) {
            let Some(href) = el.value().attr("href") else {
                continue;
            };
            let url = absolute_url(href);
            if !url.starts_with(BASE_URL) || known.contains(url.as_str()) {
                continue;
            }
            // Section-style landings end with -100.html / -102.html etc.
            if !Regex::new(r"-\d+\.html$").unwrap().is_match(&url) {
                continue;
            }
            let label = clean_whitespace(&collect_text(el));
            if label.is_empty() {
                continue;
            }
            if !discovered.iter().any(|(u, _): &(String, String)| *u == url) {
                debug!("Discovered nav section: {label} → {url}");
                discovered.push((url, label));
            }
        }
        Ok(discovered)
    }

    pub async fn browse_section(
        &self,
        section: &Section,
        limit: usize,
    ) -> Result<Vec<ArticleSummary>> {
        Ok(self.browse_section_detailed(section, limit).await?.articles)
    }

    pub async fn browse_section_detailed(
        &self,
        section: &Section,
        limit: usize,
    ) -> Result<BrowseSectionResult> {
        let mut articles = Vec::new();
        let mut seen_articles = HashSet::new();
        let mut queued = VecDeque::new();
        let mut seen_sources = HashSet::new();
        let mut report = DiscoveryReport::default();
        let max_sources = limit.max(40).div_ceil(20).clamp(6, 30);

        queued.push_back((
            section.url.to_owned(),
            section.label.to_owned(),
            DiscoverySourceKind::Section,
        ));

        while let Some((url, fallback_section, kind)) = queued.pop_front() {
            if !seen_sources.insert(url.clone()) {
                continue;
            }
            if seen_sources.len() > max_sources || articles.len() >= limit {
                break;
            }

            let html = self.fetch_html(&url).await?;
            let document = Html::parse_document(&html);
            report.record_source_visit(kind);

            self.collect_articles_from_document(
                &document,
                Some(&fallback_section),
                &url,
                kind,
                limit,
                &mut seen_articles,
                &mut articles,
                &mut report,
            );

            if articles.len() >= limit {
                break;
            }
        }

        Ok(BrowseSectionResult { articles, report })
    }

    pub async fn browse_url(
        &self,
        url: &str,
        fallback_section: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ArticleSummary>> {
        let html = self.fetch_html(url).await?;
        let document = Html::parse_document(&html);
        let mut articles = Vec::new();
        let mut seen = HashSet::new();
        self.collect_articles_from_document(
            &document,
            fallback_section,
            url,
            DiscoverySourceKind::Section,
            limit,
            &mut seen,
            &mut articles,
            &mut DiscoveryReport::default(),
        );
        Ok(articles)
    }

    /// Search via the public /suche/ endpoint. The site renders results
    /// server-side as ordinary article links, so the same link-collector works.
    pub async fn search_articles(
        &self,
        query: &str,
        max_pages: usize,
    ) -> Result<Vec<ArticleSummary>> {
        if query.trim().is_empty() {
            bail!("search query is empty");
        }

        let encoded = urlencoding::encode(query.trim());
        let mut articles = Vec::new();
        let mut seen = HashSet::new();
        let mut report = DiscoveryReport::default();

        for page in 0..max_pages {
            let url = format!(
                "{BASE_URL}/suche/?drsearch:query={encoded}&drsearch:offset={}",
                page * 20
            );
            let html = match self.fetch_html(&url).await {
                Ok(h) => h,
                Err(err) => {
                    warn!("search page fetch failed: {err:#}");
                    break;
                }
            };
            let document = Html::parse_document(&html);
            let before = articles.len();
            self.collect_articles_from_document(
                &document,
                None,
                &url,
                DiscoverySourceKind::Search,
                usize::MAX,
                &mut seen,
                &mut articles,
                &mut report,
            );
            if articles.len() == before {
                break;
            }
            for a in &mut articles[before..] {
                a.source_label = format!("search: {query}");
            }
        }

        Ok(articles)
    }

    pub async fn fetch_article(&self, url: &str) -> Result<Article> {
        info!("Fetching article: {url}");
        let html = self.fetch_html(url).await?;
        let document = Html::parse_document(&html);

        // Parse the embedded js-client-queries scripts once — used by both
        // metadata fallbacks and audio extraction.
        let scripts = parse_client_queries(&document);

        let title = first_text(&document, selectors::TITLE)
            .map(|v| {
                v.replace(" | deutschlandfunk.de", "")
                    .replace(" | Deutschlandfunk", "")
                    .trim()
                    .to_owned()
            })
            .filter(|v| !v.is_empty())
            .or_else(|| first_string_from_scripts(&scripts, &["title", "seoTitle"]))
            .unwrap_or_else(|| "Untitled".to_owned());

        let subtitle = first_attr(&document, selectors::SUBTITLE, "content")
            .or_else(|| {
                first_string_from_scripts(
                    &scripts,
                    &["teaserHeadline", "teasertext", "seoTeaserText"],
                )
            })
            .unwrap_or_default();

        let author = first_attr(&document, &["meta[property=\"article:author\"]"], "content")
            .or_else(|| first_text(&document, selectors::AUTHOR_FALLBACK))
            .or_else(|| first_string_from_scripts(&scripts, &["author", "authorText"]))
            .unwrap_or_default();

        let date = extract_date(&document, &html, url, &scripts)
            .map(|d| normalize_date(&d))
            .unwrap_or_default();

        let section = first_attr(&document, selectors::SECTION, "content")
            .or_else(|| infer_section_from_scripts(&scripts))
            .unwrap_or_else(|| infer_section_from_url(url));

        let audio = extract_audio(&scripts);
        let (kicker, leader, _news_source) = extract_article_meta_fields(&scripts);

        // 1. Primary body source: Article.articleCopyText JSON array.
        // 2. Fallback: visible <article class="b-article"> HTML.
        // 3. Last resort: lede + audio show notes.
        let mut body_text = extract_article_copy_text(&scripts)
            .or_else(|| extract_body(&document, &audio).ok())
            .unwrap_or_default();

        // If we still have very little body, prepend the leader + show notes
        // so we never produce a totally empty record for short news pieces.
        if body_text.split_whitespace().count() < 30 {
            let mut prefix = Vec::new();
            if let Some(l) = leader.as_deref().filter(|s| !s.is_empty()) {
                prefix.push(l.to_owned());
            }
            if let Some(notes) = audio.show_notes.as_deref().filter(|s| !s.is_empty()) {
                prefix.push(notes.to_owned());
            }
            if !prefix.is_empty() {
                let combined = if body_text.is_empty() {
                    prefix.join("\n\n")
                } else {
                    format!("{}\n\n{}", prefix.join("\n\n"), body_text)
                };
                body_text = combined;
            }
        }

        let word_count = body_text.split_whitespace().count();

        // Threshold: short news flashes (1–2 sentences) are still valid
        // articles. Only reject totally empty extractions.
        let has_audio = !audio.is_empty();
        if word_count < 8 && !has_audio {
            bail!("article extraction produced no body text for {url}");
        }

        // Mark very short pieces as "truncated" so the GUI flags them.
        let paywalled = word_count < 50 && has_audio;
        // Forward the kicker into the audio struct if the article had one
        // and audio didn't already set its own kicker.
        let mut audio = audio;
        if audio
            .kicker
            .as_deref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
        {
            audio.kicker = kicker;
        }

        let clean_text = build_clean_text(&title, &subtitle, &author, &date, &body_text, &audio);
        let difficulty = estimate_difficulty(&body_text);

        Ok(Article {
            url: url.to_owned(),
            title,
            subtitle,
            author,
            date,
            section,
            body_text,
            clean_text,
            word_count,
            difficulty,
            paywalled,
            fetched_at: iso_timestamp_now(),
            audio,
        })
    }

    pub async fn fetch_article_metadata(&self, url: &str) -> Result<ArticleMetadata> {
        let html = self.fetch_html(url).await?;
        let document = Html::parse_document(&html);
        let scripts = parse_client_queries(&document);

        let title = first_text(&document, selectors::TITLE)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "Untitled".to_owned());
        let date = extract_date(&document, &html, url, &scripts)
            .map(|d| normalize_date(&d))
            .unwrap_or_default();
        let section = first_attr(&document, selectors::SECTION, "content")
            .or_else(|| infer_section_from_scripts(&scripts))
            .unwrap_or_else(|| infer_section_from_url(url));

        Ok(ArticleMetadata {
            url: url.to_owned(),
            title,
            date,
            section,
        })
    }

    /// Stream-download an audio file to `dest`. Returns total bytes written.
    /// `progress` is invoked with `(downloaded, total_or_zero)` periodically.
    pub async fn download_audio<F>(
        &self,
        audio_url: &str,
        dest: &std::path::Path,
        mut progress: F,
    ) -> Result<u64>
    where
        F: FnMut(u64, u64),
    {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let response = self
            .client
            .get(audio_url)
            .send()
            .await
            .with_context(|| format!("audio request failed for {audio_url}"))?;
        let response = response
            .error_for_status()
            .with_context(|| format!("audio host rejected the request for {audio_url}"))?;
        let total = response.content_length().unwrap_or(0);

        let tmp = dest.with_extension("mp3.partial");
        let mut file = tokio::fs::File::create(&tmp)
            .await
            .with_context(|| format!("failed to create {}", tmp.display()))?;
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("audio stream broke")?;
            file.write_all(&chunk)
                .await
                .context("failed to write audio chunk")?;
            downloaded += chunk.len() as u64;
            progress(downloaded, total);
        }
        file.flush().await.ok();
        drop(file);
        tokio::fs::rename(&tmp, dest)
            .await
            .with_context(|| format!("failed to move audio to {}", dest.display()))?;
        Ok(downloaded)
    }

    async fn fetch_html(&self, url: &str) -> Result<String> {
        let mut last_error = None;
        for attempt in 1..=3 {
            debug!("HTTP GET {url} (attempt {attempt})");
            match self.client.get(url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return response
                            .text()
                            .await
                            .with_context(|| format!("network: failed to read body for {url}"));
                    }
                    let retryable = is_retryable_status(status);
                    warn!("HTTP {status} for {url} (retryable={retryable}, attempt={attempt})");
                    last_error = Some(anyhow::anyhow!(
                        "network: non-success response {} for {}",
                        status,
                        url
                    ));
                    if !retryable || attempt == 3 {
                        break;
                    }
                }
                Err(err) => {
                    warn!("HTTP request failed for {url}: {err} (attempt {attempt})");
                    last_error = Some(anyhow::anyhow!("network: request failed for {url}: {err}"));
                    if attempt == 3 {
                        break;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(450 * attempt as u64)).await;
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("network: failed to fetch {url}")))
    }

    fn collect_articles_from_document(
        &self,
        document: &Html,
        fallback_section: Option<&str>,
        source_url: &str,
        source_kind: DiscoverySourceKind,
        limit: usize,
        seen: &mut HashSet<String>,
        articles: &mut Vec<ArticleSummary>,
        report: &mut DiscoveryReport,
    ) {
        let selector = parsed::LINKS.clone();
        for link in document.select(&selector) {
            let Some(raw_href) = link.value().attr("href") else {
                continue;
            };
            let article_url = canonical_article_url(&absolute_url(raw_href));
            if !self.article_url_re.is_match(&article_url) {
                continue;
            }
            // Skip section-only landing pages (their slugs are short)
            if is_landing_url(&article_url) {
                continue;
            }
            if !seen.insert(article_url.clone()) {
                report.deduped_articles += 1;
                continue;
            }

            let title = self.extract_browse_title(link);
            if !looks_like_article_title(&title) {
                continue;
            }
            let teaser = self.extract_teaser(link);
            let section = fallback_section
                .map(str::to_owned)
                .unwrap_or_else(|| infer_section_from_url(&article_url));
            let has_audio_hint = link
                .value()
                .classes()
                .any(|c| c.contains("audio") || c.contains("podcast"));

            articles.push(ArticleSummary {
                url: article_url,
                title,
                teaser,
                section,
                source_kind,
                source_label: source_label(source_url),
                has_audio_hint,
            });
            report.record_article(source_kind);
            if articles.len() >= limit {
                break;
            }
        }
    }

    fn extract_browse_title(&self, link: ElementRef<'_>) -> String {
        let mut parent = link.parent();
        for _ in 0..3 {
            let Some(node) = parent else { break };
            if let Some(element) = ElementRef::wrap(node) {
                for selector in parsed::HEADLINE.iter() {
                    if let Some(candidate) = element
                        .select(selector)
                        .map(collect_text)
                        .map(|t| clean_whitespace(&t))
                        .find(|t| looks_like_article_title(t))
                    {
                        return candidate;
                    }
                }
            }
            parent = node.parent();
        }
        let direct = clean_whitespace(&collect_text(link));
        direct
    }

    fn extract_teaser(&self, link: ElementRef<'_>) -> String {
        let title = self.extract_browse_title(link);
        let mut parent = link.parent();
        for _ in 0..3 {
            let Some(node) = parent else { break };
            if let Some(element) = ElementRef::wrap(node) {
                let text = strip_markup(&clean_whitespace(&collect_text(element)));
                if text.len() > title.len() && text.len() > 20 {
                    return trim_chars(&text, 220);
                }
            }
            parent = node.parent();
        }
        String::new()
    }
}

// ── Body / metadata extraction ────────────────────────────────────────────

fn parse_client_queries(document: &Html) -> Vec<Value> {
    let mut out = Vec::new();
    for el in document.select(&parsed::CLIENT_QUERIES) {
        let Some(raw) = el.value().attr("data-json") else {
            continue;
        };
        // The attribute is HTML-encoded — scraper already decodes entities,
        // but in case of stray escapes we run a small fixup.
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
/// `articleCopyText` array (a list of `ParagraphText` / `ParagraphHeading`
/// / `ParagraphList` blocks). For audio pieces the field is absent — we
/// fall back to the visible HTML.
fn extract_article_copy_text(scripts: &[Value]) -> Option<String> {
    for script in scripts {
        let value = script.get("value")?;
        let typename = value
            .get("__typename")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if typename != "Article" && typename != "AudioArticle" {
            continue;
        }
        // The field name varies slightly across content types.
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
            if content.is_empty() {
                continue;
            }
            match kind {
                "ParagraphText" => blocks.push(content),
                "ParagraphHeading" | "ParagraphSubheading" => blocks.push(format!("## {content}")),
                "ParagraphList" | "ParagraphBulletList" | "ParagraphOrderedList" => {
                    // Some lists store items in a separate field; fall back to plain content.
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
                "ParagraphQuote" => blocks.push(format!("\u{201E}{content}\u{201C}")),
                _ => {} // ignore ParagraphImage, ParagraphAudio, ParagraphGallery, …
            }
        }

        if blocks.is_empty() {
            continue;
        }
        return Some(blocks.join("\n\n"));
    }
    None
}

/// Pull a few free-floating fields off the Article entry that only exist in
/// the JSON (kicker, leader, news source, etc.). Returns `(kicker, leader,
/// news_source)` — any of them may be empty.
fn extract_article_meta_fields(
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

fn extract_audio(scripts: &[Value]) -> AudioInfo {
    // Find first script whose .value.__typename == "Audio"; capture its main fields.
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

fn first_string_from_scripts(scripts: &[Value], keys: &[&str]) -> Option<String> {
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

fn extract_date(document: &Html, html: &str, url: &str, scripts: &[Value]) -> Option<String> {
    first_attr(document, selectors::DATE_TIME, "datetime")
        .or_else(|| first_attr(document, selectors::DATE_META, "content"))
        .or_else(|| {
            first_string_from_scripts(
                scripts,
                &[
                    "firstPublicationDate",
                    "publicationDate",
                    "date",
                    "dateLocalizedFormatted",
                ],
            )
        })
        .or_else(|| extract_date_from_text(html))
        .or_else(|| extract_date_from_url(url))
}

static RE_DATE_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d{1,2}\.\d{1,2}\.\d{4})\b").unwrap());
static RE_DATE_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/(\d{4})/(\d{2})/(\d{2})/").unwrap());

fn extract_date_from_text(html: &str) -> Option<String> {
    RE_DATE_TEXT
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned())
}

fn extract_date_from_url(url: &str) -> Option<String> {
    let c = RE_DATE_URL.captures(url)?;
    Some(format!(
        "{}-{}-{}",
        c.get(1)?.as_str(),
        c.get(2)?.as_str(),
        c.get(3)?.as_str()
    ))
}

fn infer_section_from_scripts(scripts: &[Value]) -> Option<String> {
    first_string_from_scripts(scripts, &["pageType", "siteName"])
}

fn infer_section_from_url(url: &str) -> String {
    let path = url
        .trim_start_matches(BASE_URL)
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("Deutschlandfunk")
        .to_owned();
    if path.is_empty() {
        return "Startseite".to_owned();
    }
    // The slug usually ends with "-NNN.html" — strip and humanize
    let slug = path.trim_end_matches(".html");
    let slug = Regex::new(r"-\d+$").unwrap().replace(slug, "");
    let humanized = slug.replace('-', " ");
    humanized
}

const EXCLUDE_ANCESTOR_TAGS: &[&str] =
    &["figure", "figcaption", "aside", "nav", "footer", "header"];
const EXCLUDE_ANCESTOR_CLASSES: &[&str] = &[
    "b-related",
    "b-teaser",
    "b-audio-player",
    "b-podcast",
    "b-share",
    "b-newsletter",
    "b-promotion",
];

fn has_excluded_ancestor(node: &ElementRef<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent_ref) = current {
        if let Some(element) = parent_ref.value().as_element() {
            let tag = element.name();
            if EXCLUDE_ANCESTOR_TAGS.contains(&tag) {
                return true;
            }
            if let Some(classes) = element.attr("class") {
                if EXCLUDE_ANCESTOR_CLASSES.iter().any(|c| classes.contains(c)) {
                    return true;
                }
            }
        }
        current = parent_ref.parent();
    }
    false
}

fn extract_body(document: &Html, audio: &AudioInfo) -> Result<String> {
    let article_selector = parsed::ARTICLE.clone();
    let para_selector = parsed::BODY_ELEMENTS.clone();
    let markers = selectors::BOILERPLATE_MARKERS;
    let mut best_blocks = Vec::new();

    for article in document.select(&article_selector) {
        let mut blocks = Vec::new();
        for node in article.select(&para_selector) {
            if has_excluded_ancestor(&node) {
                continue;
            }
            let name = node.value().name();
            let text = clean_whitespace(&collect_text(node));
            if text.is_empty() || markers.iter().any(|m| text.contains(m)) {
                continue;
            }
            match name {
                "h2" | "h3" if text.len() >= 4 => blocks.push(format!("## {text}")),
                "li" if text.len() >= 20 => blocks.push(format!("- {text}")),
                "p" if text.len() >= 45 => blocks.push(text),
                _ => {}
            }
        }
        if blocks.len() > best_blocks.len() {
            best_blocks = blocks;
        }
    }

    if best_blocks.is_empty() {
        let fallback = parsed::BODY_FALLBACK.clone();
        for node in document.select(&fallback) {
            let text = clean_whitespace(&collect_text(node));
            if text.len() >= 45 {
                best_blocks.push(text);
            }
        }
    }

    // For audio-first pieces add the show notes / leader if body is empty.
    if best_blocks.is_empty() {
        if let Some(notes) = audio.show_notes.as_ref().filter(|s| !s.trim().is_empty()) {
            for line in notes.split('\n') {
                let cleaned = clean_whitespace(line);
                if cleaned.len() >= 30 {
                    best_blocks.push(cleaned);
                }
            }
        }
        if let Some(leader) = audio.leader.as_ref().filter(|s| !s.trim().is_empty()) {
            best_blocks.push(clean_whitespace(leader));
        }
    }

    dedupe_lines(&mut best_blocks);
    if best_blocks.is_empty() {
        bail!("could not extract article body");
    }
    Ok(best_blocks.join("\n\n"))
}

fn dedupe_lines(lines: &mut Vec<String>) {
    let mut seen = HashSet::new();
    lines.retain(|line| {
        let key = trim_chars(line, 120).to_lowercase();
        seen.insert(key)
    });
}

fn build_clean_text(
    title: &str,
    subtitle: &str,
    author: &str,
    date: &str,
    body: &str,
    audio: &AudioInfo,
) -> String {
    let normalized_subtitle = clean_whitespace(subtitle);
    let normalized_body = normalize_body_for_lingq(body, title, &normalized_subtitle);

    let mut pieces = vec![title.to_owned()];
    if !normalized_subtitle.is_empty() && !same_enough(&normalized_subtitle, title) {
        pieces.push(String::new());
        pieces.push(normalized_subtitle);
    }
    if !author.is_empty() {
        pieces.push(format!("Von {author}"));
    } else if let Some(at) = audio.author_text.as_ref().filter(|s| !s.trim().is_empty()) {
        pieces.push(format!("Von {at}"));
    }
    if !date.is_empty() {
        pieces.push(date.to_owned());
    }
    if let Some(kicker) = audio.kicker.as_ref().filter(|s| !s.trim().is_empty()) {
        pieces.push(String::new());
        pieces.push(format!("[{kicker}]"));
    }
    pieces.push(String::new());
    pieces.push(normalized_body);
    pieces.join("\n")
}

fn normalize_body_for_lingq(body: &str, title: &str, subtitle: &str) -> String {
    let canon_title = canonical_text(title);
    let canon_subtitle = canonical_text(subtitle);
    let mut blocks = Vec::new();
    for raw in body.split("\n\n") {
        let block = clean_whitespace(raw);
        if block.is_empty() {
            continue;
        }
        let normalized = if let Some(h) = block.strip_prefix("## ") {
            h.trim().to_owned()
        } else {
            block
        };
        let canon = canonical_text(&normalized);
        if matches_title_or_subtitle(&canon, &canon_title, &canon_subtitle) {
            continue;
        }
        blocks.push(normalized);
    }
    dedupe_similar_blocks(&mut blocks);
    blocks.join("\n\n")
}

fn dedupe_similar_blocks(blocks: &mut Vec<String>) {
    let mut seen: HashSet<String> = HashSet::new();
    blocks.retain(|b| {
        let canon = canonical_text(b);
        if seen.contains(&canon) {
            return false;
        }
        let dup = seen.iter().any(|e| near_duplicate_text(e, &canon));
        if dup {
            return false;
        }
        seen.insert(canon)
    });
}

fn canonical_text(value: &str) -> String {
    let collapsed: String = value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ch.is_whitespace())
        .collect();
    let mut result = String::with_capacity(collapsed.len());
    for word in collapsed.split_whitespace() {
        if !result.is_empty() {
            result.push(' ');
        }
        result.push_str(word);
    }
    result.to_lowercase()
}

fn matches_title_or_subtitle(canon_block: &str, canon_title: &str, canon_subtitle: &str) -> bool {
    if canon_block.is_empty() {
        return false;
    }
    if canon_block == canon_title || overlaps_canonical(canon_block, canon_title) {
        return true;
    }
    !canon_subtitle.is_empty()
        && (canon_block == canon_subtitle || overlaps_canonical(canon_block, canon_subtitle))
}

fn same_enough(left: &str, right: &str) -> bool {
    let l = canonical_text(left);
    let r = canonical_text(right);
    !l.is_empty() && l == r
}

fn overlaps_canonical(left: &str, right: &str) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let shorter = left.len().min(right.len());
    if shorter < 40 {
        return false;
    }
    left.contains(right) || right.contains(left)
}

fn near_duplicate_text(left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    let shorter = left.len().min(right.len());
    if shorter < 80 {
        return false;
    }
    let prefix = shorter.min(180);
    trim_chars(left, prefix) == trim_chars(right, prefix)
}

fn first_text(document: &Html, selectors: &[&str]) -> Option<String> {
    for s in selectors {
        let Ok(sel) = Selector::parse(s) else {
            continue;
        };
        let value = document.select(&sel).find_map(|node| {
            let attr_content = node.value().attr("content").map(clean_whitespace);
            let text_content =
                Some(clean_whitespace(&collect_text(node))).filter(|v| !v.is_empty());
            attr_content.or(text_content)
        });
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            return Some(v);
        }
    }
    None
}

fn first_attr(document: &Html, selectors: &[&str], attr: &str) -> Option<String> {
    for s in selectors {
        let Ok(sel) = Selector::parse(s) else {
            continue;
        };
        if let Some(v) = document
            .select(&sel)
            .find_map(|node| node.value().attr(attr))
            .map(clean_whitespace)
            .filter(|v| !v.is_empty())
        {
            return Some(v);
        }
    }
    None
}

fn looks_like_article_title(title: &str) -> bool {
    title.len() >= 12
        && title.len() <= 240
        && !title.starts_with("Cookie")
        && !title.starts_with("Datenschutz")
        && !title.starts_with("Impressum")
}

fn is_landing_url(url: &str) -> bool {
    // Landing pages have very short slugs; treat any URL with <= 2 slug
    // segments before the trailing -NNN.html as "landing".
    let path = url.trim_start_matches(BASE_URL).trim_start_matches('/');
    let main = path.trim_end_matches(".html");
    let segments: Vec<&str> = main.split('-').collect();
    // remove trailing all-digit suffix
    let alpha_segments = segments
        .iter()
        .take_while(|s| !s.chars().all(|c| c.is_ascii_digit()))
        .count();
    alpha_segments <= 2
}

fn canonical_article_url(url: &str) -> String {
    // Strip query strings and fragments for dedup
    let mut u = url.to_owned();
    if let Some(idx) = u.find('?') {
        u.truncate(idx);
    }
    if let Some(idx) = u.find('#') {
        u.truncate(idx);
    }
    u
}

fn absolute_url(raw_href: &str) -> String {
    if raw_href.starts_with("http://") || raw_href.starts_with("https://") {
        return raw_href.to_owned();
    }
    if raw_href.starts_with('/') {
        return format!("{BASE_URL}{raw_href}");
    }
    format!("{BASE_URL}/{raw_href}")
}

fn collect_text(node: ElementRef<'_>) -> String {
    node.text().collect::<String>()
}

static RE_PUNCTUATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([,;:.!?)])").unwrap());
static RE_OPENING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([(\[])\s+").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

fn clean_whitespace(input: &str) -> String {
    let cleaned = input
        .replace(
            [
                '\u{00ad}', '\u{200b}', '\u{200c}', '\u{200d}', '\u{2060}', '\u{feff}',
            ],
            "",
        )
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cleaned = RE_PUNCTUATION.replace_all(&cleaned, "$1").into_owned();
    RE_OPENING.replace_all(&cleaned, "$1").into_owned()
}

fn strip_markup(input: &str) -> String {
    clean_whitespace(&RE_TAG.replace_all(input, " "))
}

fn trim_chars(input: &str, max: usize) -> String {
    input.chars().take(max).collect()
}

fn iso_timestamp_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn source_label(source_url: &str) -> String {
    if source_url == BASE_URL || source_url.trim_end_matches('/') == BASE_URL {
        return "Startseite".to_owned();
    }
    let path = source_url.trim_start_matches(BASE_URL).trim_matches('/');
    if path.is_empty() {
        "Startseite".to_owned()
    } else {
        path.to_owned()
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status.is_server_error()
        || matches!(
            status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::REQUEST_TIMEOUT
        )
}

fn normalize_date(input: &str) -> String {
    let trimmed = input.trim();
    if chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").is_ok() && trimmed.len() == 10 {
        return trimmed.to_owned();
    }
    if trimmed.len() >= 10 {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(&trimmed[..10], "%Y-%m-%d") {
            return d.format("%Y-%m-%d").to_string();
        }
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(trimmed, "%d.%m.%Y") {
        return d.format("%Y-%m-%d").to_string();
    }
    trimmed.to_owned()
}

/// Estimate article reading difficulty on a 1-5 scale for German text.
/// Simple readability heuristic tuned for this app's article previews.
pub fn estimate_difficulty(body_text: &str) -> i64 {
    let words: Vec<&str> = body_text.split_whitespace().collect();
    if words.len() < 20 {
        return 3;
    }
    let sentence_count = body_text
        .chars()
        .zip(body_text.chars().skip(1).chain(std::iter::once(' ')))
        .filter(|(c, n)| matches!(c, '.' | '!' | '?') && (n.is_whitespace() || *n == '"'))
        .count()
        .max(1);
    let avg_sentence_len = words.len() as f64 / sentence_count as f64;
    let avg_word_len =
        words.iter().map(|w| w.chars().count()).sum::<usize>() as f64 / words.len() as f64;
    let long_word_ratio =
        words.iter().filter(|w| w.chars().count() >= 10).count() as f64 / words.len() as f64;
    let s = ((avg_sentence_len - 8.0) / 20.0).clamp(0.0, 1.0);
    let w = ((avg_word_len - 4.0) / 4.0).clamp(0.0, 1.0);
    let l = (long_word_ratio / 0.25).clamp(0.0, 1.0);
    let combined = s * 0.4 + w * 0.3 + l * 0.3;
    let level = (combined * 4.0 + 1.0).round() as i64;
    level.clamp(1, 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_title_filters_short_strings() {
        assert!(!looks_like_article_title("Hi"));
        assert!(looks_like_article_title("Wie die Kommunen vorankommen"));
    }

    #[test]
    fn is_landing_url_true_for_short_slug() {
        assert!(is_landing_url(
            "https://www.deutschlandfunk.de/hintergrund-100.html"
        ));
        assert!(is_landing_url(
            "https://www.deutschlandfunk.de/sport-100.html"
        ));
    }

    #[test]
    fn is_landing_url_false_for_full_article() {
        assert!(!is_landing_url(
            "https://www.deutschlandfunk.de/klimaschonend-heizen-wie-die-kommunen-bei-der-waermeplanung-vorankommen-100.html"
        ));
    }

    #[test]
    fn normalize_date_iso_passthrough() {
        assert_eq!(normalize_date("2026-04-27"), "2026-04-27");
    }

    #[test]
    fn normalize_date_german_format() {
        assert_eq!(normalize_date("27.04.2026"), "2026-04-27");
    }

    #[test]
    fn extract_audio_returns_empty_when_no_audio_typename() {
        let scripts: Vec<Value> = vec![serde_json::json!({
            "value": {"__typename": "Teaser", "title": "X"}
        })];
        assert!(extract_audio(&scripts).is_empty());
    }

    #[test]
    fn extract_audio_pulls_fields() {
        let scripts: Vec<Value> = vec![serde_json::json!({
            "value": {
                "__typename": "Audio",
                "audioUrl": "https://ondemand-mp3.dradio.de/x.mp3",
                "downloadUrl": "https://download.deutschlandfunk.de/x.mp3",
                "audioUrlPodcast": "https://podcast-mp3.dradio.de/x.mp3",
                "fileSize": 18201056,
                "duration": 1137,
                "authorText": "Manuel Waltz",
                "audioKicker": "Hintergrund",
                "audioLeader": "Lede ...",
                "sophoraId": "klima-100",
            }
        })];
        let info = extract_audio(&scripts);
        assert_eq!(
            info.audio_url.as_deref(),
            Some("https://ondemand-mp3.dradio.de/x.mp3")
        );
        assert_eq!(info.duration_seconds, Some(1137));
        assert_eq!(info.author_text.as_deref(), Some("Manuel Waltz"));
    }

    #[test]
    fn best_download_url_prefers_download_over_streaming() {
        let mut info = AudioInfo::default();
        info.audio_url = Some("https://ondemand-mp3.dradio.de/x.mp3".to_owned());
        info.download_url = Some("https://download.deutschlandfunk.de/x.mp3".to_owned());
        assert_eq!(
            info.best_download_url(),
            Some("https://download.deutschlandfunk.de/x.mp3")
        );
    }
}
