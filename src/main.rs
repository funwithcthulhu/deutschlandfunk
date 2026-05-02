#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use deutschlandfunk_lingq_tool::{
    audio,
    database::{ArticleQuery, Database},
    deutschlandfunk::{ArticleSummary, DeutschlandfunkClient},
    gui,
    lingq::LingqClient,
    services::{
        ingest::download_article_audio,
        transcription::transcribe_saved_article,
        upload::{UploadArticleOptions, upload_article_to_lingq},
    },
};
use log::info;

#[derive(Parser)]
#[command(name = "deutschlandfunk-lingq")]
#[command(
    about = "Fetch deutschlandfunk.de articles (and their audio), store them locally, and upload them to LingQ."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Gui,
    Sections,
    Browse(BrowseArgs),
    BrowseUrl(BrowseUrlArgs),
    Fetch(FetchArgs),
    /// Download just the audio for an article URL.
    Audio(AudioArgs),
    Library(LibraryArgs),
    Upload(UploadArgs),
    /// Print resolved app data dir, audio dir, settings, DB stats, and
    /// LingQ token presence. Useful for debugging.
    Doctor(DoctorArgs),
    /// Create a SQLite-safe backup of the local library database.
    Backup(BackupArgs),
    /// Transcribe an article's local MP3 with whisper.cpp and store the
    /// transcript in the library.
    Transcribe(TranscribeArgs),
}

#[derive(Args)]
struct TranscribeArgs {
    #[arg(long)]
    id: i64,
}

#[derive(Args)]
struct DoctorArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct BackupArgs {
    #[arg(long)]
    output: Option<String>,
}

#[derive(Args)]
struct BrowseArgs {
    #[arg(long, default_value = "nachrichten")]
    section: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args)]
struct BrowseUrlArgs {
    #[arg(long)]
    url: String,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}

#[derive(Args)]
struct FetchArgs {
    #[arg(long)]
    url: String,
    /// Save the article into the local SQLite library.
    #[arg(long)]
    save: bool,
    /// Also download the audio (MP3) when present.
    #[arg(long)]
    with_audio: bool,
    /// Override the audio output directory (defaults to the configured app dir).
    #[arg(long)]
    audio_dir: Option<String>,
}

#[derive(Args)]
struct AudioArgs {
    #[arg(long)]
    url: String,
    #[arg(long)]
    audio_dir: Option<String>,
}

#[derive(Args)]
struct LibraryArgs {
    #[arg(long)]
    search: Option<String>,
    #[arg(long)]
    section: Option<String>,
    #[arg(long)]
    only_not_uploaded: bool,
    #[arg(long, default_value_t = 50)]
    limit: usize,
}

#[derive(Args)]
struct UploadArgs {
    #[arg(long)]
    id: i64,
    #[arg(long)]
    api_key: Option<String>,
    #[arg(long, default_value = "de")]
    language: String,
    #[arg(long)]
    collection: Option<i64>,
    /// Attach the article's downloaded MP3 to the LingQ lesson when present.
    #[arg(long)]
    with_audio: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();
    info!("deutschlandfunk_lingq_tool starting");
    if matches!(cli.command, None | Some(Commands::Gui)) {
        return gui::run().map_err(|err| anyhow!("failed to launch GUI: {err}"));
    }

    let scraper = DeutschlandfunkClient::new()?;

