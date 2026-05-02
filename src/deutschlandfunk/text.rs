use regex::Regex;
use scraper::ElementRef;
use std::sync::LazyLock;

static RE_PUNCTUATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([,;:.!?)])").unwrap());
static RE_OPENING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([(\[])\s+").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub(super) fn collect_text(node: ElementRef<'_>) -> String {
    node.text().collect::<String>()
}

pub(super) fn clean_whitespace(input: &str) -> String {
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

pub(super) fn strip_markup(input: &str) -> String {
    clean_whitespace(&RE_TAG.replace_all(input, " "))
}

pub(super) fn trim_chars(input: &str, max: usize) -> String {
    input.chars().take(max).collect()
}
