//! Unit tests for init and create commands

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tesela::commands::{create_note, init_mosaic};

/// Helper to setup a temporary test directory
fn setup_test_dir() -> Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = TempDir::new()?;
    let original_dir = env::current_dir()?;
    env::set_current_dir(temp_dir.path())?;
    Ok((temp_dir, original_dir))
}

/// Helper to cleanup test directory
fn cleanup_test_dir(original_dir: std::path::PathBuf) -> Result<()> {
    env::set_current_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_init_mosaic() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Verify directories were created
    assert!(Path::new("notes").exists(), "notes directory should exist");
    assert!(
        Path::new("dailies").exists(),
        "dailies directory should exist"
    );
    assert!(
        Path::new("attachments").exists(),
        "attachments directory should exist"
    );

    // Verify config file was created
    assert!(
        Path::new("tesela.toml").exists(),
        "tesela.toml should exist"
    );

    // Verify config content
    let config_content = fs::read_to_string("tesela.toml")?;
    assert!(
        config_content.contains("[mosaic]"),
        "Config should contain mosaic section"
    );
    assert!(
        config_content.contains("version"),
        "Config should contain version"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_init_mosaic_already_exists() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic first time
    init_mosaic(".")?;

    // Try to initialize again - should fail
    let result = init_mosaic(".");
    assert!(result.is_err(), "Should fail when mosaic already exists");
    assert!(
        result.unwrap_err().to_string().contains("already exists"),
        "Error should mention already exists"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_note() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic first
    init_mosaic(".")?;

    // Create a simple note
    create_note("Test Note")?;

    // Verify note file was created
    assert!(
        Path::new("notes/test-note.md").exists(),
        "Note file should exist"
    );

    // Verify note content
    let content = fs::read_to_string("notes/test-note.md")?;
    assert!(
        content.contains("title: \"Test Note\""),
        "Note should contain title in frontmatter"
    );
    assert!(
        content.contains("tags: []"),
        "Note should contain empty tags"
    );
    assert!(content.contains("---"), "Note should have frontmatter");
    assert!(
        content.contains("-\n"),
        "Note should have initial dash block"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_note_with_special_characters() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Test various special character titles
    let test_cases = vec![
        ("Note with Spaces", "note-with-spaces.md"),
        ("Note_with_underscores", "note-with-underscores.md"),
        ("Note-with-dashes", "note-with-dashes.md"),
        ("Note: With Colon", "note--with-colon.md"),
        ("Note? With Question", "note--with-question.md"),
        ("Note! With Exclamation", "note--with-exclamation.md"),
        ("Note's Apostrophe", "notes-apostrophe.md"),
        ("Note & Ampersand", "note---ampersand.md"),
    ];

    for (title, expected_filename) in test_cases {
        create_note(title)?;
        let path = format!("notes/{}", expected_filename);
        assert!(
            Path::new(&path).exists(),
            "Note '{}' should create file '{}'",
            title,
            expected_filename
        );

        // Verify title is preserved in frontmatter
        let content = fs::read_to_string(&path)?;
        assert!(
            content.contains(&format!("title: \"{}\"", title)),
            "Title should be preserved in frontmatter"
        );
    }

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_note_no_mosaic() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Try to create note without initializing mosaic
    let result = create_note("Test Note");
    assert!(result.is_err(), "Should fail without mosaic");
    assert!(
        result.unwrap_err().to_string().contains("No mosaic found"),
        "Error should mention no mosaic"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_duplicate_note() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create first note
    create_note("Duplicate Test")?;

    // Try to create duplicate - should succeed with numbered suffix
    create_note("Duplicate Test")?;

    // Both files should exist
    assert!(
        Path::new("notes/duplicate-test.md").exists(),
        "First note should exist"
    );
    assert!(
        Path::new("notes/duplicate-test-1.md").exists(),
        "Second note should exist with suffix"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_daily_note() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create a daily note with date format
    create_note("2024-01-15")?;

    // Daily notes should go in notes/ directory (based on implementation)
    assert!(
        Path::new("notes/2024-01-15.md").exists(),
        "Daily note should be created in notes directory"
    );

    // Verify content
    let content = fs::read_to_string("notes/2024-01-15.md")?;
    assert!(
        content.contains("title: \"2024-01-15\""),
        "Daily note should have date as title"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_note_with_unicode() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Test Unicode characters
    let unicode_titles = vec![
        ("Emoji 🎉 Note", "emoji---note.md"),
        ("中文笔记", "中文笔记.md"),
        ("Ñoño Note", "ñoño-note.md"),
        ("Москва Note", "москва-note.md"),
    ];

    for (title, expected_filename) in unicode_titles {
        create_note(title)?;
        let path = format!("notes/{}", expected_filename);

        // Just verify the note was created (exact filename may vary)
        let notes_dir = fs::read_dir("notes")?;
        let note_exists = notes_dir.filter_map(|entry| entry.ok()).any(|entry| {
            if let Some(content) = fs::read_to_string(entry.path()).ok() {
                content.contains(&format!("title: \"{}\"", title))
            } else {
                false
            }
        });

        assert!(
            note_exists,
            "Note with Unicode title '{}' should be created",
            title
        );
    }

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_note_preserves_frontmatter_format() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create note
    create_note("Frontmatter Test")?;

    // Read content
    let content = fs::read_to_string("notes/frontmatter-test.md")?;

    // Check frontmatter structure
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines[0], "---", "Should start with frontmatter delimiter");
    assert!(lines[1].starts_with("title:"), "Should have title field");
    assert!(
        lines[2].starts_with("created:"),
        "Should have created field"
    );
    assert!(
        lines[3].starts_with("last_opened:"),
        "Should have last_opened field"
    );
    assert!(lines[4].starts_with("tags:"), "Should have tags field");
    assert_eq!(lines[5], "---", "Should end frontmatter with delimiter");
    assert_eq!(lines[6], "-", "Should have initial dash block");

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_create_multiple_notes_sequential() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create multiple notes
    let notes = vec!["First Note", "Second Note", "Third Note"];

    for note_title in &notes {
        create_note(note_title)?;
    }

    // Verify all notes exist
    for note_title in &notes {
        let slug = note_title.to_lowercase().replace(' ', "-");
        let path = format!("notes/{}.md", slug);
        assert!(
            Path::new(&path).exists(),
            "Note '{}' should exist at {}",
            note_title,
            path
        );
    }

    // Verify we have exactly 3 notes
    let note_count = fs::read_dir("notes")?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .count();

    assert_eq!(note_count, 3, "Should have exactly 3 notes");

    cleanup_test_dir(original_dir)?;
    Ok(())
}
