//! Command implementations for the Tesela CLI.
//!
//! This module contains the business logic for all Tesela commands including:
//! - Mosaic initialization (`init_mosaic`)
//! - Note creation (`create_note`)
//! - Note listing (`list_notes`)
//!
//! All functions return `Result<()>` to propagate errors to the CLI layer
//! where they can be properly displayed to the user.

use anyhow::{anyhow, Context, Result};
use dialoguer::Completion;
use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Initializes a new Tesela mosaic (knowledge base) at the specified path.
///
/// Creates the following directory structure:
/// ```
/// path/
/// ├── tesela.toml      # Configuration file
/// ├── notes/           # Directory for markdown notes
/// ├── dailies/         # Directory for daily notes
/// └── attachments/     # Directory for file attachments
/// ```
///
/// # Arguments
/// * `path` - The directory path where the mosaic should be created
///
/// # Errors
/// Returns an error if:
/// - Directory creation fails
/// - Configuration file cannot be written
pub fn init_mosaic(path: &str) -> Result<()> {
    println!("🗿 Initializing Tesela mosaic at: {}", path);

    // Create the main directory
    fs::create_dir_all(path).with_context(|| format!("Failed to create directory {}", path))?;

    // Create subdirectories
    let notes_dir = Path::new(path).join("notes");
    let attachments_dir = Path::new(path).join("attachments");
    let dailies_dir = Path::new(path).join("dailies");

    println!("📁 Creating notes/, dailies/, and attachments/ directories...");

    fs::create_dir_all(&notes_dir).context("Failed to create notes directory")?;

    fs::create_dir_all(&attachments_dir).context("Failed to create attachments directory")?;

    fs::create_dir_all(&dailies_dir).context("Failed to create dailies directory")?;

    // Create basic configuration file
    let config_path = Path::new(path).join("tesela.toml");
    let config_content = r#"# Tesela Configuration
[mosaic]
name = "My Knowledge Mosaic"
created = "{}"

[settings]
# Default editor for notes
editor = "default"

# Auto-save interval in seconds
auto_save = 30

# Enable daily notes
daily_notes = true
"#;

    let formatted_config = config_content.replace(
        "{}",
        &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    );

    fs::write(&config_path, formatted_config).context("Failed to create config file")?;

    println!("✨ Your knowledge mosaic is ready!");
    println!("📂 Created: {}", notes_dir.display());
    println!("📅 Created: {}", dailies_dir.display());
    println!("📎 Created: {}", attachments_dir.display());
    println!("⚙️  Created: {}", config_path.display());

    Ok(())
}

/// Creates a new note in the current mosaic with the given title.
///
/// The note is created as a markdown file with YAML frontmatter containing
/// metadata like title, creation date, and tags. The filename is derived
/// from the title with special characters replaced by underscores.
///
/// # Arguments
/// * `title` - The title for the new note
///
/// # Errors
/// Returns an error if:
/// - No mosaic exists in the current directory
/// - Note file cannot be written to disk
///
/// # Example
/// ```
/// create_note("My Daily Thoughts")?;
/// // Creates: notes/my-daily-thoughts.md
/// ```
pub fn create_note(title: &str) -> Result<()> {
    println!("📝 Creating new note: '{}'", title);

    // Check if we're in a mosaic (look for tesela.toml)
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow!("❌ No mosaic found. Run 'tesela init' first."));
    }

    // Create a safe filename from the title
    let safe_filename = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .replace(' ', "-")
        .to_lowercase();

    let note_path = Path::new("notes").join(format!("{}.md", safe_filename));

    // Create note content with frontmatter and outliner format
    let note_content = format!(
        r#"---
title: "{}"
created: {}
last_opened: {}
tags: []
---
-
"#,
        title,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    fs::write(&note_path, note_content)
        .with_context(|| format!("Failed to create note: {}", note_path.display()))?;

    println!("✅ Created note: {}", note_path.display());
    println!("💡 Tip: Link to other notes with [[Note Name]]");

    Ok(())
}

/// Lists all notes in the current mosaic, sorted by modification time.
///
/// Displays up to 10 most recent notes with their titles and relative
/// timestamps (e.g., "2 hours ago"). The title is extracted from:
/// 1. YAML frontmatter `title` field (preferred)
/// 2. First markdown heading (fallback)
/// 3. Filename with dashes/underscores converted to spaces (last resort)
///
/// # Errors
/// Returns an error if:
/// - No mosaic exists in the current directory
/// - Notes directory cannot be read
///
/// # Output Format
/// ```
/// 📚 Recent notes:
///   • My Latest Note (just now) [my-latest-note.md]
///   • Project Ideas (2 hours ago) [project-ideas.md]
///   ... and 5 more notes
/// ```
pub fn list_notes() -> Result<()> {
    println!("📚 Recent notes:");

    // Check if we're in a mosaic
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow!("❌ No mosaic found. Run 'tesela init' first."));
    }

    // Check if notes directory exists
    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        println!("📁 No notes directory found.");
        return Ok(());
    }

    // Read notes directory
    let entries = fs::read_dir(notes_dir).context("Failed to read notes directory")?;

    // Collect markdown files with their metadata
    let mut notes = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    let filename = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");

                    // Try to extract title from file content
                    let title = if let Ok(content) = fs::read_to_string(&path) {
                        extract_title_from_content(&content, filename)
                    } else {
                        filename.to_string()
                    };

                    notes.push((title, modified, path));
                }
            }
        }
    }

    // Sort by modification time (newest first)
    notes.sort_by(|a, b| b.1.cmp(&a.1));

    // Display notes
    if notes.is_empty() {
        println!("📄 No notes found in this mosaic.");
        println!("💡 Create your first note with: tesela new 'My First Note'");
    } else {
        for (title, modified, path) in notes.iter().take(10) {
            let time_ago = format_time_ago(*modified);
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            println!("  • {} ({}) [{}]", title, time_ago, filename);
        }

        if notes.len() > 10 {
            println!("  ... and {} more notes", notes.len() - 10);
        }
    }

    Ok(())
}

/// Displays the contents of a note.
///
/// The note can be identified by:
/// - Full filename (e.g., "my-note.md")
/// - Filename without extension (e.g., "my-note")
/// - Partial name match (e.g., "note" matches "my-note.md")
///
/// If multiple notes match, shows a list of matches and asks the user to be more specific.
///
/// # Arguments
/// * `note_identifier` - Full or partial note name/filename
///
/// # Errors
/// Returns an error if:
/// - No mosaic exists in the current directory
/// - No notes match the identifier
/// - Note file cannot be read
pub fn cat_note(note_identifier: &str) -> Result<()> {
    // Check if we're in a mosaic
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow!("❌ No mosaic found. Run 'tesela init' first."));
    }

    // Check if notes directory exists
    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow!("📁 No notes directory found."));
    }

    // Find matching notes
    let entries = fs::read_dir(notes_dir).context("Failed to read notes directory")?;

    let mut matches = Vec::new();
    let search_term = note_identifier.to_lowercase();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

            let filename_lower = filename.to_lowercase();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Check for exact match (with or without .md)
            if filename_lower == search_term || stem == search_term {
                matches.clear();
                matches.push(path);
                break;
            }

            // Check for partial match
            if filename_lower.contains(&search_term) || stem.contains(&search_term) {
                matches.push(path);
            }
        }
    }

    // Handle results
    match matches.len() {
        0 => {
            return Err(anyhow!("❌ No notes found matching '{}'", note_identifier));
        }
        1 => {
            // Display the single matching note
            let path = &matches[0];
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read note: {}", path.display()))?;

            // Extract title for display
            let filename = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let title = extract_title_from_content(&content, filename);

            println!("📄 {}", title);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", content);

            Ok(())
        }
        _ => {
            // Multiple matches - show them and ask for more specific input
            println!("🔍 Multiple notes match '{}':", note_identifier);
            println!();
            for path in matches.iter() {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                // Try to get title
                if let Ok(content) = fs::read_to_string(path) {
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let title = extract_title_from_content(&content, stem);
                    println!("  • {} [{}]", title, filename);
                } else {
                    println!("  • {}", filename);
                }
            }
            println!();
            println!("💡 Please be more specific with your search term.");

            Err(anyhow!("Multiple matches found"))
        }
    }
}