    match cli.command.expect("handled gui/default case above") {
        Commands::Gui => unreachable!(),
        Commands::Sections => {
            for section in scraper.sections() {
                println!("{:<14} {:<28} {}", section.id, section.label, section.url);
            }
        }
        Commands::Browse(args) => {
            let section = scraper
                .section_by_id(&args.section)
                .ok_or_else(|| anyhow!("unknown section '{}'", args.section))?;
            let articles = scraper.browse_section(section, args.limit).await?;
            print_summaries(&articles);
        }
        Commands::BrowseUrl(args) => {
            let articles = scraper.browse_url(&args.url, None, args.limit).await?;
            print_summaries(&articles);
        }
        Commands::Fetch(args) => {
            let article = scraper.fetch_article(&args.url).await?;
            println!("Title: {}", article.title);
            if !article.subtitle.is_empty() {
                println!("Subtitle: {}", article.subtitle);
            }
            if !article.author.is_empty() {
                println!("Author: {}", article.author);
            }
            if !article.date.is_empty() {
                println!("Date: {}", article.date);
            }
            println!("Section: {}", article.section);
            println!("Words: {}", article.word_count);
            if let Some(url) = article.audio.best_download_url() {
                println!(
                    "Audio: {} ({}, {})",
                    url,
                    audio::format_duration(article.audio.duration_seconds.unwrap_or(0)),
                    audio::format_size(article.audio.file_size_bytes.unwrap_or(0)),
                );
            }
            println!();
            println!("{}", article.clean_text);

            let mut local_audio_path: Option<String> = None;
            if args.with_audio {
                match download_article_audio(
                    &scraper,
                    &article,
                    args.audio_dir.as_deref().unwrap_or(""),
                    |downloaded, total| {
                        if total > 0 && downloaded % (1024 * 1024) < 65_536 {
                            let pct = (downloaded as f64 / total as f64) * 100.0;
                            eprintln!("  {pct:>5.1}%  ({downloaded} / {total} bytes)");
                        }
                    },
                )
                .await?
                {
                    Some(download) => {
                        if download.reused_existing {
                            println!("Using existing audio at {}", download.path.display());
                        } else {
                            println!(
                                "Saved {} bytes to {}",
                                download.bytes,
                                download.path.display()
                            );
                        }
                        local_audio_path = Some(download.path.to_string_lossy().into_owned());
                    }
                    None => println!("(no audio available for this article)"),
                }
            }

            if args.save {
                let db = Database::open_default()?;
                let id = db.save_article(&article)?;
                if let Some(path) = local_audio_path.as_deref() {
                    db.set_audio_local_path(id, path)?;
                }
                println!();
                println!("Saved as article #{id}");
            }
        }
        Commands::Audio(args) => {
            let article = scraper.fetch_article(&args.url).await?;
            let Some(download) = download_article_audio(
                &scraper,
                &article,
                args.audio_dir.as_deref().unwrap_or(""),
                |downloaded, total| {
                    if total > 0 && downloaded % (1024 * 1024) < 65_536 {
                        let pct = (downloaded as f64 / total as f64) * 100.0;
                        eprintln!("  {pct:>5.1}%  ({downloaded} / {total} bytes)");
                    }
                },
            )
            .await?
            else {
                anyhow::bail!("no audio attachment found for {}", args.url);
            };
            if download.reused_existing {
                println!("Using existing audio at {}", download.path.display());
            } else {
                println!(
                    "Saved {} bytes to {}",
                    download.bytes,
                    download.path.display()
                );
            }
        }
        Commands::Library(args) => {
            let db = Database::open_default()?;
            let rows = db.list_articles(&ArticleQuery {
                search: args.search.clone(),
                section: args.section.clone(),
                only_not_uploaded: args.only_not_uploaded,
                limit: args.limit,
                ..Default::default()
            })?;

            for row in rows {
                let uploaded = if row.uploaded_to_lingq {
                    "uploaded"
                } else {
                    "local"
                };
                let audio_flag = if row.has_audio() { "♪" } else { " " };
                println!(
                    "#{:<4} {} {:<8} {:<20} {:>5}w {}",
                    row.id, audio_flag, uploaded, row.section, row.word_count, row.title
                );
                println!("      {}", row.url);
                if !row.lingq_lesson_url.is_empty() {
                    println!("      LingQ: {}", row.lingq_lesson_url);
                }
                if !row.audio_local_path.is_empty() {
                    println!("      Audio: {}", row.audio_local_path);
                }
            }
        }
        Commands::Upload(args) => {
            let db = Database::open_default()?;
            let article = db
                .get_article(args.id)?
                .ok_or_else(|| anyhow!("article #{} not found", args.id))?;

            let api_key = resolve_api_key(args.api_key)?;
            let lingq = LingqClient::new()?;
            let upload = upload_article_to_lingq(
                &lingq,
                &db,
                article.id,
                &UploadArticleOptions {
                    api_key,
                    language_code: args.language.clone(),
                    collection_id: args.collection,
                    attach_audio: args.with_audio,
                },
            )
            .await?;

            println!(
                "Uploaded article #{} to LingQ lesson {}",
                upload.article_id, upload.lesson_id
            );
            println!("{}", upload.lesson_url);
        }
        Commands::Doctor(args) => run_doctor(args.json)?,
        Commands::Backup(args) => run_backup(args.output.as_deref())?,
        Commands::Transcribe(args) => {
            use deutschlandfunk_lingq_tool::{settings::SettingsStore, transcribe};
            let db = Database::open_default()?;
            let settings = SettingsStore::load_default()?;
            let cfg = transcribe::WhisperConfig::from_settings(settings.data())?;
            let article = db
                .get_article(args.id)?
                .ok_or_else(|| anyhow!("article #{} not found", args.id))?;
            println!("Transcribing {} ...", article.audio_local_path);
            let outcome = transcribe_saved_article(&db, args.id, &cfg).await?;
            println!(
                "Saved {} chars of transcript ({}).",
                outcome.chars, outcome.source
            );
        }
    }

    Ok(())
}

