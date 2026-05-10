use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tesela_core::regex_cache::{BLOCK_REF_RE, LOGSEQ_DATE_RE, PRIORITY_RE};

const SOURCE_PATH_KEY: &str = "source_logseq_path";
const SOURCE_SHA_KEY: &str = "source_logseq_sha";

/// Import notes from a LogSeq graph into a Tesela mosaic.
///
/// Phase 13.D upgrade: idempotent via `source_logseq_path` +
/// `source_logseq_sha` frontmatter. Re-running against the same graph
/// is a silent no-op for unchanged files; changed files are skipped +
/// logged to `<mosaic>/_import-skipped.log` so the user resolves
/// manually rather than losing work.
pub async fn run(mosaic: &Path, source: PathBuf, dry_run: bool) -> Result<()> {
    let journals_dir = source.join("journals");
    let pages_dir = source.join("pages");
    let notes_dir = mosaic.join("notes");

    if !source.exists() {
        anyhow::bail!("LogSeq graph not found: {}", source.display());
    }

    let _ = std::fs::create_dir_all(&notes_dir);

    let mut stats = ImportStats::default();
    let mut log: Vec<String> = Vec::new();

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
                tracing::warn!("Skipping non-date journal: {}", name);
                stats.warnings += 1;
                continue;
            }

            let target_path = notes_dir.join(format!("{}.md", date_id));
            let source_path = entry.path();
            let rel_str = format!("journals/{}", name);

            handle_one(
                &source_path,
                &target_path,
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
                dry_run,
                &mut stats,
                &mut log,
            )?;
        }
    }

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

            // Phase 13.D — namespaced pages (`Parent/Child`) imply
            // tagging the imported note with the parent slug, so a
            // user can find the children via the existing tag system.
            let namespace_tags: Vec<String> = clean_name
                .split('/')
                .take(clean_name.matches('/').count()) // all but last segment
                .map(|s| s.replace([' ', ':'], "-").to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let title = clean_name.split('/').next_back().unwrap_or(&clean_name).to_string();

            handle_one(
                &source_path,
                &target_path,
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
                        title,
                        tags,
                        SOURCE_PATH_KEY,
                        rel_str,
                        SOURCE_SHA_KEY,
                        sha,
                        converted
                    )
                },
                dry_run,
                &mut stats,
                &mut log,
            )?;
        }
    }

    // Phase 13.D — copy assets/ wholesale into the mosaic's attachments.
    let assets_src = source.join("assets");
    if assets_src.exists() {
        let attach_dst = mosaic.join("attachments");
        std::fs::create_dir_all(&attach_dst)?;
        for entry in walkdir::WalkDir::new(&assets_src) {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("walk assets/: {}", e);
                    continue;
                }
            };
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
            if !dry_run {
                if let Err(e) = std::fs::copy(entry.path(), &dst) {
                    tracing::warn!("copy asset {}: {}", entry.path().display(), e);
                    stats.warnings += 1;
                    continue;
                }
            }
            stats.attachments += 1;
        }
    }

    // Phase 13.D — Logseq whiteboards aren't representable in Tesela.
    // Log + skip rather than silently dropping.
    let whiteboards = source.join("whiteboards");
    if whiteboards.exists() {
        for entry in std::fs::read_dir(&whiteboards)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            log.push(format!(
                "[skip] whiteboards/{} (no Tesela equivalent)",
                name
            ));
            stats.warnings += 1;
        }
    }

    if !log.is_empty() && !dry_run {
        let log_path = mosaic.join("_import-skipped.log");
        let mut content = format!(
            "Logseq import @ {}\nSource: {}\n\n",
            chrono::Local::now().to_rfc3339(),
            source.display()
        );
        content.push_str(&log.join("\n"));
        content.push('\n');
        std::fs::write(&log_path, content)?;
    }

    if dry_run {
        println!("\nDry run complete:");
    } else {
        println!("\nImport complete:");
    }
    println!("  Imported: {}", stats.imported);
    println!("  Unchanged (idempotent): {}", stats.unchanged);
    println!("  Conflicts (skipped): {}", stats.conflicts);
    println!("  Attachments: {}", stats.attachments);
    if stats.warnings > 0 {
        println!("  Warnings: {}", stats.warnings);
    }
    if !log.is_empty() && !dry_run {
        println!("  Log: {}", mosaic.join("_import-skipped.log").display());
    }
    if !dry_run {
        println!("\nRestart tesela-server to index imported notes.");
    }

    Ok(())
}

#[derive(Debug, Default)]
struct ImportStats {
    imported: usize,
    unchanged: usize,
    conflicts: usize,
    attachments: usize,
    warnings: usize,
}

