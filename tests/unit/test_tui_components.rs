//! Unit tests for TUI components

use anyhow::Result;
use std::path::Path;
use std::time::Instant;
use tesela::tui::app::{
    App, InputMode, InputType, ListItem, ListType, ListingMode, Mode, SearchMode,
};

#[test]
fn test_app_state_transitions() -> Result<()> {
    let mut app = App::new()?;

    // Initial state should be MainMenu
    assert!(
        matches!(app.mode, Mode::MainMenu),
        "App should start in MainMenu mode"
    );

    // Test transition to Input mode
    app.mode = Mode::Input(InputMode {
        prompt: "Test: ".to_string(),
        input: String::new(),
        cursor_position: 0,
        input_type: InputType::NewNote,
        suggestions: vec![],
        suggestion_index: None,
    });
    assert!(
        matches!(app.mode, Mode::Input(_)),
        "Should transition to Input mode"
    );

    // Test transition to Listing mode
    app.mode = Mode::Listing(ListingMode {
        title: "Test List".to_string(),
        items: vec![],
        selected: 0,
        list_type: ListType::Notes,
        preview_content: None,
        preview_scroll: 0,
    });
    assert!(
        matches!(app.mode, Mode::Listing(_)),
        "Should transition to Listing mode"
    );

    // Test transition to Search mode
    app.mode = Mode::Search(SearchMode {
        query: String::new(),
        cursor_position: 0,
        results: vec![],
        selected_result: 0,
    });
    assert!(
        matches!(app.mode, Mode::Search(_)),
        "Should transition to Search mode"
    );

    // Test transition to Message mode
    app.mode = Mode::Message("Test message".to_string(), Instant::now());
    assert!(
        matches!(app.mode, Mode::Message(_, _)),
        "Should transition to Message mode"
    );

    Ok(())
}

#[test]
fn test_input_mode_creation() {
    // Test NewNote input type
    let input_mode = InputMode {
        prompt: "New Note: ".to_string(),
        input: "Test Note".to_string(),
        cursor_position: 9,
        input_type: InputType::NewNote,
        suggestions: vec!["Test Note 1".to_string(), "Test Note 2".to_string()],
        suggestion_index: Some(0),
    };

    assert_eq!(input_mode.prompt, "New Note: ");
    assert_eq!(input_mode.input, "Test Note");
    assert_eq!(input_mode.cursor_position, 9);
    assert!(matches!(input_mode.input_type, InputType::NewNote));
    assert_eq!(input_mode.suggestions.len(), 2);
    assert_eq!(input_mode.suggestion_index, Some(0));

    // Test EditNote input type
    let edit_mode = InputMode {
        prompt: "Edit Note: ".to_string(),
        input: String::new(),
        cursor_position: 0,
        input_type: InputType::EditNote,
        suggestions: vec![],
        suggestion_index: None,
    };

    assert!(matches!(edit_mode.input_type, InputType::EditNote));
    assert!(edit_mode.suggestions.is_empty());
    assert_eq!(edit_mode.suggestion_index, None);

    // Test SearchQuery input type
    let search_mode = InputMode {
        prompt: "Search: ".to_string(),
        input: "test query".to_string(),
        cursor_position: 10,
        input_type: InputType::SearchQuery,
        suggestions: vec![],
        suggestion_index: None,
    };

    assert!(matches!(search_mode.input_type, InputType::SearchQuery));
    assert_eq!(search_mode.input, "test query");
}

#[test]
fn test_suggestion_cycling() {
    let mut input_mode = InputMode {
        prompt: "Test: ".to_string(),
        input: "test".to_string(),
        cursor_position: 4,
        input_type: InputType::EditNote,
        suggestions: vec![
            "test-note-1".to_string(),
            "test-note-2".to_string(),
            "test-note-3".to_string(),
        ],
        suggestion_index: None,
    };

    // Initial state - no selection
    assert_eq!(input_mode.suggestion_index, None);

    // First tab - select first suggestion
    input_mode.suggestion_index = Some(0);
    assert_eq!(input_mode.suggestion_index, Some(0));

    // Second tab - cycle to next
    input_mode.suggestion_index = Some(1);
    assert_eq!(input_mode.suggestion_index, Some(1));

    // Third tab - cycle to next
    input_mode.suggestion_index = Some(2);
    assert_eq!(input_mode.suggestion_index, Some(2));

    // Fourth tab - cycle back to first
    input_mode.suggestion_index = Some(0);
    assert_eq!(input_mode.suggestion_index, Some(0));
}