/// Attach a file to an existing note
///
/// This command copies a file into the mosaic's attachments directory and
/// creates a reference to it in the specified note. The attachment is organized
/// by note ID and the original filename is preserved.
///
/// # Arguments
/// * `note_identifier` - The note to attach the file to (ID or partial name)
/// * `file_path` - Path to the file to attach
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Note cannot be found or is ambiguous
/// - File to attach doesn't exist
/// - File cannot be copied to attachments directory
/// - Note cannot be updated with attachment reference
///
/// # Example
/// ```no_run
/// use tesela::attach_file;
/// attach_file("meeting-notes", "/path/to/document.pdf")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn attach_file(note_identifier: &str, file_path: &str) -> Result<()> {
    println!("📎 Attaching file to note: '{}'", note_identifier);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    // Find the target note
    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow::anyhow!("Notes directory not found"));
    }

    let entries = fs::read_dir(notes_dir).context("Failed to read notes directory")?;
    let mut matches = Vec::new();
    let search_term = note_identifier.to_lowercase();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let filename_lower = filename.to_lowercase();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if filename_lower == search_term || stem == search_term {
                matches.clear();
                matches.push(path);
                break;
            }

            if filename_lower.contains(&search_term) || stem.contains(&search_term) {
                matches.push(path);
            }
        }
    }

    if matches.is_empty() {
        return Err(anyhow::anyhow!(
            "No notes found matching '{}'",
            note_identifier
        ));
    }

    if matches.len() > 1 {
        println!("❓ Multiple notes found matching '{}':", note_identifier);
        for file_path in &matches {
            if let Some(file_name) = file_path.file_stem() {
                println!("  • {}", file_name.to_string_lossy());
            }
        }
        return Err(anyhow::anyhow!(
            "Please be more specific - {} notes matched",
            matches.len()
        ));
    }

    let note_file = &matches[0];
    let note_id = note_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid note filename"))?;

    // Check if source file exists
    let source_path = Path::new(file_path);
    if !source_path.exists() {
        return Err(anyhow::anyhow!("File not found: {}", file_path));
    }

    // Create attachments directory for this note
    let note_attachments_dir = PathBuf::from("attachments").join(note_id);
    fs::create_dir_all(&note_attachments_dir)?;

    // Copy file to attachments directory
    let file_name = source_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid source file path"))?;
    let dest_path = note_attachments_dir.join(file_name);

    fs::copy(source_path, &dest_path)?;

    // Update note content to include attachment reference
    let note_content = fs::read_to_string(note_file)?;
    let attachment_ref = format!(
        "\n\n## Attachments\n\n- [{}]({})\n",
        file_name.to_string_lossy(),
        dest_path.to_string_lossy()
    );

    // Check if note already has attachments section
    let updated_content = if note_content.contains("## Attachments") {
        note_content.replace(
            "## Attachments\n\n",
            &format!(
                "## Attachments\n\n- [{}]({})\n",
                file_name.to_string_lossy(),
                dest_path.to_string_lossy()
            ),
        )
    } else {
        note_content + &attachment_ref
    };

    fs::write(note_file, updated_content)?;

    println!(
        "✅ Attached '{}' to '{}'",
        file_name.to_string_lossy(),
        note_id
    );
    println!("📂 Stored in: {}", dest_path.display());

    Ok(())
}

/// Export a note to different formats
///
/// This command exports a note to various formats such as HTML, PDF, or plain text.
/// The exported file is created in the current directory with an appropriate extension.
///
/// # Arguments
/// * `note_identifier` - The note to export (ID or partial name)
/// * `format` - Export format ("html", "markdown", "txt")
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Note cannot be found or is ambiguous
/// - Export format is not supported
/// - Export file cannot be written
///
/// # Example
/// ```no_run
/// use tesela::export_note;
/// export_note("project-plan", "html")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn export_note(note_identifier: &str, format: &str) -> Result<()> {
    use pulldown_cmark::{html, Event, Parser, Tag};

    println!("📤 Exporting note '{}' as {}", note_identifier, format);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    // Find the target note
    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow::anyhow!("Notes directory not found"));
    }

    let entries = fs::read_dir(notes_dir).context("Failed to read notes directory")?;
    let mut matches = Vec::new();
    let search_term = note_identifier.to_lowercase();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let filename_lower = filename.to_lowercase();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            if filename_lower == search_term || stem == search_term {
                matches.clear();
                matches.push(path);
                break;
            }

            if filename_lower.contains(&search_term) || stem.contains(&search_term) {
                matches.push(path);
            }
        }
    }

    if matches.is_empty() {
        return Err(anyhow::anyhow!(
            "No notes found matching '{}'",
            note_identifier
        ));
    }

    if matches.len() > 1 {
        println!("❓ Multiple notes found matching '{}':", note_identifier);
        for file_path in &matches {
            if let Some(file_name) = file_path.file_stem() {
                println!("  • {}", file_name.to_string_lossy());
            }
        }
        return Err(anyhow::anyhow!(
            "Please be more specific - {} notes matched",
            matches.len()
        ));
    }

    let note_file = &matches[0];
    let note_id = note_file
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid note filename"))?;

    // Read note content
    let content = fs::read_to_string(note_file)?;

    // Determine output filename and process content
    let (output_filename, processed_content) = match format.to_lowercase().as_str() {
        "html" => {
            let parser = Parser::new(&content);
            let mut html_output = String::new();
            html::push_html(&mut html_output, parser);

            let html_content = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; max-width: 800px; margin: 0 auto; padding: 2rem; }}
        h1, h2, h3 {{ color: #333; }}
        code {{ background: #f4f4f4; padding: 0.2rem 0.4rem; border-radius: 3px; }}
        pre {{ background: #f4f4f4; padding: 1rem; border-radius: 5px; overflow-x: auto; }}
        blockquote {{ border-left: 4px solid #ddd; margin: 0; padding-left: 1rem; color: #666; }}
    </style>
</head>
<body>
{}
</body>
</html>"#,
                note_id, html_output
            );
            (format!("{}.html", note_id), html_content)
        }
        "markdown" | "md" => (format!("{}.md", note_id), content.clone()),
        "txt" | "text" => {
            let parser = Parser::new(&content);
            let mut plain_text = String::new();

            for event in parser {
                match event {
                    Event::Text(text) => plain_text.push_str(&text),
                    Event::Code(code) => plain_text.push_str(&code),
                    Event::Start(Tag::Heading(..)) => plain_text.push_str("\n\n"),
                    Event::End(Tag::Heading(..)) => plain_text.push('\n'),
                    Event::Start(Tag::Paragraph) => plain_text.push('\n'),
                    Event::End(Tag::Paragraph) => plain_text.push('\n'),
                    Event::HardBreak | Event::SoftBreak => plain_text.push('\n'),
                    _ => {}
                }
            }

            let cleaned = plain_text
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string();

            (format!("{}.txt", note_id), cleaned)
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported export format '{}'. Supported formats: html, markdown, txt",
                format
            ));
        }
    };

    // Write exported content
    fs::write(&output_filename, processed_content)?;

    println!("✅ Exported to: {}", output_filename);

    Ok(())
}

/// Create a link between two notes
///
/// This command creates a bidirectional link between two notes by adding
/// link references in both notes' content.
///
/// # Arguments
/// * `from_note` - Source note identifier (ID or partial name)
/// * `to_note` - Target note identifier (ID or partial name)
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Either note cannot be found or is ambiguous
/// - Notes cannot be updated with link references
///
/// # Example
/// ```no_run
/// use tesela::link_notes;
/// link_notes("project-plan", "meeting-notes")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn link_notes(from_note: &str, to_note: &str) -> Result<()> {
    println!("🔗 Creating link from '{}' to '{}'", from_note, to_note);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow::anyhow!("Notes directory not found"));
    }

    // Find both notes
    let from_matches = find_matching_notes(notes_dir, from_note)?;
    let to_matches = find_matching_notes(notes_dir, to_note)?;

    if from_matches.is_empty() {
        return Err(anyhow::anyhow!("Source note '{}' not found", from_note));
    }
    if to_matches.is_empty() {
        return Err(anyhow::anyhow!("Target note '{}' not found", to_note));
    }

    if from_matches.len() > 1 {
        return Err(anyhow::anyhow!(
            "Multiple matches for source note '{}'",
            from_note
        ));
    }
    if to_matches.len() > 1 {
        return Err(anyhow::anyhow!(
            "Multiple matches for target note '{}'",
            to_note
        ));
    }

    let from_path = &from_matches[0];
    let to_path = &to_matches[0];

    let from_id = from_path.file_stem().and_then(|s| s.to_str()).unwrap();
    let to_id = to_path.file_stem().and_then(|s| s.to_str()).unwrap();

    // Read both notes
    let mut from_content = fs::read_to_string(from_path)?;
    let mut to_content = fs::read_to_string(to_path)?;

    let from_title = extract_title_from_content(&from_content, from_id);
    let to_title = extract_title_from_content(&to_content, to_id);

    // Add link in from_note
    let forward_link = format!("\n- [[{}]]\n", to_title);
    if !from_content.contains(&format!("[[{}]]", to_title)) {
        from_content.push_str(&forward_link);
        fs::write(from_path, from_content)?;
    }

    // Add backlink in to_note
    let back_link = format!("\n- [[{}]]\n", from_title);
    if !to_content.contains(&format!("[[{}]]", from_title)) {
        to_content.push_str(&back_link);
        fs::write(to_path, to_content)?;
    }

    println!(
        "✅ Created bidirectional link between '{}' and '{}'",
        from_title, to_title
    );
    Ok(())
}