/// Apply the per-note flow: read source, SHA-check vs existing target,
/// write or skip, update stats. The `build_full` closure produces the
/// final markdown body (frontmatter + converted content) — it varies
/// per note kind (journal vs page).
fn handle_one(
    source_path: &Path,
    target_path: &Path,
    rel_str: &str,
    build_full: impl FnOnce(&str, &str) -> String,
    dry_run: bool,
    stats: &mut ImportStats,
    log: &mut Vec<String>,
) -> Result<()> {
    let raw = match std::fs::read_to_string(source_path) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("read {}: {}", source_path.display(), e);
            stats.warnings += 1;
            return Ok(());
        }
    };
    let sha = sha256_hex(&raw);

    if target_path.exists() {
        let existing = std::fs::read_to_string(target_path).unwrap_or_default();
        if let Some(prev_sha) = extract_frontmatter_value(&existing, SOURCE_SHA_KEY) {
            if prev_sha == sha {
                stats.unchanged += 1;
                return Ok(());
            }
            log.push(format!(
                "[conflict] {} → {} target exists with different SHA. Skipped.",
                rel_str,
                target_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default()
            ));
            stats.conflicts += 1;
            return Ok(());
        }
        log.push(format!(
            "[conflict] {} → {} target exists and was not produced by this importer. Skipped.",
            rel_str,
            target_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
        ));
        stats.conflicts += 1;
        return Ok(());
    }

    let full = build_full(&raw, &sha);

    if dry_run {
        println!("  [import] {} → {}", rel_str, target_path.display());
    } else {
        if let Err(e) = std::fs::write(target_path, &full) {
            tracing::warn!("write {}: {}", target_path.display(), e);
            stats.warnings += 1;
            return Ok(());
        }
    }
    stats.imported += 1;
    Ok(())
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

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip LogSeq queries
        if trimmed.starts_with("#+BEGIN_QUERY") {
            in_query = true;
            continue;
        }
        if trimmed.contains("#+END_QUERY") {
            in_query = false;
            continue;
        }
        if in_query {
            continue;
        }

        // Skip logseq-specific properties
        if trimmed.starts_with("collapsed:: ") {
            continue;
        }
        if trimmed.starts_with("id:: ") {
            continue;
        }
        if trimmed.starts_with("file:: ") {
            continue;
        }
        if trimmed.starts_with("file-path:: ") {
            continue;
        }

        // Calculate leading whitespace
        let leading = line.len() - line.trim_start().len();
        let indent_str = &line[..leading];

        // Convert task markers
        if let Some((status, rest_text)) = strip_task_marker(trimmed) {
            let (priority, clean_text) = extract_priority(&rest_text);
            let prop_indent = format!("{}  ", indent_str);

            lines.push(format!("{}{}", indent_str, clean_text));
            lines.push(format!("{}status:: {}", prop_indent, status));
            if let Some(p) = priority {
                lines.push(format!("{}priority:: {}", prop_indent, p));
            }
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
                let date = caps.get(1).unwrap().as_str();
                result = format!("{}{}:: {}", indent_str, key, date);
            }
        }

        // Convert ((block-ref-uuid)) → [ref]
        result = BLOCK_REF_RE.replace_all(&result, "[ref]").to_string();

        // Convert tab indentation to 2-space
        result = result.replace('\t', "  ");

        lines.push(result);
    }

    lines.join("\n")
}

fn strip_task_marker(line: &str) -> Option<(String, String)> {
    let (without_dash, prefix) = if let Some(stripped) = line.strip_prefix("- ") {
        (stripped, "- ")
    } else {
        (line, "")
    };

    for (marker, status) in [
        ("TODO ", "todo"),
        ("DOING ", "doing"),
        ("DONE ", "done"),
        ("LATER ", "backlog"),
        ("NOW ", "doing"),
        ("WAITING ", "backlog"),
        ("CANCELED ", "canceled"),
    ] {
        if let Some(rest) = without_dash.strip_prefix(marker) {
            return Some((status.to_string(), format!("{}{}", prefix, rest)));
        }
    }
    None
}

fn extract_priority(text: &str) -> (Option<String>, String) {
    if let Some(caps) = PRIORITY_RE.captures(text) {
        let priority = match caps.get(1).unwrap().as_str() {
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
        fs::write(
            root.join("pages/Foo.md"),
            "title:: Foo\n- regular page\n",
        )
        .unwrap();
        // Namespaced page: Logseq stores `Parent/Child` as
        // `Parent___Child.md` on disk.
        fs::write(
            root.join("pages/Parent___Child.md"),
            "- nested page\n",
        )
        .unwrap();
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

        // Journal landed as 2026-05-10.md
        assert!(mosaic.join("notes/2026-05-10.md").exists());
        // Namespaced page → parent-child.md with #parent tag
        let nested = fs::read_to_string(mosaic.join("notes/parent-child.md")).unwrap();
        assert!(
            nested.contains("\"parent\""),
            "expected namespace tag, got:\n{}",
            nested
        );
        // Assets were copied
        assert!(mosaic.join("attachments/thing.png").exists());
        // Whiteboards skip got logged
        let log = fs::read_to_string(mosaic.join("_import-skipped.log")).unwrap();
        assert!(log.contains("whiteboards/board.edn"));
    }

    #[tokio::test]
    async fn re_import_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_graph(&graph);

        run(&mosaic, graph.clone(), false).await.unwrap();
        let first =
            fs::read_to_string(mosaic.join("notes/2026-05-10.md")).unwrap();
        let first_mtime =
            fs::metadata(mosaic.join("notes/2026-05-10.md")).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        run(&mosaic, graph.clone(), false).await.unwrap();
        let second =
            fs::read_to_string(mosaic.join("notes/2026-05-10.md")).unwrap();
        let second_mtime =
            fs::metadata(mosaic.join("notes/2026-05-10.md")).unwrap().modified().unwrap();
        assert_eq!(first, second);
        assert_eq!(first_mtime, second_mtime);
    }

    #[tokio::test]
    async fn changed_source_is_skipped_with_log() {
        let temp = TempDir::new().unwrap();
        let graph = temp.path().join("graph");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_graph(&graph);

        run(&mosaic, graph.clone(), false).await.unwrap();

        // Modify the source — re-import should refuse to overwrite.
        fs::write(graph.join("journals/2026_05_10.md"), "- changed\n").unwrap();
        run(&mosaic, graph.clone(), false).await.unwrap();

        let log = fs::read_to_string(mosaic.join("_import-skipped.log")).unwrap();
        assert!(log.contains("2026_05_10.md"));
        // The target file should still be the *first* import's content.
        let body = fs::read_to_string(mosaic.join("notes/2026-05-10.md")).unwrap();
        assert!(body.contains("Write tests"));
        assert!(!body.contains("- changed"));
    }
}