#[test]
fn test_list_item_creation() {
    // Test basic list item
    let item = ListItem {
        title: "Test Note".to_string(),
        subtitle: "notes/test-note.md".to_string(),
        metadata: "📝 Note • 2 hours ago".to_string(),
        context: None,
        match_indices: vec![],
    };

    assert_eq!(item.title, "Test Note");
    assert_eq!(item.subtitle, "notes/test-note.md");
    assert_eq!(item.metadata, "📝 Note • 2 hours ago");
    assert!(item.context.is_none());
    assert!(item.match_indices.is_empty());

    // Test search result item with context
    let search_item = ListItem {
        title: "Search Result".to_string(),
        subtitle: "notes/search-result.md".to_string(),
        metadata: "🔍 3 matches".to_string(),
        context: Some("This is the matching line with search term".to_string()),
        match_indices: vec![(32, 38)], // "search" indices
    };

    assert_eq!(search_item.title, "Search Result");
    assert!(search_item.context.is_some());
    assert_eq!(search_item.match_indices.len(), 1);
    assert_eq!(search_item.match_indices[0], (32, 38));
}

#[test]
fn test_listing_mode_creation() {
    // Test notes listing
    let notes_listing = ListingMode {
        title: "📚 All Notes (5)".to_string(),
        items: vec![
            ListItem {
                title: "Note 1".to_string(),
                subtitle: "notes/note-1.md".to_string(),
                metadata: "📝 Note • just now".to_string(),
                context: None,
                match_indices: vec![],
            },
            ListItem {
                title: "Note 2".to_string(),
                subtitle: "notes/note-2.md".to_string(),
                metadata: "📝 Note • 1 hour ago".to_string(),
                context: None,
                match_indices: vec![],
            },
        ],
        selected: 0,
        list_type: ListType::Notes,
        preview_content: Some("# Note 1\n\nThis is the content".to_string()),
        preview_scroll: 0,
    };

    assert_eq!(notes_listing.title, "📚 All Notes (5)");
    assert_eq!(notes_listing.items.len(), 2);
    assert_eq!(notes_listing.selected, 0);
    assert!(matches!(notes_listing.list_type, ListType::Notes));
    assert!(notes_listing.preview_content.is_some());
    assert_eq!(notes_listing.preview_scroll, 0);

    // Test search results listing
    let search_listing = ListingMode {
        title: "🔍 Search Results (3)".to_string(),
        items: vec![],
        selected: 0,
        list_type: ListType::SearchResults,
        preview_content: None,
        preview_scroll: 0,
    };

    assert!(matches!(search_listing.list_type, ListType::SearchResults));
    assert!(search_listing.preview_content.is_none());
}

#[test]
fn test_search_mode_creation() {
    let search_mode = SearchMode {
        query: "test search".to_string(),
        cursor_position: 11,
        results: vec![ListItem {
            title: "Result 1".to_string(),
            subtitle: "notes/result-1.md".to_string(),
            metadata: "2 matches".to_string(),
            context: Some("Line containing test search term".to_string()),
            match_indices: vec![(16, 20), (21, 27)],
        }],
        selected_result: 0,
    };

    assert_eq!(search_mode.query, "test search");
    assert_eq!(search_mode.cursor_position, 11);
    assert_eq!(search_mode.results.len(), 1);
    assert_eq!(search_mode.selected_result, 0);

    let first_result = &search_mode.results[0];
    assert_eq!(first_result.title, "Result 1");
    assert!(first_result.context.is_some());
    assert_eq!(first_result.match_indices.len(), 2);
}

#[test]
fn test_scroll_management() {
    let mut listing_mode = ListingMode {
        title: "Test".to_string(),
        items: vec![],
        selected: 0,
        list_type: ListType::Notes,
        preview_content: Some("Long content...".to_string()),
        preview_scroll: 0,
    };

    // Initial scroll position
    assert_eq!(listing_mode.preview_scroll, 0);

    // Scroll down
    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_add(5);
    assert_eq!(listing_mode.preview_scroll, 5);

    // Scroll down more
    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_add(5);
    assert_eq!(listing_mode.preview_scroll, 10);

    // Scroll up
    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_sub(5);
    assert_eq!(listing_mode.preview_scroll, 5);

    // Scroll up to top
    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_sub(10);
    assert_eq!(listing_mode.preview_scroll, 0);

    // Test saturating_sub doesn't go negative
    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_sub(5);
    assert_eq!(listing_mode.preview_scroll, 0);
}

