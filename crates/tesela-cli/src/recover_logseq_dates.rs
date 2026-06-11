//! `recover-logseq-dates` subcommand — one-off data recovery.
//!
//! The old Logseq importer's `LOGSEQ_DATE_RE` only matched the bare
//! `<YYYY-MM-DD Day>` planning form, so any `SCHEDULED:`/`DEADLINE:` stamp
//! carrying an HH:MM time and/or a repeater (`<2026-06-12 Fri 10:00 .+1w>`)
//! was silently left unconverted — no `scheduled::`/`deadline::` property
//! ever reached the mosaic (fixed for future imports in a2eecf0). This tool
//! re-reads the ORIGINAL vault, finds those timed/repeating stamps, matches
//! their task text against blocks in the live mosaic, and restores the
//! properties through the STRUCTURED op path (`BlockPropertySet` SetScalar),
//! mirroring `backfill_task`'s whole shape: engine-open via the server
//! flock, dry-run by default, `--apply` to write, idempotent re-runs.
//!
//! Matching is deliberately conservative: exactly one block whose normalized
//! text equals the vault task's text → recover; zero → report as missing;
//! more than one → skip as ambiguous (never guess). A block that already
//! carries the property (the user added it by hand since the import) is
//! skipped as already-present, which is also what makes `--apply` idempotent.
//!
//! Residency: the Loro engine only holds docs for notes that have flowed
//! through it; the canonical content for everything else is the engine's own
//! materialized `<mosaic>/notes/<slug>.md` files. The matcher therefore scans
//! BOTH — engine render for resident notes, the materialized file otherwise —
//! and `--apply` hydrates a non-resident note first with the same op every
//! editor save and `reseed_from_disk` use: a `NoteUpsert` of the file content
//! under the system-wide stable id `blake3(slug)[..16]` (see the server's
//! `stable_uuid_from_slug`). `NoteUpsert` is a non-destructive per-bid
//! reconcile on every engine (2026-06-10 semantics), so hydration cannot
//! clobber concurrent state.
//!
//! Repeaters map to the `recurring::` vocabulary `tesela_core::recurrence`
//! parses (`daily`, `weekly`, `every 2 weeks`, ...), mirroring the org
//! importer's mapping. Unmappable repeater forms are reported and skipped —
//! the date itself is still recovered.

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use tesela_sync::{Hlc, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

use crate::backfill_task::{acquire_mosaic_lock, hex16, line_property_key, load_device_id};

/// Logseq planning timestamp with the pieces the old importer dropped.
/// Mirrors the NEW `LOGSEQ_DATE_RE` (date, optional weekday, optional
/// HH:MM, tolerant `[^>]*` tail) but ALSO captures the repeater token
/// (`.+1w` / `++2d` / `+1m`) as group 3 — recovery needs it for
/// `recurring::` where the importer just ignores it.
static STAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"<(\d{4}-\d{2}-\d{2})(?:\s+[A-Za-z]+)?(?:\s+(\d{2}:\d{2}))?(?:\s+([.+][^>\s]+))?[^>]*>",
    )
    .unwrap()
});

/// One recoverable stamp found in the vault: a task's normalized text plus
/// the dated property the old importer should have emitted for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultItem {
    /// Vault-relative file path (for reporting).
    pub file: String,
    /// The task's text, normalized exactly the way the importer normalizes
    /// it (marker + priority stripped) so it equals the mosaic block text.
    pub text: String,
    /// `"scheduled"` or `"deadline"`.
    pub key: &'static str,
    /// `YYYY-MM-DD` or `YYYY-MM-DD HH:MM` — the same value format the
    /// importer emits and the agenda's `parse_dated_value` accepts.
    pub value: String,
    /// Mapped `recurring::` value (a string `tesela_core::recurrence::parse`
    /// accepts), when the stamp carried a mappable repeater.
    pub recurring: Option<String>,
    /// Raw repeater token that could NOT be mapped (reported; date still
    /// recovered).
    pub unmapped_repeater: Option<String>,
}

/// What happened to one vault item during the mosaic match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Exactly one block matched and lacked the property.
    Recovered {
        note_id: [u8; 16],
        block_id: [u8; 16],
        /// Note slug (display alias) for human-readable output.
        slug: String,
        /// Whether the `recurring::` part will be / was written too
        /// (false when unmapped or already present on the block).
        write_recurring: bool,
        /// The note is not Loro-resident — apply records a `NoteUpsert`
        /// of the materialized file first (per-bid reconcile, the same
        /// op every editor save uses).
        needs_hydration: bool,
    },
    /// No block matched (deleted or edited since import).
    Missing,
    /// More than one block matched — never guess.
    Ambiguous(usize),
    /// The matched block already carries the property.
    AlreadyPresent,
}

pub struct ItemReport {
    pub item: VaultItem,
    pub outcome: Outcome,
}

