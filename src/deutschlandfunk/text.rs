use regex::Regex;
use scraper::ElementRef;
use std::sync::LazyLock;

static RE_PUNCTUATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+([,;:.!?)])").unwrap());
static RE_OPENING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([(\[])\s+").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub(super) fn collect_text(node: ElementRef<'_>) -> String {
    node.text().collect::<Vec<_>>().join(" ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn collect_text_separates_adjacent_nodes() {
        let html = Html::parse_fragment(
            r#"<a><span>Schwarz-rote Koalition</span><span>Merz mahnt SPD</span></a>"#,
        );
        let selector = Selector::parse("a").unwrap();
        let link = html.select(&selector).next().unwrap();

        assert_eq!(
            clean_whitespace(&collect_text(link)),
            "Schwarz-rote Koalition Merz mahnt SPD"
        );
    }

    #[test]
    fn collect_text_preserves_sentence_boundary_between_nodes() {
        let html =
            Html::parse_fragment(r#"<a><span>Ein Jahr Leo XIV.</span><span>Der Papst</span></a>"#);
        let selector = Selector::parse("a").unwrap();
        let link = html.select(&selector).next().unwrap();

        assert_eq!(
            clean_whitespace(&collect_text(link)),
            "Ein Jahr Leo XIV. Der Papst"
        );
    }
}