#[test]
fn test_search_highlighting_indices() {
    // Test single match
    let item = ListItem {
        title: "Test".to_string(),
        subtitle: "test.md".to_string(),
        metadata: "1 match".to_string(),
        context: Some("This line contains a match here".to_string()),
        match_indices: vec![(21, 26)], // "match"
    };

    assert_eq!(item.match_indices.len(), 1);
    let (start, end) = item.match_indices[0];
    assert_eq!(start, 21);
    assert_eq!(end, 26);

    // Test multiple matches
    let multi_item = ListItem {
        title: "Multi".to_string(),
        subtitle: "multi.md".to_string(),
        metadata: "3 matches".to_string(),
        context: Some("test test test in this line".to_string()),
        match_indices: vec![(0, 4), (5, 9), (10, 14)], // Three "test" matches
    };

    assert_eq!(multi_item.match_indices.len(), 3);
    for (i, &(start, end)) in multi_item.match_indices.iter().enumerate() {
        assert_eq!(end - start, 4, "Each 'test' match should be 4 chars");
        if i > 0 {
            assert!(
                start > multi_item.match_indices[i - 1].1,
                "Matches should not overlap"
            );
        }
    }
}

#[test]
fn test_error_display() {
    let mut app = App::new().unwrap();

    // Test error message creation
    let error_msg = "Test error occurred";
    app.mode = Mode::Message(format!("❌ {}", error_msg), Instant::now());

    if let Mode::Message(msg, _) = &app.mode {
        assert!(msg.starts_with("❌"), "Error message should start with ❌");
        assert!(
            msg.contains("Test error occurred"),
            "Error message should contain the error text"
        );
    } else {
        panic!("Mode should be Message");
    }

    // Test non-error message
    app.mode = Mode::Message("✅ Success".to_string(), Instant::now());

    if let Mode::Message(msg, _) = &app.mode {
        assert!(
            msg.starts_with("✅"),
            "Success message should start with ✅"
        );
        assert!(
            !msg.starts_with("❌"),
            "Success message should not be an error"
        );
    }
}

#[test]
fn test_list_navigation_bounds() {
    let mut listing = ListingMode {
        title: "Test".to_string(),
        items: vec![
            ListItem {
                title: "Item 1".to_string(),
                subtitle: "1.md".to_string(),
                metadata: String::new(),
                context: None,
                match_indices: vec![],
            },
            ListItem {
                title: "Item 2".to_string(),
                subtitle: "2.md".to_string(),
                metadata: String::new(),
                context: None,
                match_indices: vec![],
            },
            ListItem {
                title: "Item 3".to_string(),
                subtitle: "3.md".to_string(),
                metadata: String::new(),
                context: None,
                match_indices: vec![],
            },
        ],
        selected: 0,
        list_type: ListType::Notes,
        preview_content: None,
        preview_scroll: 0,
    };

    // Initial position
    assert_eq!(listing.selected, 0);

    // Move down
    listing.selected = 1;
    assert_eq!(listing.selected, 1);

    // Move to last item
    listing.selected = 2;
    assert_eq!(listing.selected, 2);

    // Try to move beyond last item (should stay at 2)
    let max_index = listing.items.len().saturating_sub(1);
    listing.selected = listing.selected.min(max_index);
    assert_eq!(listing.selected, 2);

    // Move back up
    listing.selected = listing.selected.saturating_sub(1);
    assert_eq!(listing.selected, 1);

    // Move to top
    listing.selected = 0;
    assert_eq!(listing.selected, 0);

    // Try to move above first item (should stay at 0)
    listing.selected = listing.selected.saturating_sub(1);
    assert_eq!(listing.selected, 0);
}

#[test]
fn test_empty_list_handling() {
    let empty_listing = ListingMode {
        title: "Empty List".to_string(),
        items: vec![],
        selected: 0,
        list_type: ListType::Notes,
        preview_content: None,
        preview_scroll: 0,
    };

    assert!(empty_listing.items.is_empty());
    assert_eq!(empty_listing.selected, 0);
    assert!(empty_listing.preview_content.is_none());

    // Test that empty list doesn't cause issues with bounds
    let max_index = empty_listing.items.len().saturating_sub(1);
    assert_eq!(max_index, 0); // saturating_sub prevents underflow
}