/// Show graph connections for a note
///
/// This command displays all the connections (links) for a specific note,
/// showing both outgoing links and backlinks.
///
/// # Arguments
/// * `note_identifier` - The note to show connections for (ID or partial name)
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Note cannot be found or is ambiguous
///
/// # Example
/// ```no_run
/// use tesela::show_graph;
/// show_graph("my-note")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn show_graph(note_identifier: &str) -> Result<()> {
    println!("🕸️  Showing connections for: '{}'", note_identifier);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow::anyhow!("Notes directory not found"));
    }

    let matching_files = find_matching_notes(notes_dir, note_identifier)?;

    if matching_files.is_empty() {
        return Err(anyhow::anyhow!(
            "No notes found matching '{}'",
            note_identifier
        ));
    }

    if matching_files.len() > 1 {
        return Err(anyhow::anyhow!("Multiple notes matched, be more specific"));
    }

    let note_file = &matching_files[0];
    let note_id = note_file.file_stem().and_then(|s| s.to_str()).unwrap();
    let content = fs::read_to_string(note_file)?;
    let title = extract_title_from_content(&content, note_id);

    println!();
    println!("📄 {} [{}]", title, note_id);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Find outgoing links
    let mut outgoing_links = Vec::new();
    for line in content.lines() {
        if line.contains("[[") && line.contains("]]") {
            // Extract all [[link]] patterns
            let mut start = 0;
            while let Some(start_pos) = line[start..].find("[[") {
                let actual_start = start + start_pos + 2;
                if let Some(end_pos) = line[actual_start..].find("]]") {
                    let link = &line[actual_start..actual_start + end_pos];
                    outgoing_links.push(link.to_string());
                    start = actual_start + end_pos + 2;
                } else {
                    break;
                }
            }
        }
    }

    // Find backlinks by searching other notes
    let mut backlinks = Vec::new();
    let entries = fs::read_dir(notes_dir)?;
    let search_pattern = format!("[[{}]]", title);

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path != *note_file {
            if let Ok(other_content) = fs::read_to_string(&path) {
                if other_content.contains(&search_pattern) {
                    let other_id = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    let other_title = extract_title_from_content(&other_content, other_id);
                    backlinks.push(other_title);
                }
            }
        }
    }

    // Display connections
    println!("🔗 Outgoing Links ({}):", outgoing_links.len());
    if outgoing_links.is_empty() {
        println!("   (none)");
    } else {
        for link in outgoing_links {
            println!("   → {}", link);
        }
    }

    println!();
    println!("🔙 Backlinks ({}):", backlinks.len());
    if backlinks.is_empty() {
        println!("   (none)");
    } else {
        for backlink in backlinks {
            println!("   ← {}", backlink);
        }
    }

    println!();
    println!(
        "💡 Use 'tesela link {} <other-note>' to create new connections",
        note_id
    );

    Ok(())
}

/// Create or open today's daily note
///
/// This command creates a daily note for today with a standardized format,
/// or opens it if it already exists.
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Daily note cannot be created or opened
///
/// # Example
/// ```no_run
/// use tesela::daily_note;
/// daily_note()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn daily_note() -> Result<()> {
    use chrono::Local;

    let today = Local::now().format("%Y-%m-%d").to_string();
    println!("📅 Opening daily note for {}", today);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let dailies_dir = Path::new("dailies");
    if !dailies_dir.exists() {
        fs::create_dir_all(dailies_dir)?;
    }

    let daily_filename = format!("daily-{}.md", today);
    let daily_path = dailies_dir.join(&daily_filename);

    if daily_path.exists() {
        println!("📖 Daily note already exists: {}", daily_filename);
        // Display the existing daily note
        let content = fs::read_to_string(&daily_path)?;
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("{}", content);
    } else {
        // Create new daily note
        let formatted_date = Local::now().format("%B %d, %Y").to_string();

        let daily_content = format!(
            r#"---
title: "Daily Note - {}"
created: {}
last_opened: {}
tags: ["daily"]
---
-
"#,
            formatted_date,
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );

        fs::write(&daily_path, daily_content)?;
        println!("✅ Created daily note: {}", daily_filename);
        println!("📝 Daily notes help track progress and maintain focus");
    }

    Ok(())
}

/// Update the frontmatter of a note file with the last_opened timestamp
///
/// This function reads a markdown file, parses its frontmatter, and adds or updates
/// the `last_opened` field with the current timestamp. If the file doesn't have
/// frontmatter, it creates it.
fn update_last_opened_timestamp(file_path: &Path) -> Result<()> {
    use chrono::Local;

    if !file_path.exists() {
        return Ok(()); // File doesn't exist, nothing to update
    }

    let content = fs::read_to_string(file_path)?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let new_content = if content.starts_with("---\n") {
        // File has frontmatter
        let parts: Vec<&str> = content.splitn(3, "---\n").collect();
        if parts.len() >= 3 {
            let frontmatter = parts[1];
            let body = parts[2];

            // Parse frontmatter lines
            let mut frontmatter_lines: Vec<String> =
                frontmatter.lines().map(|s| s.to_string()).collect();

            // Check if last_opened already exists
            let mut found_last_opened = false;
            for line in &mut frontmatter_lines {
                if line.starts_with("last_opened:") {
                    *line = format!("last_opened: {}", now);
                    found_last_opened = true;
                    break;
                }
            }

            // If not found, add it after created if it exists, otherwise at the end
            if !found_last_opened {
                let mut inserted = false;
                for (i, line) in frontmatter_lines.iter().enumerate() {
                    if line.starts_with("created:") {
                        frontmatter_lines.insert(i + 1, format!("last_opened: {}", now));
                        inserted = true;
                        break;
                    }
                }
                if !inserted {
                    frontmatter_lines.push(format!("last_opened: {}", now));
                }
            }

            format!("---\n{}\n---\n{}", frontmatter_lines.join("\n"), body)
        } else {
            // Malformed frontmatter, just add to beginning
            format!("---\nlast_opened: {}\n---\n{}", now, content)
        }
    } else {
        // No frontmatter, create it
        format!("---\nlast_opened: {}\n---\n{}", now, content)
    };

    fs::write(file_path, new_content)?;
    Ok(())
}

/// Create and open today's daily note in the editor
///
/// This function creates today's daily note (if it doesn't exist) and then
/// immediately opens it in the configured editor (vim or $EDITOR).
///
/// # Examples
///
/// ```no_run
/// use tesela::daily_note_and_edit;
/// daily_note_and_edit()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn daily_note_and_edit() -> Result<()> {
    use chrono::Local;
    use std::env;
    use std::process::Command;

    let today = Local::now().format("%Y-%m-%d").to_string();
    println!("📅 Opening daily note for {} in editor...", today);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let dailies_dir = Path::new("dailies");
    if !dailies_dir.exists() {
        fs::create_dir_all(dailies_dir)?;
    }

    let daily_filename = format!("daily-{}.md", today);
    let daily_path = dailies_dir.join(&daily_filename);

    if !daily_path.exists() {
        // Create new daily note
        let formatted_date = Local::now().format("%B %d, %Y").to_string();

        let daily_content = format!(
            r#"---
title: "Daily Note - {}"
created: {}
last_opened: {}
tags: ["daily"]
---
-
"#,
            formatted_date,
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );

        fs::write(&daily_path, daily_content)?;
        println!("✅ Created daily note: {}", daily_filename);
    } else {
        println!("📖 Daily note already exists: {}", daily_filename);
    }

    // Update last_opened timestamp before opening in editor
    if let Err(e) = update_last_opened_timestamp(&daily_path) {
        eprintln!("⚠️  Warning: Failed to update last_opened timestamp: {}", e);
    }

    // Get editor from environment or default to vim
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    println!("📝 Opening '{}' in {}...", daily_path.display(), editor);

    // Execute the editor
    let status = Command::new(&editor)
        .arg(&daily_path)
        .status()
        .context("Failed to launch editor")?;

    if !status.success() {
        return Err(anyhow!(
            "Editor exited with error code: {:?}",
            status.code()
        ));
    }

    println!("✅ Daily note editing completed");
    Ok(())
}

