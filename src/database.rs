use crate::{deutschlandfunk::Article, ids::ArticleId};
use anyhow::{Context, Result};
use log::{debug, info};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

static IN_MEMORY_DB_COUNTER: AtomicU64 = AtomicU64::new(1);

// SQL column list constants. Centralized here so SELECT and INSERT columns stay
// in sync across queries.

/// Columns inserted when saving an article (excludes auto-generated id, uploaded fields).
const INSERT_COLS: &str = concat!(
    "url, title, subtitle, author, date, section, clean_text, word_count, difficulty, ",
    "fetched_at, paywalled, audio_url, audio_download_url, audio_duration_seconds, ",
    "audio_size_bytes, audio_kicker, sophora_id",
);

/// ON CONFLICT UPDATE clause shared by save_article and save_articles_batch.
/// Note: downloaded audio state is intentionally not overwritten by empty
/// refetch metadata, so a temporary parser/API miss does not detach existing
/// audio from the article.
const UPSERT_SET: &str = r#"
    title = excluded.title,
    subtitle = excluded.subtitle,
    author = excluded.author,
    date = excluded.date,
    section = excluded.section,
    clean_text = excluded.clean_text,
    word_count = excluded.word_count,
    difficulty = excluded.difficulty,
    fetched_at = excluded.fetched_at,
    paywalled = excluded.paywalled,
    audio_url = CASE
        WHEN excluded.audio_url <> '' THEN excluded.audio_url
        ELSE audio_url
    END,
    audio_download_url = CASE
        WHEN excluded.audio_download_url <> '' THEN excluded.audio_download_url
        ELSE audio_download_url
    END,
    audio_duration_seconds = CASE
        WHEN excluded.audio_duration_seconds <> 0 THEN excluded.audio_duration_seconds
        ELSE audio_duration_seconds
    END,
    audio_size_bytes = CASE
        WHEN excluded.audio_size_bytes <> 0 THEN excluded.audio_size_bytes
        ELSE audio_size_bytes
    END,
    audio_kicker = CASE
        WHEN excluded.audio_kicker <> '' THEN excluded.audio_kicker
        ELSE audio_kicker
    END,
    sophora_id = CASE
        WHEN excluded.sophora_id <> '' THEN excluded.sophora_id
        ELSE sophora_id
    END
"#;

/// All columns for a full StoredArticle row, unqualified for single-table queries.
const SELECT_ALL_COLS: &str = concat!(
    "id, url, title, subtitle, author, date, section, clean_text, word_count, ",
    "difficulty, fetched_at, uploaded_to_lingq, lingq_lesson_id, lingq_lesson_url, ",
    "paywalled, audio_url, audio_download_url, audio_local_path, ",
    "audio_duration_seconds, audio_size_bytes, audio_kicker, sophora_id, ",
    "transcript_text, transcript_source",
);

/// All columns for a full StoredArticle row, table-qualified for JOIN queries.
const SELECT_ALL_COLS_A: &str = concat!(
    "a.id, a.url, a.title, a.subtitle, a.author, a.date, a.section, a.clean_text, ",
    "a.word_count, a.difficulty, a.fetched_at, a.uploaded_to_lingq, ",
    "a.lingq_lesson_id, a.lingq_lesson_url, a.paywalled, a.audio_url, ",
    "a.audio_download_url, a.audio_local_path, a.audio_duration_seconds, ",
    "a.audio_size_bytes, a.audio_kicker, a.sophora_id, a.transcript_text, ",
    "a.transcript_source",
);

/// Metadata-only columns for StoredArticleMeta (no clean_text).
const SELECT_META_COLS: &str = concat!(
    "id, url, title, subtitle, author, date, section, word_count, difficulty, ",
    "fetched_at, uploaded_to_lingq, lingq_lesson_id, lingq_lesson_url, paywalled, ",
    "audio_url, audio_download_url, audio_local_path, audio_duration_seconds",
);

/// Metadata-only columns, table-qualified for JOIN queries.
const SELECT_META_COLS_A: &str = concat!(
    "a.id, a.url, a.title, a.subtitle, a.author, a.date, a.section, a.word_count, ",
    "a.difficulty, a.fetched_at, a.uploaded_to_lingq, a.lingq_lesson_id, ",
    "a.lingq_lesson_url, a.paywalled, a.audio_url, a.audio_download_url, ",
    "a.audio_local_path, a.audio_duration_seconds",
);

#[derive(Debug, Clone)]
pub struct StoredArticle {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub date: String,
    pub section: String,
    pub clean_text: String,
    pub word_count: i64,
    pub difficulty: i64,
    pub fetched_at: String,
    pub uploaded_to_lingq: bool,
    pub lingq_lesson_id: Option<i64>,
    pub lingq_lesson_url: String,
    pub paywalled: bool,
    pub audio_url: String,
    pub audio_download_url: String,
    pub audio_local_path: String,
    pub audio_duration_seconds: i64,
    pub audio_size_bytes: i64,
    pub audio_kicker: String,
    pub sophora_id: String,
    pub transcript_text: String,
    pub transcript_source: String,
}

impl StoredArticle {
    pub fn has_audio(&self) -> bool {
        !self.audio_url.is_empty() || !self.audio_download_url.is_empty()
    }

    pub fn has_transcript(&self) -> bool {
        !self.transcript_text.trim().is_empty()
    }

    /// The text body to upload to LingQ. Prefer the full transcript if one
    /// has been generated, otherwise fall back to clean_text (lede etc.).
    pub fn upload_text(&self) -> &str {
        if self.has_transcript() {
            &self.transcript_text
        } else {
            &self.clean_text
        }
    }
}

/// Lightweight article metadata for list display. Excludes clean_text
/// to avoid loading megabytes of text when only metadata columns are needed.
#[derive(Debug, Clone)]
pub struct StoredArticleMeta {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub date: String,
    pub section: String,
    pub word_count: i64,
    pub difficulty: i64,
    pub fetched_at: String,
    pub uploaded_to_lingq: bool,
    pub lingq_lesson_id: Option<i64>,
    pub lingq_lesson_url: String,
    pub paywalled: bool,
    pub audio_url: String,
    pub audio_download_url: String,
    pub audio_local_path: String,
    pub audio_duration_seconds: i64,
}

