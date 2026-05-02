#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub id: &'static str,
    pub label: &'static str,
    pub url: &'static str,
}

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
    pub(super) fn record_source_visit(&mut self, source_kind: DiscoverySourceKind) {
        self.source_pages_visited += 1;
        match source_kind {
            DiscoverySourceKind::Section => self.section_pages_visited += 1,
            DiscoverySourceKind::Subsection => self.subsection_pages_visited += 1,
            DiscoverySourceKind::Topic => self.topic_pages_visited += 1,
            DiscoverySourceKind::Search => {}
        }
    }

    pub(super) fn record_article(&mut self, source_kind: DiscoverySourceKind) {
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

    /// Pick the URL most suitable for downloading the MP3. Prefer the
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
