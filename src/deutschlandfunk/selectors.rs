use scraper::Selector;
use std::sync::LazyLock;

pub(super) const TITLE: &[&str] = &["meta[property=\"og:title\"]", "h1", "title"];
pub(super) const SUBTITLE: &[&str] = &["meta[name=\"description\"]"];
pub(super) const AUTHOR_FALLBACK: &[&str] = &["[rel=\"author\"]", ".author", "[class*=\"author\"]"];
pub(super) const SECTION: &[&str] = &["meta[property=\"article:section\"]"];
pub(super) const DATE_TIME: &[&str] = &["time[datetime]"];
pub(super) const DATE_META: &[&str] = &[
    "meta[property=\"article:published_time\"]",
    "meta[name=\"date\"]",
];

/// The visible article container. Deutschlandfunk wraps article content in
/// `<article class="b-article">` (sometimes additional modifier classes).
pub(super) const ARTICLE: &str = "article.b-article, article[class*=\"b-article\"], article";
pub(super) const BODY_ELEMENTS: &str = "p, h2, h3, li";
pub(super) const BODY_FALLBACK: &str = "main p";
pub(super) const LINKS: &str = "a[href]";
pub(super) const HEADLINE: &[&str] = &[".headline", "h1", "h2", "h3", ".b-teaser-headline"];

/// Boilerplate / navigation strings that creep into the body text.
pub(super) const BOILERPLATE_MARKERS: &[&str] = &[
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

pub(super) mod parsed {
    use super::*;

    pub(in crate::deutschlandfunk) static LINKS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::LINKS).unwrap());
    pub(in crate::deutschlandfunk) static ARTICLE: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::ARTICLE).unwrap());
    pub(in crate::deutschlandfunk) static BODY_ELEMENTS: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::BODY_ELEMENTS).unwrap());
    pub(in crate::deutschlandfunk) static BODY_FALLBACK: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse(super::BODY_FALLBACK).unwrap());
    pub(in crate::deutschlandfunk) static HEADLINE: LazyLock<Vec<Selector>> = LazyLock::new(|| {
        super::HEADLINE
            .iter()
            .filter_map(|s| Selector::parse(s).ok())
            .collect()
    });
    pub(in crate::deutschlandfunk) static CLIENT_QUERIES: LazyLock<Selector> =
        LazyLock::new(|| Selector::parse("script.js-client-queries[data-json]").unwrap());
}
