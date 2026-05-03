pub(super) const STATUS_LOADING: &str = "Loading DLF sections, library, and LingQ status.";
pub(super) const SELECT_ARTICLE_TO_UPLOAD: &str = "Select at least one saved article to upload.";
pub(super) const SAVE_LINGQ_TOKEN_FIRST: &str = "Open LingQ settings and save a token first.";
pub(super) const SELECT_BROWSE_ARTICLE: &str = "Select at least one article first.";

pub(super) fn health_failed(error: &str) -> String {
    format!("Health check failed: {error}")
}

pub(super) fn diagnostics_failed(error: &str) -> String {
    format!("Diagnostics export failed: {error}")
}
