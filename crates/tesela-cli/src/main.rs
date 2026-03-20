use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tesela_core::traits::plugin::PluginRegistry;
use tesela_core::{
    config::Config,
    daily::DailyNoteConfig,
    db::SqliteIndex,
    export::{export_note, ExportFormat},
    note::NoteId,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};

#[derive(Parser)]
#[command(
    name = "tesela",
    version,
    about = "Keyboard-first, file-based note-taking"
)]
struct Cli {
    /// Path to mosaic directory (defaults to config default_mosaic)
    #[arg(short, long, global = true)]
    mosaic: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new mosaic
    Init {
        /// Path for the new mosaic (defaults to current directory)
        path: Option<PathBuf>,
    },
    /// Create a new note
    New {
        /// Note title
        title: String,
        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
        /// Initial content
        #[arg(short, long)]
        content: Option<String>,
    },
    /// List notes
    List {
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,
        /// Maximum results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Print note content
    Cat {
        /// Note ID or title
        query: String,
    },
    /// Edit a note in external editor
    Edit {
        /// Note ID or title
        query: String,
    },
    /// Search notes with full-text search
    Search {
        /// Search query
        query: String,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Get or create today's daily note
    Daily {
        /// Date (defaults to today, format: YYYY-MM-DD)
        date: Option<String>,
    },
    /// Show backlinks for a note
    Links {
        /// Note ID or title
        query: String,
    },
    /// Export a note
    Export {
        /// Note ID or title
        query: String,
        /// Format: html, text, markdown
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Rebuild the search index
    Reindex,
    /// Launch the TUI interface
    Tui,
    /// Generate shell completions
    Completions {
        /// Shell (bash, zsh, fish, elvish, powershell)
        shell: clap_complete::Shell,
    },
    /// Install tesela-server as a macOS LaunchAgent (runs on login)
    Install,
    /// Uninstall the tesela-server LaunchAgent
    Uninstall,
}

struct Ctx {
    store: Arc<FsNoteStore>,
    index: Arc<SqliteIndex>,
    registry: PluginRegistry,
}

impl Ctx {
    async fn new(mosaic: PathBuf) -> Result<Self> {
        let db_path = mosaic.join(".tesela").join("tesela.db");

        let store = Arc::new(FsNoteStore::open(mosaic.clone()).context("Failed to open mosaic")?);
        let index = Arc::new(
            SqliteIndex::open(&db_path)
                .await
                .context("Failed to open search index")?,
        );
        let registry = tesela_plugins::load_all_plugins(&mosaic);

        Ok(Self {
            store,
            index,
            registry,
        })
    }
}

fn resolve_mosaic(cli_arg: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = cli_arg {
        return Ok(p);
    }

    let config_path = Config::default_path();
    if config_path.exists() {
        if let Ok(config) = Config::load(&config_path) {
            if let Some(mosaic) = config.general.default_mosaic {
                return Ok(mosaic);
            }
        }
    }

    // Walk up from current dir looking for .tesela/
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".tesela").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    anyhow::bail!("No mosaic found. Run 'tesela init' or specify --mosaic")
}

async fn cmd_init(path: Option<PathBuf>) -> Result<()> {
    let root = path.unwrap_or_else(|| std::env::current_dir().unwrap());
    let tesela_dir = root.join(".tesela");
    std::fs::create_dir_all(&tesela_dir)?;
    std::fs::create_dir_all(root.join("notes"))?;
    std::fs::create_dir_all(root.join("attachments"))?;

    // Write default config
    let config = Config::default();
    config
        .save(&tesela_dir.join("config.toml"))
        .context("Failed to save config")?;

    // Initialize SQLite DB
    SqliteIndex::open(&tesela_dir.join("tesela.db"))
        .await
        .context("Failed to initialize database")?;

    println!("Initialized mosaic at {}", root.display());
    Ok(())
}

async fn cmd_new(
    ctx: &Ctx,
    title: String,
    tags: Option<String>,
    content: Option<String>,
) -> Result<()> {
    let tag_list: Vec<&str> = tags
        .as_deref()
        .map(|t| t.split(',').map(str::trim).collect())
        .unwrap_or_default();

    let body = content.as_deref().unwrap_or("");
    let note = ctx
        .store
        .create(&title, body, &tag_list)
        .await
        .context("Failed to create note")?;
    ctx.index
        .upsert_note(&note)
        .await
        .context("Failed to index note")?;

    if let Err(e) = ctx.registry.dispatch_note_created(&note) {
        tracing::warn!("Plugin hook on_note_created failed: {}", e);
    }

    println!("Created: {} ({})", note.title, note.id);
    Ok(())
}

async fn cmd_list(ctx: &Ctx, tag: Option<String>, limit: usize) -> Result<()> {
    let notes = ctx
        .store
        .list(tag.as_deref(), limit, 0)
        .await
        .context("Failed to list notes")?;
    if notes.is_empty() {
        println!("No notes found.");
        return Ok(());
    }
    for note in &notes {
        let tags = note.metadata.tags.join(", ");
        if tags.is_empty() {
            println!("  {} — {}", note.id, note.title);
        } else {
            println!("  {} — {} [{}]", note.id, note.title, tags);
        }
    }
    Ok(())
}

async fn cmd_search(ctx: &Ctx, query: String, limit: usize) -> Result<()> {
    let hits = ctx
        .index
        .search(&query, limit, 0)
        .await
        .context("Search failed")?;
    if hits.is_empty() {
        println!("No results for '{}'", query);
        return Ok(());
    }
    for hit in &hits {
        println!("  {} — {}", hit.note_id, hit.title);
        if !hit.snippet.is_empty() {
            println!("    {}", hit.snippet);
        }
    }
    Ok(())
}

async fn cmd_cat(ctx: &Ctx, query: String) -> Result<()> {
    let note = resolve_note(ctx, &query).await?;
    print!("{}", note.body);
    Ok(())
}

async fn cmd_edit(ctx: &Ctx, query: String) -> Result<()> {
    let note = resolve_note(ctx, &query).await?;

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let mosaic_root = ctx.store.mosaic_root().await;
    let full_path = mosaic_root.join(&note.path);
    std::process::Command::new(&editor)
        .arg(&full_path)
        .status()
        .with_context(|| format!("Failed to launch editor: {}", editor))?;

    // Re-read and reindex
    if let Some(updated) = ctx
        .store
        .get(&note.id)
        .await
        .context("Failed to re-read note")?
    {
        ctx.index
            .upsert_note(&updated)
            .await
            .context("Failed to reindex note")?;
        if let Err(e) = ctx.registry.dispatch_note_updated(&updated) {
            tracing::warn!("Plugin hook on_note_updated failed: {}", e);
        }
    }
    Ok(())
}

async fn cmd_daily(ctx: &Ctx, date: Option<String>) -> Result<()> {
    let parsed_date = date
        .as_deref()
        .map(|d| {
            chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                .with_context(|| format!("Invalid date format: {}", d))
        })
        .transpose()?;

    let daily_config = DailyNoteConfig::default();
    let note = ctx
        .store
        .daily_note(parsed_date, &daily_config)
        .await
        .context("Failed to create daily note")?;
    println!("{}", note.path.display());
    Ok(())
}

async fn cmd_links(ctx: &Ctx, query: String) -> Result<()> {
    let note = resolve_note(ctx, &query).await?;

    let backlinks = ctx
        .index
        .get_backlinks(&note.id)
        .await
        .context("Failed to get backlinks")?;

    if backlinks.is_empty() {
        println!("No backlinks to '{}'", note.title);
    } else {
        println!("Backlinks to '{}':", note.title);
        for link in &backlinks {
            println!("  <- {} (\"{}\")", link.target, link.text);
        }
    }
    Ok(())
}

async fn cmd_export(ctx: &Ctx, query: String, format: String) -> Result<()> {
    let note = resolve_note(ctx, &query).await?;

    let fmt = match format.as_str() {
        "html" => ExportFormat::Html,
        "text" | "txt" => ExportFormat::PlainText,
        "markdown" | "md" => ExportFormat::Markdown,
        other => anyhow::bail!("Unknown format: {}. Use html, text, or markdown", other),
    };

    print!("{}", export_note(&note, fmt));
    Ok(())
}

async fn cmd_reindex(ctx: &Ctx) -> Result<()> {
    let notes = ctx
        .store
        .list(None, usize::MAX, 0)
        .await
        .context("Failed to list notes")?;
    let bar = indicatif::ProgressBar::new(notes.len() as u64);

    for note in &notes {
        ctx.index
            .upsert_note(note)
            .await
            .context("Failed to index note")?;
        bar.inc(1);
    }
    bar.finish();
    println!("Indexed {} notes", notes.len());
    Ok(())
}

const LAUNCHD_LABEL: &str = "com.tesela.server";
const PLIST_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{BINARY_PATH}</string>
    </array>
    <key>WorkingDirectory</key>
    <string>{NOTES_DIR}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{LOG_PATH}</string>
    <key>StandardErrorPath</key>
    <string>{LOG_PATH}</string>
</dict>
</plist>"#;

fn plist_path() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", LAUNCHD_LABEL)))
}

async fn cmd_install(mosaic: PathBuf) -> Result<()> {
    // Find tesela-server binary next to the current executable
    let exe_dir = std::env::current_exe()
        .context("Cannot determine executable path")?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine executable directory"))?;

    let server_bin = exe_dir.join("tesela-server");
    if !server_bin.exists() {
        anyhow::bail!(
            "tesela-server binary not found at {}. Build with `cargo build --workspace`.",
            server_bin.display()
        );
    }

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    let log_path = home
        .join("Library")
        .join("Logs")
        .join("tesela-server.log")
        .to_string_lossy()
        .into_owned();

    let agents_dir = home.join("Library").join("LaunchAgents");
    std::fs::create_dir_all(&agents_dir)
        .context("Failed to create ~/Library/LaunchAgents")?;

    let plist = PLIST_TEMPLATE
        .replace("{LABEL}", LAUNCHD_LABEL)
        .replace("{BINARY_PATH}", &server_bin.to_string_lossy())
        .replace("{NOTES_DIR}", &mosaic.to_string_lossy())
        .replace("{LOG_PATH}", &log_path);

    let plist_file = plist_path()?;
    std::fs::write(&plist_file, &plist)
        .with_context(|| format!("Failed to write plist to {}", plist_file.display()))?;

    let status = std::process::Command::new("launchctl")
        .args(["load", "-w", plist_file.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl")?;

    if !status.success() {
        anyhow::bail!("launchctl load failed with status: {}", status);
    }

    println!("tesela-server installed as LaunchAgent.");
    println!("  Binary:  {}", server_bin.display());
    println!("  Plist:   {}", plist_file.display());
    println!("  Logs:    {}", log_path);
    println!("  API:     http://127.0.0.1:7474");
    Ok(())
}

async fn cmd_uninstall() -> Result<()> {
    let plist_file = plist_path()?;
    if !plist_file.exists() {
        println!("LaunchAgent plist not found — already uninstalled.");
        return Ok(());
    }

    let status = std::process::Command::new("launchctl")
        .args(["unload", plist_file.to_str().unwrap()])
        .status()
        .context("Failed to run launchctl")?;

    if !status.success() {
        anyhow::bail!("launchctl unload failed with status: {}", status);
    }

    std::fs::remove_file(&plist_file)
        .with_context(|| format!("Failed to remove {}", plist_file.display()))?;

    println!("tesela-server LaunchAgent uninstalled.");
    Ok(())
}

/// Resolve a note by ID or title.
async fn resolve_note(ctx: &Ctx, query: &str) -> Result<tesela_core::Note> {
    let id = NoteId::new(query);
    if let Some(note) = ctx
        .store
        .get(&id)
        .await
        .context("Failed to look up note by ID")?
    {
        return Ok(note);
    }
    if let Some(note) = ctx
        .store
        .get_by_title(query)
        .await
        .context("Failed to look up note by title")?
    {
        return Ok(note);
    }
    anyhow::bail!("Note not found: {}", query)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging
    let level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(level))
        .with_writer(std::io::stderr)
        .init();

    // Handle init specially (no mosaic needed yet)
    if let Commands::Init { ref path } = cli.command {
        return cmd_init(path.clone()).await;
    }

    // Handle completions specially (no mosaic needed)
    if let Commands::Completions { shell } = cli.command {
        use clap::CommandFactory;
        clap_complete::generate(shell, &mut Cli::command(), "tesela", &mut std::io::stdout());
        return Ok(());
    }

    // Handle uninstall without a mosaic
    if matches!(cli.command, Commands::Uninstall) {
        return cmd_uninstall().await;
    }

    let mosaic = resolve_mosaic(cli.mosaic)?;

    // Handle install — needs mosaic path but not a full Ctx
    if matches!(cli.command, Commands::Install) {
        return cmd_install(mosaic).await;
    }

    let ctx = Ctx::new(mosaic).await?;

    match cli.command {
        Commands::Init { .. } | Commands::Completions { .. } | Commands::Install | Commands::Uninstall => unreachable!(),
        Commands::New {
            title,
            tags,
            content,
        } => cmd_new(&ctx, title, tags, content).await?,
        Commands::List { tag, limit } => cmd_list(&ctx, tag, limit).await?,
        Commands::Cat { query } => cmd_cat(&ctx, query).await?,
        Commands::Edit { query } => cmd_edit(&ctx, query).await?,
        Commands::Search { query, limit } => cmd_search(&ctx, query, limit).await?,
        Commands::Daily { date } => cmd_daily(&ctx, date).await?,
        Commands::Links { query } => cmd_links(&ctx, query).await?,
        Commands::Export { query, format } => cmd_export(&ctx, query, format).await?,
        Commands::Reindex => cmd_reindex(&ctx).await?,
        Commands::Tui => {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let tui_name = "tesela-tui";
            let mut cmd = if let Some(dir) = &exe_dir {
                let local = dir.join(tui_name);
                if local.exists() {
                    std::process::Command::new(local)
                } else {
                    std::process::Command::new(tui_name)
                }
            } else {
                std::process::Command::new(tui_name)
            };
            cmd.status()
                .context("Failed to launch tesela-tui. Is it installed?")?;
        }
    }

    Ok(())
}