impl StoredArticleMeta {
    pub fn has_audio(&self) -> bool {
        !self.audio_url.is_empty() || !self.audio_download_url.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct SectionCount {
    pub section: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct LibraryStats {
    pub total_articles: i64,
    pub uploaded_articles: i64,
    pub average_word_count: i64,
    pub sections: Vec<SectionCount>,
}

#[derive(Debug, Clone)]
pub struct DatabaseHealth {
    pub schema_version: i64,
    pub migration_count: i64,
    pub journal_mode: String,
    pub foreign_keys_enabled: bool,
    pub integrity_check: String,
    pub page_count: i64,
    pub freelist_count: i64,
    pub database_size_bytes: Option<u64>,
    pub latest_backup: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ArticleQuery {
    pub search: Option<String>,
    pub section: Option<String>,
    pub only_not_uploaded: bool,
    pub min_words: Option<i64>,
    pub max_words: Option<i64>,
    pub sort: Option<String>,
    pub limit: usize,
}

pub struct Database {
    /// Write connection used for INSERT, UPDATE, DELETE, and migrations.
    write_conn: Mutex<Connection>,
    /// Read-only connection used for SELECT queries. WAL mode allows
    /// readers to proceed concurrently with a writer.
    read_conn: Mutex<Connection>,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        let db_path = crate::app_data_dir()?.join("deutschlandfunk_lingq_tool.db");
        Self::open(&db_path)
    }

    pub fn open(path: &Path) -> Result<Self> {
        info!("Opening database at {}", path.display());
        let is_memory = path.to_str() == Some(":memory:");
        let memory_uri = is_memory.then(shared_memory_uri);

        // For :memory: databases each open(":memory:") creates an independent DB.
        // Use a unique shared-cache URI so the paired read/write connections for
        // one Database instance see the same data without colliding with other
        // in-memory databases running in parallel tests.
        let write_conn = if is_memory {
            Connection::open(memory_uri.as_deref().expect("memory URI available"))
                .context("failed to open shared in-memory database")?
        } else {
            Connection::open(path)
                .with_context(|| format!("failed to open database {}", path.display()))?
        };
        configure_connection(&write_conn)
            .context("failed to configure write database connection")?;

        // WAL mode allows concurrent readers + one writer without blocking.
        if !is_memory {
            write_conn
                .pragma_update(None, "journal_mode", "WAL")
                .context("failed to enable WAL mode")?;
        }

        let read_conn = if is_memory {
            Connection::open(memory_uri.as_deref().expect("memory URI available"))
                .context("failed to open shared in-memory read connection")?
        } else {
            Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
                    | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .with_context(|| format!("failed to open read-only database {}", path.display()))?
        };
        configure_connection(&read_conn).context("failed to configure read database connection")?;

        let database = Self {
            write_conn: Mutex::new(write_conn),
            read_conn: Mutex::new(read_conn),
        };
        database.migrate()?;
        Ok(database)
    }

    /// Acquire the write connection.
    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.write_conn.lock().map_err(|_| {
            anyhow::anyhow!("database write mutex poisoned; a background thread likely panicked")
        })
    }

    /// Acquire the read-only connection (does not block writers).
    fn read(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.read_conn.lock().map_err(|_| {
            anyhow::anyhow!("database read mutex poisoned; a background thread likely panicked")
        })
    }

    pub fn save_article(&self, article: &Article) -> Result<i64> {
        debug!("Saving article: {} ({})", article.title, article.url);
        let conn = self.conn()?;
        let sql = format!(
            "INSERT INTO articles ({INSERT_COLS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(url) DO UPDATE SET {UPSERT_SET}
             RETURNING id"
        );
        let id: i64 = conn.query_row(
            &sql,
            params![
                article.url,
                article.title,
                article.subtitle,
                article.author,
                article.date,
                article.section,
                article.clean_text,
                article.word_count as i64,
                article.difficulty,
                article.fetched_at,
                article.paywalled as i64,
                article.audio.audio_url.clone().unwrap_or_default(),
                article.audio.download_url.clone().unwrap_or_default(),
                article.audio.duration_seconds.unwrap_or(0),
                article.audio.file_size_bytes.unwrap_or(0),
                article.audio.kicker.clone().unwrap_or_default(),
                article.audio.sophora_id.clone().unwrap_or_default(),
            ],
            |row| row.get(0),
        )?;
        sync_audio_asset(&conn, id, article)?;

        Ok(id)
    }

    /// Set the local file path for an article's downloaded audio. Only the
    /// path is updated; other audio metadata is left untouched.
    pub fn set_audio_local_path(&self, id: i64, path: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE articles SET audio_local_path = ?1 WHERE id = ?2",
            params![path, id],
        )?;
        conn.execute(
            r#"
            INSERT INTO audio_assets (article_id, local_path, updated_at)
            VALUES (?1, ?2, CURRENT_TIMESTAMP)
            ON CONFLICT(article_id) DO UPDATE SET
                local_path = excluded.local_path,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![id, path],
        )?;
        Ok(())
    }

    /// Store a transcript for an article. `source` is a freeform tag like
    /// "whisper-large-v3" or "manual". An empty `text` clears the column.
    pub fn set_transcript(&self, id: i64, text: &str, source: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE articles SET transcript_text = ?1, transcript_source = ?2 WHERE id = ?3",
            params![text, source, id],
        )?;
        if text.trim().is_empty() {
            conn.execute("DELETE FROM transcripts WHERE article_id = ?1", params![id])?;
        } else {
            conn.execute(
                r#"
                INSERT INTO transcripts (article_id, transcript_text, transcript_source)
                VALUES (?1, ?2, ?3)
                "#,
                params![id, text, source],
            )?;
        }
        Ok(())
    }

    /// Save multiple articles in a single transaction for better performance.
    /// Returns the number of articles successfully saved.
    pub fn save_articles_batch(&self, articles: &[Article]) -> Result<usize> {
        let conn = self.conn()?;
        conn.execute_batch("BEGIN")?;
        let sql = format!(
            "INSERT INTO articles ({INSERT_COLS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(url) DO UPDATE SET {UPSERT_SET}
             RETURNING id"
        );
        let mut saved = 0;
        for article in articles {
            debug!("Batch saving: {} ({})", article.title, article.url);
            match conn.query_row(
                &sql,
                params![
                    article.url,
                    article.title,
                    article.subtitle,
                    article.author,
                    article.date,
                    article.section,
                    article.clean_text,
                    article.word_count as i64,
                    article.difficulty,
                    article.fetched_at,
                    article.paywalled as i64,
                    article.audio.audio_url.clone().unwrap_or_default(),
                    article.audio.download_url.clone().unwrap_or_default(),
                    article.audio.duration_seconds.unwrap_or(0),
                    article.audio.file_size_bytes.unwrap_or(0),
                    article.audio.kicker.clone().unwrap_or_default(),
                    article.audio.sophora_id.clone().unwrap_or_default(),
                ],
                |row| row.get::<_, i64>(0),
            ) {
                Ok(article_id) => {
                    saved += 1;
                    if let Err(err) = sync_audio_asset(&conn, article_id, article) {
                        log::warn!("Audio asset sync failed for {}: {err:#}", article.url);
                    }
                }
                Err(err) => log::warn!("Batch save failed for {}: {err:#}", article.url),
            }
        }
        if let Err(err) = conn.execute_batch("COMMIT") {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(err.into());
        }
        Ok(saved)
    }

