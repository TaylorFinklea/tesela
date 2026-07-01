use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tesela_core::traits::plugin::PluginRegistry;

mod backfill_task;
mod import_logseq;
mod import_obsidian;
mod import_org;
mod mosaic_notes;
mod recover_logseq_dates;
mod repair_daily_tags;
mod repair_garbled_blocks;
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
    /// Export a single note as html / text / markdown
    ExportNote {
        /// Note ID or title
        query: String,
        /// Format: html, text, markdown
        #[arg(short, long, default_value = "markdown")]
        format: String,
    },
    /// Export the entire mosaic as a portable markdown directory
    Export {
        /// Output directory (will be created)
        out: PathBuf,
        /// `full` (round-trippable, all property:: lines kept) or
        /// `portable` (lossy — strips Tesela-internal properties for
        /// Obsidian/Logseq compatibility). Default: full.
        #[arg(long, default_value = "full")]
        mode: String,
        /// Also copy the `attachments/` directory (default: skip).
        #[arg(long)]
        attachments: bool,
    },
    /// Back up the mosaic to a timestamped, manifest-validated archive
    Backup {
        /// External output directory (defaults to <mosaic>/.tesela/backups/).
        /// When set, the backup is encrypted with the mosaic's age identity
        /// (run `tesela backup-keygen` first if one doesn't exist yet).
        #[arg(short, long, conflicts_with = "git_remote")]
        output: Option<PathBuf>,
        /// Push to a configured git remote (e.g. `git@github.com:me/backups.git`).
        /// Maintains a local mirror at <mosaic>/.tesela/backups/.git-mirror/
        /// and pushes each backup as a commit. Encryption is always ON for
        /// git destinations.
        #[arg(long)]
        git_remote: Option<String>,
        /// Branch name for git destination (defaults to `main`).
        #[arg(long, default_value = "main")]
        git_branch: String,
        /// Force encryption on local backups too (default: encrypt only when
        /// `--output` points outside the mosaic, or always when `--git-remote`).
        #[arg(long)]
        encrypt: bool,
        /// Skip the post-write round-trip validation. Off by default —
        /// validation is the whole point of the new backup pipeline.
        #[arg(long)]
        no_validate: bool,
        /// Skip GFS retention pruning (keeps every prior backup).
        #[arg(long)]
        no_prune: bool,
    },
    /// Generate and store an age keypair for this mosaic in the macOS Keychain
    BackupKeygen,
    /// Re-run round-trip validation on an existing backup
    BackupVerify {
        /// Path to the backup directory (e.g. `.tesela/backups/backup-...`)
        path: PathBuf,
    },
    /// List backups under a destination root
    BackupList {
        /// Destination root (defaults to <mosaic>/.tesela/backups/)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Apply GFS retention manually
    BackupPrune {
        /// Destination root (defaults to <mosaic>/.tesela/backups/)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Show what would be deleted without removing anything
        #[arg(long)]
        dry_run: bool,
    },
    /// Import notes from a LogSeq graph
    ImportLogseq {
        /// Path to the LogSeq graph directory (containing journals/ and pages/)
        #[arg(long)]
        source: PathBuf,
        /// Dry run — show what would be imported without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Import notes from an Obsidian vault
    ImportObsidian {
        /// Path to the vault root
        #[arg(long)]
        source: PathBuf,
        /// Dry run — show what would be imported without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Import notes from a directory of `.org` files (e.g. an org-roam vault)
    ImportOrg {
        /// Path to a single `.org` file or a directory containing them
        #[arg(long)]
        source: PathBuf,
        /// Dry run — show what would be imported without writing
        #[arg(long)]
        dry_run: bool,
    },
    /// Add #Task to every status-bearing block that lacks it (dry-run unless --apply)
    BackfillTask {
        /// Actually write the tags. Default: dry-run — summary + per-note rollup.
        #[arg(long)]
        apply: bool,
        /// Also print the full per-block list (can run to thousands of lines).
        #[arg(long)]
        verbose: bool,
    },
    /// One-off recovery: restore timed/repeating SCHEDULED/DEADLINE stamps the
    /// old Logseq importer dropped, by re-reading the original vault (dry-run
    /// unless --apply)
    RecoverLogseqDates {
        /// Path to the original LogSeq graph directory (containing journals/ and pages/)
        #[arg(long)]
        source: PathBuf,
        /// Actually write the properties. Default: dry-run — print the recovery table.
        #[arg(long)]
        apply: bool,
    },
    /// One-off repair: find canonical YYYY-MM-DD.md dailies whose frontmatter
    /// is missing the `daily` tag and add it (dry-run unless --apply).
    RepairDailyTags {
        /// Actually write the tags. Default: dry-run — list the candidates.
        #[arg(long)]
        apply: bool,
    },
    /// One-off repair (tesela-49d): collapse residual disjoint-lineage TWIN
    /// blocks (same block_id on >1 live Loro node) to the deterministic winner
    /// the live sync uses (dry-run unless --apply). A single-node UNION
    /// concatenation is not a twin and must be fixed manually.
    RepairGarbledBlocks {
        /// Actually collapse the twins. Default: dry-run — report what would heal.
        #[arg(long)]
        apply: bool,
    },
    /// Restore a mosaic from a backup
    Restore {
        /// Backup directory to restore from (e.g., .tesela/backups/backup-20260404-120000)
        source: PathBuf,
        /// Replace the current mosaic instead of creating a sibling.
        /// The current mosaic is renamed to `<root>.before-restore-<timestamp>`
        /// before the restore writes — never silently destroyed.
        #[arg(long)]
        in_place: bool,
        /// Allow restoring a backup written by a newer Tesela than this binary
        #[arg(long)]
        allow_newer: bool,
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
    // Migrate older config location if needed (best-effort).
    let _ = Config::migrate_legacy_config();

    if let Some(p) = cli_arg {
        return Ok(p);
    }

    // Resolution order matches the server's find_mosaic for
    // consistency: env → cwd-walk → config default → standard
    // fallback. Cwd-walk wins over config default so being inside a
    // mosaic dir overrides the saved preference.
    if let Ok(env) = std::env::var("TESELA_DEFAULT_MOSAIC") {
        let p = PathBuf::from(env);
        if p.join(".tesela").exists() {
            return Ok(p);
        }
    }

    if let Ok(start) = std::env::current_dir() {
        let mut dir = start;
        loop {
            if dir.join(".tesela").exists() {
                return Ok(dir);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    let config_path = Config::default_path();
    if config_path.exists() {
        if let Ok(config) = Config::load(&config_path) {
            if let Some(mosaic) = config.general.default_mosaic {
                if mosaic.join(".tesela").exists() {
                    return Ok(mosaic);
                }
            }
        }
    }

    // Final fallback: the per-OS standard mosaic location. CLI does
    // NOT auto-init here — explicit `tesela init` is the way to
    // create a mosaic from the command line. Server auto-inits in
    // the equivalent fallback (different module, intentional split).
    let default = Config::default_mosaic_path();
    if default.join(".tesela").exists() {
        return Ok(default);
    }

    anyhow::bail!(
        "No mosaic found. Run `tesela init` (will create one at {}) or pass --mosaic <path>",
        default.display()
    )
}

async fn cmd_init(path: Option<PathBuf>) -> Result<()> {
    // No path → use the per-OS standard mosaic location instead of
    // cwd. Cwd-init was the old default; it's almost never what
    // first-time users want (they end up with a mosaic embedded in
    // whatever directory they happened to be in). Pass `.` to opt
    // into cwd explicitly.
    let root = path.unwrap_or_else(Config::default_mosaic_path);
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

    // Seed the default rail widgets (Dailies, Pages, Tasks, …) so the
    // first-launch UX isn't an empty rail. Idempotent.
    let seeded =
        tesela_core::system_widgets::seed(&root).context("Failed to seed system widgets")?;

    println!(
        "Initialized mosaic at {}{}",
        root.display(),
        if seeded > 0 {
            format!(" (seeded {} widget pages)", seeded)
        } else {
            String::new()
        }
    );
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

fn cmd_export_mosaic(mosaic: &Path, out: PathBuf, mode: String, attachments: bool) -> Result<()> {
    use tesela_core::export::markdown::{export_mosaic, ExportOptions, MarkdownMode};
    let mode = match mode.as_str() {
        "full" => MarkdownMode::Full,
        "portable" => MarkdownMode::Portable,
        other => anyhow::bail!("Unknown export mode: {}. Use `full` or `portable`.", other),
    };
    let outcome = export_mosaic(
        mosaic,
        &out,
        &ExportOptions {
            mode,
            include_attachments: attachments,
        },
    )?;
    println!(
        "Exported {} note{} ({} mode) → {}",
        outcome.note_count,
        if outcome.note_count == 1 { "" } else { "s" },
        match mode {
            MarkdownMode::Full => "full",
            MarkdownMode::Portable => "portable",
        },
        out.display()
    );
    if attachments {
        println!(
            "Attachments: {} file{}",
            outcome.attachment_count,
            if outcome.attachment_count == 1 {
                ""
            } else {
                "s"
            }
        );
    }
    if matches!(mode, MarkdownMode::Portable) {
        println!(
            "Stripped {} Tesela-internal propert{} (see README.md in the export root)",
            outcome.stripped_property_count,
            if outcome.stripped_property_count == 1 {
                "y"
            } else {
                "ies"
            }
        );
    }
    Ok(())
}

async fn cmd_export_note(ctx: &Ctx, query: String, format: String) -> Result<()> {
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

async fn cmd_backup(
    mosaic: &Path,
    output: Option<PathBuf>,
    git_remote: Option<String>,
    git_branch: String,
    force_encrypt: bool,
    validate: bool,
    prune: bool,
) -> Result<()> {
    if !mosaic.join("notes").exists() {
        anyhow::bail!("Notes directory not found in {}", mosaic.display());
    }

    // Pre-stage the SQLite snapshot. `VACUUM INTO` is consistent under
    // WAL even with concurrent writers (e.g. a running tesela-server),
    // so we don't need to take down anything. The snapshot lives in a
    // tempfile that we hand off to tesela-backup as an "extra file".
    let mut extra_files = Vec::new();
    let db_path = mosaic.join(".tesela").join("tesela.db");
    let _snapshot_holder = if db_path.exists() {
        let snapshot = tempfile::Builder::new()
            .prefix("tesela-vacuum-")
            .suffix(".db")
            .tempfile()
            .context("failed to create vacuum tempfile")?;
        let snap_path = snapshot.path().to_path_buf();
        let index = tesela_core::db::SqliteIndex::open(&db_path)
            .await
            .context("open SQLite for vacuum snapshot")?;
        index
            .vacuum_into(&snap_path)
            .await
            .context("VACUUM INTO snapshot")?;
        extra_files.push((".tesela/tesela.db".to_string(), snap_path));
        Some(snapshot)
    } else {
        None
    };

    let (destination, encrypt_default) = if let Some(remote) = git_remote {
        let mirror = mosaic.join(".tesela").join("backups").join(".git-mirror");
        (
            tesela_backup::Destination::Git {
                remote,
                branch: git_branch,
                local_mirror: mirror,
            },
            true,
        )
    } else if let Some(path) = output {
        (tesela_backup::Destination::External { path }, true)
    } else {
        (tesela_backup::Destination::Local, false)
    };
    let should_encrypt = force_encrypt || encrypt_default;
    let encryption = if should_encrypt {
        let identity = tesela_backup::encrypt::load_identity_for_mosaic(mosaic)
            .map_err(|e| anyhow::anyhow!("{}", e))?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no age identity in Keychain for this mosaic. Run `tesela backup-keygen` first."
                )
            })?;
        tesela_backup::ManifestEncryption::Age {
            recipient: identity.to_public().to_string(),
        }
    } else {
        tesela_backup::ManifestEncryption::None
    };

    let outcome = tesela_backup::backup(
        mosaic,
        tesela_backup::BackupOptions {
            destination,
            validate,
            extra_files,
            retention: if prune {
                Some(tesela_backup::GfsPolicy::default())
            } else {
                None
            },
            encryption,
        },
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!(
        "Backup complete: {} ({} files)",
        outcome.path.display(),
        outcome.manifest.files.len()
    );
    if let Some(v) = &outcome.manifest.validated {
        if v.ok {
            println!("Validated: round-trip OK in {} ms", v.elapsed_ms);
        } else {
            println!(
                "Validated: FAILED — {}",
                v.note.as_deref().unwrap_or("unknown")
            );
        }
    } else {
        println!("Validation skipped (--no-validate)");
    }
    if !outcome.pruned.removed.is_empty() {
        println!(
            "Pruned {} old backup(s) per GFS retention",
            outcome.pruned.removed.len()
        );
    }

    Ok(())
}

fn cmd_backup_keygen(mosaic: &Path) -> Result<()> {
    let recipient =
        tesela_backup::encrypt::keygen_for_mosaic(mosaic).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Generated age identity for {}", mosaic.display());
    println!(
        "Stored in Keychain (service=tesela-backup, account={})",
        mosaic.display()
    );
    println!("Public recipient: {}", recipient);
    println!("\nFuture `tesela backup --output <path>` runs will encrypt to this recipient.");
    Ok(())
}

async fn cmd_backup_verify(path: &Path) -> Result<()> {
    let status = tesela_backup::verify(path).map_err(|e| anyhow::anyhow!("{}", e))?;
    if status.ok {
        println!(
            "Verified: round-trip OK in {} ms ({})",
            status.elapsed_ms,
            status.checked_at.format("%Y-%m-%d %H:%M:%S")
        );
        Ok(())
    } else {
        anyhow::bail!(
            "Verification FAILED: {}",
            status.note.unwrap_or_else(|| "unknown".to_string())
        );
    }
}

async fn cmd_backup_list(mosaic: &Path, output: Option<PathBuf>) -> Result<()> {
    let root = output.unwrap_or_else(|| mosaic.join(".tesela").join("backups"));
    let backups = tesela_backup::list(&root).map_err(|e| anyhow::anyhow!("{}", e))?;
    if backups.is_empty() {
        println!("No backups found in {}", root.display());
        return Ok(());
    }
    println!("{:<32} {:<25} {:>8} validated", "name", "created", "files");
    for (path, manifest) in backups {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let validated = match &manifest.validated {
            Some(v) if v.ok => "OK",
            Some(_) => "FAIL",
            None => "—",
        };
        println!(
            "{:<32} {:<25} {:>8} {}",
            name,
            manifest.created_at.format("%Y-%m-%d %H:%M:%S"),
            manifest.files.len(),
            validated
        );
    }
    Ok(())
}

async fn cmd_backup_prune(mosaic: &Path, output: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let root = output.unwrap_or_else(|| mosaic.join(".tesela").join("backups"));
    let outcome = tesela_backup::prune_gfs(&root, tesela_backup::GfsPolicy::default(), dry_run)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    if dry_run {
        println!(
            "Dry run: would keep {}, would remove {}",
            outcome.kept.len(),
            outcome.removed.len()
        );
    } else {
        println!(
            "Kept {} backup(s); removed {}",
            outcome.kept.len(),
            outcome.removed.len()
        );
    }
    for path in &outcome.removed {
        println!(
            "  {} {}",
            if dry_run { "would remove" } else { "removed" },
            path.display()
        );
    }
    Ok(())
}

async fn cmd_restore(
    mosaic: &Path,
    source: PathBuf,
    in_place: bool,
    allow_newer: bool,
) -> Result<()> {
    let outcome = tesela_backup::restore(
        &source,
        mosaic,
        tesela_backup::RestoreOptions {
            in_place,
            target_override: None,
            allow_newer,
        },
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Restored to {}", outcome.target.display());
    if let Some(prev) = &outcome.renamed_previous {
        println!(
            "Previous mosaic preserved at {} (you can rm -rf when satisfied)",
            prev.display()
        );
    }
    println!(
        "Manifest: {} files, written {}",
        outcome.manifest.files.len(),
        outcome.manifest.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("\nRestart tesela-server to reindex if needed.");
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
        // `reindex` (not `upsert_note`) so the type-system caches
        // (`tag_defs`/`property_defs` via `index_type_info`) are rebuilt
        // too — otherwise `tesela reindex` rebuilds only the search index
        // and Tag/Property pages stay unresolved (GET /types empty).
        ctx.index
            .reindex(note)
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
    std::fs::create_dir_all(&agents_dir).context("Failed to create ~/Library/LaunchAgents")?;

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

    // Handle backup — needs mosaic path but not a full Ctx
    if let Commands::Backup {
        output,
        git_remote,
        git_branch,
        encrypt,
        no_validate,
        no_prune,
    } = cli.command
    {
        return cmd_backup(
            &mosaic,
            output,
            git_remote,
            git_branch,
            encrypt,
            !no_validate,
            !no_prune,
        )
        .await;
    }

    if matches!(cli.command, Commands::BackupKeygen) {
        return cmd_backup_keygen(&mosaic);
    }

    if let Commands::BackupVerify { path } = &cli.command {
        return cmd_backup_verify(path).await;
    }

    if let Commands::BackupList { output } = cli.command {
        return cmd_backup_list(&mosaic, output).await;
    }

    if let Commands::BackupPrune { output, dry_run } = cli.command {
        return cmd_backup_prune(&mosaic, output, dry_run).await;
    }

    if let Commands::Export {
        out,
        mode,
        attachments,
    } = cli.command
    {
        return cmd_export_mosaic(&mosaic, out, mode, attachments);
    }

    // Handle restore — needs mosaic path but not a full Ctx
    if let Commands::Restore {
        source,
        in_place,
        allow_newer,
    } = cli.command
    {
        return cmd_restore(&mosaic, source, in_place, allow_newer).await;
    }

    // Handle LogSeq import — needs mosaic path but not a full Ctx
    if let Commands::ImportLogseq { source, dry_run } = cli.command {
        return import_logseq::run(&mosaic, source, dry_run).await;
    }

    if let Commands::ImportObsidian { source, dry_run } = cli.command {
        return import_obsidian::run(&mosaic, source, dry_run).await;
    }

    if let Commands::ImportOrg { source, dry_run } = cli.command {
        return import_org::run(&mosaic, source, dry_run).await;
    }

    // Backfill #Task — needs the Loro engine over the mosaic, not a full Ctx.
    if let Commands::BackfillTask { apply, verbose } = cli.command {
        return backfill_task::run(&mosaic, apply, verbose).await;
    }

    // Recover dropped Logseq dates — Loro engine over the mosaic, no Ctx.
    if let Commands::RecoverLogseqDates { source, apply } = cli.command {
        return recover_logseq_dates::run(&mosaic, &source, apply).await;
    }

    // Repair missing `daily` tags on date-slug dailies — pure filesystem walk,
    // no Ctx / no Loro engine required.
    if let Commands::RepairDailyTags { apply } = cli.command {
        return repair_daily_tags::run(&mosaic, apply).await;
    }

    // Repair residual disjoint-lineage twin blocks — opens the Loro engine
    // directly (locks the mosaic), no Ctx required.
    if let Commands::RepairGarbledBlocks { apply } = cli.command {
        return repair_garbled_blocks::run(&mosaic, apply).await;
    }

    let ctx = Ctx::new(mosaic).await?;

    match cli.command {
        Commands::Init { .. }
        | Commands::Completions { .. }
        | Commands::Install
        | Commands::Uninstall
        | Commands::Backup { .. }
        | Commands::BackupKeygen
        | Commands::BackupVerify { .. }
        | Commands::BackupList { .. }
        | Commands::BackupPrune { .. }
        | Commands::Restore { .. }
        | Commands::Export { .. }
        | Commands::ImportLogseq { .. }
        | Commands::ImportObsidian { .. }
        | Commands::ImportOrg { .. }
        | Commands::BackfillTask { .. }
        | Commands::RecoverLogseqDates { .. }
        | Commands::RepairDailyTags { .. }
        | Commands::RepairGarbledBlocks { .. } => unreachable!(),
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
        Commands::ExportNote { query, format } => cmd_export_note(&ctx, query, format).await?,
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