#[derive(Default)]
pub struct Report {
    pub items: Vec<ItemReport>,
    /// Same (text, key) seen in the vault with CONFLICTING values —
    /// dropped entirely (recovering either would be a guess).
    pub vault_conflicts: Vec<String>,
    pub applied_ops: usize,
    /// Notes hydrated into the engine (NoteUpsert) during apply.
    pub hydrated_notes: usize,
}

impl Report {
    fn count(&self, f: impl Fn(&Outcome) -> bool) -> usize {
        self.items.iter().filter(|r| f(&r.outcome)).count()
    }
    pub fn recovered(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Recovered { .. }))
    }
    pub fn ambiguous(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Ambiguous(_)))
    }
    pub fn missing(&self) -> usize {
        self.count(|o| matches!(o, Outcome::Missing))
    }
    pub fn already_present(&self) -> usize {
        self.count(|o| matches!(o, Outcome::AlreadyPresent))
    }
}

/// A parsed `SCHEDULED:`/`DEADLINE:` continuation line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampLine {
    pub key: &'static str,
    pub date: String,
    pub time: Option<String>,
    pub repeater: Option<String>,
}

/// Parse a trimmed `SCHEDULED: <...>` / `DEADLINE: <...>` line. Returns
/// `None` for anything else (including planning lines whose `<...>` body
/// doesn't carry a date).
pub fn parse_stamp_line(trimmed: &str) -> Option<StampLine> {
    let key = if trimmed.starts_with("SCHEDULED:") {
        "scheduled"
    } else if trimmed.starts_with("DEADLINE:") {
        "deadline"
    } else {
        return None;
    };
    let caps = STAMP_RE.captures(trimmed)?;
    Some(StampLine {
        key,
        date: caps.get(1).unwrap().as_str().to_string(),
        time: caps.get(2).map(|m| m.as_str().to_string()),
        repeater: caps.get(3).map(|m| m.as_str().to_string()),
    })
}

/// Map a Logseq/org repeater token (`.+1w`, `++2d`, `+1m`, ...) to the
/// `recurring::` vocabulary `tesela_core::recurrence::parse` accepts.
/// Mirrors the org importer's mapping (`import_org.rs::parse_planning`).
/// `None` for unmappable forms (e.g. hour units).
pub fn map_repeater(raw: &str) -> Option<String> {
    let stripped = raw.trim_start_matches(['.', '+']);
    if stripped == raw {
        return None; // no repeater prefix at all
    }
    if stripped.len() < 2 {
        return None;
    }
    let (n_str, unit) = stripped.split_at(stripped.len() - 1);
    let n: u32 = n_str.parse().ok()?;
    if n == 0 {
        return None;
    }
    Some(match (n, unit) {
        (1, "d") => "daily".to_string(),
        (1, "w") => "weekly".to_string(),
        (1, "m") => "monthly".to_string(),
        (1, "y") => "yearly".to_string(),
        (n, "d") => format!("every {} days", n),
        (n, "w") => format!("every {} weeks", n),
        (n, "m") => format!("every {} months", n),
        (n, "y") => format!("every {} years", n),
        _ => return None,
    })
}

/// Normalize a trimmed bullet line to the block text the importer would
/// have produced for it — task marker stripped, `[#A]` priority stripped
/// (task bullets), or the asset-URL/tab rewrite (plain bullets). This must
/// stay in lockstep with `import_logseq::convert_content`'s text transform
/// or matching silently fails.
pub fn normalize_bullet_text(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("- ")?;
    // Same marker → status table as the importer's `strip_task_marker`
    // (only the text transform matters here, not the status).
    for marker in [
        "TODO ",
        "DOING ",
        "IN-PROGRESS ",
        "DONE ",
        "LATER ",
        "NOW ",
        "WAITING ",
        "WAIT ",
        "CANCELED ",
        "CANCELLED ",
    ] {
        if let Some(text) = rest.strip_prefix(marker) {
            let clean = tesela_core::regex_cache::PRIORITY_RE
                .replace(text, "")
                .to_string();
            return Some(clean.trim().to_string());
        }
    }
    // Non-task bullet: the importer's pass-through path applies the asset
    // URL rewrite and tab → 2-space conversion before writing.
    Some(
        rest.replace("../assets/", "../attachments/")
            .replace('\t', "  ")
            .trim()
            .to_string(),
    )
}

/// Scan one vault file's content for timed/repeating stamps under bullets.
/// Date-only stamps are EXCLUDED — the old importer converted those fine.
pub fn scan_content(rel: &str, content: &str, out: &mut Vec<VaultItem>) {
    let mut in_code_block = false;
    let mut current_text: Option<String> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        // Skip fenced code blocks, like the importer does — a literal
        // "SCHEDULED:" inside user code is not a planning line.
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if trimmed.starts_with("- ") || trimmed == "-" {
            current_text = normalize_bullet_text(trimmed).filter(|t| !t.is_empty());
            continue;
        }
        let Some(text) = current_text.as_ref() else {
            continue;
        };
        let Some(stamp) = parse_stamp_line(trimmed) else {
            continue;
        };
        // The old importer only dropped stamps with a time and/or repeater;
        // bare dates were imported correctly and need no recovery.
        if stamp.time.is_none() && stamp.repeater.is_none() {
            continue;
        }
        let value = match &stamp.time {
            Some(t) => format!("{} {}", stamp.date, t),
            None => stamp.date.clone(),
        };
        let (recurring, unmapped) = match &stamp.repeater {
            Some(raw) => match map_repeater(raw) {
                Some(rule) => (Some(rule), None),
                None => (None, Some(raw.clone())),
            },
            None => (None, None),
        };
        out.push(VaultItem {
            file: rel.to_string(),
            text: text.clone(),
            key: stamp.key,
            value,
            recurring,
            unmapped_repeater: unmapped,
        });
    }
}

