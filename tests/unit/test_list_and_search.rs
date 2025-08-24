//! Unit tests for list and search commands

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;
use tesela::commands::{
    create_note, format_time_ago, get_notes_with_paths, init_mosaic, list_notes, search_notes,
};

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
fn test_list_notes_sorting() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create notes with delays to ensure different timestamps
    create_note("First Note")?;
    thread::sleep(Duration::from_millis(100));

    create_note("Second Note")?;
    thread::sleep(Duration::from_millis(100));

    create_note("Third Note")?;

    // Get notes list
    let notes = get_notes_with_paths()?;

    // Verify count
    assert_eq!(notes.len(), 3, "Should have 3 notes");

    // Verify order - newest first
    assert!(
        notes[0].1.contains("Third"),
        "First item should be 'Third Note' (newest)"
    );
    assert!(
        notes[1].1.contains("Second"),
        "Second item should be 'Second Note'"
    );
    assert!(
        notes[2].1.contains("First"),
        "Third item should be 'First Note' (oldest)"
    );

    // Verify timestamps are in descending order
    for i in 0..notes.len() - 1 {
        assert!(
            notes[i].2 >= notes[i + 1].2,
            "Notes should be sorted by modification time (newest first)"
        );
    }

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_list_notes_empty_mosaic() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic but don't create any notes
    init_mosaic(".")?;

    // List should work but return empty
    let notes = get_notes_with_paths()?;
    assert_eq!(notes.len(), 0, "Should have no notes");

    // The list_notes command should not error
    list_notes()?;

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_list_notes_with_modification() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create notes with delays
    create_note("Note A")?;
    thread::sleep(Duration::from_millis(100));

    create_note("Note B")?;
    thread::sleep(Duration::from_millis(100));

    create_note("Note C")?;
    thread::sleep(Duration::from_millis(100));

    // Modify the first note
    let note_a_path = Path::new("notes/note-a.md");
    let content = fs::read_to_string(note_a_path)?;
    fs::write(note_a_path, format!("{}\n\nModified content", content))?;

    // Get notes list
    let notes = get_notes_with_paths()?;

    // Note A should now be first (most recently modified)
    assert!(
        notes[0].1.contains("Note A"),
        "Modified note should be first"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_search_notes_basic() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create notes with specific content
    create_note("Search Test 1")?;
    fs::write(
        "notes/search-test-1.md",
        "---\ntitle: \"Search Test 1\"\ntags: []\n---\n# Search Test 1\n\nThis note contains the word apple.",
    )?;

    create_note("Search Test 2")?;
    fs::write(
        "notes/search-test-2.md",
        "---\ntitle: \"Search Test 2\"\ntags: []\n---\n# Search Test 2\n\nThis note contains the word banana.",
    )?;

    create_note("Search Test 3")?;
    fs::write(
        "notes/search-test-3.md",
        "---\ntitle: \"Search Test 3\"\ntags: []\n---\n# Search Test 3\n\nThis note contains both apple and banana.",
    )?;

    // Search for "apple"
    let results = search_notes("apple")?;
    assert_eq!(results.len(), 2, "Should find 2 notes containing 'apple'");

    // Search for "banana"
    let results = search_notes("banana")?;
    assert_eq!(results.len(), 2, "Should find 2 notes containing 'banana'");

    // Search for both
    let results = search_notes("apple banana")?;
    assert!(
        results.len() >= 1,
        "Should find at least 1 note with both terms"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_search_notes_case_insensitive() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create note with mixed case content
    create_note("Case Test")?;
    fs::write(
        "notes/case-test.md",
        "---\ntitle: \"Case Test\"\ntags: []\n---\n# Case Test\n\nThis contains UPPERCASE, lowercase, and MiXeD case.",
    )?;

    // Search with different cases
    let results_lower = search_notes("uppercase")?;
    let results_upper = search_notes("UPPERCASE")?;
    let results_mixed = search_notes("UpPeRcAsE")?;

    assert_eq!(results_lower.len(), 1, "Should find with lowercase search");
    assert_eq!(results_upper.len(), 1, "Should find with uppercase search");
    assert_eq!(results_mixed.len(), 1, "Should find with mixed case search");

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_search_notes_no_results() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create some notes
    create_note("Note 1")?;
    create_note("Note 2")?;

    // Search for non-existent term
    let results = search_notes("nonexistentterm12345")?;
    assert_eq!(results.len(), 0, "Should find no results");

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_format_time_ago() -> Result<()> {
    // Test various time differences
    let now = SystemTime::now();

    // Just now (0 seconds)
    assert_eq!(format_time_ago(now), "just now");

    // 30 seconds ago
    let thirty_secs_ago = now - Duration::from_secs(30);
    assert_eq!(format_time_ago(thirty_secs_ago), "just now");

    // 1 minute ago
    let one_min_ago = now - Duration::from_secs(60);
    assert_eq!(format_time_ago(one_min_ago), "1 minute ago");

    // 5 minutes ago
    let five_mins_ago = now - Duration::from_secs(5 * 60);
    assert_eq!(format_time_ago(five_mins_ago), "5 minutes ago");

    // 1 hour ago
    let one_hour_ago = now - Duration::from_secs(60 * 60);
    assert_eq!(format_time_ago(one_hour_ago), "1 hour ago");

    // 3 hours ago
    let three_hours_ago = now - Duration::from_secs(3 * 60 * 60);
    assert_eq!(format_time_ago(three_hours_ago), "3 hours ago");

    // 1 day ago
    let one_day_ago = now - Duration::from_secs(24 * 60 * 60);
    assert_eq!(format_time_ago(one_day_ago), "1 day ago");

    // 5 days ago
    let five_days_ago = now - Duration::from_secs(5 * 24 * 60 * 60);
    assert_eq!(format_time_ago(five_days_ago), "5 days ago");

    // 1 week ago
    let one_week_ago = now - Duration::from_secs(7 * 24 * 60 * 60);
    assert_eq!(format_time_ago(one_week_ago), "1 week ago");

    // 3 weeks ago
    let three_weeks_ago = now - Duration::from_secs(3 * 7 * 24 * 60 * 60);
    assert_eq!(format_time_ago(three_weeks_ago), "3 weeks ago");

    // 1 month ago
    let one_month_ago = now - Duration::from_secs(30 * 24 * 60 * 60);
    assert_eq!(format_time_ago(one_month_ago), "1 month ago");

    // 6 months ago
    let six_months_ago = now - Duration::from_secs(6 * 30 * 24 * 60 * 60);
    assert_eq!(format_time_ago(six_months_ago), "6 months ago");

    // 1 year ago
    let one_year_ago = now - Duration::from_secs(365 * 24 * 60 * 60);
    assert_eq!(format_time_ago(one_year_ago), "1 year ago");

    // 2 years ago
    let two_years_ago = now - Duration::from_secs(2 * 365 * 24 * 60 * 60);
    assert_eq!(format_time_ago(two_years_ago), "2 years ago");

    Ok(())
}

#[test]
fn test_get_notes_with_paths_includes_dailies() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create regular notes
    create_note("Regular Note 1")?;
    create_note("Regular Note 2")?;

    // Create daily notes manually in dailies directory
    fs::write(
        "dailies/2024-01-15.md",
        "---\ntitle: \"2024-01-15\"\ntags: [daily]\n---\n# Daily Note\n\nToday's tasks.",
    )?;

    fs::write(
        "dailies/2024-01-16.md",
        "---\ntitle: \"2024-01-16\"\ntags: [daily]\n---\n# Daily Note\n\nTomorrow's plan.",
    )?;

    // Get all notes
    let notes = get_notes_with_paths()?;

    // Should include both regular and daily notes
    assert_eq!(notes.len(), 4, "Should have 2 regular + 2 daily notes");

    // Check that paths include both directories
    let has_regular = notes.iter().any(|(path, _, _)| path.starts_with("notes/"));
    let has_dailies = notes
        .iter()
        .any(|(path, _, _)| path.starts_with("dailies/"));

    assert!(has_regular, "Should include regular notes");
    assert!(has_dailies, "Should include daily notes");

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_search_notes_special_characters() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create notes with special characters
    create_note("Special Chars")?;
    fs::write(
        "notes/special-chars.md",
        "---\ntitle: \"Special Chars\"\ntags: []\n---\n# Special Characters\n\nEmail: user@example.com\nPath: /usr/local/bin\nCode: function() { return true; }",
    )?;

    // Search for email
    let results = search_notes("user@example.com")?;
    assert_eq!(results.len(), 1, "Should find email address");

    // Search for path
    let results = search_notes("/usr/local")?;
    assert_eq!(results.len(), 1, "Should find path");

    // Search for code
    let results = search_notes("function()")?;
    assert_eq!(results.len(), 1, "Should find code snippet");

    cleanup_test_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_list_notes_performance_with_many_notes() -> Result<()> {
    let (temp_dir, original_dir) = setup_test_dir()?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create 100 notes
    for i in 0..100 {
        create_note(&format!("Performance Test Note {}", i))?;
    }

    // Measure time to list notes
    let start = std::time::Instant::now();
    let notes = get_notes_with_paths()?;
    let duration = start.elapsed();

    // Should have all 100 notes
    assert_eq!(notes.len(), 100, "Should have 100 notes");

    // Should complete in reasonable time (less than 1 second)
    assert!(
        duration.as_secs() < 1,
        "Listing 100 notes should take less than 1 second"
    );

    cleanup_test_dir(original_dir)?;
    Ok(())
}
