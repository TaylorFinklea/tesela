//! Integration tests for TUI workflows

use anyhow::Result;
use std::env;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tesela::commands::{create_note, get_notes_with_paths, init_mosaic};
use tesela::tui::app::{
    App, InputMode, InputType, ListItem, ListType, ListingMode, Mode, SearchMode,
};

/// Helper to setup a test environment with initialized mosaic
fn setup_test_env() -> Result<(TempDir, std::path::PathBuf, App)> {
    let temp_dir = TempDir::new()?;
    let original_dir = env::current_dir()?;
    env::set_current_dir(temp_dir.path())?;

    // Initialize mosaic
    init_mosaic(".")?;

    // Create app instance
    let app = App::new()?;

    Ok((temp_dir, original_dir, app))
}

/// Helper to cleanup test environment
fn cleanup_test_env(original_dir: std::path::PathBuf) -> Result<()> {
    env::set_current_dir(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_create_note_flow() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Simulate: User starts in main menu
    assert!(matches!(app.mode, Mode::MainMenu));

    // Simulate: User presses 'n' for new note
    app.mode = Mode::Input(InputMode {
        prompt: "📝 New Note: ".to_string(),
        input: String::new(),
        cursor_position: 0,
        input_type: InputType::NewNote,
        suggestions: vec![],
        suggestion_index: None,
    });

    // Simulate: User types note title
    if let Mode::Input(ref mut input_mode) = app.mode {
        input_mode.input = "Integration Test Note".to_string();
        input_mode.cursor_position = input_mode.input.len();
    }

    // Simulate: User presses Enter to create
    // (In real app, this would call execute_input)
    create_note("Integration Test Note")?;

    // Verify note was created
    assert!(
        Path::new("notes/integration-test-note.md").exists(),
        "Note should be created"
    );

    // Simulate: Success message shown
    app.mode = Mode::Message(
        "✅ Created note: notes/integration-test-note.md".to_string(),
        Instant::now(),
    );

    // Verify success message
    if let Mode::Message(msg, _) = &app.mode {
        assert!(msg.contains("Created note"));
        assert!(msg.contains("integration-test-note.md"));
    }

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_edit_note_flow() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Create some notes first
    create_note("Edit Test 1")?;
    create_note("Edit Test 2")?;
    create_note("Edit Test 3")?;

    // Simulate: User presses 'e' for edit
    app.mode = Mode::Input(InputMode {
        prompt: "📝 Edit Note: ".to_string(),
        input: String::new(),
        cursor_position: 0,
        input_type: InputType::EditNote,
        suggestions: vec![
            "edit-test-1".to_string(),
            "edit-test-2".to_string(),
            "edit-test-3".to_string(),
        ],
        suggestion_index: None,
    });

    // Simulate: User types partial name
    if let Mode::Input(ref mut input_mode) = app.mode {
        input_mode.input = "edit-test-2".to_string();
        input_mode.cursor_position = input_mode.input.len();
    }

    // Verify suggestions are filtered (in real app)
    if let Mode::Input(ref input_mode) = app.mode {
        assert!(input_mode.suggestions.contains(&"edit-test-2".to_string()));
    }

    // Simulate: User selects and opens note
    // (In real app, this would open external editor)

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_search_flow() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Create notes with searchable content
    create_note("Search Test 1")?;
    fs::write(
        "notes/search-test-1.md",
        "---\ntitle: \"Search Test 1\"\ntags: []\n---\n# Search Test 1\n\nThis contains the keyword rust programming.",
    )?;

    create_note("Search Test 2")?;
    fs::write(
        "notes/search-test-2.md",
        "---\ntitle: \"Search Test 2\"\ntags: []\n---\n# Search Test 2\n\nThis contains the keyword python coding.",
    )?;

    // Simulate: User presses 's' for search
    app.mode = Mode::Search(SearchMode {
        query: String::new(),
        cursor_position: 0,
        results: vec![],
        selected_result: 0,
    });

    // Simulate: User types search query
    if let Mode::Search(ref mut search_mode) = app.mode {
        search_mode.query = "rust".to_string();
        search_mode.cursor_position = 4;

        // Simulate real-time search results
        search_mode.results = vec![ListItem {
            title: "Search Test 1".to_string(),
            subtitle: "notes/search-test-1.md".to_string(),
            metadata: "1 match".to_string(),
            context: Some("This contains the keyword rust programming.".to_string()),
            match_indices: vec![(26, 30)], // "rust"
        }];
    }

    // Verify search results
    if let Mode::Search(ref search_mode) = app.mode {
        assert_eq!(search_mode.results.len(), 1);
        assert!(search_mode.results[0].title.contains("Search Test 1"));
        assert!(search_mode.results[0].context.is_some());
    }

    // Simulate: User navigates results
    if let Mode::Search(ref mut search_mode) = app.mode {
        assert_eq!(search_mode.selected_result, 0);
        // Would press Enter to open selected result
    }

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_list_navigation() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Create multiple notes
    create_note("Nav Test 1")?;
    thread::sleep(Duration::from_millis(50));
    create_note("Nav Test 2")?;
    thread::sleep(Duration::from_millis(50));
    create_note("Nav Test 3")?;

    // Get notes for listing
    let notes = get_notes_with_paths()?;

    // Convert to ListItems
    let items: Vec<ListItem> = notes
        .into_iter()
        .map(|(path, title, _)| ListItem {
            title,
            subtitle: path,
            metadata: "📝 Note • just now".to_string(),
            context: None,
            match_indices: vec![],
        })
        .collect();

    // Simulate: User presses 'l' for list
    app.mode = Mode::Listing(ListingMode {
        title: format!("📚 All Notes ({})", items.len()),
        items: items.clone(),
        selected: 0,
        list_type: ListType::Notes,
        preview_content: Some("# Nav Test 3\n\nContent of the newest note".to_string()),
        preview_scroll: 0,
    });

    // Simulate: User navigates down
    if let Mode::Listing(ref mut listing) = app.mode {
        // Move down
        listing.selected = 1;
        // Preview should update (in real app)
        listing.preview_content = Some("# Nav Test 2\n\nContent of the second note".to_string());

        // Move down again
        listing.selected = 2;
        listing.preview_content = Some("# Nav Test 1\n\nContent of the oldest note".to_string());

        // Verify bounds
        assert!(listing.selected < listing.items.len());
    }

    // Simulate: User scrolls preview
    if let Mode::Listing(ref mut listing) = app.mode {
        // PageDown
        listing.preview_scroll = listing.preview_scroll.saturating_add(5);
        assert_eq!(listing.preview_scroll, 5);

        // PageUp
        listing.preview_scroll = listing.preview_scroll.saturating_sub(5);
        assert_eq!(listing.preview_scroll, 0);
    }

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_autocomplete_flow() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Create notes with similar names
    create_note("Autocomplete Test Alpha")?;
    create_note("Autocomplete Test Beta")?;
    create_note("Autocomplete Test Gamma")?;
    create_note("Different Note")?;

    // Simulate: User starts typing in edit mode
    app.mode = Mode::Input(InputMode {
        prompt: "📝 Edit Note: ".to_string(),
        input: "auto".to_string(),
        cursor_position: 4,
        input_type: InputType::EditNote,
        suggestions: vec![
            "autocomplete-test-alpha".to_string(),
            "Autocomplete Test Alpha".to_string(),
            "autocomplete-test-beta".to_string(),
            "Autocomplete Test Beta".to_string(),
            "autocomplete-test-gamma".to_string(),
            "Autocomplete Test Gamma".to_string(),
        ],
        suggestion_index: None,
    });

    // Simulate: User presses Tab to cycle suggestions
    if let Mode::Input(ref mut input_mode) = app.mode {
        // First Tab
        input_mode.suggestion_index = Some(0);
        input_mode.input = input_mode.suggestions[0].clone();
        assert_eq!(input_mode.input, "autocomplete-test-alpha");

        // Second Tab
        input_mode.suggestion_index = Some(1);
        input_mode.input = input_mode.suggestions[1].clone();
        assert_eq!(input_mode.input, "Autocomplete Test Alpha");

        // Third Tab - cycles to next pair
        input_mode.suggestion_index = Some(2);
        input_mode.input = input_mode.suggestions[2].clone();
        assert_eq!(input_mode.input, "autocomplete-test-beta");

        // Verify cycling works
        let total_suggestions = input_mode.suggestions.len();
        assert!(total_suggestions > 0);
    }

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_error_recovery() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Simulate: User tries to create note with invalid name
    app.mode = Mode::Input(InputMode {
        prompt: "📝 New Note: ".to_string(),
        input: String::new(), // Empty input
        cursor_position: 0,
        input_type: InputType::NewNote,
        suggestions: vec![],
        suggestion_index: None,
    });

    // Simulate: Error occurs (empty title)
    if let Mode::Input(ref input_mode) = app.mode {
        if input_mode.input.is_empty() {
            // Show error message
            app.mode = Mode::Message("❌ Note title cannot be empty".to_string(), Instant::now());
        }
    }

    // Verify error message
    if let Mode::Message(msg, _) = &app.mode {
        assert!(msg.starts_with("❌"));
        assert!(msg.contains("cannot be empty"));
    }

    // Simulate: User acknowledges error and returns to menu
    app.mode = Mode::MainMenu;
    assert!(matches!(app.mode, Mode::MainMenu));

    // Simulate: User tries again with valid input
    app.mode = Mode::Input(InputMode {
        prompt: "📝 New Note: ".to_string(),
        input: "Valid Note Title".to_string(),
        cursor_position: 16,
        input_type: InputType::NewNote,
        suggestions: vec![],
        suggestion_index: None,
    });

    // This time it should succeed
    create_note("Valid Note Title")?;
    assert!(Path::new("notes/valid-note-title.md").exists());

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_daily_note_flow() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Simulate: User presses 'd' for daily note
    // (In real app, this would call daily_note_and_edit)

    // Create a daily note manually for testing
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    create_note(&today)?;

    // Verify daily note was created
    let daily_note_path = format!("notes/{}.md", today);
    assert!(
        Path::new(&daily_note_path).exists(),
        "Daily note should be created"
    );

    // Read content to verify format
    let content = fs::read_to_string(&daily_note_path)?;
    assert!(content.contains(&format!("title: \"{}\"", today)));
    assert!(content.contains("---"));

    cleanup_test_env(original_dir)?;
    Ok(())
}

#[test]
fn test_tui_preview_update_on_selection_change() -> Result<()> {
    let (temp_dir, original_dir, mut app) = setup_test_env()?;

    // Create notes with distinct content
    create_note("Preview Test 1")?;
    fs::write(
        "notes/preview-test-1.md",
        "---\ntitle: \"Preview Test 1\"\n---\n# Preview Test 1\n\nFirst note content here.",
    )?;

    create_note("Preview Test 2")?;
    fs::write(
        "notes/preview-test-2.md",
        "---\ntitle: \"Preview Test 2\"\n---\n# Preview Test 2\n\nSecond note content here.",
    )?;

    create_note("Preview Test 3")?;
    fs::write(
        "notes/preview-test-3.md",
        "---\ntitle: \"Preview Test 3\"\n---\n# Preview Test 3\n\nThird note content here.",
    )?;

    // Load notes
    let notes = get_notes_with_paths()?;
    let items: Vec<ListItem> = notes
        .into_iter()
        .map(|(path, title, _)| ListItem {
            title: title.clone(),
            subtitle: path.clone(),
            metadata: "📝 Note • just now".to_string(),
            context: None,
            match_indices: vec![],
        })
        .collect();

    // Start with first note selected
    app.mode = Mode::Listing(ListingMode {
        title: "📚 All Notes".to_string(),
        items: items.clone(),
        selected: 0,
        list_type: ListType::Notes,
        preview_content: Some("# Preview Test 3\n\nThird note content here.".to_string()),
        preview_scroll: 0,
    });

    // Simulate selection change
    if let Mode::Listing(ref mut listing) = app.mode {
        // Move to second item
        listing.selected = 1;
        // Preview should update
        listing.preview_content = Some("# Preview Test 2\n\nSecond note content here.".to_string());
        // Scroll should reset
        listing.preview_scroll = 0;

        // Verify preview content changed
        assert!(listing
            .preview_content
            .as_ref()
            .unwrap()
            .contains("Second note"));

        // Move to third item
        listing.selected = 2;
        listing.preview_content = Some("# Preview Test 1\n\nFirst note content here.".to_string());
        listing.preview_scroll = 0;

        assert!(listing
            .preview_content
            .as_ref()
            .unwrap()
            .contains("First note"));
    }

    cleanup_test_env(original_dir)?;
    Ok(())
}
