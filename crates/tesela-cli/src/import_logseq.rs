use anyhow::Result;
use std::path::PathBuf;
use tesela_core::regex_cache::{BLOCK_REF_RE, LOGSEQ_DATE_RE, PRIORITY_RE};

/// Import notes from a LogSeq graph into a Tesela mosaic.
pub async fn run(mosaic: &PathBuf, source: PathBuf, dry_run: bool) -> Result<()> {
    let journals_dir = source.join("journals");
    let pages_dir = source.join("pages");
    let notes_dir = mosaic.join("notes");

    if !source.exists() {
        anyhow::bail!("LogSeq graph not found: {}", source.display());
    }

    let _ = std::fs::create_dir_all(&notes_dir);

    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = 0;

    // Import journals (YYYY_MM_DD.md → notes/YYYY-MM-DD.md as daily notes)
    if journals_dir.exists() {
        for entry in std::fs::read_dir(&journals_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") { continue; }

            let stem = name.trim_end_matches(".md");
            let date_id = stem.replace('_', "-");

            if date_id.len() != 10 || date_id.chars().filter(|c| *c == '-').count() != 2 {
                tracing::warn!("Skipping non-date journal: {}", name);
                skipped += 1;
                continue;
            }

            let target_path = notes_dir.join(format!("{}.md", date_id));
            if target_path.exists() {
                skipped += 1;
                continue;
            }

            match std::fs::read_to_string(entry.path()) {
                Ok(content) => {
                    let converted = convert_content(&content);
                    let full = format!(
                        "---\ntitle: \"{}\"\ntags: [\"daily\"]\ncreated: {}T00:00:00Z\n---\n{}",
                        date_id, date_id, converted
                    );

                    if dry_run {
                        println!("  [journal] {} → {}", name, target_path.display());
                    } else {
                        if let Err(e) = std::fs::write(&target_path, &full) {
                            tracing::warn!("Failed to write {}: {}", target_path.display(), e);
                            errors += 1;
                            continue;
                        }
                    }
                    imported += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to read {}: {}", entry.path().display(), e);
                    errors += 1;
                }
            }
        }
    }

    // Import pages (page___name.md → notes/page-name.md)
    if pages_dir.exists() {
        for entry in std::fs::read_dir(&pages_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") { continue; }

            let stem = name.trim_end_matches(".md");
            let clean_name = stem
                .replace("___", "/")
                .replace("%3A", ":")
                .replace("%2F", "/");
            let safe_name = clean_name
                .replace('/', "-")
                .replace(':', "-")
                .replace(' ', "-")
                .to_lowercase();

            if safe_name == "contents" || safe_name == "favorites" { continue; }

            let target_path = notes_dir.join(format!("{}.md", safe_name));
            if target_path.exists() {
                skipped += 1;
                continue;
            }

            match std::fs::read_to_string(entry.path()) {
                Ok(content) => {
                    let converted = convert_content(&content);
                    let title = clean_name.split('/').last().unwrap_or(&clean_name);
                    let full = format!(
                        "---\ntitle: \"{}\"\ntags: []\n---\n{}",
                        title, converted
                    );

                    if dry_run {
                        println!("  [page] {} → {}", name, target_path.display());
                    } else {
                        if let Err(e) = std::fs::write(&target_path, &full) {
                            tracing::warn!("Failed to write {}: {}", target_path.display(), e);
                            errors += 1;
                            continue;
                        }
                    }
                    imported += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to read {}: {}", entry.path().display(), e);
                    errors += 1;
                }
            }
        }
    }

    if dry_run {
        println!("\nDry run complete:");
    } else {
        println!("\nImport complete:");
    }
    println!("  Imported: {} files", imported);
    println!("  Skipped (existing): {} files", skipped);
    if errors > 0 {
        println!("  Errors: {}", errors);
    }
    if !dry_run {
        println!("\nRestart tesela-server to index imported notes.");
    }

    Ok(())
}

/// Convert LogSeq markdown content to Tesela format.
fn convert_content(content: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut in_query = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip LogSeq queries
        if trimmed.starts_with("#+BEGIN_QUERY") { in_query = true; continue; }
        if trimmed.contains("#+END_QUERY") { in_query = false; continue; }
        if in_query { continue; }

        // Skip logseq-specific properties
        if trimmed.starts_with("collapsed:: ") { continue; }
        if trimmed.starts_with("id:: ") { continue; }
        if trimmed.starts_with("file:: ") { continue; }
        if trimmed.starts_with("file-path:: ") { continue; }

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
            let key = if trimmed.starts_with("DEADLINE") { "deadline" } else { "scheduled" };
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
    let without_dash = if line.starts_with("- ") { &line[2..] } else { line };
    let prefix = if line.starts_with("- ") { "- " } else { "" };

    for (marker, status) in [
        ("TODO ", "todo"),
        ("DOING ", "doing"),
        ("DONE ", "done"),
        ("LATER ", "backlog"),
        ("NOW ", "doing"),
        ("WAITING ", "backlog"),
        ("CANCELED ", "canceled"),
    ] {
        if without_dash.starts_with(marker) {
            let rest = &without_dash[marker.len()..];
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