/// Walk `<source>/**/*.md` and collect recoverable stamps. Skips hidden
/// directories and Logseq's own `logseq/` config dir (its `bak/` holds
/// stale page copies that would only manufacture conflicts/ambiguity —
/// the importer never read it either). Same-`(text, key)` duplicates with
/// identical values are deduped; conflicting values are dropped and
/// reported (recovering either would be a guess).
pub fn scan_vault(source: &Path) -> Result<(Vec<VaultItem>, Vec<String>)> {
    anyhow::ensure!(
        source.is_dir(),
        "source vault {} is not a directory",
        source.display()
    );
    let mut raw: Vec<VaultItem> = Vec::new();
    for entry in walkdir::WalkDir::new(source)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.depth() > 0
                && e.file_type().is_dir()
                && (name.starts_with('.') || name == "logseq"))
        })
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(source)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .to_string();
        let content = std::fs::read_to_string(entry.path())
            .with_context(|| format!("read {}", entry.path().display()))?;
        scan_content(&rel, &content, &mut raw);
    }

    // Dedupe / conflict-detect by (text, key).
    let mut by_key: HashMap<(String, &'static str), Vec<VaultItem>> = HashMap::new();
    for item in raw {
        by_key
            .entry((item.text.clone(), item.key))
            .or_default()
            .push(item);
    }
    let mut items: Vec<VaultItem> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();
    for ((text, key), group) in by_key {
        let first = &group[0];
        if group
            .iter()
            .all(|i| i.value == first.value && i.recurring == first.recurring)
        {
            items.push(first.clone());
        } else {
            conflicts.push(format!(
                "{}:: for {:?} has conflicting vault values ({})",
                key,
                text,
                group
                    .iter()
                    .map(|i| format!("{} [{}]", i.value, i.file))
                    .collect::<Vec<_>>()
                    .join(" vs ")
            ));
        }
    }
    // Deterministic output order for reporting/tests.
    items.sort_by(|a, b| (&a.file, &a.text, a.key).cmp(&(&b.file, &b.text, b.key)));
    conflicts.sort();
    Ok((items, conflicts))
}

/// True if the block's folded text already carries `key:: ...` (in-text or
/// materialized container property — both render to a property line).
fn block_has_property(block_text: &str, key: &str) -> bool {
    block_text
        .lines()
        .any(|l| line_property_key(l).as_deref() == Some(key))
}

/// System-wide stable note id: blake3(slug) truncated to 16 bytes —
/// mirrors the server's `stable_uuid_from_slug` (routes/notes.rs) and
/// the engine's `reseed_from_disk`, so the CLI addresses the SAME doc
/// every other surface does.
fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
}

