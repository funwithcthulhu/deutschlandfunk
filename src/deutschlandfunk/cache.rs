use std::collections::HashMap;
use std::time::Duration;

const MAX_CACHE_ENTRIES: usize = 32;

#[derive(Clone)]
pub(super) struct Cached<T> {
    stored_at: tokio::time::Instant,
    value: T,
}

pub(super) fn cached_value<T: Clone>(
    cache: &mut HashMap<String, Cached<T>>,
    key: &str,
    ttl: Duration,
) -> Option<T> {
    let now = tokio::time::Instant::now();
    cache.retain(|_, entry| now.duration_since(entry.stored_at) <= ttl);
    cache.get(key).map(|entry| entry.value.clone())
}

pub(super) fn store_cached_value<T: Clone>(
    cache: &mut HashMap<String, Cached<T>>,
    key: String,
    value: T,
) {
    if cache.len() >= MAX_CACHE_ENTRIES
        && let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.stored_at)
            .map(|(key, _)| key.clone())
    {
        cache.remove(&oldest_key);
    }
    cache.insert(
        key,
        Cached {
            stored_at: tokio::time::Instant::now(),
            value,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_stores_and_retrieves_recent_values() {
        let mut cache = HashMap::new();
        store_cached_value(&mut cache, "one".to_owned(), vec![1, 2, 3]);

        assert_eq!(
            cached_value(&mut cache, "one", Duration::from_secs(60)),
            Some(vec![1, 2, 3])
        );
    }
}