/// Print a snapshot of the runtime environment to stdout. No network calls.
fn run_doctor(json: bool) -> Result<()> {
    use deutschlandfunk_lingq_tool::{app_data_dir, settings::SettingsStore};
    if json {
        let app_data = app_data_dir().ok();
        let mut settings_probe = deutschlandfunk_lingq_tool::settings::AppSettings::default();
        let token = deutschlandfunk_lingq_tool::settings::load_api_key(&mut settings_probe);
        let stats = Database::open_default().and_then(|db| db.get_stats()).ok();
        let settings = SettingsStore::load_default().ok();
        let report = serde_json::json!({
            "app_data_dir": app_data.as_ref().map(|p| p.display().to_string()),
            "settings": settings.as_ref().map(|store| {
                let s = store.data();
                serde_json::json!({
                    "browse_section": s.browse_section,
                    "lingq_language": s.lingq_language,
                    "lingq_collection_id": s.lingq_collection_id,
                    "audio_dir": if s.audio_dir.trim().is_empty() {
                        app_data.as_ref().map(|p| p.join("audio").display().to_string()).unwrap_or_default()
                    } else {
                        s.audio_dir.clone()
                    },
                    "download_audio_on_fetch": s.download_audio_on_fetch,
                    "upload_audio_to_lingq": s.upload_audio_to_lingq,
                    "whisper_language": s.whisper_language,
                    "whisper_cli_configured": !s.whisper_cli_path.trim().is_empty(),
                    "whisper_model_configured": !s.whisper_model_path.trim().is_empty()
                })
            }),
            "lingq_token_present": !token.trim().is_empty(),
            "lingq_token_length": token.trim().len(),
            "library": stats.as_ref().map(|s| serde_json::json!({
                "total_articles": s.total_articles,
                "uploaded_articles": s.uploaded_articles,
                "average_word_count": s.average_word_count,
                "sections": s.sections.iter().map(|section| serde_json::json!({
                    "section": section.section,
                    "count": section.count
                })).collect::<Vec<_>>()
            }))
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    println!("Deutschlandfunk Reader — doctor report");
    println!("======================================");

    match app_data_dir() {
        Ok(p) => println!("App data dir:     {}", p.display()),
        Err(err) => println!("App data dir:     ERROR ({err:#})"),
    }

    let settings = match SettingsStore::load_default() {
        Ok(s) => Some(s),
        Err(err) => {
            println!("Settings:         ERROR loading ({err:#})");
            None
        }
    };
    if let Some(store) = settings.as_ref() {
        let s = store.data();
        println!("Browse section:   {}", s.browse_section);
        println!("LingQ language:   {}", s.lingq_language);
        println!("LingQ collection: {:?}", s.lingq_collection_id);
        println!(
            "Audio dir:        {}",
            if s.audio_dir.trim().is_empty() {
                match app_data_dir() {
                    Ok(p) => p.join("audio").display().to_string(),
                    Err(_) => "(unavailable)".to_owned(),
                }
            } else {
                s.audio_dir.clone()
            }
        );
        println!("Auto-download MP3 on fetch:  {}", s.download_audio_on_fetch);
        println!("Attach MP3 to LingQ upload:  {}", s.upload_audio_to_lingq);
    }

    // LingQ token presence (length only — never print the token).
    let mut probe = deutschlandfunk_lingq_tool::settings::AppSettings::default();
    let token = deutschlandfunk_lingq_tool::settings::load_api_key(&mut probe);
    let trimmed = token.trim();
    if trimmed.is_empty() {
        println!("LingQ token:      (none — log in via the GUI or set LINGQ_API_KEY)");
    } else {
        println!("LingQ token:      present ({} chars)", trimmed.len());
    }

    // DB stats.
    match Database::open_default() {
        Ok(db) => match db.get_stats() {
            Ok(stats) => {
                println!("\nLibrary database");
                println!("  Total articles:     {}", stats.total_articles);
                println!("  Uploaded to LingQ:  {}", stats.uploaded_articles);
                println!("  Avg word count:     {}", stats.average_word_count);
                println!("  Sections:");
                for sc in stats.sections.iter().take(20) {
                    println!("    {:>4}  {}", sc.count, sc.section);
                }
                if stats.sections.len() > 20 {
                    println!("    … and {} more", stats.sections.len() - 20);
                }
            }
            Err(err) => println!("Library database: ERROR reading stats ({err:#})"),
        },
        Err(err) => println!("Library database: ERROR opening ({err:#})"),
    }

    Ok(())
}

fn run_backup(output: Option<&str>) -> Result<()> {
    let db = Database::open_default()?;
    let path = match output {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            deutschlandfunk_lingq_tool::app_data_dir()?
                .join("backups")
                .join(format!("deutschlandfunk_lingq_tool-{stamp}.db"))
        }
    };
    db.backup_to(&path)?;
    println!("Backup written to {}", path.display());
    Ok(())
}

fn print_summaries(articles: &[ArticleSummary]) {
    for (index, article) in articles.iter().enumerate() {
        let audio_flag = if article.has_audio_hint { " ♪" } else { "" };
        println!("{}.{} {}", index + 1, audio_flag, article.title);
        println!("   {}", article.url);
        if !article.section.is_empty() {
            println!("   Section: {}", article.section);
        }
        if !article.teaser.is_empty() {
            println!("   {}", article.teaser);
        }
    }
}

fn resolve_api_key(cli_value: Option<String>) -> Result<String> {
    cli_value
        .or_else(|| std::env::var("LINGQ_API_KEY").ok())
        .or_else(|| {
            // Try the GUI's saved token file
            let mut settings = deutschlandfunk_lingq_tool::settings::AppSettings::default();
            let key = deutschlandfunk_lingq_tool::settings::load_api_key(&mut settings);
            if key.trim().is_empty() {
                None
            } else {
                Some(key)
            }
        })
        .filter(|value| !value.trim().is_empty())
        .context("provide --api-key, set LINGQ_API_KEY, or log in via the GUI")
}