/// `title:` from a YAML frontmatter block (mirrors the engine's reseed
/// title extraction); `None` if absent.
fn frontmatter_title(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim_end_matches('\r') != "---" {
        return None;
    }
    for line in lines {
        if line.trim_end_matches('\r') == "---" {
            return None;
        }
        if let Some(v) = line.strip_prefix("title:") {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// One matchable block, from either source of truth.
struct Candidate {
    note_id: [u8; 16],
    block_id: [u8; 16],
    slug: String,
    block_text: String,
    /// `Some(file content)` when the note is NOT Loro-resident and apply
    /// must hydrate it (NoteUpsert of this content) first.
    hydrate_content: Option<String>,
}

/// Core match + (optional) apply over an open engine, mirroring
/// `backfill_task::backfill`: a read-only scan builds the text index and
/// decides every outcome before any write happens.
///
/// `notes_dir` is the engine's own materialized notes directory — blocks
/// in notes the engine doesn't hold resident are matched from their
/// `.md` files and hydrated on apply (see module docs).
pub async fn recover(
    engine: &LoroEngine,
    notes_dir: Option<&Path>,
    items: Vec<VaultItem>,
    vault_conflicts: Vec<String>,
    apply: bool,
) -> Result<Report> {
    // Pass 1a — index every Loro-resident block by its normalized first
    // text line (engine render is authority for resident notes).
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    let slugs: HashMap<String, String> = engine
        .index_entries()
        .await
        .into_iter()
        .map(|e| (e.note_id, e.slug))
        .collect();
    let mut resident_slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
    for note_id in engine.note_ids().await {
        let Some(md) = engine.render_note_full(note_id).await else {
            continue;
        };
        let slug = slugs
            .get(&hex16(&note_id))
            .cloned()
            .unwrap_or_else(|| hex16(&note_id));
        resident_slugs.insert(slug.clone());
        let tree = tesela_core::note_tree::parse_note(&md);
        for block in &tree.blocks {
            candidates.push(Candidate {
                note_id,
                block_id: *block.id.as_bytes(),
                slug: slug.clone(),
                block_text: block.text.clone(),
                hydrate_content: None,
            });
        }
    }
    // Pass 1b — non-resident notes: the engine's materialized `.md`
    // files are the canonical content; match from disk, hydrate on apply.
    if let Some(dir) = notes_dir {
        if dir.is_dir() {
            let mut paths: Vec<_> = std::fs::read_dir(dir)
                .with_context(|| format!("read notes dir {}", dir.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
                .collect();
            paths.sort();
            for path in paths {
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if resident_slugs.contains(stem) {
                    continue; // engine version already indexed
                }
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("read {}", path.display()))?;
                let note_id = stable_uuid_from_slug(stem);
                let tree = tesela_core::note_tree::parse_note(&content);
                for block in &tree.blocks {
                    candidates.push(Candidate {
                        note_id,
                        block_id: *block.id.as_bytes(),
                        slug: stem.to_string(),
                        block_text: block.text.clone(),
                        hydrate_content: Some(content.clone()),
                    });
                }
            }
        }
    }
    for (i, c) in candidates.iter().enumerate() {
        let first = c.block_text.lines().next().unwrap_or("").trim().to_string();
        if !first.is_empty() {
            index.entry(first).or_default().push(i);
        }
    }

    // Pass 2 — decide every outcome (read-only).
    let mut report = Report {
        vault_conflicts,
        ..Report::default()
    };
    for item in items {
        let outcome = match index.get(&item.text) {
            None => Outcome::Missing,
            Some(matches) if matches.is_empty() => Outcome::Missing,
            Some(matches) if matches.len() > 1 => Outcome::Ambiguous(matches.len()),
            Some(matches) => {
                let c = &candidates[matches[0]];
                if block_has_property(&c.block_text, item.key) {
                    Outcome::AlreadyPresent
                } else {
                    let write_recurring =
                        item.recurring.is_some() && !block_has_property(&c.block_text, "recurring");
                    Outcome::Recovered {
                        note_id: c.note_id,
                        block_id: c.block_id,
                        slug: c.slug.clone(),
                        write_recurring,
                        needs_hydration: c.hydrate_content.is_some(),
                    }
                }
            }
        };
        report.items.push(ItemReport { item, outcome });
    }

    // Pass 3 — apply through the structured property path. Non-resident
    // notes are hydrated once (NoteUpsert of their materialized content —
    // the per-bid-reconcile op every editor save records) before their
    // property sets.
    if apply {
        let mut hydrated: std::collections::HashSet<[u8; 16]> = std::collections::HashSet::new();
        // slug → file content for notes that need hydration.
        let hydrate_by_note: HashMap<[u8; 16], (String, String)> = candidates
            .iter()
            .filter_map(|c| {
                c.hydrate_content
                    .as_ref()
                    .map(|content| (c.note_id, (c.slug.clone(), content.clone())))
            })
            .collect();
        for r in &report.items {
            let Outcome::Recovered {
                note_id,
                block_id,
                write_recurring,
                needs_hydration,
                ..
            } = &r.outcome
            else {
                continue;
            };
            if *needs_hydration && !hydrated.contains(note_id) {
                let (slug, content) = hydrate_by_note
                    .get(note_id)
                    .expect("needs_hydration implies recorded content");
                engine
                    .record_local(OpPayload::NoteUpsert {
                        note_id: *note_id,
                        display_alias: Some(slug.clone()),
                        title: frontmatter_title(content).unwrap_or_else(|| slug.clone()),
                        content: content.clone(),
                        created_at_millis: 0,
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("hydrate note {slug}: {e}"))?;
                hydrated.insert(*note_id);
                report.hydrated_notes += 1;
            }
            engine
                .record_local(OpPayload::BlockPropertySet {
                    note_id: *note_id,
                    block_id: *block_id,
                    key: r.item.key.to_string(),
                    value: PropOp::SetScalar(PropScalar::Text(r.item.value.clone())),
                })
                .await
                .map_err(|e| anyhow::anyhow!("set {}:: {}: {e}", r.item.key, r.item.value))?;
            report.applied_ops += 1;
            if *write_recurring {
                let rule = r
                    .item
                    .recurring
                    .clone()
                    .expect("write_recurring implies Some");
                engine
                    .record_local(OpPayload::BlockPropertySet {
                        note_id: *note_id,
                        block_id: *block_id,
                        key: "recurring".to_string(),
                        value: PropOp::SetScalar(PropScalar::Text(rule)),
                    })
                    .await
                    .map_err(|e| anyhow::anyhow!("set recurring::: {e}"))?;
                report.applied_ops += 1;
            }
        }
    }

    Ok(report)
}

/// CLI entry: lock the mosaic (refuse while the server/desktop holds it),
/// open the Loro engine over its snapshots, scan the vault, match, report.
pub async fn run(mosaic: &Path, source: &Path, apply: bool) -> Result<()> {
    let _lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before running recover-logseq-dates (single-writer).",
    )?;

    let (items, conflicts) = scan_vault(source)?;
    if items.is_empty() && conflicts.is_empty() {
        println!(
            "recover-logseq-dates: no timed/repeating SCHEDULED/DEADLINE stamps found under {} — nothing to do.",
            source.display()
        );
        return Ok(());
    }

    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir.clone()))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;

    let report = recover(&engine, Some(&notes_dir), items, conflicts, apply).await?;
    print_report(&report, apply);
    Ok(())
}

fn print_report(report: &Report, apply: bool) {
    if apply {
        println!(
            "recover-logseq-dates: wrote {} property op(s) across {} block(s) ({} note(s) hydrated):",
            report.applied_ops,
            report.recovered(),
            report.hydrated_notes
        );
    } else {
        println!(
            "recover-logseq-dates (DRY RUN — re-run with --apply to write): {} block(s) would be recovered:",
            report.recovered()
        );
    }
    for r in &report.items {
        let prefix: String = r.item.text.chars().take(60).collect();
        let mut line = format!("{}:: {}", r.item.key, r.item.value);
        match &r.outcome {
            Outcome::Recovered {
                slug,
                write_recurring,
                needs_hydration,
                ..
            } => {
                if *write_recurring {
                    line.push_str(&format!(
                        " + recurring:: {}",
                        r.item.recurring.as_deref().unwrap_or("?")
                    ));
                }
                let hydrate = if *needs_hydration {
                    "  [hydrates note]"
                } else {
                    ""
                };
                println!("  RECOVER  {}:{}  + {}{}", slug, prefix, line, hydrate);
            }
            Outcome::Missing => {
                println!(
                    "  MISSING  {}:{}  ({} — no matching block; deleted or edited?)",
                    r.item.file, prefix, line
                );
            }
            Outcome::Ambiguous(n) => {
                println!(
                    "  AMBIG    {}:{}  ({} — {} blocks match; skipped, never guess)",
                    r.item.file, prefix, line, n
                );
            }
            Outcome::AlreadyPresent => {
                println!(
                    "  PRESENT  {}:{}  ({} — block already has {}::)",
                    r.item.file, prefix, line, r.item.key
                );
            }
        }
        if let Some(raw) = &r.item.unmapped_repeater {
            println!(
                "           ^ repeater {:?} not mappable to recurring:: — date recovered, recurrence skipped",
                raw
            );
        }
    }
    for c in &report.vault_conflicts {
        println!("  CONFLICT {}", c);
    }
    println!(
        "\ncounts: recovered={} ambiguous={} missing={} already-present={} vault-conflicts={}",
        report.recovered(),
        report.ambiguous(),
        report.missing(),
        report.already_present(),
        report.vault_conflicts.len()
    );
    if !apply && report.recovered() > 0 {
        println!("\nRun the same command with --apply to write (stop the server first).");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tesela_sync::DeviceId;

    // ── stamp parsing ──────────────────────────────────────────────

    #[test]
    fn stamp_with_time_parses() {
        let s = parse_stamp_line("SCHEDULED: <2026-06-12 Fri 10:00>").unwrap();
        assert_eq!(s.key, "scheduled");
        assert_eq!(s.date, "2026-06-12");
        assert_eq!(s.time.as_deref(), Some("10:00"));
        assert_eq!(s.repeater, None);
    }

    #[test]
    fn stamp_with_repeater_parses() {
        let s = parse_stamp_line("DEADLINE: <2026-06-13 Sat .+1w>").unwrap();
        assert_eq!(s.key, "deadline");
        assert_eq!(s.date, "2026-06-13");
        assert_eq!(s.time, None);
        assert_eq!(s.repeater.as_deref(), Some(".+1w"));
    }

    #[test]
    fn stamp_with_time_and_repeater_parses() {
        let s = parse_stamp_line("SCHEDULED: <2026-06-12 Fri 10:00 .+1d>").unwrap();
        assert_eq!(s.time.as_deref(), Some("10:00"));
        assert_eq!(s.repeater.as_deref(), Some(".+1d"));
    }

    #[test]
    fn plusplus_and_plain_plus_repeaters_parse() {
        let s = parse_stamp_line("SCHEDULED: <2026-06-12 Fri ++2w>").unwrap();
        assert_eq!(s.repeater.as_deref(), Some("++2w"));
        let s = parse_stamp_line("SCHEDULED: <2026-06-12 Fri +1m>").unwrap();
        assert_eq!(s.repeater.as_deref(), Some("+1m"));
    }

    #[test]
    fn date_only_stamp_parses_but_scan_excludes_it() {
        // parse_stamp_line still reads it...
        let s = parse_stamp_line("SCHEDULED: <2026-05-19 Tue>").unwrap();
        assert_eq!(s.time, None);
        assert_eq!(s.repeater, None);
        // ...but the scan skips it (the old importer handled bare dates).
        let mut out = Vec::new();
        scan_content(
            "journals/x.md",
            "- TODO water plants\n  SCHEDULED: <2026-05-19 Tue>\n",
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn non_stamp_lines_do_not_parse() {
        assert_eq!(parse_stamp_line("- TODO call dentist"), None);
        assert_eq!(parse_stamp_line("scheduled:: 2026-06-12"), None);
        assert_eq!(parse_stamp_line("SCHEDULED: someday"), None);
    }

    // ── repeater mapping ───────────────────────────────────────────

    #[test]
    fn repeater_mapping_table() {
        for (raw, expect) in [
            (".+1d", "daily"),
            (".+1w", "weekly"),
            (".+2w", "every 2 weeks"),
            ("++1m", "monthly"),
            ("+1y", "yearly"),
            (".+3d", "every 3 days"),
            (".+6m", "every 6 months"),
        ] {
            assert_eq!(map_repeater(raw).as_deref(), Some(expect), "{raw}");
        }
    }

    #[test]
    fn mapped_repeaters_are_accepted_by_recurrence_parse() {
        for raw in [".+1d", ".+1w", ".+2w", "++1m", "+1y", ".+3d", ".+10w"] {
            let rule = map_repeater(raw).unwrap();
            assert!(
                tesela_core::recurrence::parse(&rule).is_some(),
                "recurrence::parse rejected {rule:?} (from {raw})"
            );
        }
    }

    #[test]
    fn unmappable_repeaters_return_none() {
        assert_eq!(map_repeater(".+1h"), None, "hour units unsupported");
        assert_eq!(map_repeater(".+0d"), None, "zero interval");
        assert_eq!(map_repeater("1w"), None, "no repeater prefix");
        assert_eq!(map_repeater(".+w"), None, "no count");
    }

    // ── vault scan ─────────────────────────────────────────────────

    #[test]
    fn scan_normalizes_marker_and_priority_and_captures_both_keys() {
        let mut out = Vec::new();
        scan_content(
            "journals/2026_06_01.md",
            "- TODO [#A] call dentist\n  SCHEDULED: <2026-06-12 Fri 10:00 .+1w>\n  DEADLINE: <2026-06-13 Sat 09:30>\n",
            &mut out,
        );
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].text, "call dentist");
        assert_eq!(out[0].key, "scheduled");
        assert_eq!(out[0].value, "2026-06-12 10:00");
        assert_eq!(out[0].recurring.as_deref(), Some("weekly"));
        assert_eq!(out[1].key, "deadline");
        assert_eq!(out[1].value, "2026-06-13 09:30");
        assert_eq!(out[1].recurring, None);
    }

    #[test]
    fn scan_keeps_date_for_unmappable_repeater() {
        let mut out = Vec::new();
        scan_content(
            "pages/p.md",
            "- TODO standup\n  SCHEDULED: <2026-06-12 Fri 09:00 .+1h>\n",
            &mut out,
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, "2026-06-12 09:00");
        assert_eq!(out[0].recurring, None);
        assert_eq!(out[0].unmapped_repeater.as_deref(), Some(".+1h"));
    }

    #[test]
    fn scan_ignores_code_fences_and_stampless_files() {
        let mut out = Vec::new();
        scan_content(
            "pages/code.md",
            "- a snippet\n  ```\n  SCHEDULED: <2026-06-12 Fri 10:00>\n  ```\n- plain bullet\n",
            &mut out,
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn scan_vault_dedupes_identical_and_reports_conflicts() {
        let temp = TempDir::new().unwrap();
        let v = temp.path();
        fs::create_dir_all(v.join("journals")).unwrap();
        fs::create_dir_all(v.join("pages")).unwrap();
        // Identical duplicate → deduped to one item.
        fs::write(
            v.join("journals/a.md"),
            "- TODO pay rent\n  SCHEDULED: <2026-07-01 Wed 08:00>\n",
        )
        .unwrap();
        fs::write(
            v.join("journals/b.md"),
            "- TODO pay rent\n  SCHEDULED: <2026-07-01 Wed 08:00>\n",
        )
        .unwrap();
        // Conflicting values for the same (text, key) → dropped + reported.
        fs::write(
            v.join("pages/c.md"),
            "- TODO review budget\n  DEADLINE: <2026-07-02 Thu 10:00>\n",
        )
        .unwrap();
        fs::write(
            v.join("pages/d.md"),
            "- TODO review budget\n  DEADLINE: <2026-07-03 Fri 11:00>\n",
        )
        .unwrap();
        // Inside logseq/ (bak etc.) → ignored entirely.
        fs::create_dir_all(v.join("logseq/bak/pages")).unwrap();
        fs::write(
            v.join("logseq/bak/pages/stale.md"),
            "- TODO pay rent\n  SCHEDULED: <2020-01-01 Wed 01:00>\n",
        )
        .unwrap();

        let (items, conflicts) = scan_vault(v).unwrap();
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].text, "pay rent");
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].contains("review budget"), "{conflicts:?}");
    }

    // ── matcher ────────────────────────────────────────────────────

    fn engine() -> LoroEngine {
        let dev = DeviceId::from_bytes([7u8; 16]);
        LoroEngine::new(dev, Arc::new(Hlc::new(dev)))
    }

    async fn upsert(engine: &LoroEngine, note_id: [u8; 16], content: &str) {
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("t".into()),
                title: "t".into(),
                content: content.to_string(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
    }

    const NOTE_A: [u8; 16] = [0xa1; 16];
    const NOTE_B: [u8; 16] = [0xb2; 16];
    const BID1: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    const BID2: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";

    fn item(text: &str, key: &'static str, value: &str, recurring: Option<&str>) -> VaultItem {
        VaultItem {
            file: "journals/x.md".into(),
            text: text.into(),
            key,
            value: value.into(),
            recurring: recurring.map(|s| s.to_string()),
            unmapped_repeater: None,
        }
    }

    #[tokio::test]
    async fn one_match_recovers_and_dry_run_writes_nothing() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- call dentist <!-- bid:{BID1} -->\n  status:: todo\n"),
        )
        .await;

        let items = vec![item(
            "call dentist",
            "scheduled",
            "2026-06-12 10:00",
            Some("weekly"),
        )];
        let report = recover(&e, None, items.clone(), vec![], false)
            .await
            .unwrap();
        assert_eq!(report.recovered(), 1);
        assert_eq!(report.applied_ops, 0, "dry-run writes nothing");
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(!md.contains("scheduled::"), "dry-run wrote nothing: {md:?}");

        let report = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(report.applied_ops, 2, "scheduled + recurring");
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(md.contains("scheduled:: 2026-06-12 10:00"), "{md:?}");
        assert!(md.contains("recurring:: weekly"), "{md:?}");
    }

    #[tokio::test]
    async fn zero_matches_reports_missing() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- some other text <!-- bid:{BID1} -->\n"),
        )
        .await;

        let items = vec![item("call dentist", "scheduled", "2026-06-12 10:00", None)];
        let report = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(report.missing(), 1);
        assert_eq!(report.applied_ops, 0);
    }

    #[tokio::test]
    async fn multiple_matches_skip_as_ambiguous() {
        let e = engine();
        upsert(&e, NOTE_A, &format!("- call dentist <!-- bid:{BID1} -->\n")).await;
        upsert(&e, NOTE_B, &format!("- call dentist <!-- bid:{BID2} -->\n")).await;

        let items = vec![item("call dentist", "scheduled", "2026-06-12 10:00", None)];
        let report = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(report.ambiguous(), 1);
        assert_eq!(report.applied_ops, 0, "never guess");
        for note in [NOTE_A, NOTE_B] {
            let md = e.render_note_full(note).await.unwrap();
            assert!(!md.contains("scheduled::"), "{md:?}");
        }
    }

    #[tokio::test]
    async fn already_present_property_is_skipped() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- call dentist <!-- bid:{BID1} -->\n  scheduled:: 2026-06-20\n"),
        )
        .await;

        let items = vec![item("call dentist", "scheduled", "2026-06-12 10:00", None)];
        let report = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(report.already_present(), 1);
        assert_eq!(report.applied_ops, 0);
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(
            md.contains("scheduled:: 2026-06-20"),
            "hand-set value untouched: {md:?}"
        );
        assert!(!md.contains("2026-06-12"), "{md:?}");
    }

    #[tokio::test]
    async fn apply_is_idempotent() {
        let e = engine();
        upsert(&e, NOTE_A, &format!("- call dentist <!-- bid:{BID1} -->\n")).await;
        let items = vec![item(
            "call dentist",
            "scheduled",
            "2026-06-12 10:00",
            Some("weekly"),
        )];

        let first = recover(&e, None, items.clone(), vec![], true)
            .await
            .unwrap();
        assert_eq!(first.applied_ops, 2);
        let second = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(second.already_present(), 1, "second run skips");
        assert_eq!(second.applied_ops, 0);
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert_eq!(md.matches("scheduled::").count(), 1, "{md:?}");
        assert_eq!(md.matches("recurring::").count(), 1, "{md:?}");
    }

    #[tokio::test]
    async fn recurring_already_present_still_recovers_date() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- water plants <!-- bid:{BID1} -->\n  recurring:: daily\n"),
        )
        .await;

        let items = vec![item(
            "water plants",
            "scheduled",
            "2026-06-12 07:00",
            Some("weekly"),
        )];
        let report = recover(&e, None, items, vec![], true).await.unwrap();
        assert_eq!(report.recovered(), 1);
        assert_eq!(report.applied_ops, 1, "date only — recurring kept as-is");
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(md.contains("scheduled:: 2026-06-12 07:00"), "{md:?}");
        assert!(md.contains("recurring:: daily"), "user value kept: {md:?}");
        assert!(!md.contains("recurring:: weekly"), "{md:?}");
    }

    #[tokio::test]
    async fn non_resident_note_matches_from_disk_and_hydrates_on_apply() {
        // The live mosaic's Loro dir only holds notes that have flowed
        // through the engine; everything else lives as materialized
        // `notes/<slug>.md`. Matching must see those, and apply must
        // hydrate (NoteUpsert) before the property set.
        let temp = TempDir::new().unwrap();
        let notes = temp.path().join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(
            notes.join("2026-01-02.md"),
            format!(
                "---\ntitle: \"2026-01-02\"\n---\n\n- call dentist <!-- bid:{BID1} -->\n  status:: todo\n"
            ),
        )
        .unwrap();
        let e = engine();

        let items = vec![item(
            "call dentist",
            "scheduled",
            "2026-06-12 10:00",
            Some("weekly"),
        )];
        // Dry-run sees the disk block and flags hydration.
        let report = recover(&e, Some(&notes), items.clone(), vec![], false)
            .await
            .unwrap();
        assert_eq!(report.recovered(), 1);
        assert!(
            matches!(
                report.items[0].outcome,
                Outcome::Recovered {
                    needs_hydration: true,
                    ..
                }
            ),
            "disk-only note must flag hydration"
        );
        assert_eq!(report.applied_ops, 0);
        assert_eq!(report.hydrated_notes, 0);

        // Apply hydrates under the stable blake3(slug) id then sets props.
        let report = recover(&e, Some(&notes), items.clone(), vec![], true)
            .await
            .unwrap();
        assert_eq!(report.hydrated_notes, 1);
        assert_eq!(report.applied_ops, 2);
        let note_id = stable_uuid_from_slug("2026-01-02");
        let md = e.render_note_full(note_id).await.unwrap();
        assert!(md.contains("call dentist"), "content preserved: {md:?}");
        assert!(md.contains("status:: todo"), "props preserved: {md:?}");
        assert!(md.contains("scheduled:: 2026-06-12 10:00"), "{md:?}");
        assert!(md.contains("recurring:: weekly"), "{md:?}");

        // Now the note is resident — a re-run matches the ENGINE copy
        // (no double match with the disk file) and skips as present.
        let report = recover(&e, Some(&notes), items, vec![], true)
            .await
            .unwrap();
        assert_eq!(report.already_present(), 1, "idempotent");
        assert_eq!(report.ambiguous(), 0, "no resident+disk double match");
        assert_eq!(report.applied_ops, 0);
    }

    // ── end-to-end smoke: temp mosaic + temp vault, full run() ─────

    #[tokio::test]
    async fn e2e_run_against_temp_mosaic_and_vault() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("mosaic");
        let vault = temp.path().join("vault");
        fs::create_dir_all(vault.join("journals")).unwrap();
        fs::write(
            vault.join("journals/2026_06_01.md"),
            "- TODO call dentist\n  SCHEDULED: <2026-06-12 Fri 10:00 .+1w>\n\
             - TODO water plants\n  SCHEDULED: <2026-05-19 Tue>\n",
        )
        .unwrap();

        // Seed the mosaic the way the OLD importer would have left it:
        // marker stripped to status::, no scheduled:: property.
        let dev = DeviceId::from_bytes([9u8; 16]);
        {
            let e = LoroEngine::with_dirs(
                dev,
                Arc::new(Hlc::new(dev)),
                mosaic.join(".tesela").join("loro"),
                Some(mosaic.join("notes")),
            )
            .await
            .unwrap();
            upsert(
                &e,
                NOTE_A,
                &format!("- call dentist <!-- bid:{BID1} -->\n  status:: todo\n  tags:: Task\n"),
            )
            .await;
        }

        // Dry-run writes nothing.
        run(&mosaic, &vault, false).await.unwrap();
        {
            let e = LoroEngine::with_dirs(
                dev,
                Arc::new(Hlc::new(dev)),
                mosaic.join(".tesela").join("loro"),
                Some(mosaic.join("notes")),
            )
            .await
            .unwrap();
            let md = e.render_note_full(NOTE_A).await.unwrap();
            assert!(!md.contains("scheduled::"), "dry-run wrote nothing: {md:?}");
        }

        // Apply writes the property + recurring; re-open and verify.
        run(&mosaic, &vault, true).await.unwrap();
        let e = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            mosaic.join(".tesela").join("loro"),
            Some(mosaic.join("notes")),
        )
        .await
        .unwrap();
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(md.contains("scheduled:: 2026-06-12 10:00"), "{md:?}");
        assert!(md.contains("recurring:: weekly"), "{md:?}");

        // Second apply run is a no-op (idempotent through the CLI path too).
        run(&mosaic, &vault, true).await.unwrap();
        let e2 = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            mosaic.join(".tesela").join("loro"),
            Some(mosaic.join("notes")),
        )
        .await
        .unwrap();
        let md = e2.render_note_full(NOTE_A).await.unwrap();
        assert_eq!(md.matches("scheduled::").count(), 1, "{md:?}");
    }
}