/// Create a backup of the entire mosaic
///
/// This command creates a timestamped backup of the entire mosaic directory,
/// preserving all notes, attachments, and configuration.
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Backup directory cannot be created
/// - Files cannot be copied
///
/// # Example
/// ```no_run
/// use tesela::backup_mosaic;
/// backup_mosaic()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn backup_mosaic() -> Result<()> {
    use chrono::Local;

    println!("💾 Creating mosaic backup...");

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_name = format!("tesela_backup_{}", timestamp);
    let backup_path = Path::new(&backup_name);

    // Create backup directory
    fs::create_dir_all(&backup_path)?;

    // Copy all mosaic contents
    let items_to_backup = ["notes", "attachments", "tesela.toml"];

    for item in &items_to_backup {
        let source = Path::new(item);
        if source.exists() {
            let dest = backup_path.join(item);

            if source.is_dir() {
                copy_dir_all(source, &dest)?;
            } else {
                fs::copy(source, &dest)?;
            }
        }
    }

    // Create backup manifest
    let manifest = format!(
        r#"# Tesela Mosaic Backup
Created: {}
Original Location: {}
Contents:
- notes/ ({} files)
- attachments/ (if present)
- tesela.toml (configuration)

To restore:
1. Copy contents to desired location
2. Run 'tesela list' to verify
"#,
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        std::env::current_dir()?.display(),
        count_files(Path::new("notes"))?
    );

    fs::write(backup_path.join("BACKUP_INFO.md"), manifest)?;

    println!("✅ Backup created: {}", backup_name);
    println!("📦 Backup size: {} items", items_to_backup.len());
    println!("💡 To restore, copy contents back to your mosaic directory");

    Ok(())
}

/// Import notes from external formats
///
/// This command imports notes from various external formats including
/// plain text files, markdown files, and other note-taking applications.
///
/// # Arguments
/// * `source_path` - Path to file or directory to import
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Source path doesn't exist
/// - Import fails due to format issues
///
/// # Example
/// ```no_run
/// use tesela::import_notes;
/// import_notes("/path/to/notes")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn import_notes(source_path: &str) -> Result<()> {
    println!("📥 Importing notes from: {}", source_path);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let source = Path::new(source_path);
    if !source.exists() {
        return Err(anyhow::anyhow!(
            "Source path does not exist: {}",
            source_path
        ));
    }

    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        fs::create_dir_all(notes_dir)?;
    }

    let mut imported_count = 0;

    if source.is_file() {
        // Import single file
        imported_count += import_single_file(source, notes_dir)?;
    } else if source.is_dir() {
        // Import directory
        imported_count += import_directory(source, notes_dir)?;
    }

    if imported_count > 0 {
        println!("✅ Successfully imported {} note(s)", imported_count);
        println!("📝 Run 'tesela list' to see your imported notes");
    } else {
        println!("⚠️  No compatible files found to import");
        println!("💡 Supported formats: .md, .markdown, .txt");
    }

    Ok(())
}

/// Start interactive mode for Tesela
///
/// This command starts an interactive shell where users can run multiple
/// commands without restarting the application.
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized (for commands that require it)
/// - Interactive session encounters an error
///
/// # Example
/// ```no_run
/// use tesela::interactive_mode;
/// interactive_mode()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```

/// Open a note in an external editor (vim by default)
///
/// This function finds a note by identifier and opens it in the user's
/// preferred editor. Falls back to vim if no EDITOR environment variable is set.
///
/// # Arguments
/// * `note_identifier` - Note filename or partial name to match
///
/// # Errors
/// Returns an error if:
/// - Note is not found
/// - Editor command fails to execute
///
/// # Example
/// ```no_run
/// use tesela::open_note_in_editor;
/// open_note_in_editor("my-note")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn open_note_in_editor(note_identifier: &str) -> Result<()> {
    use std::env;
    use std::process::Command;

    // Check if we're in a mosaic
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow!("No mosaic found. Run 'tesela init' to create one."));
    }

    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        return Err(anyhow!(
            "Notes directory not found. Your mosaic may be corrupted."
        ));
    }

    // Find the note file in both notes and dailies directories
    let mut matches = Vec::new();

    // Search in notes directory
    if notes_dir.exists() {
        matches.extend(find_matching_notes(notes_dir, note_identifier)?);
    }

    // Search in dailies directory
    let dailies_dir = Path::new("dailies");
    if dailies_dir.exists() {
        matches.extend(find_matching_notes(dailies_dir, note_identifier)?);
    }

    let note_path = match matches.len() {
        0 => {
            return Err(anyhow!("No notes found matching '{}'", note_identifier));
        }
        1 => &matches[0],
        _ => {
            println!("🔍 Multiple notes match '{}':", note_identifier);
            println!();
            for (i, path) in matches.iter().enumerate() {
                let title = extract_title_from_file(path)?;
                let filename = path.file_name().unwrap().to_string_lossy();
                println!("  {}. {} [{}]", i + 1, title, filename);
            }
            println!();

            use dialoguer::{theme::ColorfulTheme, Select};
            let theme = ColorfulTheme::default();
            let selection = Select::with_theme(&theme)
                .with_prompt("Which note would you like to open?")
                .items(
                    &matches
                        .iter()
                        .map(|p| {
                            extract_title_from_file(p).unwrap_or_else(|_| "Untitled".to_string())
                        })
                        .collect::<Vec<_>>(),
                )
                .interact()?;

            &matches[selection]
        }
    };

    // Update last_opened timestamp before opening in editor
    if let Err(e) = update_last_opened_timestamp(note_path) {
        eprintln!("⚠️  Warning: Failed to update last_opened timestamp: {}", e);
    }

    // Get editor from environment or default to vim
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

    println!("📝 Opening '{}' in {}...", note_path.display(), editor);

    // Execute the editor
    let status = Command::new(&editor)
        .arg(note_path)
        .status()
        .context("Failed to launch editor")?;

    if !status.success() {
        return Err(anyhow!(
            "Editor exited with error code: {:?}",
            status.code()
        ));
    }

    println!("✅ Note editing completed");
    Ok(())
}

/// Extract title from a note file by reading its content
fn extract_title_from_file(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path).context("Failed to read note file")?;

    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled");

    Ok(extract_title_from_content(&content, filename))
}

/// Enhanced autocomplete implementation for notes, tags, and search keywords
struct SmartCompletion {
    notes: Vec<(String, SystemTime)>, // (name, modified_time) for sorting
    tags: Vec<String>,
    search_keywords: Vec<String>,
    completion_type: CompletionType,
    // State for cycling through completions
    cycle_state: RefCell<CycleState>,
}

#[derive(Debug)]
struct CycleState {
    original_input: String, // The partial input that generated the matches
    current_matches: Vec<String>,
    current_index: usize,
}

#[derive(Clone)]
enum CompletionType {
    Notes,
    Search,
    Mixed,
}

impl SmartCompletion {
    fn new_for_notes() -> Result<Self> {
        let notes = get_note_names_with_timestamps()?;
        Ok(SmartCompletion {
            notes,
            tags: Vec::new(),
            search_keywords: Vec::new(),
            completion_type: CompletionType::Notes,
            cycle_state: RefCell::new(CycleState {
                original_input: String::new(),
                current_matches: Vec::new(),
                current_index: 0,
            }),
        })
    }

    fn new_for_search() -> Result<Self> {
        let notes = get_note_names_with_timestamps()?;
        let tags = get_tag_names()?;
        let search_keywords = get_search_keywords();
        Ok(SmartCompletion {
            notes,
            tags,
            search_keywords,
            completion_type: CompletionType::Search,
            cycle_state: RefCell::new(CycleState {
                original_input: String::new(),
                current_matches: Vec::new(),
                current_index: 0,
            }),
        })
    }

    fn new_mixed() -> Result<Self> {
        let notes = get_note_names_with_timestamps()?;
        let tags = get_tag_names()?;
        let search_keywords = get_search_keywords();
        Ok(SmartCompletion {
            notes,
            tags,
            search_keywords,
            completion_type: CompletionType::Mixed,
            cycle_state: RefCell::new(CycleState {
                original_input: String::new(),
                current_matches: Vec::new(),
                current_index: 0,
            }),
        })
    }