    pub fn list_articles(&self, query: &ArticleQuery) -> Result<Vec<StoredArticle>> {
        let order_clause = match query.sort.as_deref() {
            Some("oldest") => "date ASC, id ASC",
            Some("longest") => "word_count DESC",
            Some("shortest") => "word_count ASC",
            Some("title") => "title ASC",
            _ => "date DESC, id DESC",
        };

        // Use FTS5 MATCH when search term is provided; fall back to LIKE for
        // terms that contain FTS special characters that might trip up the parser.
        let fts_term = query.search.as_deref().map(sanitize_fts_query);
        let use_fts = fts_term.as_ref().is_some_and(|t| !t.is_empty());

        let sql = if use_fts {
            format!(
                "SELECT {SELECT_ALL_COLS_A}
                FROM articles a
                INNER JOIN articles_fts ON articles_fts.rowid = a.id
                WHERE articles_fts MATCH ?1
                  AND (?2 IS NULL OR a.section = ?2)
                  AND (?3 = 0 OR a.uploaded_to_lingq = 0)
                  AND (?4 IS NULL OR a.word_count >= ?4)
                  AND (?5 IS NULL OR a.word_count <= ?5)
                ORDER BY {order_clause}
                LIMIT ?6"
            )
        } else {
            format!(
                "SELECT {SELECT_ALL_COLS}
                FROM articles
                WHERE (?1 IS NULL OR title LIKE '%' || ?1 || '%' OR clean_text LIKE '%' || ?1 || '%')
                  AND (?2 IS NULL OR section = ?2)
                  AND (?3 = 0 OR uploaded_to_lingq = 0)
                  AND (?4 IS NULL OR word_count >= ?4)
                  AND (?5 IS NULL OR word_count <= ?5)
                ORDER BY {order_clause}
                LIMIT ?6"
            )
        };

        let search_param: Option<String> = if use_fts {
            fts_term
        } else {
            query.search.clone()
        };

        let conn = self.read()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                search_param.as_deref(),
                query.section.as_deref(),
                if query.only_not_uploaded { 1 } else { 0 },
                query.min_words,
                query.max_words,
                query.limit as i64,
            ],
            map_article_row,
        )?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// List articles returning only metadata (no clean_text) for list display.
    pub fn list_articles_meta(&self, query: &ArticleQuery) -> Result<Vec<StoredArticleMeta>> {
        let order_clause = match query.sort.as_deref() {
            Some("oldest") => "date ASC, id ASC",
            Some("longest") => "word_count DESC",
            Some("shortest") => "word_count ASC",
            Some("title") => "title ASC",
            _ => "date DESC, id DESC",
        };

        let fts_term = query.search.as_deref().map(sanitize_fts_query);
        let use_fts = fts_term.as_ref().is_some_and(|t| !t.is_empty());

        let sql = if use_fts {
            format!(
                "SELECT {SELECT_META_COLS_A}
                FROM articles a
                INNER JOIN articles_fts ON articles_fts.rowid = a.id
                WHERE articles_fts MATCH ?1
                  AND (?2 IS NULL OR a.section = ?2)
                  AND (?3 = 0 OR a.uploaded_to_lingq = 0)
                  AND (?4 IS NULL OR a.word_count >= ?4)
                  AND (?5 IS NULL OR a.word_count <= ?5)
                ORDER BY {order_clause}
                LIMIT ?6"
            )
        } else {
            format!(
                "SELECT {SELECT_META_COLS}
                FROM articles
                WHERE (?1 IS NULL OR title LIKE '%' || ?1 || '%' OR clean_text LIKE '%' || ?1 || '%')
                  AND (?2 IS NULL OR section = ?2)
                  AND (?3 = 0 OR uploaded_to_lingq = 0)
                  AND (?4 IS NULL OR word_count >= ?4)
                  AND (?5 IS NULL OR word_count <= ?5)
                ORDER BY {order_clause}
                LIMIT ?6"
            )
        };

        let search_param: Option<String> = if use_fts {
            fts_term
        } else {
            query.search.clone()
        };

        let conn = self.read()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![
                search_param.as_deref(),
                query.section.as_deref(),
                if query.only_not_uploaded { 1 } else { 0 },
                query.min_words,
                query.max_words,
                query.limit as i64,
            ],
            map_article_meta_row,
        )?;

        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get_article(&self, id: i64) -> Result<Option<StoredArticle>> {
        self.read()?
            .query_row(
                &format!("SELECT {SELECT_ALL_COLS} FROM articles WHERE id = ?1"),
                params![id],
                map_article_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_article_by_id(&self, id: ArticleId) -> Result<Option<StoredArticle>> {
        self.get_article(id.get())
    }

    pub fn get_all_article_urls(&self) -> Result<HashSet<String>> {
        let conn = self.read()?;
        let mut stmt = conn.prepare("SELECT url FROM articles")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let urls = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(urls.into_iter().collect())
    }

    pub fn mark_uploaded(&self, id: i64, lesson_id: i64, lesson_url: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            concat!(
                "UPDATE articles SET uploaded_to_lingq = 1, lingq_lesson_id = ?1, ",
                "lingq_lesson_url = ?2, lingq_upload_status = 'succeeded', ",
                "lingq_upload_error = '', ",
                "lingq_upload_attempted_at = CURRENT_TIMESTAMP WHERE id = ?3",
            ),
            params![lesson_id, lesson_url, id],
        )?;
        conn.execute(
            r#"
            INSERT INTO lingq_uploads (article_id, lesson_id, lesson_url)
            VALUES (?1, ?2, ?3)
            "#,
            params![id, lesson_id, lesson_url],
        )?;
        conn.execute(
            r#"
            INSERT INTO sync_events (event_kind, article_id, status, message)
            VALUES ('lingq_upload', ?1, 'succeeded', ?2)
            "#,
            params![id, format!("lesson_id={lesson_id}")],
        )?;
        Ok(())
    }

    pub fn mark_uploaded_by_id(
        &self,
        id: ArticleId,
        lesson_id: i64,
        lesson_url: &str,
    ) -> Result<()> {
        self.mark_uploaded(id.get(), lesson_id, lesson_url)
    }

    pub fn set_upload_status(
        &self,
        id: ArticleId,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "UPDATE articles SET lingq_upload_status = ?1, lingq_upload_error = ?2, lingq_upload_attempted_at = CURRENT_TIMESTAMP WHERE id = ?3",
            params![status, error.unwrap_or_default(), id.get()],
        )?;
        conn.execute(
            r#"
            INSERT INTO sync_events (event_kind, article_id, status, message)
            VALUES ('lingq_upload', ?1, ?2, ?3)
            "#,
            params![id.get(), status, error.unwrap_or_default()],
        )?;
        Ok(())
    }

    pub fn delete_article(&self, id: i64) -> Result<()> {
        // Capture the audio path before deleting the row so the local MP3 can
        // be removed with the article record.
        let audio_path: Option<String> = self
            .read()?
            .query_row(
                "SELECT audio_local_path FROM articles WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;

        let conn = self.conn()?;
        conn.execute("DELETE FROM articles WHERE id = ?1", params![id])?;
        if conn.changes() == 0 {
            log::warn!("delete_article: no article found with id {id}");
        }
        drop(conn);

        // Remove the local MP3 if one was tracked. A filesystem error should
        // not roll back the database deletion.
        if let Some(path) = audio_path.filter(|p| !p.trim().is_empty()) {
            match std::fs::remove_file(&path) {
                Ok(()) => log::info!("removed orphan audio file {path}"),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => log::warn!("could not remove audio file {path}: {err}"),
            }
        }
        Ok(())
    }

    pub fn backup_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create backup directory {}", parent.display())
            })?;
        }
        self.conn()?
            .execute("VACUUM INTO ?1", params![path.to_string_lossy().as_ref()])?;
        Ok(())
    }

    /// Export all articles as CSV text.
    pub fn export_csv(&self) -> Result<String> {
        let conn = self.read()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_ALL_COLS} FROM articles ORDER BY date DESC, id DESC"
        ))?;
        let articles = stmt
            .query_map([], map_article_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut csv = String::from(
            "id,url,title,subtitle,author,date,section,word_count,difficulty,fetched_at,uploaded_to_lingq,lingq_lesson_id,lingq_lesson_url\n",
        );
        for a in &articles {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                a.id,
                csv_escape(&a.url),
                csv_escape(&a.title),
                csv_escape(&a.subtitle),
                csv_escape(&a.author),
                csv_escape(&a.date),
                csv_escape(&a.section),
                a.word_count,
                a.difficulty,
                csv_escape(&a.fetched_at),
                a.uploaded_to_lingq,
                a.lingq_lesson_id
                    .map(|id| id.to_string())
                    .unwrap_or_default(),
                csv_escape(&a.lingq_lesson_url),
            ));
        }
        Ok(csv)
    }

    /// Export all articles as JSON text.
    pub fn export_json(&self) -> Result<String> {
        let conn = self.read()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {SELECT_ALL_COLS} FROM articles ORDER BY date DESC, id DESC"
        ))?;
        let articles = stmt
            .query_map([], map_article_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let entries: Vec<String> = articles
            .iter()
            .map(|a| {
                format!(
                    concat!(
                        r#"  {{"id":{},"url":{},"title":{},"subtitle":{},"author":{},"#,
                        r#""date":{},"section":{},"word_count":{},"difficulty":{},"#,
                        r#""fetched_at":{},"uploaded_to_lingq":{},"lingq_lesson_id":{},"#,
                        r#""lingq_lesson_url":{}}}"#,
                    ),
                    a.id,
                    json_escape(&a.url),
                    json_escape(&a.title),
                    json_escape(&a.subtitle),
                    json_escape(&a.author),
                    json_escape(&a.date),
                    json_escape(&a.section),
                    a.word_count,
                    a.difficulty,
                    json_escape(&a.fetched_at),
                    a.uploaded_to_lingq,
                    a.lingq_lesson_id
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| "null".to_owned()),
                    json_escape(&a.lingq_lesson_url),
                )
            })
            .collect();

        Ok(format!("[\n{}\n]", entries.join(",\n")))
    }

    pub fn get_stats(&self) -> Result<LibraryStats> {
        let conn = self.read()?;
        let total_articles: i64 =
            conn.query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))?;
        let uploaded_articles: i64 = conn.query_row(
            "SELECT COUNT(*) FROM articles WHERE uploaded_to_lingq = 1",
            [],
            |row| row.get(0),
        )?;
        let average_word_count: i64 = conn.query_row(
            "SELECT CAST(COALESCE(ROUND(AVG(word_count)), 0) AS INTEGER) FROM articles",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = conn.prepare(
            "SELECT section, COUNT(*) FROM articles GROUP BY section ORDER BY COUNT(*) DESC, section ASC",
        )?;
        let section_rows = stmt.query_map([], |row| {
            Ok(SectionCount {
                section: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                count: row.get(1)?,
            })
        })?;
        let sections = section_rows.collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(LibraryStats {
            total_articles,
            uploaded_articles,
            average_word_count,
            sections,
        })
    }

    pub fn get_health(
        &self,
        db_path: Option<&Path>,
        backups_dir: Option<&Path>,
    ) -> Result<DatabaseHealth> {
        let conn = self.read()?;
        let schema_version =
            max_version(&conn, "schema_version")?.max(max_version(&conn, "schema_migrations")?);
        let migration_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })?;
        let journal_mode: String =
            conn.query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))?;
        let foreign_keys_enabled: i64 =
            conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        let integrity_check: String =
            conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let freelist_count: i64 = conn.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        let database_size_bytes =
            db_path.and_then(|path| std::fs::metadata(path).ok().map(|meta| meta.len()));
        let latest_backup = backups_dir.and_then(latest_backup_path);

        Ok(DatabaseHealth {
            schema_version,
            migration_count,
            journal_mode,
            foreign_keys_enabled: foreign_keys_enabled != 0,
            integrity_check,
            page_count,
            freelist_count,
            database_size_bytes,
            latest_backup,
        })
    }

    pub fn optimize(&self) -> Result<()> {
        self.conn()?.execute_batch("PRAGMA optimize;")?;
        Ok(())
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn()?;
        let current_version = current_schema_version(&conn)?;

        if current_version < 1 {
            conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS articles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    subtitle TEXT NOT NULL DEFAULT '',
                    author TEXT NOT NULL DEFAULT '',
                    date TEXT NOT NULL DEFAULT '',
                    section TEXT NOT NULL DEFAULT '',
                    body_text TEXT NOT NULL,
                    clean_text TEXT NOT NULL,
                    word_count INTEGER NOT NULL DEFAULT 0,
                    fetched_at TEXT NOT NULL,
                    uploaded_to_lingq INTEGER NOT NULL DEFAULT 0,
                    lingq_lesson_id INTEGER,
                    lingq_lesson_url TEXT NOT NULL DEFAULT ''
                );

                CREATE INDEX IF NOT EXISTS idx_articles_section ON articles(section);
                CREATE INDEX IF NOT EXISTS idx_articles_uploaded ON articles(uploaded_to_lingq);
                CREATE INDEX IF NOT EXISTS idx_articles_word_count ON articles(word_count);

                INSERT INTO schema_version (version) VALUES (1);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 1, "create articles table")?;
        }

        if current_version < 2 {
            conn.execute_batch(
                r#"
                BEGIN;

                CREATE VIRTUAL TABLE IF NOT EXISTS articles_fts USING fts5(
                    title,
                    subtitle,
                    body_text,
                    content='articles',
                    content_rowid='id'
                );

                -- Populate FTS index from existing articles
                INSERT INTO articles_fts(rowid, title, subtitle, body_text)
                    SELECT id, title, subtitle, body_text FROM articles;

                -- Keep FTS in sync on INSERT
                CREATE TRIGGER IF NOT EXISTS articles_ai AFTER INSERT ON articles BEGIN
                    INSERT INTO articles_fts(rowid, title, subtitle, body_text)
                        VALUES (new.id, new.title, new.subtitle, new.body_text);
                END;

                -- Keep FTS in sync on DELETE
                CREATE TRIGGER IF NOT EXISTS articles_ad AFTER DELETE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, body_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.body_text);
                END;

                -- Keep FTS in sync on UPDATE
                CREATE TRIGGER IF NOT EXISTS articles_au AFTER UPDATE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, body_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.body_text);
                    INSERT INTO articles_fts(rowid, title, subtitle, body_text)
                        VALUES (new.id, new.title, new.subtitle, new.body_text);
                END;

                INSERT INTO schema_version (version) VALUES (2);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 2, "add article full-text search")?;
        }

        if current_version < 3 {
            conn.execute_batch(
                r#"
                BEGIN;

                ALTER TABLE articles ADD COLUMN difficulty INTEGER NOT NULL DEFAULT 3;

                -- Backfill difficulty for existing articles using a simple heuristic:
                -- longer articles with longer average words tend to be harder.
                -- This is a rough approximation; re-fetching will compute proper scores.
                UPDATE articles SET difficulty =
                    CASE
                        WHEN word_count < 200 THEN 1
                        WHEN word_count < 400 THEN 2
                        WHEN word_count < 700 THEN 3
                        WHEN word_count < 1200 THEN 4
                        ELSE 5
                    END;

                INSERT INTO schema_version (version) VALUES (3);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 3, "add difficulty column")?;
        }

        if current_version < 4 {
            conn.execute_batch(
                r#"
                BEGIN;

                -- Composite index for the common library filter: uploaded + word_count range
                CREATE INDEX IF NOT EXISTS idx_articles_upload_words
                    ON articles(uploaded_to_lingq, word_count);

                -- Composite index for date-sorted queries filtered by section
                CREATE INDEX IF NOT EXISTS idx_articles_section_date
                    ON articles(section, date DESC);

                INSERT INTO schema_version (version) VALUES (4);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 4, "add library filter indexes")?;
        }

        if current_version < 5 {
            conn.execute_batch(
                r#"
                BEGIN;

                -- Rebuild FTS5 index to include clean_text for better search coverage
                DROP TRIGGER IF EXISTS articles_ai;
                DROP TRIGGER IF EXISTS articles_ad;
                DROP TRIGGER IF EXISTS articles_au;
                DROP TABLE IF EXISTS articles_fts;

                CREATE VIRTUAL TABLE articles_fts USING fts5(
                    title,
                    subtitle,
                    body_text,
                    clean_text,
                    content='articles',
                    content_rowid='id'
                );

                INSERT INTO articles_fts(rowid, title, subtitle, body_text, clean_text)
                    SELECT id, title, subtitle, body_text, clean_text FROM articles;

                CREATE TRIGGER articles_ai AFTER INSERT ON articles BEGIN
                    INSERT INTO articles_fts(rowid, title, subtitle, body_text, clean_text)
                        VALUES (new.id, new.title, new.subtitle, new.body_text, new.clean_text);
                END;

                CREATE TRIGGER articles_ad AFTER DELETE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, body_text, clean_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.body_text, old.clean_text);
                END;

                CREATE TRIGGER articles_au AFTER UPDATE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, body_text, clean_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.body_text, old.clean_text);
                    INSERT INTO articles_fts(rowid, title, subtitle, body_text, clean_text)
                        VALUES (new.id, new.title, new.subtitle, new.body_text, new.clean_text);
                END;

                INSERT INTO schema_version (version) VALUES (5);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 5, "include clean text in full-text search")?;
        }

        if current_version < 6 {
            conn.execute_batch(
                r#"
                BEGIN;

                -- Drop triggers and FTS first (they reference body_text),
                -- then drop the column (SQLite 3.35+).
                DROP TRIGGER IF EXISTS articles_ai;
                DROP TRIGGER IF EXISTS articles_ad;
                DROP TRIGGER IF EXISTS articles_au;
                DROP TABLE IF EXISTS articles_fts;

                ALTER TABLE articles DROP COLUMN body_text;

                CREATE VIRTUAL TABLE articles_fts USING fts5(
                    title,
                    subtitle,
                    clean_text,
                    content='articles',
                    content_rowid='id'
                );

                INSERT INTO articles_fts(rowid, title, subtitle, clean_text)
                    SELECT id, title, subtitle, clean_text FROM articles;

                CREATE TRIGGER articles_ai AFTER INSERT ON articles BEGIN
                    INSERT INTO articles_fts(rowid, title, subtitle, clean_text)
                        VALUES (new.id, new.title, new.subtitle, new.clean_text);
                END;

                CREATE TRIGGER articles_ad AFTER DELETE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, clean_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.clean_text);
                END;

                CREATE TRIGGER articles_au AFTER UPDATE ON articles BEGIN
                    INSERT INTO articles_fts(articles_fts, rowid, title, subtitle, clean_text)
                        VALUES ('delete', old.id, old.title, old.subtitle, old.clean_text);
                    INSERT INTO articles_fts(rowid, title, subtitle, clean_text)
                        VALUES (new.id, new.title, new.subtitle, new.clean_text);
                END;

                INSERT INTO schema_version (version) VALUES (6);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 6, "drop body text column from article storage")?;
        }

        if current_version < 7 {
            conn.execute_batch(
                r#"
                BEGIN;

                ALTER TABLE articles ADD COLUMN paywalled INTEGER NOT NULL DEFAULT 0;

                INSERT INTO schema_version (version) VALUES (7);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 7, "add truncated article flag")?;
        }

        if current_version < 8 {
            // Audio metadata for Deutschlandfunk pieces. All columns are nullable
            // strings or zero-defaulted integers so legacy rows continue to work.
            conn.execute_batch(
                r#"
                BEGIN;

                ALTER TABLE articles ADD COLUMN audio_url TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN audio_download_url TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN audio_local_path TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN audio_duration_seconds INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE articles ADD COLUMN audio_size_bytes INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE articles ADD COLUMN audio_kicker TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN sophora_id TEXT NOT NULL DEFAULT '';

                CREATE INDEX IF NOT EXISTS idx_articles_has_audio
                    ON articles(audio_url) WHERE audio_url <> '';

                INSERT INTO schema_version (version) VALUES (8);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 8, "add audio metadata columns")?;
        }

        if current_version < 9 {
            // Whisper-generated transcripts. transcript_text holds the full
            // transcript (when available); transcript_source is a freeform
            // tag like "whisper-large-v3" or "manual" for provenance.
            conn.execute_batch(
                r#"
                BEGIN;

                ALTER TABLE articles ADD COLUMN transcript_text TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN transcript_source TEXT NOT NULL DEFAULT '';

                INSERT INTO schema_version (version) VALUES (9);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 9, "add transcript columns")?;
        }

        if current_version < 10 {
            conn.execute_batch(
                r#"
                BEGIN;

                CREATE TABLE IF NOT EXISTS audio_assets (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    article_id INTEGER NOT NULL UNIQUE
                        REFERENCES articles(id) ON DELETE CASCADE,
                    audio_url TEXT NOT NULL DEFAULT '',
                    download_url TEXT NOT NULL DEFAULT '',
                    local_path TEXT NOT NULL DEFAULT '',
                    duration_seconds INTEGER NOT NULL DEFAULT 0,
                    size_bytes INTEGER NOT NULL DEFAULT 0,
                    kicker TEXT NOT NULL DEFAULT '',
                    sophora_id TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE IF NOT EXISTS transcripts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    article_id INTEGER NOT NULL
                        REFERENCES articles(id) ON DELETE CASCADE,
                    transcript_text TEXT NOT NULL,
                    transcript_source TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE TABLE IF NOT EXISTS lingq_uploads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    article_id INTEGER NOT NULL
                        REFERENCES articles(id) ON DELETE CASCADE,
                    lesson_id INTEGER NOT NULL,
                    lesson_url TEXT NOT NULL,
                    uploaded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE INDEX IF NOT EXISTS idx_audio_assets_article
                    ON audio_assets(article_id);
                CREATE INDEX IF NOT EXISTS idx_transcripts_article
                    ON transcripts(article_id, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_lingq_uploads_article
                    ON lingq_uploads(article_id, uploaded_at DESC);

                INSERT OR IGNORE INTO audio_assets (
                    article_id, audio_url, download_url, local_path,
                    duration_seconds, size_bytes, kicker, sophora_id
                )
                SELECT id, audio_url, audio_download_url, audio_local_path,
                       audio_duration_seconds, audio_size_bytes, audio_kicker, sophora_id
                FROM articles
                WHERE audio_url <> ''
                   OR audio_download_url <> ''
                   OR audio_local_path <> '';

                INSERT INTO transcripts (article_id, transcript_text, transcript_source)
                SELECT id, transcript_text, transcript_source
                FROM articles
                WHERE transcript_text <> '';

                INSERT INTO lingq_uploads (article_id, lesson_id, lesson_url)
                SELECT id, lingq_lesson_id, lingq_lesson_url
                FROM articles
                WHERE uploaded_to_lingq = 1 AND lingq_lesson_id IS NOT NULL;

                INSERT INTO schema_version (version) VALUES (10);

                COMMIT;
                "#,
            )?;
            record_migration(&conn, 10, "add related audio transcript and upload tables")?;
        }

        if current_version < 11 {
            conn.execute_batch(
                r#"
                BEGIN;

                ALTER TABLE articles ADD COLUMN lingq_upload_status TEXT NOT NULL DEFAULT 'idle';
                ALTER TABLE articles ADD COLUMN lingq_upload_error TEXT NOT NULL DEFAULT '';
                ALTER TABLE articles ADD COLUMN lingq_upload_attempted_at TEXT NOT NULL DEFAULT '';

                UPDATE articles
                   SET lingq_upload_status = CASE
                       WHEN uploaded_to_lingq = 1 THEN 'succeeded'
                       ELSE 'idle'
                   END;

                CREATE TABLE IF NOT EXISTS sync_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_kind TEXT NOT NULL,
                    article_id INTEGER REFERENCES articles(id) ON DELETE SET NULL,
                    status TEXT NOT NULL,
                    message TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );

                CREATE INDEX IF NOT EXISTS idx_articles_lingq_lesson
                    ON articles(lingq_lesson_id)
                    WHERE lingq_lesson_id IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_articles_audio_local_path
                    ON articles(audio_local_path)
                    WHERE audio_local_path <> '';
                CREATE INDEX IF NOT EXISTS idx_articles_date
                    ON articles(date DESC, id DESC);
                CREATE INDEX IF NOT EXISTS idx_articles_upload_status
                    ON articles(lingq_upload_status, lingq_upload_attempted_at DESC);
                CREATE INDEX IF NOT EXISTS idx_articles_section_uploaded_words
                    ON articles(section, uploaded_to_lingq, word_count);
                CREATE INDEX IF NOT EXISTS idx_sync_events_article
                    ON sync_events(article_id, created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_sync_events_kind_status
                    ON sync_events(event_kind, status, created_at DESC);

                INSERT INTO sync_events (event_kind, article_id, status, message)
                SELECT 'lingq_upload', id, 'succeeded', 'backfilled from uploaded article state'
                  FROM articles
                 WHERE uploaded_to_lingq = 1;

                INSERT INTO schema_version (version) VALUES (11);

                COMMIT;
                ANALYZE;
                PRAGMA optimize;
                "#,
            )?;
            record_migration(
                &conn,
                11,
                "add upload status sync events and diagnostics indexes",
            )?;
        }

        Ok(())
    }
}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_secs(5))
        .context("failed to set SQLite busy timeout")?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .context("failed to enable SQLite foreign keys")?;
    Ok(())
}

