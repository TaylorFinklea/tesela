use crate::regex_cache::{LOGSEQ_DATE_RE, PRIORITY_RE};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SOURCE_PATH_KEY: &str = "source_logseq_path";
const SOURCE_SHA_KEY: &str = "source_logseq_sha";

const PREVIEW_CHARS: usize = 600;

/// Top-level plan returned by `build_plan`. Carries everything `apply_plan`
/// needs to write the imported files (rendered content baked into each
/// item) plus enough metadata for a UI to show a preview + decide what
/// to do with conflicts.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportPlan {
    pub items: Vec<PlanItem>,
    pub source: String,
    pub mosaic: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlanItem {
    /// Relative path inside the source graph (e.g. `journals/2026_05_10.md`).
    pub source_rel: String,
    /// SHA-256 of the raw source file content (or empty for HardSkip).
    pub source_sha: String,
    /// Tesela note id (filename stem inside `notes/`).
    pub target_id: String,
    /// Absolute target path in the mosaic.
    pub target_path: String,
    /// What would happen for this item.
    pub kind: PlanKind,
    /// Reason text (populated for conflicts / hard skips).
    #[serde(default)]
    pub reason: Option<String>,
    /// First N chars of the rendered Tesela markdown (source converted).
    /// None for HardSkip / Unchanged (no rewrite would happen anyway).
    #[serde(default)]
    pub rendered_preview: Option<String>,
    /// First N chars of the *existing* target on disk, when one exists.
    #[serde(default)]
    pub existing_preview: Option<String>,
    /// SHA stored in the existing target's frontmatter (if any).
    #[serde(default)]
    pub existing_sha: Option<String>,
    /// Full rendered content (used by apply_plan). Kept on the wire so
    /// the apply phase doesn't have to re-walk + re-convert.
    #[serde(default)]
    pub rendered_full: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    /// Target doesn't exist yet — straightforward import.
    NewImport,
    /// Target exists and was produced by us with the same SHA — silent
    /// no-op on apply.
    Unchanged,
    /// Target exists with a *different* SHA than this importer last
    /// wrote — user has edits OR the upstream changed. Resolvable.
    ConflictDiffSha,
    /// Target exists with no source-tracking frontmatter (manually
    /// authored by user). Resolvable.
    ConflictForeign,
    /// Source file is structurally unimportable (whiteboard, etc.).
    /// Informational only — cannot resolve.
    HardSkip,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// Don't touch the target.
    #[default]
    Skip,
    /// Replace the target with the imported content.
    Overwrite,
    /// Write to a sibling path with the given suffix appended to the
    /// note id, e.g. suffix="-imported" → `<id>-imported.md`.
    Rename { suffix: String },
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ApplyDecisions {
    /// Per-source-rel override. Keys must match `PlanItem.source_rel`.
    #[serde(default)]
    pub per_item: HashMap<String, Decision>,
    /// Default decision applied to any conflict item not in `per_item`.
    #[serde(default)]
    pub default: Decision,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ApplyOutcome {
    pub imported: usize,
    pub overwritten: usize,
    pub renamed: usize,
    pub skipped: usize,
    pub unchanged: usize,
    pub assets_copied: usize,
    pub errors: Vec<String>,
}

// ──────────────────────────────────────────────────────────────────────
// Public entry points
// ──────────────────────────────────────────────────────────────────────

/// Backward-compat: walk the source, plan, and apply with the default
/// (Skip-conflicts) policy. Matches the pre-refactor behavior so the
/// CLI's existing `tesela import-logseq` keeps working unchanged.
pub async fn run(mosaic: &Path, source: PathBuf, dry_run: bool) -> Result<()> {
    let plan = build_plan(&source, mosaic).context("plan logseq import")?;

    // Summary print mirroring the old output format.
    let counts = summarize(&plan);
    if dry_run {
        println!("Dry run complete:");
    } else {
        println!("Import complete:");
    }
    println!("  Would import: {}", counts.new_imports);
    println!("  Unchanged (idempotent): {}", counts.unchanged);
    println!("  Conflicts: {}", counts.conflicts);
    println!("  Hard-skipped: {}", counts.hard_skips);

    if dry_run {
        return Ok(());
    }

    let outcome =
        apply_plan(&plan, &ApplyDecisions::default(), mosaic).context("apply logseq import")?;
    println!("  Imported: {}", outcome.imported);
    println!("  Overwritten: {}", outcome.overwritten);
    println!("  Renamed: {}", outcome.renamed);
    println!("  Skipped: {}", outcome.skipped);
    println!("  Assets copied: {}", outcome.assets_copied);
    if !outcome.errors.is_empty() {
        println!("  Errors: {}", outcome.errors.len());
        for e in &outcome.errors {
            println!("    {}", e);
        }
    }
    println!("\nRestart tesela-server to index imported notes.");
    Ok(())
}

#[derive(Debug, Default)]
pub struct PlanCounts {
    pub new_imports: usize,
    pub unchanged: usize,
    pub conflicts: usize,
    pub hard_skips: usize,
}

pub fn summarize(plan: &ImportPlan) -> PlanCounts {
    let mut c = PlanCounts::default();
    for item in &plan.items {
        match item.kind {
            PlanKind::NewImport => c.new_imports += 1,
            PlanKind::Unchanged => c.unchanged += 1,
            PlanKind::ConflictDiffSha | PlanKind::ConflictForeign => c.conflicts += 1,
            PlanKind::HardSkip => c.hard_skips += 1,
        }
    }
    c
}

// ──────────────────────────────────────────────────────────────────────
// Plan
// ──────────────────────────────────────────────────────────────────────

pub fn build_plan(source: &Path, mosaic: &Path) -> Result<ImportPlan> {
    if !source.exists() {
        anyhow::bail!("LogSeq graph not found: {}", source.display());
    }
    let notes_dir = mosaic.join("notes");
    let _ = std::fs::create_dir_all(&notes_dir);

    let mut items: Vec<PlanItem> = Vec::new();

    // Journals: YYYY_MM_DD.md → notes/YYYY-MM-DD.md as daily notes.
    let journals_dir = source.join("journals");
    if journals_dir.exists() {
        for entry in std::fs::read_dir(&journals_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            let stem = name.trim_end_matches(".md");
            let date_id = stem.replace('_', "-");
            if date_id.len() != 10 || date_id.chars().filter(|c| *c == '-').count() != 2 {
                continue;
            }
            let target_path = notes_dir.join(format!("{}.md", date_id));
            let source_path = entry.path();
            let rel_str = format!("journals/{}", name);
            let Some(item) = plan_one(
                &source_path,
                &target_path,
                &date_id,
                &rel_str,
                |raw, sha| {
                    let converted = convert_content(raw);
                    format!(
                        "---\ntitle: \"{}\"\ntags: [\"daily\"]\ncreated: {}T00:00:00Z\n{}: \"{}\"\n{}: \"{}\"\n---\n{}",
                        date_id,
                        date_id,
                        SOURCE_PATH_KEY,
                        rel_str,
                        SOURCE_SHA_KEY,
                        sha,
                        converted
                    )
                },
            )?
            else {
                continue;
            };
            items.push(item);
        }
    }

    // Pages: Logseq's `Foo___Bar.md` → namespaced page (parent slug → tag).
    let pages_dir = source.join("pages");
    if pages_dir.exists() {
        for entry in std::fs::read_dir(&pages_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            let stem = name.trim_end_matches(".md");
            let clean_name = stem
                .replace("___", "/")
                .replace("%3A", ":")
                .replace("%2F", "/");
            let safe_name = clean_name.replace(['/', ':', ' '], "-").to_lowercase();
            if safe_name == "contents" || safe_name == "favorites" {
                continue;
            }
            let target_path = notes_dir.join(format!("{}.md", safe_name));
            let source_path = entry.path();
            let rel_str = format!("pages/{}", name);

            let namespace_tags: Vec<String> = clean_name
                .split('/')
                .take(clean_name.matches('/').count())
                .map(|s| s.replace([' ', ':'], "-").to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let title = clean_name
                .split('/')
                .next_back()
                .unwrap_or(&clean_name)
                .to_string();
            let safe_name_for_id = safe_name.clone();
            let Some(item) = plan_one(
                &source_path,
                &target_path,
                &safe_name_for_id,
                &rel_str,
                |raw, sha| {
                    let converted = convert_content(raw);
                    let tags = if namespace_tags.is_empty() {
                        "[]".to_string()
                    } else {
                        format!(
                            "[{}]",
                            namespace_tags
                                .iter()
                                .map(|t| format!("\"{}\"", t))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    format!(
                        "---\ntitle: \"{}\"\ntags: {}\n{}: \"{}\"\n{}: \"{}\"\n---\n{}",
                        title, tags, SOURCE_PATH_KEY, rel_str, SOURCE_SHA_KEY, sha, converted
                    )
                },
            )?
            else {
                continue;
            };
            items.push(item);
        }
    }

    // Whiteboards are structurally unimportable — surface them as
    // HardSkip plan items so the UI can show them under "Won't import".
    let whiteboards = source.join("whiteboards");
    if whiteboards.exists() {
        for entry in std::fs::read_dir(&whiteboards)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            items.push(PlanItem {
                source_rel: format!("whiteboards/{}", name),
                source_sha: String::new(),
                target_id: String::new(),
                target_path: String::new(),
                kind: PlanKind::HardSkip,
                reason: Some("Logseq whiteboards have no Tesela equivalent".to_string()),
                rendered_preview: None,
                existing_preview: None,
                existing_sha: None,
                rendered_full: None,
            });
        }
    }

    Ok(ImportPlan {
        items,
        source: source.to_string_lossy().into_owned(),
        mosaic: mosaic.to_string_lossy().into_owned(),
    })
}

fn plan_one(
    source_path: &Path,
    target_path: &Path,
    target_id: &str,
    source_rel: &str,
    build_full: impl FnOnce(&str, &str) -> String,
) -> Result<Option<PlanItem>> {
    let raw = match std::fs::read_to_string(source_path) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let sha = sha256_hex(&raw);
    let rendered = build_full(&raw, &sha);
    let rendered_preview = Some(truncate(&rendered, PREVIEW_CHARS));

    let (kind, reason, existing_preview, existing_sha) = if target_path.exists() {
        let existing = std::fs::read_to_string(target_path).unwrap_or_default();
        let prev_sha = extract_frontmatter_value(&existing, SOURCE_SHA_KEY);
        let preview = Some(truncate(&existing, PREVIEW_CHARS));
        match prev_sha {
            Some(p) if p == sha => (PlanKind::Unchanged, None, preview, Some(p)),
            Some(p) => (
                PlanKind::ConflictDiffSha,
                Some(format!(
                    "Target was produced by this importer earlier but its SHA changed (existing: {p}, source: {sha})."
                )),
                preview,
                Some(p),
            ),
            None => (
                PlanKind::ConflictForeign,
                Some(
                    "Target already exists and was not produced by this importer."
                        .to_string(),
                ),
                preview,
                None,
            ),
        }
    } else {
        (PlanKind::NewImport, None, None, None)
    };

    Ok(Some(PlanItem {
        source_rel: source_rel.to_string(),
        source_sha: sha,
        target_id: target_id.to_string(),
        target_path: target_path.to_string_lossy().into_owned(),
        kind,
        reason,
        rendered_preview,
        existing_preview,
        existing_sha,
        rendered_full: Some(rendered),
    }))
}

// ──────────────────────────────────────────────────────────────────────
// Apply
// ──────────────────────────────────────────────────────────────────────

pub fn apply_plan(
    plan: &ImportPlan,
    decisions: &ApplyDecisions,
    mosaic: &Path,
) -> Result<ApplyOutcome> {
    let mut outcome = ApplyOutcome::default();

    for item in &plan.items {
        let decision = decisions
            .per_item
            .get(&item.source_rel)
            .cloned()
            .unwrap_or_else(|| decisions.default.clone());
        let target_path = PathBuf::from(&item.target_path);

        match item.kind {
            PlanKind::HardSkip => {
                outcome.skipped += 1;
            }
            PlanKind::Unchanged => {
                outcome.unchanged += 1;
            }
            PlanKind::NewImport => {
                // New imports always proceed unless the user explicitly
                // chose Skip in the per-item map.
                let explicit_skip = matches!(
                    decisions.per_item.get(&item.source_rel),
                    Some(Decision::Skip)
                );
                if explicit_skip {
                    outcome.skipped += 1;
                    continue;
                }
                if let Some(parent) = target_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let content = item.rendered_full.as_deref().unwrap_or_default();
                match std::fs::write(&target_path, content) {
                    Ok(_) => outcome.imported += 1,
                    Err(e) => {
                        outcome
                            .errors
                            .push(format!("write {}: {}", target_path.display(), e))
                    }
                }
            }
            PlanKind::ConflictDiffSha | PlanKind::ConflictForeign => {
                let content = item.rendered_full.as_deref().unwrap_or_default();
                match decision {
                    Decision::Skip => outcome.skipped += 1,
                    Decision::Overwrite => match std::fs::write(&target_path, content) {
                        Ok(_) => outcome.overwritten += 1,
                        Err(e) => outcome.errors.push(format!(
                            "overwrite {}: {}",
                            target_path.display(),
                            e
                        )),
                    },
                    Decision::Rename { suffix } => {
                        let renamed = target_path.with_file_name(format!(
                            "{}{}.md",
                            item.target_id,
                            sanitize_suffix(&suffix)
                        ));
                        if renamed.exists() {
                            outcome.errors.push(format!(
                                "rename target {} already exists",
                                renamed.display()
                            ));
                            continue;
                        }
                        if let Some(parent) = renamed.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        match std::fs::write(&renamed, content) {
                            Ok(_) => outcome.renamed += 1,
                            Err(e) => outcome.errors.push(format!(
                                "rename write {}: {}",
                                renamed.display(),
                                e
                            )),
                        }
                    }
                }
            }
        }
    }

    // Assets are still copied wholesale on apply — they're not part of
    // the conflict-resolution surface (target file collisions on
    // attachments are vanishingly rare and skipping already covers the
    // existing-file case).
    let assets_src = PathBuf::from(&plan.source).join("assets");
    if assets_src.exists() {
        let attach_dst = mosaic.join("attachments");
        let _ = std::fs::create_dir_all(&attach_dst);
        for entry in walkdir::WalkDir::new(&assets_src).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = match entry.path().strip_prefix(&assets_src) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let dst = attach_dst.join(rel);
            if dst.exists() {
                continue;
            }
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(e) = std::fs::copy(entry.path(), &dst) {
                outcome
                    .errors
                    .push(format!("copy asset {}: {}", entry.path().display(), e));
                continue;
            }
            outcome.assets_copied += 1;
        }
    }

    Ok(outcome)
}

fn sanitize_suffix(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "-imported".to_string();
    }
    let mut out = String::with_capacity(s.len() + 1);
    if !s.starts_with('-') {
        out.push('-');
    }
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            out.push(c.to_ascii_lowercase());
        }
    }
    if out == "-" {
        "-imported".to_string()
    } else {
        out
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = s[..end].to_string();
        out.push('…');
        out
    }
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// Extract a YAML scalar value from a markdown file's frontmatter.
/// Quick enough for our two source-tracking keys.
fn extract_frontmatter_value(content: &str, key: &str) -> Option<String> {
    let after_open = if content.starts_with("---\n") {
        4
    } else if content.starts_with("---\r\n") {
        5
    } else {
        return None;
    };
    let rest = &content[after_open..];
    for line in rest.lines() {
        if line.trim_end_matches('\r') == "---" {
            return None;
        }
        let trimmed = line.trim_start();
        if let Some((k, v)) = trimmed.split_once(':') {
            if k.trim() == key {
                return Some(v.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Convert LogSeq markdown content to Tesela format.
fn convert_content(content: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut in_query = false;
    // Track triple-backtick fences so we don't convert markdown that's
    // inside a code block. Logseq nests its `#+BEGIN_QUERY` blocks via
    // a leading `- `, so we strip that bullet from the marker check.
    let mut in_code_block = false;
    // After converting a task marker we owe the block a `tags:: Task`
    // continuation line (the same materialized form the engine emits for
    // a structured tag-add — see `tesela-cli`'s backfill-task). Held
    // pending so it can union into an existing `tags::` continuation
    // line instead of duplicating it; flushed when the block's property
    // region ends. Carries the block's property indent.
    let mut pending_task_tag: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        let leading = line.len() - line.trim_start().len();
        let indent_str = &line[..leading];

        if let Some(task_indent) = pending_task_tag.take() {
            if !trimmed.starts_with("- ")
                && property_line_key(trimmed).is_some_and(|k| k.eq_ignore_ascii_case("tags"))
            {
                // The task block already has a tags continuation line —
                // union Task into it (idempotent if already present).
                lines.push(merge_task_into_tags_line(line).replace('\t', "  "));
                continue;
            }
            if is_block_property_continuation(trimmed) {
                pending_task_tag = Some(task_indent);
            } else {
                lines.push(format!("{}tags:: Task", task_indent));
            }
        }

        // Toggle triple-backtick code fences. Inside a fence we
        // pass content through verbatim — task/block-ref/asset URL
        // conversions are syntactic so they must not touch user code.
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            lines.push(line.to_string());
            continue;
        }
        if in_code_block {
            lines.push(line.to_string());
            continue;
        }

        // Logseq queries — preserve the datalog inside a fenced
        // ```query block so the content is still visible. Logseq
        // wraps `#+BEGIN_QUERY` lines with `- ` when the query is a
        // block child, so we accept both forms.
        let query_marker = trimmed.trim_start_matches("- ");
        if query_marker.starts_with("#+BEGIN_QUERY") {
            in_query = true;
            lines.push(format!("{}```query", indent_str));
            continue;
        }
        if in_query {
            if query_marker.starts_with("#+END_QUERY") {
                in_query = false;
                lines.push(format!("{}```", indent_str));
                continue;
            }
            lines.push(line.to_string());
            continue;
        }

        // Skip Logseq-specific metadata properties that have no
        // Tesela equivalent. Other properties (status:: etc.) pass
        // through. We strip a leading `- ` so both bullet-form and
        // bare-form properties match (Logseq uses both).
        let prop_check = trimmed.trim_start_matches("- ");
        if prop_check.starts_with("collapsed:: ")
            || prop_check.starts_with("id:: ")
            || prop_check.starts_with("file:: ")
            || prop_check.starts_with("file-path:: ")
        {
            continue;
        }

        // Convert task markers
        if let Some((status, rest_text)) = strip_task_marker(trimmed) {
            let (priority, clean_text) = extract_priority(&rest_text);
            let prop_indent = format!("{}  ", indent_str);

            lines.push(format!("{}{}", indent_str, clean_text));
            lines.push(format!("{}status:: {}", prop_indent, status));
            if let Some(p) = priority {
                lines.push(format!("{}priority:: {}", prop_indent, p));
            }
            // Tag the block Task so task surfaces (Tasks widget, agenda)
            // actually see it — `status::` alone is invisible to them.
            // Skip when the text already carries an inline #Task.
            pending_task_tag = if has_inline_task_tag(&clean_text) {
                None
            } else {
                Some(prop_indent)
            };
            continue;
        }

        let mut result = line.to_string();

        // Convert DEADLINE/SCHEDULED
        if trimmed.starts_with("DEADLINE:") || trimmed.starts_with("SCHEDULED:") {
            let key = if trimmed.starts_with("DEADLINE") {
                "deadline"
            } else {
                "scheduled"
            };
            if let Some(caps) = LOGSEQ_DATE_RE.captures(trimmed) {
                // LOGSEQ_DATE_RE is `<(\d{4}-\d{2}-\d{2})...>` — group 1 is the
                // date and is always present when the regex matches.
                let date = caps
                    .get(1)
                    .expect("LOGSEQ_DATE_RE always has group 1 when it matches")
                    .as_str();
                // Keep an HH:MM time when present (the agenda parses
                // "YYYY-MM-DD HH:MM"); repeaters etc. are dropped.
                result = match caps.get(2) {
                    Some(t) => format!("{}{}:: {} {}", indent_str, key, date, t.as_str()),
                    None => format!("{}{}:: {}", indent_str, key, date),
                };
            }
        }

        // Preserve block refs `((uuid))` literally rather than
        // collapsing to `[ref]`. The uuid is the only handle the user
        // has to recover the link target in their original graph if
        // Tesela's link resolver doesn't grok the format yet.
        // (No-op: we used to do `BLOCK_REF_RE.replace_all(_, "[ref]")`.)

        // Rewrite asset URLs from Logseq's `../assets/` convention to
        // Tesela's `../attachments/` so imported pages can find their
        // referenced images / PDFs (the files themselves are copied in
        // `apply_plan`).
        result = result.replace("../assets/", "../attachments/");

        // Convert tab indentation to 2-space
        result = result.replace('\t', "  ");

        lines.push(result);
    }

    if let Some(task_indent) = pending_task_tag {
        lines.push(format!("{}tags:: Task", task_indent));
    }

    lines.join("\n")
}

/// Key of a `key:: value` block-property line (identifier-ish key — Logseq
/// allows dashes, e.g. `file-path`), or `None`.
fn property_line_key(trimmed: &str) -> Option<&str> {
    let (key, value) = trimmed.split_once(":: ")?;
    if value.is_empty() {
        return None;
    }
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return None;
    }
    Some(key)
}

/// True if the line is still part of the current block's property region
/// (a non-bullet `key:: value` line or a SCHEDULED/DEADLINE planning line)
/// — i.e. a pending `tags:: Task` must wait, not interleave.
fn is_block_property_continuation(trimmed: &str) -> bool {
    if trimmed.starts_with("- ") {
        return false;
    }
    trimmed.starts_with("SCHEDULED:")
        || trimmed.starts_with("DEADLINE:")
        || property_line_key(trimmed).is_some()
}

/// Union `Task` into an existing `tags:: a, b` line (no-op if any
/// comma-separated value already equals Task, case-insensitively).
fn merge_task_into_tags_line(line: &str) -> String {
    let already = line
        .split_once(":: ")
        .map(|(_, v)| v.split(',').any(|t| t.trim().eq_ignore_ascii_case("task")))
        .unwrap_or(false);
    if already {
        line.to_string()
    } else {
        format!("{}, Task", line.trim_end())
    }
}

/// Whole-token inline `#Task` (case-insensitive) — matches the detection
/// the backfill-task migration uses, so neither path double-tags.
fn has_inline_task_tag(text: &str) -> bool {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
        .any(|tok| tok.eq_ignore_ascii_case("#Task"))
}

fn strip_task_marker(line: &str) -> Option<(String, String)> {
    let (without_dash, prefix) = if let Some(stripped) = line.strip_prefix("- ") {
        (stripped, "- ")
    } else {
        (line, "")
    };

    // Status values must come from the seed Status property's choices
    // (backlog/todo/doing/in-review/done/canceled — `status.md` seed in
    // tesela-server's ensure_seed pages). Markers cover Logseq's full
    // built-in workflow set.
    for (marker, status) in [
        ("TODO ", "todo"),
        ("DOING ", "doing"),
        ("IN-PROGRESS ", "doing"),
        ("DONE ", "done"),
        ("LATER ", "backlog"),
        ("NOW ", "doing"),
        ("WAITING ", "backlog"),
        ("WAIT ", "backlog"),
        ("CANCELED ", "canceled"),
        ("CANCELLED ", "canceled"),
    ] {
        if let Some(rest) = without_dash.strip_prefix(marker) {
            return Some((status.to_string(), format!("{}{}", prefix, rest)));
        }
    }
    None
}

fn extract_priority(text: &str) -> (Option<String>, String) {
    if let Some(caps) = PRIORITY_RE.captures(text) {
        // PRIORITY_RE is `\[#([ABC])\]\s*` — group 1 is the priority
        // letter and is always present when the regex matches.
        let priority = match caps
            .get(1)
            .expect("PRIORITY_RE always has group 1 when it matches")
            .as_str()
        {
            "A" => "high",
            "B" => "medium",
            "C" => "low",
            _ => "medium",
        };
        let clean = PRIORITY_RE.replace(text, "").to_string();
        (Some(priority.to_string()), clean)
    } else {
        (None, text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_graph(root: &Path) {
        fs::create_dir_all(root.join("journals")).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("whiteboards")).unwrap();

        fs::write(
            root.join("journals/2026_05_10.md"),
            "- TODO Write tests\n- DONE Eat lunch\n",
        )
        .unwrap();
        fs::write(root.join("pages/Foo.md"), "title:: Foo\n- regular page\n").unwrap();
        fs::write(root.join("pages/Parent___Child.md"), "- nested page\n").unwrap();
        fs::write(root.join("assets/thing.png"), b"\x89PNG\r\n").unwrap();
        fs::write(root.join("whiteboards/board.edn"), "{}").unwrap();
    }

    #[tokio::test]
    async fn import_handles_namespaces_and_assets() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_graph(&graph);

        run(&mosaic, graph.clone(), false).await.unwrap();

        assert!(mosaic.join("notes/2026-05-10.md").exists());
        let nested = fs::read_to_string(mosaic.join("notes/parent-child.md")).unwrap();
        assert!(nested.contains("\"parent\""));
        assert!(mosaic.join("attachments/thing.png").exists());
    }

    #[tokio::test]
    async fn re_import_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_graph(&graph);

        run(&mosaic, graph.clone(), false).await.unwrap();
        let mtime1 = fs::metadata(mosaic.join("notes/2026-05-10.md"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        run(&mosaic, graph.clone(), false).await.unwrap();
        let mtime2 = fs::metadata(mosaic.join("notes/2026-05-10.md"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime1, mtime2);
    }

    #[test]
    fn plan_identifies_conflicts() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        let notes = mosaic.join("notes");
        fs::create_dir_all(&notes).unwrap();
        make_graph(&graph);

        // Pre-seed a foreign target so we get a ConflictForeign item.
        fs::write(notes.join("foo.md"), "manual user note\n").unwrap();

        let plan = build_plan(&graph, &mosaic).unwrap();
        let counts = summarize(&plan);
        assert!(
            counts.new_imports >= 2,
            "expected new imports: {:?}",
            counts
        );
        assert!(counts.conflicts >= 1, "expected ≥1 conflict: {:?}", counts);
        assert!(counts.hard_skips >= 1, "expected hard skip: {:?}", counts);

        let foreign = plan
            .items
            .iter()
            .find(|i| i.target_id == "foo")
            .expect("foo plan item");
        assert!(matches!(foreign.kind, PlanKind::ConflictForeign));
        assert!(foreign.existing_preview.is_some());
    }

    /// Comprehensive feature fixture — exercises every Logseq construct
    /// observed in the user's real graph, plus a few we want to be sure
    /// don't regress. Used by `feature_coverage_audit` below to assert
    /// the importer preserves each one. Add a new feature to this
    /// fixture and a corresponding `assert!` below when you add support
    /// for the next thing.
    fn make_coverage_graph(root: &Path) {
        fs::create_dir_all(root.join("journals")).unwrap();
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();

        // One page that hits every text-level feature in one file.
        let body = "\
title:: Coverage
tags:: idea, project
- TODO [#A] Write tests
  SCHEDULED: <2026-05-19 Tue>
  DEADLINE: <2026-05-21 Thu>
- DOING Implementing
- DONE Eat lunch
- LATER Sleep on it
- CANCELED Bad plan
- Plain note with a [[Wikilink]] and a #hashtag
- Reference to another block: ((675f6317-aaa6-4301-8ebb-df2b414dec4c))
- External link: [Apple](https://apple.com)
- An image: ![diagram](../assets/diagram.png)
- A code block follows; nothing inside should be touched:
  ```rust
  fn TODO_should_not_match() {}
  let s = \"((not-a-block-ref))\";
  ```
- ![pdf](../assets/notes.pdf)
- #+BEGIN_QUERY
  {:title \"Today's Deadline\"
   :query [:find (pull ?b [*])
           :in $ ?today
           :where ...]}
  #+END_QUERY
- collapsed:: true
  id:: 11111111-2222-3333-4444-555555555555
  file:: /should/be/stripped
- Multi-paragraph block.
  Continuation line stays attached.
";
        fs::write(root.join("pages/Coverage.md"), body).unwrap();
        fs::write(
            root.join("pages/Parent___Child.md"),
            "- Nested namespace page\n  - Links back to [[Coverage]]\n",
        )
        .unwrap();
        fs::write(
            root.join("journals/2026_05_19.md"),
            "- Daily journal entry\n- TODO Daily task\n",
        )
        .unwrap();
        fs::write(root.join("assets/diagram.png"), b"\x89PNG\r\nfake").unwrap();
        fs::write(root.join("assets/notes.pdf"), b"%PDF-fake").unwrap();
    }

    #[tokio::test]
    async fn feature_coverage_audit() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_coverage_graph(&graph);

        run(&mosaic, graph.clone(), false).await.unwrap();

        let coverage = fs::read_to_string(mosaic.join("notes/coverage.md")).unwrap();

        // ── Tasks — all five states observed in the user's vault ──
        assert!(
            coverage.contains("status:: todo"),
            "missing TODO state\n{}",
            coverage
        );
        assert!(coverage.contains("status:: doing"), "missing DOING state");
        assert!(coverage.contains("status:: done"), "missing DONE state");
        assert!(
            coverage.contains("status:: backlog"),
            "missing LATER → backlog"
        );
        assert!(
            coverage.contains("status:: canceled"),
            "missing CANCELED state"
        );

        // ── Every converted task is a REAL task: tagged `Task` so the
        //    Tasks widget / agenda surfaces see it ──
        assert_eq!(
            coverage.matches("tags:: Task").count(),
            5,
            "all five task markers tagged Task\n{}",
            coverage
        );
        let journal = fs::read_to_string(mosaic.join("notes/2026-05-19.md")).unwrap();
        assert!(
            journal.contains("tags:: Task"),
            "journal task tagged Task\n{}",
            journal
        );

        // ── Priority ──
        assert!(
            coverage.contains("priority:: high"),
            "missing [#A] → priority high"
        );

        // ── DEADLINE / SCHEDULED ──
        assert!(
            coverage.contains("scheduled:: 2026-05-19"),
            "missing SCHEDULED conversion"
        );
        assert!(
            coverage.contains("deadline:: 2026-05-21"),
            "missing DEADLINE conversion"
        );

        // ── Wikilinks + hashtags + external links pass through ──
        assert!(coverage.contains("[[Wikilink]]"), "wikilink mangled");
        assert!(coverage.contains("#hashtag"), "hashtag mangled");
        assert!(
            coverage.contains("[Apple](https://apple.com)"),
            "external link mangled"
        );

        // ── Block refs preserve the uuid (was lossy: replaced with `[ref]`) ──
        assert!(
            coverage.contains("((675f6317-aaa6-4301-8ebb-df2b414dec4c))"),
            "block ref uuid lost\n{}",
            coverage
        );

        // ── Asset URL rewritten from ../assets/ to ../attachments/ ──
        assert!(
            coverage.contains("![diagram](../attachments/diagram.png)"),
            "asset URL not rewritten\n{}",
            coverage
        );
        assert!(
            coverage.contains("![pdf](../attachments/notes.pdf)"),
            "pdf URL not rewritten"
        );

        // ── Queries preserved (as a code block — content stays visible) ──
        assert!(
            coverage.contains("```query") || coverage.contains("BEGIN_QUERY"),
            "query block dropped entirely\n{}",
            coverage
        );
        assert!(
            coverage.contains("Today's Deadline"),
            "query content lost\n{}",
            coverage
        );

        // ── Code-block fence respected — content inside is NOT
        //    converted. The fake TODO/block-ref inside ```rust``` must
        //    survive verbatim. ──
        assert!(
            coverage.contains("fn TODO_should_not_match()"),
            "code block mangled by task conversion"
        );
        assert!(
            coverage.contains("\"((not-a-block-ref))\""),
            "code block mangled by block-ref conversion"
        );

        // ── Logseq-specific metadata properties stripped ──
        assert!(
            !coverage.contains("collapsed::"),
            "collapsed should be stripped"
        );
        assert!(!coverage.contains("id::"), "id should be stripped");
        assert!(!coverage.contains("file::"), "file should be stripped");

        // ── Assets copied + attachments dir exists ──
        assert!(
            mosaic.join("attachments/diagram.png").exists(),
            "diagram not copied"
        );
        assert!(
            mosaic.join("attachments/notes.pdf").exists(),
            "pdf not copied"
        );

        // ── Namespace flattening ──
        let nested = fs::read_to_string(mosaic.join("notes/parent-child.md")).unwrap();
        assert!(
            nested.contains("Nested namespace"),
            "namespace page body lost"
        );

        // ── Journal renamed to ISO date ──
        assert!(
            mosaic.join("notes/2026-05-19.md").exists(),
            "journal not renamed to ISO"
        );
    }

    // ── Task tag — converted markers must produce REAL Tesela tasks.
    // Every task surface (Tasks widget `kind:block tag:Task -status:done`,
    // agenda) filters on the Task tag, so `status::` alone is invisible. ──

    #[test]
    fn task_marker_gets_status_and_task_tag() {
        let out = convert_content("- TODO buy milk\n");
        assert_eq!(out, "- buy milk\n  status:: todo\n  tags:: Task");
    }

    #[test]
    fn task_tag_unions_into_existing_tags_continuation() {
        let out = convert_content("- TODO buy milk\n  tags:: errand\n");
        assert_eq!(out.matches("tags::").count(), 1, "{out}");
        assert!(out.contains("tags:: errand, Task"), "{out}");
    }

    #[test]
    fn task_tag_not_duplicated_when_already_tagged() {
        let out = convert_content("- TODO buy milk\n  tags:: Task\n");
        assert_eq!(out.matches("Task").count(), 1, "{out}");
        let out = convert_content("- TODO buy milk #Task\n");
        assert!(
            !out.contains("tags::"),
            "inline #Task already marks it: {out}"
        );
    }

    #[test]
    fn consecutive_tasks_each_get_their_tag() {
        let out = convert_content("- TODO first\n- DONE second\n");
        assert_eq!(out.matches("tags:: Task").count(), 2, "{out}");
        // The first task's tag lands before the second bullet.
        let first_tag = out.find("tags:: Task").unwrap();
        let second_bullet = out.find("- second").unwrap();
        assert!(first_tag < second_bullet, "{out}");
    }

    #[test]
    fn scheduled_with_time_and_repeater_still_converts() {
        let out = convert_content(
            "- TODO call dentist\n  SCHEDULED: <2026-06-12 Fri 10:00 .+1w>\n  DEADLINE: <2026-06-13 Sat>\n",
        );
        assert!(out.contains("scheduled:: 2026-06-12 10:00"), "{out}");
        assert!(out.contains("deadline:: 2026-06-13"), "{out}");
        assert!(out.contains("tags:: Task"), "{out}");
    }

    #[test]
    fn extended_logseq_markers_map_to_seed_statuses() {
        // Canonical status choices: backlog/todo/doing/in-review/done/
        // canceled (status.md seed in crates/tesela-server/src/main.rs).
        for (line, want) in [
            ("- WAIT for review\n", "status:: backlog"),
            ("- IN-PROGRESS shipping\n", "status:: doing"),
            ("- CANCELLED bad idea\n", "status:: canceled"),
        ] {
            let out = convert_content(line);
            assert!(out.contains(want), "{line:?} → {out}");
        }
    }

    #[test]
    fn apply_rename_writes_suffixed_file() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        let notes = mosaic.join("notes");
        fs::create_dir_all(&notes).unwrap();
        make_graph(&graph);
        fs::write(notes.join("foo.md"), "user-authored").unwrap();

        let plan = build_plan(&graph, &mosaic).unwrap();
        let foo_rel = plan
            .items
            .iter()
            .find(|i| i.target_id == "foo")
            .unwrap()
            .source_rel
            .clone();

        let mut decisions = ApplyDecisions::default();
        decisions.per_item.insert(
            foo_rel,
            Decision::Rename {
                suffix: "imported".to_string(),
            },
        );
        let outcome = apply_plan(&plan, &decisions, &mosaic).unwrap();
        assert_eq!(outcome.renamed, 1);
        assert!(notes.join("foo-imported.md").exists());
        // Original untouched.
        assert_eq!(
            fs::read_to_string(notes.join("foo.md")).unwrap(),
            "user-authored"
        );
    }
}