    fn find_matches(&self, input: &str) -> Vec<String> {
        let input_lower = input.to_lowercase();
        let mut matches = Vec::new();

        match self.completion_type {
            CompletionType::Notes => {
                // Create a vector of (name, modified_time) for sorting
                let mut note_matches: Vec<(String, SystemTime)> = Vec::new();

                // Priority 1: Exact prefix matches
                for (note, modified_time) in &self.notes {
                    if note.to_lowercase().starts_with(&input_lower) {
                        note_matches.push((note.clone(), *modified_time));
                    }
                }

                // Priority 2: Contains matches (only if no prefix matches)
                if note_matches.is_empty() {
                    for (note, modified_time) in &self.notes {
                        if note.to_lowercase().contains(&input_lower) {
                            note_matches.push((note.clone(), *modified_time));
                        }
                    }
                }

                // Sort by modification time (most recent first)
                note_matches.sort_by(|a, b| b.1.cmp(&a.1));
                matches = note_matches.into_iter().map(|(name, _)| name).collect();
            }
            CompletionType::Search => {
                // Search keywords first
                for keyword in &self.search_keywords {
                    if keyword.to_lowercase().starts_with(&input_lower) {
                        matches.push(keyword.clone());
                    }
                }
                // Then tags with tag: prefix
                for tag in &self.tags {
                    if tag.to_lowercase().starts_with(&input_lower) {
                        matches.push(format!("tag:{}", tag));
                    }
                }
                // Then note names (sorted by modification time)
                let mut note_matches: Vec<(String, SystemTime)> = Vec::new();
                for (note, modified_time) in &self.notes {
                    if note.to_lowercase().contains(&input_lower) {
                        note_matches.push((note.clone(), *modified_time));
                    }
                }
                note_matches.sort_by(|a, b| b.1.cmp(&a.1));
                matches.extend(note_matches.into_iter().map(|(name, _)| name));
            }
            CompletionType::Mixed => {
                // All completion types with time-sorted notes
                let mut note_matches: Vec<(String, SystemTime)> = Vec::new();
                for (note, modified_time) in &self.notes {
                    if note.to_lowercase().starts_with(&input_lower) {
                        note_matches.push((note.clone(), *modified_time));
                    }
                }
                note_matches.sort_by(|a, b| b.1.cmp(&a.1));
                matches.extend(note_matches.into_iter().map(|(name, _)| name));

                for tag in &self.tags {
                    if tag.to_lowercase().starts_with(&input_lower) {
                        matches.push(format!("#{}", tag));
                    }
                }
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        matches.retain(|item| seen.insert(item.clone()));

        matches
    }
}

impl Completion for SmartCompletion {
    fn get(&self, input: &str) -> Option<String> {
        let mut state = self.cycle_state.borrow_mut();

        // Check if input matches any of our current completions (cycling behavior)
        if !state.current_matches.is_empty() {
            if let Some(pos) = state.current_matches.iter().position(|m| m == input) {
                // We found the current completion - cycle to the next one
                state.current_index = (pos + 1) % state.current_matches.len();
                return Some(state.current_matches[state.current_index].clone());
            }
        }

        // New search - find all matches
        let matches = self.find_matches(input);
        if matches.is_empty() {
            // Reset state for empty matches
            state.original_input = String::new();
            state.current_matches = Vec::new();
            state.current_index = 0;
            return None;
        }

        // Initialize new completion session
        state.original_input = input.to_string();
        state.current_matches = matches.clone();
        state.current_index = 0;

        // Return first match
        Some(matches[0].clone())
    }
}

/// Get all note names with modification timestamps for autocomplete
fn get_note_names_with_timestamps() -> Result<Vec<(String, SystemTime)>> {
    let mut all_notes = Vec::new();

    // Scan both notes and dailies directories
    let directories = ["notes", "dailies"];

    for dir_name in directories {
        let dir_path = Path::new(dir_name);
        if dir_path.exists() {
            let dir_notes = scan_directory_for_notes(dir_path)?;
            all_notes.extend(dir_notes);
        }
    }

    // Remove duplicates while preserving the most recent timestamp
    let mut seen = std::collections::HashMap::new();
    for (name, time) in all_notes {
        seen.entry(name)
            .and_modify(|existing_time| {
                if time > *existing_time {
                    *existing_time = time;
                }
            })
            .or_insert(time);
    }

    let notes: Vec<(String, SystemTime)> = seen.into_iter().collect();
    Ok(notes)
}

/// Helper function to scan a single directory for note files
fn scan_directory_for_notes(dir_path: &Path) -> Result<Vec<(String, SystemTime)>> {
    let mut notes = Vec::new();

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // Get modification time
                let modified_time = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                // Convert filename to title-like format for better UX
                let title = extract_title_from_file(&path).unwrap_or_else(|_| stem.to_string());

                // For daily notes, prefer the user-friendly title over filename
                // For other notes, use smart deduplication
                let title_lower = title.to_lowercase();
                let stem_lower = stem.to_lowercase();
                let is_daily_note = stem.starts_with("daily-")
                    || dir_path.file_name().map_or(false, |n| n == "dailies");

                if is_daily_note {
                    // For daily notes, always prefer the title if it's meaningful
                    if title.len() > stem.len() && title.contains(" ") {
                        notes.push((title, modified_time));
                    } else {
                        notes.push((stem.to_string(), modified_time));
                    }
                } else if title_lower != stem_lower
                    && title_lower.replace(" ", "-") != stem_lower
                    && title_lower.replace(" ", "_") != stem_lower
                {
                    // Title is meaningfully different, add both
                    notes.push((title.clone(), modified_time));
                    notes.push((stem.to_string(), modified_time));
                } else {
                    // Title is similar to stem, just add the better one
                    if title.len() > stem.len() && title.contains(" ") {
                        notes.push((title, modified_time));
                    } else {
                        notes.push((stem.to_string(), modified_time));
                    }
                }
            }
        }
    }

    Ok(notes)
}

/// Get all note names (without .md extension) for autocomplete - legacy function
fn get_note_names() -> Result<Vec<String>> {
    let notes_with_timestamps = get_note_names_with_timestamps()?;
    Ok(notes_with_timestamps
        .into_iter()
        .map(|(name, _)| name)
        .collect())
}