fn current_schema_version(conn: &Connection) -> Result<i64> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )?;

    let legacy_version = max_version(conn, "schema_version")?;
    let ledger_version = max_version(conn, "schema_migrations")?;

    if ledger_version < legacy_version {
        let mut stmt = conn.prepare(
            "INSERT OR IGNORE INTO schema_migrations (version, name)
             VALUES (?1, ?2)",
        )?;
        for version in (ledger_version + 1)..=legacy_version {
            stmt.execute(params![version, format!("legacy migration {version}")])?;
        }
    }

    Ok(legacy_version.max(ledger_version))
}

fn max_version(conn: &Connection, table: &str) -> Result<i64> {
    let sql = format!("SELECT COALESCE(MAX(version), 0) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get(0)).unwrap_or(0))
}

fn record_migration(conn: &Connection, version: i64, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name) VALUES (?1, ?2)",
        params![version, name],
    )?;
    Ok(())
}

fn latest_backup_path(backups_dir: &Path) -> Option<String> {
    std::fs::read_dir(backups_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let is_db = path.extension().is_some_and(|extension| extension == "db");
            if !is_db {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path.display().to_string())
}

fn sync_audio_asset(conn: &Connection, article_id: i64, article: &Article) -> Result<()> {
    if article.audio.is_empty() {
        return Ok(());
    }
    conn.execute(
        r#"
        INSERT INTO audio_assets (
            article_id, audio_url, download_url, duration_seconds,
            size_bytes, kicker, sophora_id, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, CURRENT_TIMESTAMP)
        ON CONFLICT(article_id) DO UPDATE SET
            audio_url = excluded.audio_url,
            download_url = excluded.download_url,
            duration_seconds = excluded.duration_seconds,
            size_bytes = excluded.size_bytes,
            kicker = excluded.kicker,
            sophora_id = excluded.sophora_id,
            updated_at = CURRENT_TIMESTAMP
        "#,
        params![
            article_id,
            article.audio.audio_url.clone().unwrap_or_default(),
            article.audio.download_url.clone().unwrap_or_default(),
            article.audio.duration_seconds.unwrap_or(0),
            article.audio.file_size_bytes.unwrap_or(0),
            article.audio.kicker.clone().unwrap_or_default(),
            article.audio.sophora_id.clone().unwrap_or_default(),
        ],
    )?;
    Ok(())
}

fn map_article_meta_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredArticleMeta> {
    Ok(StoredArticleMeta {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        subtitle: row.get(3)?,
        author: row.get(4)?,
        date: row.get(5)?,
        section: row.get(6)?,
        word_count: row.get(7)?,
        difficulty: row.get(8)?,
        fetched_at: row.get(9)?,
        uploaded_to_lingq: row.get::<_, i64>(10)? != 0,
        lingq_lesson_id: row.get(11)?,
        lingq_lesson_url: row.get(12)?,
        paywalled: row.get::<_, i64>(13)? != 0,
        audio_url: row.get(14)?,
        audio_download_url: row.get(15)?,
        audio_local_path: row.get(16)?,
        audio_duration_seconds: row.get(17)?,
    })
}

fn map_article_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredArticle> {
    Ok(StoredArticle {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        subtitle: row.get(3)?,
        author: row.get(4)?,
        date: row.get(5)?,
        section: row.get(6)?,
        clean_text: row.get(7)?,
        word_count: row.get(8)?,
        difficulty: row.get(9)?,
        fetched_at: row.get(10)?,
        uploaded_to_lingq: row.get::<_, i64>(11)? != 0,
        lingq_lesson_id: row.get(12)?,
        lingq_lesson_url: row.get(13)?,
        paywalled: row.get::<_, i64>(14)? != 0,
        audio_url: row.get(15)?,
        audio_download_url: row.get(16)?,
        audio_local_path: row.get(17)?,
        audio_duration_seconds: row.get(18)?,
        audio_size_bytes: row.get(19)?,
        audio_kicker: row.get(20)?,
        sophora_id: row.get(21)?,
        transcript_text: row.get(22)?,
        transcript_source: row.get(23)?,
    })
}

fn shared_memory_uri() -> String {
    let id = IN_MEMORY_DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("file:dlf_mem_{id}?mode=memory&cache=shared")
}

/// Sanitize user input for FTS5 MATCH queries.
/// Strips FTS5 operators and wraps each word with `"..."` to treat them as literals,
/// joined with implicit AND. Returns empty string if nothing usable remains.
fn sanitize_fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            // Strip FTS5 special chars: " * ^ ( ) { } : + -
            let clean: String = word
                .chars()
                .filter(|ch| !matches!(ch, '"' | '*' | '^' | '(' | ')' | '{' | '}' | ':' | '+'))
                .collect();
            clean.trim_matches('-').to_owned()
        })
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escape a field for CSV: wrap in quotes if it contains commas, quotes, or newlines.
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Escape a string as a JSON string literal (with surrounding quotes).
fn json_escape(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_fts_plain_words() {
        assert_eq!(sanitize_fts_query("hello world"), r#""hello" "world""#);
    }

    #[test]
    fn sanitize_fts_strips_operators() {
        assert_eq!(
            sanitize_fts_query(r#"hello "world" NOT"#),
            r#""hello" "world" "NOT""#
        );
    }

    #[test]
    fn sanitize_fts_strips_stars_and_parens() {
        assert_eq!(sanitize_fts_query("test* (group)"), r#""test" "group""#);
    }

    #[test]
    fn sanitize_fts_trims_leading_trailing_dashes() {
        assert_eq!(
            sanitize_fts_query("-negated- --double--"),
            r#""negated" "double""#
        );
    }

    #[test]
    fn sanitize_fts_empty_input() {
        assert_eq!(sanitize_fts_query(""), "");
    }

    #[test]
    fn sanitize_fts_only_special_chars() {
        assert_eq!(sanitize_fts_query(r#""*^(){}:+"#), "");
    }

    #[test]
    fn sanitize_fts_preserves_german_chars() {
        assert_eq!(sanitize_fts_query("Über Straße"), r#""Über" "Straße""#);
    }

    #[test]
    fn save_and_retrieve_article() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let article = Article {
            url: "https://taz.de/test/!1234/".to_owned(),
            title: "Test Article".to_owned(),
            subtitle: "A subtitle".to_owned(),
            author: "Author".to_owned(),
            date: "2025-01-15".to_owned(),
            section: "Politik".to_owned(),
            body_text: "Body.".to_owned(),
            clean_text: "Clean text here.".to_owned(),
            word_count: 3,
            difficulty: 2,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        };
        let id = db.save_article(&article).unwrap();
        assert!(id > 0);

        let stored = db.get_article(id).unwrap().unwrap();
        assert_eq!(stored.title, "Test Article");
        assert_eq!(stored.url, "https://taz.de/test/!1234/");
        assert!(!stored.uploaded_to_lingq);
        assert!(!stored.paywalled);
    }

    #[test]
    fn save_article_upsert_returns_same_id() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let article = Article {
            url: "https://taz.de/test/!1234/".to_owned(),
            title: "Original".to_owned(),
            subtitle: String::new(),
            author: String::new(),
            date: String::new(),
            section: String::new(),
            body_text: "Body.".to_owned(),
            clean_text: "Clean.".to_owned(),
            word_count: 1,
            difficulty: 3,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        };
        let id1 = db.save_article(&article).unwrap();

        let mut updated = article.clone();
        updated.title = "Updated".to_owned();
        let id2 = db.save_article(&updated).unwrap();
        assert_eq!(id1, id2);

        let stored = db.get_article(id1).unwrap().unwrap();
        assert_eq!(stored.title, "Updated");
    }

    #[test]
    fn save_article_upsert_preserves_audio_when_refetch_loses_audio_metadata() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let url =
            "https://www.deutschlandfunk.de/politik/2026/05/29/audio-metadaten-bleiben-100.html";
        let mut article = make_article(url, "Audio-Metadaten bleiben");
        article.audio.audio_url =
            Some("https://ondemand-mp3.dradio.de/audio-metadaten.mp3".to_owned());
        article.audio.download_url =
            Some("https://download.deutschlandfunk.de/audio-metadaten.mp3".to_owned());
        article.audio.duration_seconds = Some(612);
        article.audio.file_size_bytes = Some(8_765_432);
        article.audio.kicker = Some("Hintergrund".to_owned());
        article.audio.sophora_id = Some("audio-metadaten-bleiben-100".to_owned());

        let id = db.save_article(&article).unwrap();
        db.set_audio_local_path(id, "C:\\audio\\audio-metadaten.mp3")
            .unwrap();

        let mut refetched = make_article(url, "Audio-Metadaten bleiben");
        refetched.clean_text =
            "Aktualisierter Artikeltext ohne ausgelieferten Audio-Datenblock.".to_owned();
        let refetched_id = db.save_article(&refetched).unwrap();

        assert_eq!(refetched_id, id);
        let stored = db.get_article(id).unwrap().unwrap();
        assert_eq!(
            stored.clean_text,
            "Aktualisierter Artikeltext ohne ausgelieferten Audio-Datenblock."
        );
        assert_eq!(
            stored.audio_url,
            "https://ondemand-mp3.dradio.de/audio-metadaten.mp3"
        );
        assert_eq!(
            stored.audio_download_url,
            "https://download.deutschlandfunk.de/audio-metadaten.mp3"
        );
        assert_eq!(stored.audio_duration_seconds, 612);
        assert_eq!(stored.audio_size_bytes, 8_765_432);
        assert_eq!(stored.audio_kicker, "Hintergrund");
        assert_eq!(stored.sophora_id, "audio-metadaten-bleiben-100");
        assert_eq!(stored.audio_local_path, "C:\\audio\\audio-metadaten.mp3");

        let audio_asset_count: i64 = db
            .read()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM audio_assets WHERE article_id = ?1 AND download_url <> '' AND local_path <> ''",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(audio_asset_count, 1);
    }

    #[test]
    fn mark_uploaded_and_query() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let article = Article {
            url: "https://taz.de/test/!5678/".to_owned(),
            title: "Upload Test".to_owned(),
            subtitle: String::new(),
            author: String::new(),
            date: String::new(),
            section: "Kultur".to_owned(),
            body_text: "Body.".to_owned(),
            clean_text: "Some clean.".to_owned(),
            word_count: 2,
            difficulty: 3,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        };
        let id = db.save_article(&article).unwrap();
        db.mark_uploaded(id, 999, "https://lingq.com/lesson/999/")
            .unwrap();

        let stored = db.get_article(id).unwrap().unwrap();
        assert!(stored.uploaded_to_lingq);
        assert_eq!(stored.lingq_lesson_id, Some(999));
        let success_events: i64 = db
            .read()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM sync_events WHERE article_id = ?1 AND status = 'succeeded'",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(success_events, 1);

        let results = db
            .list_articles(&ArticleQuery {
                only_not_uploaded: true,
                limit: 100,
                ..Default::default()
            })
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn delete_article_removes_it() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let article = Article {
            url: "https://taz.de/test/!9999/".to_owned(),
            title: "Delete Me".to_owned(),
            subtitle: String::new(),
            author: String::new(),
            date: String::new(),
            section: String::new(),
            body_text: "Body.".to_owned(),
            clean_text: "Clean.".to_owned(),
            word_count: 1,
            difficulty: 3,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        };
        let id = db.save_article(&article).unwrap();
        db.delete_article(id).unwrap();
        assert!(db.get_article(id).unwrap().is_none());
    }

    #[test]
    fn stats_reflect_articles() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_articles, 0);

        let article = Article {
            url: "https://taz.de/test/!1111/".to_owned(),
            title: "Stats Test".to_owned(),
            subtitle: String::new(),
            author: String::new(),
            date: String::new(),
            section: "Sport".to_owned(),
            body_text: "Body.".to_owned(),
            clean_text: "One two three four five.".to_owned(),
            word_count: 5,
            difficulty: 2,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        };
        db.save_article(&article).unwrap();

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_articles, 1);
        assert_eq!(stats.uploaded_articles, 0);
        assert_eq!(stats.average_word_count, 5);
        assert_eq!(stats.sections.len(), 1);
        assert_eq!(stats.sections[0].section, "Sport");
    }

    #[test]
    fn migrations_are_recorded_in_schema_migrations() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let conn = db.read().unwrap();
        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(version, 11);
        assert_eq!(count, 11);
    }

    #[test]
    fn health_reports_database_pragmas_and_schema() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let health = db.get_health(None, None).unwrap();

        assert_eq!(health.schema_version, 11);
        assert_eq!(health.migration_count, 11);
        assert!(health.foreign_keys_enabled);
        assert_eq!(health.integrity_check, "ok");
        assert!(health.page_count > 0);
    }

    #[test]
    fn upload_status_writes_sync_event() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let id = db
            .save_article(&make_article("https://taz.de/a/!status/", "Status"))
            .unwrap();
        let article_id = ArticleId::new(id).unwrap();

        db.set_upload_status(article_id, "failed", Some("network"))
            .unwrap();

        let conn = db.read().unwrap();
        let status: String = conn
            .query_row(
                "SELECT lingq_upload_status FROM articles WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_events WHERE article_id = ?1 AND status = 'failed'",
                params![id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(status, "failed");
        assert_eq!(event_count, 1);
    }

    #[test]
    fn upload_text_ignores_whitespace_only_transcript() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let id = db
            .save_article(&make_article(
                "https://www.deutschlandfunk.de/kultur/2026/05/29/leeres-transkript-100.html",
                "Leeres Transkript",
            ))
            .unwrap();
        db.set_transcript(id, " \r\n\t ", "whisper:empty").unwrap();

        let stored = db.get_article(id).unwrap().unwrap();
        assert!(!stored.has_transcript());
        assert_eq!(stored.upload_text(), stored.clean_text);

        let transcript_rows: i64 = db
            .read()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE article_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(transcript_rows, 0);
    }

    #[test]
    fn migrates_legacy_v1_database_to_latest_schema() {
        let path = std::env::temp_dir().join(format!(
            "dlf_legacy_v1_{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE schema_version (version INTEGER NOT NULL);
                CREATE TABLE articles (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    url TEXT NOT NULL UNIQUE,
                    title TEXT NOT NULL,
                    subtitle TEXT NOT NULL DEFAULT '',
                    author TEXT NOT NULL DEFAULT '',
                    date TEXT NOT NULL DEFAULT '',
                    section TEXT NOT NULL DEFAULT '',
                    body_text TEXT NOT NULL,
                    clean_text TEXT NOT NULL,
                    word_count INTEGER NOT NULL DEFAULT 0,
                    fetched_at TEXT NOT NULL,
                    uploaded_to_lingq INTEGER NOT NULL DEFAULT 0,
                    lingq_lesson_id INTEGER,
                    lingq_lesson_url TEXT NOT NULL DEFAULT ''
                );
                INSERT INTO schema_version (version) VALUES (1);
                INSERT INTO articles (
                    url, title, subtitle, author, date, section, body_text,
                    clean_text, word_count, fetched_at
                ) VALUES (
                    'https://example.test/legacy',
                    'Legacy',
                    '',
                    '',
                    '2025-01-01',
                    'Test',
                    'Legacy body text',
                    'Legacy clean text',
                    3,
                    '2025-01-01T00:00:00Z'
                );
                "#,
            )
            .unwrap();
        }

        let db = Database::open(&path).unwrap();
        let health = db.get_health(Some(&path), None).unwrap();
        let rows = db
            .list_articles_meta(&ArticleQuery {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let conn = db.read().unwrap();
        let sync_events_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'sync_events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);
        drop(db);
        let _ = std::fs::remove_file(path);

        assert_eq!(health.schema_version, 11);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Legacy");
        assert_eq!(sync_events_exists, 1);
    }

    #[test]
    fn related_tables_mirror_audio_transcripts_and_uploads() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let mut article = make_article("https://taz.de/a/!related/", "Related Tables");
        article.audio.audio_url = Some("https://ondemand-mp3.dradio.de/related.mp3".to_owned());
        article.audio.download_url =
            Some("https://download.deutschlandfunk.de/related.mp3".to_owned());
        article.audio.duration_seconds = Some(42);

        let id = db.save_article(&article).unwrap();
        db.set_audio_local_path(id, "C:\\audio\\related.mp3")
            .unwrap();
        db.set_transcript(id, "Hallo Welt", "whisper:test").unwrap();
        db.mark_uploaded(id, 123, "https://lingq.com/lesson/123/")
            .unwrap();

        let conn = db.read().unwrap();
        let audio_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audio_assets WHERE article_id = ?1 AND local_path <> ''",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let transcript_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE article_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        let upload_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lingq_uploads WHERE article_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(audio_count, 1);
        assert_eq!(transcript_count, 1);
        assert_eq!(upload_count, 1);
    }

    fn make_article(url: &str, title: &str) -> Article {
        Article {
            url: url.to_owned(),
            title: title.to_owned(),
            subtitle: String::new(),
            author: String::new(),
            date: "2025-01-01".to_owned(),
            section: "Test".to_owned(),
            body_text: String::new(),
            clean_text: "Clean.".to_owned(),
            word_count: 1,
            difficulty: 3,
            fetched_at: "2025-01-15T10:00:00Z".to_owned(),
            paywalled: false,
            audio: crate::deutschlandfunk::AudioInfo::default(),
        }
    }

    #[test]
    fn save_articles_batch_saves_multiple() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let articles = vec![
            make_article("https://taz.de/a/!1/", "First"),
            make_article("https://taz.de/a/!2/", "Second"),
            make_article("https://taz.de/a/!3/", "Third"),
        ];
        let saved = db.save_articles_batch(&articles).unwrap();
        assert_eq!(saved, 3);

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_articles, 3);
    }

    #[test]
    fn save_articles_batch_handles_duplicates() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let articles = vec![
            make_article("https://taz.de/a/!1/", "First"),
            make_article("https://taz.de/a/!1/", "First Updated"),
        ];
        let saved = db.save_articles_batch(&articles).unwrap();
        assert_eq!(saved, 2); // Both succeed (second is an upsert)

        let stats = db.get_stats().unwrap();
        assert_eq!(stats.total_articles, 1);
    }

    #[test]
    fn save_articles_batch_empty_input() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let saved = db.save_articles_batch(&[]).unwrap();
        assert_eq!(saved, 0);
    }

    #[test]
    fn export_csv_includes_header_and_data() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db.save_article(&make_article("https://taz.de/a/!1/", "Export Test"))
            .unwrap();
        let csv = db.export_csv().unwrap();
        assert!(csv.starts_with("id,url,title,"));
        assert!(csv.contains("Export Test"));
    }

    #[test]
    fn export_json_is_valid_array() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db.save_article(&make_article("https://taz.de/a/!1/", "JSON Test"))
            .unwrap();
        let json = db.export_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = parsed.as_array().unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["title"], "JSON Test");
    }

    #[test]
    fn list_articles_meta_detects_download_only_audio() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let mut article = make_article("https://taz.de/a/!audio/", "Audio Test");
        article.audio.download_url =
            Some("https://download.deutschlandfunk.de/test.mp3".to_owned());
        let id = db.save_article(&article).unwrap();

        let rows = db
            .list_articles_meta(&ArticleQuery {
                limit: 20,
                ..Default::default()
            })
            .unwrap();

        let stored = rows.into_iter().find(|row| row.id == id).unwrap();
        assert!(stored.audio_url.is_empty());
        assert_eq!(
            stored.audio_download_url,
            "https://download.deutschlandfunk.de/test.mp3"
        );
        assert!(stored.has_audio());
    }

    #[test]
    fn csv_escape_wraps_commas() {
        assert_eq!(csv_escape("hello, world"), "\"hello, world\"");
    }

    #[test]
    fn csv_escape_doubles_quotes() {
        assert_eq!(csv_escape(r#"say "hi""#), r#""say ""hi""""#);
    }

    #[test]
    fn json_escape_handles_special_chars() {
        assert_eq!(json_escape("line1\nline2"), r#""line1\nline2""#);
        assert_eq!(json_escape(r#"say "hi""#), r#""say \"hi\"""#);
    }
}
