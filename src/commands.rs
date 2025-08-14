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
use std::fs;
use std::path::Path;
use std::time::SystemTime;

/// Initializes a new Tesela mosaic (knowledge base) at the specified path.
///
/// Creates the following directory structure:
/// ```
/// path/
/// ├── tesela.toml      # Configuration file
/// ├── notes/           # Directory for markdown notes
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

    println!("📁 Creating notes/ and attachments/ directories...");

    fs::create_dir_all(&notes_dir).context("Failed to create notes directory")?;

    fs::create_dir_all(&attachments_dir).context("Failed to create attachments directory")?;

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

    // Create note content with frontmatter
    let note_content = format!(
        r#"---
title: "{}"
created: {}
tags: []
---

# {}

Your note content goes here...

## Links
- Link to other notes with [[Note Name]]

## Tags
Use #tag to add tags to your note
"#,
        title,
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        title
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