/// Get all tag names for autocomplete
fn get_tag_names() -> Result<Vec<String>> {
    // This is a simplified implementation - in a real system you'd query the database
    // For now, we'll return common tags that users might want to search for
    let mut tags = std::collections::HashSet::new();

    // Scan both notes and dailies directories
    let directories = ["notes", "dailies"];

    for dir_name in directories {
        let dir_path = Path::new(dir_name);
        if dir_path.exists() {
            for entry in fs::read_dir(dir_path)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        // Extract hashtags from content
                        for word in content.split_whitespace() {
                            if word.starts_with('#') && word.len() > 1 {
                                let tag =
                                    word[1..].trim_end_matches(|c: char| !c.is_alphanumeric());
                                if !tag.is_empty() {
                                    tags.insert(tag.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut tag_vec: Vec<String> = tags.into_iter().collect();
    tag_vec.sort();
    Ok(tag_vec)
}

/// Get search keywords for autocomplete
fn get_search_keywords() -> Vec<String> {
    vec![
        "tag:".to_string(),
        "title:".to_string(),
        "content:".to_string(),
        "created:".to_string(),
        "modified:".to_string(),
        "recent".to_string(),
        "today".to_string(),
        "yesterday".to_string(),
        "week".to_string(),
        "month".to_string(),
    ]
}

/// Provide autocomplete suggestions for CLI users
///
/// This command helps CLI users discover available notes and get autocomplete
/// suggestions without entering interactive mode. Shows cycling order and
/// modification time-based sorting.
///
/// # Arguments
/// * `partial` - Partial text to autocomplete
/// * `completion_type` - Type of completion: "notes", "search", or "all"
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not found
/// - Cannot read notes directory
///
/// # Example
/// ```no_run
/// use tesela::autocomplete_suggestions;
/// autocomplete_suggestions("my", "notes")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn autocomplete_suggestions(partial: &str, completion_type: &str) -> Result<()> {
    if !Path::new("tesela.toml").exists() {
        println!("❌ No mosaic found. Run 'tesela init' first.");
        return Ok(());
    }

    let completion = match completion_type {
        "notes" => SmartCompletion::new_for_notes()?,
        "search" => SmartCompletion::new_for_search()?,
        _ => SmartCompletion::new_for_notes()?, // Default to notes
    };

    let matches = completion.find_matches(partial);

    if matches.is_empty() {
        println!("🔍 No matches found for '{}'", partial);
        return Ok(());
    }

    println!("💡 Tab completion for '{}':", partial);
    println!("📋 Notes are ordered by modification time (most recent first)");
    println!();

    for (i, suggestion) in matches.iter().take(10).enumerate() {
        if i == 0 {
            println!("  TAB 1: {} ← First completion", suggestion);
        } else {
            println!("  TAB {}: {}", i + 1, suggestion);
        }
    }

    if matches.len() > 10 {
        println!("  ... and {} more matches", matches.len() - 10);
    }

    if matches.len() > 1 {
        println!();
        println!("🔄 In interactive mode:");
        println!("   • Type '{}' and press TAB → '{}'", partial, matches[0]);
        println!(
            "   • Press TAB again → '{}'",
            matches.get(1).unwrap_or(&matches[0])
        );
        if matches.len() > 2 {
            println!(
                "   • Press TAB again → '{}'",
                matches.get(2).unwrap_or(&matches[0])
            );
        }
        println!("   • Cycles through all {} matches", matches.len());
    }

    Ok(())
}

/// Clear the terminal screen
fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
    std::io::stdout().flush().unwrap_or_default();
}

pub fn interactive_mode() -> Result<()> {
    use dialoguer::{theme::ColorfulTheme, Input};
    use std::io::{self, Write};

    clear_screen();
    println!("🔮 Welcome to Tesela Interactive Mode");
    println!("✨ Single keystrokes for lightning-fast note management!");
    println!();

    let theme = ColorfulTheme::default();

    loop {
        // Clear screen for clean interface
        clear_screen();
        println!("🔮 Tesela Interactive Mode");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Show current status
        let status = if Path::new("tesela.toml").exists() {
            "📚 Mosaic Ready"
        } else {
            "⚠️  No Mosaic Found"
        };
        println!("{}", status);
        println!();

        // Display menu with keystroke shortcuts
        println!("🚀 Quick Commands:");
        println!("╭─────────────────────────────┬─────────────────────────────╮");
        println!("│ [N] 📝 Create new note      │ [L] 📚 List notes           │");
        println!("│ [S] 🔍 Search notes         │ [E] 📝 Edit note            │");
        println!("│ [K] 🔗 Link notes           │ [G] 🕸️  Show graph          │");
        println!("│ [D] 📅 Daily note           │ [B] 💾 Backup               │");
        println!("│ [I] 📥 Import               │ [M] ⚙️  Initialize mosaic   │");
        println!("│ [H] ❓ Help                 │ [Q] 🚪 Quit                 │");
        println!("╰─────────────────────────────┴─────────────────────────────╯");
        println!();

        print!("💫 Choose action: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        let action = match input.as_str() {
            "n" | "new" | "create" => 0,
            "l" | "list" => 1,
            "s" | "search" => 2,
            "e" | "edit" => 3,
            "k" | "link" => 4,
            "g" | "graph" => 5,
            "d" | "daily" => 6,
            "b" | "backup" => 7,
            "i" | "import" => 8,
            "m" | "mosaic" | "init" => 9,
            "h" | "help" => 10,
            "q" | "quit" | "exit" => 11,
            _ => {
                println!("❌ Unknown command '{}'. Try 'h' for help.", input);
                continue;
            }
        };

        match action {
            0 => {
                // Create new note
                println!("\n📝 Creating new note...");
                let title: String = Input::with_theme(&theme)
                    .with_prompt("Note title")
                    .interact_text()?;

                if let Err(e) = create_note(&title) {
                    println!("❌ Error: {}", e);
                } else {
                    println!("✅ Note created successfully!");
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            1 => {
                // List notes
                println!("\n📚 Recent notes:");
                if let Err(e) = list_notes() {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            2 => {
                // Search notes
                println!("\n🔍 Searching notes...");
                let completion =
                    SmartCompletion::new_for_search().unwrap_or_else(|_| SmartCompletion {
                        notes: Vec::new(),
                        tags: Vec::new(),
                        search_keywords: Vec::new(),
                        completion_type: CompletionType::Search,
                        cycle_state: RefCell::new(CycleState {
                            original_input: String::new(),
                            current_matches: Vec::new(),
                            current_index: 0,
                        }),
                    });
                let query: String = Input::with_theme(&theme)
                    .with_prompt("Search query (tab: notes/tags/keywords)")
                    .completion_with(&completion)
                    .interact_text()?;

                if let Err(e) = search_notes(&query) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            3 => {
                // View/Edit note
                println!("\n📝 Opening note in editor...");
                let completion =
                    SmartCompletion::new_for_notes().unwrap_or_else(|_| SmartCompletion {
                        notes: Vec::new(),
                        tags: Vec::new(),
                        search_keywords: Vec::new(),
                        completion_type: CompletionType::Notes,
                        cycle_state: RefCell::new(CycleState {
                            original_input: String::new(),
                            current_matches: Vec::new(),
                            current_index: 0,
                        }),
                    });
                let note: String = Input::with_theme(&theme)
                    .with_prompt("Note identifier (tab to autocomplete)")
                    .completion_with(&completion)
                    .interact_text()?;

                if let Err(e) = open_note_in_editor(&note) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            4 => {
                // Link notes
                println!("\n🔗 Creating note links...");
                let completion =
                    SmartCompletion::new_for_notes().unwrap_or_else(|_| SmartCompletion {
                        notes: Vec::new(),
                        tags: Vec::new(),
                        search_keywords: Vec::new(),
                        completion_type: CompletionType::Notes,
                        cycle_state: RefCell::new(CycleState {
                            original_input: String::new(),
                            current_matches: Vec::new(),
                            current_index: 0,
                        }),
                    });
                let from: String = Input::with_theme(&theme)
                    .with_prompt("From note (tab to autocomplete)")
                    .completion_with(&completion)
                    .interact_text()?;
                let to: String = Input::with_theme(&theme)
                    .with_prompt("To note (tab to autocomplete)")
                    .completion_with(&completion)
                    .interact_text()?;

                if let Err(e) = link_notes(&from, &to) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            5 => {
                // Show graph
                println!("\n🕸️  Showing note connections...");
                let completion =
                    SmartCompletion::new_for_notes().unwrap_or_else(|_| SmartCompletion {
                        notes: Vec::new(),
                        tags: Vec::new(),
                        search_keywords: Vec::new(),
                        completion_type: CompletionType::Notes,
                        cycle_state: RefCell::new(CycleState {
                            original_input: String::new(),
                            current_matches: Vec::new(),
                            current_index: 0,
                        }),
                    });
                let note: String = Input::with_theme(&theme)
                    .with_prompt("Note identifier (tab to autocomplete)")
                    .completion_with(&completion)
                    .interact_text()?;

                if let Err(e) = show_graph(&note) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            6 => {
                // Daily note
                println!("\n📅 Opening daily note in editor...");
                if let Err(e) = daily_note_and_edit() {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            7 => {
                // Backup
                println!("\n💾 Creating backup...");
                if let Err(e) = backup_mosaic() {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            8 => {
                // Import
                println!("\n📥 Importing notes...");
                let path: String = Input::with_theme(&theme)
                    .with_prompt("Import path")
                    .interact_text()?;

                if let Err(e) = import_notes(&path) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            9 => {
                // Initialize mosaic
                println!("\n⚙️  Initializing mosaic...");
                let path: String = Input::with_theme(&theme)
                    .with_prompt("Mosaic path")
                    .default(".".to_string())
                    .interact_text()?;

                if let Err(e) = init_mosaic(&path) {
                    println!("❌ Error: {}", e);
                }

                println!("\nPress Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            10 => {
                // Help
                clear_screen();
                println!("📖 Tesela Interactive Mode Help");
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!();
                println!("🚀 Single Keystroke Shortcuts:");
                println!("  N - Create new note (Start a new markdown note)");
                println!("  L - List notes (Show all recent notes)");
                println!("  S - Search notes (Full-text search across notes)");
                println!("  E - Edit note (Open note in external editor)");
                println!("  K - Link notes (Create bidirectional links)");
                println!("  G - Show graph (Display note connections)");
                println!("  D - Daily note (Create/open today's daily note)");
                println!("  B - Backup (Create timestamped backup)");
                println!("  I - Import (Import notes from files/directories)");
                println!("  M - Initialize mosaic (Set up new knowledge base)");
                println!("  H - Help (Show this help message)");
                println!("  Q - Quit (Exit interactive mode)");
                println!();
                println!("💡 Features:");
                println!("  • Tab autocomplete with cycling (multiple tabs cycle through matches)");
                println!("  • Notes ordered by modification time (most recent first)");
                println!("  • Vim integration for seamless editing");
                println!("  • Context-aware suggestions for notes vs. search");
                println!();

                println!("Press Enter to continue...");
                let mut dummy = String::new();
                io::stdin().read_line(&mut dummy)?;
            }
            11 => {
                // Quit
                clear_screen();
                println!("👋 Goodbye! Your knowledge mosaic awaits your return.");
                break;
            }
            _ => unreachable!(),
        }

        println!();
    }

    Ok(())
}

/// Generate shell completions for Tesela
///
/// This command generates shell completion scripts for various shells
/// to enable tab completion of Tesela commands and options.
///
/// # Arguments
/// * `shell` - Target shell (bash, zsh, fish, powershell, elvish)
///
/// # Errors
/// Returns an error if:
/// - Shell type is not supported
/// - Completion generation fails
///
/// # Example
/// ```no_run
/// use tesela::generate_completions;
/// generate_completions("bash")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn generate_completions(shell: &str) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, shells::*};
    use std::io;

    // Create our CLI command structure
    #[derive(clap::Parser)]
    #[command(name = "tesela")]
    #[command(about = "A keyboard-first, file-based note-taking system")]
    struct Cli {
        #[command(subcommand)]
        command: Option<Commands>,
    }

    #[derive(clap::Subcommand)]
    enum Commands {
        Init { path: Option<String> },
        New { title: String },
        List,
        Cat { note: String },
        Attach { note: String, file: String },
        Export { note: String, format: String },
        Search { query: String },
        Link { from: String, to: String },
        Graph { note: String },
        Daily,
        Backup,
        Import { path: String },
        Interactive,
        Completions { shell: String },
    }

    let mut cmd = Cli::command();
    let shell_type = match shell.to_lowercase().as_str() {
        "bash" => {
            println!("# Generating bash completions for Tesela");
            println!("# To install, add this to your ~/.bashrc:");
            println!("# eval \"$(tesela completions bash)\"");
            println!();
            generate(Bash, &mut cmd, "tesela", &mut io::stdout());
            return Ok(());
        }
        "zsh" => {
            println!("# Generating zsh completions for Tesela");
            println!("# To install, add this to your ~/.zshrc:");
            println!("# eval \"$(tesela completions zsh)\"");
            println!();
            generate(Zsh, &mut cmd, "tesela", &mut io::stdout());
            return Ok(());
        }
        "fish" => {
            println!("# Generating fish completions for Tesela");
            println!("# To install, save this to ~/.config/fish/completions/tesela.fish");
            println!();
            generate(Fish, &mut cmd, "tesela", &mut io::stdout());
            return Ok(());
        }
        "powershell" | "pwsh" => {
            println!("# Generating PowerShell completions for Tesela");
            println!("# To install, add this to your PowerShell profile");
            println!();
            generate(PowerShell, &mut cmd, "tesela", &mut io::stdout());
            return Ok(());
        }
        "elvish" => {
            println!("# Generating Elvish completions for Tesela");
            println!();
            generate(Elvish, &mut cmd, "tesela", &mut io::stdout());
            return Ok(());
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported shell '{}'. Supported shells: bash, zsh, fish, powershell, elvish",
                shell
            ));
        }
    };
}

/// Run basic performance benchmarks
///
/// This command runs a series of performance tests to measure the speed
/// of various Tesela operations and provides timing information.
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Benchmark operations fail
///
/// # Example
/// ```no_run
/// use tesela::benchmark_performance;
/// benchmark_performance()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn benchmark_performance() -> Result<()> {
    use std::time::Instant;

    println!("🏃 Running Tesela Performance Benchmarks");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    let notes_dir = Path::new("notes");
    if !notes_dir.exists() {
        println!("⚠️  No notes directory found, creating test notes...");
        fs::create_dir_all(notes_dir)?;
    }

    // Benchmark 1: Note Creation
    println!("📝 Benchmarking note creation...");
    let start = Instant::now();

    for i in 0..10 {
        let test_title = format!("Benchmark Note {}", i);
        let _ = create_note(&test_title);
    }

    let creation_time = start.elapsed();
    println!(
        "   ✅ Created 10 notes in {:?} ({:.2}ms per note)",
        creation_time,
        creation_time.as_millis() as f64 / 10.0
    );

    // Benchmark 2: Note Listing
    println!("📚 Benchmarking note listing...");
    let start = Instant::now();

    for _ in 0..50 {
        let _ = list_notes();
    }

    let listing_time = start.elapsed();
    println!(
        "   ✅ Listed notes 50 times in {:?} ({:.2}ms per list)",
        listing_time,
        listing_time.as_millis() as f64 / 50.0
    );

    // Benchmark 3: Search Performance
    println!("🔍 Benchmarking search performance...");
    let start = Instant::now();

    for _ in 0..20 {
        let _ = search_notes("Benchmark");
    }

    let search_time = start.elapsed();
    println!(
        "   ✅ Searched 20 times in {:?} ({:.2}ms per search)",
        search_time,
        search_time.as_millis() as f64 / 20.0
    );

    // Benchmark 4: File Operations
    println!("📁 Benchmarking file operations...");
    let start = Instant::now();

    let test_file_path = "benchmark_test.txt";
    fs::write(test_file_path, "Benchmark test content")?;

    for i in 0..5 {
        let note_name = format!("benchmark-note-{}", i);
        let _ = attach_file(&note_name, test_file_path);
    }

    fs::remove_file(test_file_path)?;
    let attach_time = start.elapsed();
    println!(
        "   ✅ Attached file 5 times in {:?} ({:.2}ms per attachment)",
        attach_time,
        attach_time.as_millis() as f64 / 5.0
    );

    // Summary
    println!();
    println!("📊 Benchmark Summary:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!(
        "Note Creation:  {:.2}ms per note",
        creation_time.as_millis() as f64 / 10.0
    );
    println!(
        "Note Listing:   {:.2}ms per list",
        listing_time.as_millis() as f64 / 50.0
    );
    println!(
        "Search Query:   {:.2}ms per search",
        search_time.as_millis() as f64 / 20.0
    );
    println!(
        "File Attach:    {:.2}ms per attachment",
        attach_time.as_millis() as f64 / 5.0
    );

    let total_time = creation_time + listing_time + search_time + attach_time;
    println!("Total Time:     {:?}", total_time);

    // Performance assessment
    let avg_creation = creation_time.as_millis() as f64 / 10.0;
    let avg_search = search_time.as_millis() as f64 / 20.0;

    println!();
    if avg_creation < 50.0 && avg_search < 100.0 {
        println!("🚀 Performance: Excellent! All operations are fast.");
    } else if avg_creation < 100.0 && avg_search < 200.0 {
        println!("✅ Performance: Good! Operations are reasonably fast.");
    } else {
        println!("⚠️  Performance: Consider optimization for large mosaics.");
    }

    println!("💡 Run benchmarks periodically to monitor performance");

    Ok(())
}

// Helper functions for CLI commands

/// Find notes matching a given identifier
fn find_matching_notes(notes_dir: &Path, identifier: &str) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(notes_dir).context("Failed to read notes directory")?;
    let mut matches = Vec::new();
    let search_term = identifier.to_lowercase();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("md") {
            let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let filename_lower = filename.to_lowercase();
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();

            // Check for exact match (with or without .md)
            if filename_lower == search_term || stem == search_term {
                matches.clear();
                matches.push(path);
                break;
            }

            // Also check title from file content
            let mut title_match = false;
            if let Ok(content) = fs::read_to_string(&path) {
                let title = extract_title_from_content(&content, &stem);
                let title_lower = title.to_lowercase();

                // Check for exact title match
                if title_lower == search_term {
                    matches.clear();
                    matches.push(path);
                    break;
                }

                // Check for partial title match
                if title_lower.contains(&search_term) {
                    title_match = true;
                }
            }

            // Check for partial match in filename, stem, or title
            if filename_lower.contains(&search_term) || stem.contains(&search_term) || title_match {
                matches.push(path);
            }
        }
    }

    Ok(matches)
}

/// Recursively copy a directory
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }

    Ok(())
}

/// Count files in a directory
fn count_files(dir: &Path) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        }
    }

    Ok(count)
}

/// Import a single file as a note
fn import_single_file(source: &Path, notes_dir: &Path) -> Result<usize> {
    let extension = source.extension().and_then(|s| s.to_str());

    match extension {
        Some("md") | Some("markdown") => {
            // Copy markdown files directly
            let filename = source.file_name().unwrap();
            let dest = notes_dir.join(filename);
            fs::copy(source, dest)?;
            println!("📄 Imported: {}", filename.to_string_lossy());
            Ok(1)
        }
        Some("txt") => {
            // Convert text files to markdown
            let content = fs::read_to_string(source)?;
            let filename = source.file_stem().unwrap().to_string_lossy();
            let title = filename.replace("_", " ").replace("-", " ");

            let markdown_content = format!(
                r#"---
title: "{}"
created: {}
tags: ["imported"]
---

# {}

{}
"#,
                title,
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                title,
                content
            );

            let dest = notes_dir.join(format!("{}.md", filename));
            fs::write(dest, markdown_content)?;
            println!("📄 Converted and imported: {}.md", filename);
            Ok(1)
        }
        _ => {
            println!("⚠️  Skipping unsupported file: {}", source.display());
            Ok(0)
        }
    }
}

/// Import all compatible files from a directory
fn import_directory(source: &Path, notes_dir: &Path) -> Result<usize> {
    let mut imported_count = 0;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            imported_count += import_single_file(&path, notes_dir)?;
        } else if path.is_dir() {
            // Recursively import subdirectories
            imported_count += import_directory(&path, notes_dir)?;
        }
    }

    Ok(imported_count)
}

/// Search for notes using full-text search
///
/// This command searches through all notes in the mosaic using the provided query.
/// It supports basic text matching and displays results with context and relevance.
///
/// # Arguments
/// * `query` - The search query string
///
/// # Errors
/// Returns an error if:
/// - Mosaic is not initialized
/// - Database cannot be accessed
/// - Search query is invalid
///
/// # Example
/// ```no_run
/// use tesela::search_notes;
/// search_notes("rust programming")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn search_notes(query: &str) -> Result<()> {
    println!("🔍 Searching for: '{}'", query);

    // Check if mosaic exists
    if !Path::new("tesela.toml").exists() {
        return Err(anyhow::anyhow!(
            "No mosaic found. Run 'tesela init' first to create one."
        ));
    }

    if query.trim().is_empty() {
        return Err(anyhow::anyhow!("Search query cannot be empty"));
    }

    // For now, implement a simple file-based search
    // Later this will be replaced with proper database search
    let mut matches = Vec::new();
    let search_term = query.to_lowercase();

    // Search through both notes and dailies directories
    let directories = ["notes", "dailies"];

    for dir_name in directories {
        let dir_path = Path::new(dir_name);
        if !dir_path.exists() {
            continue;
        }

        let entries = match fs::read_dir(dir_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        // Search through all markdown files in this directory
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if content.to_lowercase().contains(&search_term) {
                        let filename = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        let title = extract_title_from_content(&content, filename);

                        // Find matching lines for context
                        let matching_lines: Vec<_> = content
                            .lines()
                            .enumerate()
                            .filter(|(_, line)| line.to_lowercase().contains(&search_term))
                            .take(3) // Limit to first 3 matches per file
                            .map(|(num, line)| (num, line.to_string()))
                            .collect();

                        // Include directory info for context
                        let display_filename = if dir_name == "dailies" {
                            format!("{} (daily)", filename)
                        } else {
                            filename.to_string()
                        };

                        matches.push((title, display_filename, matching_lines));
                    }
                }
            }
        }
    }

    // Check if we found any directories to search
    if !Path::new("notes").exists() && !Path::new("dailies").exists() {
        println!("📂 No notes or dailies directories found");
        return Ok(());
    }

    // Display results
    if matches.is_empty() {
        println!("❌ No notes found matching '{}'", query);
        println!("💡 Try a different search term or check your spelling");
    } else {
        println!("📄 Found {} note(s) matching '{}':", matches.len(), query);
        println!();

        for (title, filename, lines) in matches {
            println!("📝 {} [{}]", title, filename);

            for (line_num, line) in lines {
                let highlighted = line.replace(
                    &query.to_lowercase(),
                    &format!("**{}**", query.to_lowercase()),
                );
                println!("   {}. {}", line_num + 1, highlighted.trim());
            }
            println!();
        }

        println!("💡 Use 'tesela cat <note>' to view full content");
    }

    Ok(())
}

/// Extracts a title from note content using multiple strategies.
///
/// # Priority
/// 1. YAML frontmatter `title:` field
/// 2. First `# Heading` in the content
/// 3. Fallback string with dashes/underscores replaced by spaces
///
/// # Arguments
/// * `content` - The full markdown content of the note
/// * `fallback` - Filename to use if no title is found
fn extract_title_from_content(content: &str, fallback: &str) -> String {
    // Look for title in frontmatter first
    if content.starts_with("---") {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_frontmatter = false;

        for line in lines {
            if line.trim() == "---" {
                if in_frontmatter {
                    break;
                } else {
                    in_frontmatter = true;
                    continue;
                }
            }

            if in_frontmatter && line.starts_with("title:") {
                if let Some(title) = line.strip_prefix("title:") {
                    let title = title.trim().trim_matches('"').trim_matches('\'');
                    if !title.is_empty() {
                        return title.to_string();
                    }
                }
            }
        }
    }

    // Look for first # heading
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            let title = trimmed.strip_prefix("# ").unwrap_or("").trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }

    // Fallback to filename
    fallback.replace('-', " ").replace('_', " ")
}

/// Formats a `SystemTime` as a human-readable relative time string.
///
/// # Examples
/// - Less than 60 seconds: "just now"
/// - 1-59 minutes: "X minutes ago"
/// - 1-23 hours: "X hours ago"
/// - 1-6 days: "X days ago"
/// - 1-4 weeks: "X weeks ago"
/// - 1+ months: "X months ago"
fn format_time_ago(time: SystemTime) -> String {
    if let Ok(duration) = time.elapsed() {
        let seconds = duration.as_secs();

        if seconds < 60 {
            "just now".to_string()
        } else if seconds < 3600 {
            let minutes = seconds / 60;
            if minutes == 1 {
                "1 minute ago".to_string()
            } else {
                format!("{} minutes ago", minutes)
            }
        } else if seconds < 86400 {
            let hours = seconds / 3600;
            if hours == 1 {
                "1 hour ago".to_string()
            } else {
                format!("{} hours ago", hours)
            }
        } else {
            let days = seconds / 86400;
            if days == 1 {
                "1 day ago".to_string()
            } else if days < 7 {
                format!("{} days ago", days)
            } else if days < 30 {
                let weeks = days / 7;
                if weeks == 1 {
                    "1 week ago".to_string()
                } else {
                    format!("{} weeks ago", weeks)
                }
            } else {
                let months = days / 30;
                if months == 1 {
                    "1 month ago".to_string()
                } else {
                    format!("{} months ago", months)
                }
            }
        }
    } else {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[test]
    fn test_simple_extract_title() {
        // Test without any file system operations
        assert_eq!(extract_title_from_content("# Hello", "fallback"), "Hello");
    }

    #[test]
    fn test_extract_title_from_frontmatter() {
        let content = r#"---
title: "My Test Note"
created: 2024-01-01
---

# Some Heading

Content here."#;
        assert_eq!(
            extract_title_from_content(content, "fallback"),
            "My Test Note"
        );
    }

    #[test]
    fn test_extract_title_from_heading() {
        let content = r#"# This is the Heading

No frontmatter here, just content."#;
        assert_eq!(
            extract_title_from_content(content, "fallback"),
            "This is the Heading"
        );
    }

    #[test]
    fn test_extract_title_fallback() {
        let content = "Just some content without title or heading.";
        assert_eq!(
            extract_title_from_content(content, "my-filename"),
            "my filename"
        );
    }

    #[test]
    fn test_init_mosaic() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let path = temp_dir.path().join("test-mosaic");

        init_mosaic(path.to_str().unwrap())?;

        assert!(path.join("tesela.toml").exists());
        assert!(path.join("notes").exists());
        assert!(path.join("attachments").exists());

        Ok(())
    }

    #[test]
    fn test_create_note() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // First init a mosaic
        init_mosaic(".")?;

        // Then create a note
        create_note("Test Note")?;

        let note_path = Path::new("notes/test-note.md");
        assert!(note_path.exists());

        let content = fs::read_to_string(note_path)?;
        assert!(content.contains("title: \"Test Note\""));

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_list_notes_no_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = list_notes();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No mosaic found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_list_notes_empty() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic
        init_mosaic(".")?;

        // List should work but show no notes
        list_notes()?;

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_list_notes_with_notes() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic and create some notes
        init_mosaic(".")?;
        create_note("First Note")?;
        create_note("Second Note")?;
        create_note("Third Note")?;

        // List should work and show notes
        list_notes()?;

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_format_time_ago() {
        use std::time::Duration;

        let now = SystemTime::now();
        assert_eq!(format_time_ago(now), "just now");

        let one_minute_ago = now - Duration::from_secs(60);
        assert_eq!(format_time_ago(one_minute_ago), "1 minute ago");

        let five_minutes_ago = now - Duration::from_secs(300);
        assert_eq!(format_time_ago(five_minutes_ago), "5 minutes ago");

        let one_hour_ago = now - Duration::from_secs(3600);
        assert_eq!(format_time_ago(one_hour_ago), "1 hour ago");

        let one_day_ago = now - Duration::from_secs(86400);
        assert_eq!(format_time_ago(one_day_ago), "1 day ago");

        let one_week_ago = now - Duration::from_secs(604800);
        assert_eq!(format_time_ago(one_week_ago), "1 week ago");
    }

    #[test]
    fn test_cat_note_no_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(temp_dir.path()).unwrap();

        let result = cat_note("test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No mosaic found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_cat_note_exact_match() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic and create a note
        init_mosaic(".")?;
        create_note("Test Note")?;

        // Test exact match without extension
        let result = cat_note("test-note");
        assert!(result.is_ok());

        // Test exact match with extension
        let result = cat_note("test-note.md");
        assert!(result.is_ok());

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_cat_note_partial_match() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic and create a note
        init_mosaic(".")?;
        create_note("My Special Note")?;

        // Test partial match
        let result = cat_note("special");
        assert!(result.is_ok());

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_cat_note_multiple_matches() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic and create multiple notes with similar names
        init_mosaic(".")?;
        create_note("Test Note One")?;
        create_note("Test Note Two")?;

        // Test ambiguous match
        let result = cat_note("test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Multiple matches found"));

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_cat_note_no_matches() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic with some notes
        init_mosaic(".")?;
        create_note("First Note")?;
        create_note("Second Note")?;

        // Test non-existent note
        let result = cat_note("nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No notes found matching"));

        env::set_current_dir(original_dir)?;
        Ok(())
    }

    #[test]
    fn test_cat_note_no_notes_directory() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let original_dir = env::current_dir()?;
        env::set_current_dir(temp_dir.path())?;

        // Initialize mosaic and remove notes directory
        init_mosaic(".")?;
        fs::remove_dir("notes")?;

        // Test with missing notes directory
        let result = cat_note("any");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No notes directory found"));

        env::set_current_dir(original_dir)?;
        Ok(())
    }
}
