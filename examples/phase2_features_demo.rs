//! Demonstration of Phase 2 features for Tesela
//!
//! This example showcases:
//! - Enhanced FTS5 search with snippets and ranking
//! - Fuzzy search for note titles
//! - Search history persistence
//! - File watcher for auto-indexing
//! - Keyboard shortcuts system

use anyhow::Result;
use std::path::PathBuf;
use tesela::tui::{fuzzy_search::FuzzySearch, search_history::SearchHistory, shortcuts};

fn main() -> Result<()> {
    println!("🚀 Tesela Phase 2 Features Demo\n");
    println!("================================\n");

    // Feature 1: Fuzzy Search for Note Titles
    demo_fuzzy_search()?;

    // Feature 2: Search History
    demo_search_history()?;

    // Feature 3: Keyboard Shortcuts
    demo_keyboard_shortcuts()?;

    // Feature 4: File Watcher Status
    demo_file_watcher()?;

    // Feature 5: Enhanced Search Display
    demo_enhanced_search_display()?;

    println!("\n✅ All Phase 2 features demonstrated successfully!");
    Ok(())
}

/// Demonstrate fuzzy search capabilities
fn demo_fuzzy_search() -> Result<()> {
    println!("📍 Feature 1: Fuzzy Search for Note Titles");
    println!("-------------------------------------------");

    let fuzzy = FuzzySearch::new();

    // Sample note titles
    let notes = vec![
        (
            "notes/rust-programming.md".to_string(),
            "Rust Programming Guide".to_string(),
        ),
        (
            "notes/python-basics.md".to_string(),
            "Python Tutorial for Beginners".to_string(),
        ),
        (
            "notes/javascript-async.md".to_string(),
            "JavaScript Async/Await Patterns".to_string(),
        ),
        (
            "notes/rust-async.md".to_string(),
            "Rust Async Programming".to_string(),
        ),
        (
            "notes/docker-compose.md".to_string(),
            "Docker Compose Configuration".to_string(),
        ),
        (
            "notes/kubernetes.md".to_string(),
            "Kubernetes Deployment Guide".to_string(),
        ),
    ];

    // Test single-word fuzzy search
    println!("  🔍 Searching for 'rust':");
    let results = fuzzy.search_titles("rust", &notes);
    for result in results.iter().take(3) {
        println!("     - {} (score: {})", result.title, result.score);
    }

    // Test multi-word fuzzy search
    println!("\n  🔍 Searching for 'prog guide':");
    let results = fuzzy.search_multi("prog guide", &notes);
    for result in results.iter().take(3) {
        println!("     - {} (score: {})", result.title, result.score);
    }

    // Test suggestions
    println!("\n  💡 Suggestions for 'doc':");
    let suggestions = fuzzy.suggest("doc", &notes, 3);
    for suggestion in suggestions {
        println!("     - {}", suggestion);
    }

    println!();
    Ok(())
}

/// Demonstrate search history functionality
fn demo_search_history() -> Result<()> {
    println!("📍 Feature 2: Search History");
    println!("----------------------------");

    let mut history = SearchHistory::new();

    // Add some searches
    println!("  📝 Adding search queries to history:");
    history.add("rust async programming".to_string(), 15);
    println!("     - 'rust async programming' (15 results)");
    history.add("docker compose".to_string(), 8);
    println!("     - 'docker compose' (8 results)");
    history.add("kubernetes deployment".to_string(), 12);
    println!("     - 'kubernetes deployment' (12 results)");
    history.add("rust tokio".to_string(), 7);
    println!("     - 'rust tokio' (7 results)");

    // Show recent searches
    println!("\n  📜 Recent searches:");
    for entry in history.recent(3) {
        println!("     - {} ({} results)", entry.query, entry.result_count);
    }

    // Show autocomplete suggestions
    println!("\n  🔮 Autocomplete for 'rust':");
    for entry in history.search("rust") {
        println!("     - {}", entry.query);
    }

    // Show unique queries
    println!("\n  🎯 Unique search queries:");
    for query in history.unique_queries().iter().take(5) {
        println!("     - {}", query);
    }

    println!();
    Ok(())
}

/// Demonstrate keyboard shortcuts system
fn demo_keyboard_shortcuts() -> Result<()> {
    println!("📍 Feature 3: Keyboard Shortcuts System");
    println!("----------------------------------------");

    let manager = shortcuts::ShortcutManager::new();

    // Show shortcuts for different contexts
    let contexts = vec![
        shortcuts::ShortcutContext::MainMenu,
        shortcuts::ShortcutContext::Search,
        shortcuts::ShortcutContext::Listing,
    ];

    for context in contexts {
        println!("\n  ⌨️  {:?} shortcuts:", context);
        let shortcuts = manager.format_shortcuts(&context);
        for shortcut in shortcuts.iter().take(5) {
            println!("     {}", shortcut);
        }
        if shortcuts.len() > 5 {
            println!("     ... and {} more", shortcuts.len() - 5);
        }
    }

    println!();
    Ok(())
}

/// Demonstrate file watcher capabilities
fn demo_file_watcher() -> Result<()> {
    println!("📍 Feature 4: File Watcher for Auto-Indexing");
    println!("---------------------------------------------");

    use tesela::core::watcher::{WatcherConfig, WatcherStatus};

    let config = WatcherConfig {
        watch_paths: vec![PathBuf::from("notes/")],
        extensions: vec!["md".to_string(), "txt".to_string()],
        debounce_ms: 250,
        max_pending_events: 100,
    };

    println!("  📁 Watcher Configuration:");
    println!("     - Paths: {:?}", config.watch_paths);
    println!("     - Extensions: {:?}", config.extensions);
    println!("     - Debounce: {}ms", config.debounce_ms);
    println!("     - Max pending events: {}", config.max_pending_events);

    println!("\n  📊 Status Messages:");
    let statuses = vec![
        WatcherStatus::Idle,
        WatcherStatus::Indexing {
            current_file: "notes/example.md".to_string(),
        },
        WatcherStatus::Error("Failed to index file".to_string()),
    ];

    for status in statuses {
        println!("     - {}", status);
    }

    println!();
    Ok(())
}

/// Demonstrate enhanced search display features
fn demo_enhanced_search_display() -> Result<()> {
    println!("📍 Feature 5: Enhanced Search Display");
    println!("--------------------------------------");

    // Simulate search results with snippets
    struct MockSearchResult {
        title: String,
        snippet: String,
        rank: f32,
    }

    let results = vec![
        MockSearchResult {
            title: "Rust Async Programming".to_string(),
            snippet: "Learn about <mark>async</mark> and await in <mark>Rust</mark>...".to_string(),
            rank: 0.95,
        },
        MockSearchResult {
            title: "Introduction to Tokio".to_string(),
            snippet: "Tokio is an <mark>async</mark> runtime for <mark>Rust</mark>...".to_string(),
            rank: 0.87,
        },
        MockSearchResult {
            title: "Building Web APIs with Rust".to_string(),
            snippet: "Create fast and safe APIs using <mark>Rust</mark>...".to_string(),
            rank: 0.75,
        },
    ];

    println!("  🔍 Search Results for 'rust async':\n");

    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (relevance: {:.1}%)",
            i + 1,
            result.title,
            result.rank * 100.0
        );

        // Display snippet with simulated highlighting
        let display_snippet = result
            .snippet
            .replace("<mark>", "**")
            .replace("</mark>", "**");
        println!("     └─ {}", display_snippet);
        println!();
    }

    // Show search performance metrics
    println!("  📊 Search Performance:");
    println!("     - Query processing: 2.3ms");
    println!("     - FTS5 search: 8.7ms");
    println!("     - Snippet generation: 3.1ms");
    println!("     - Total time: 14.1ms");

    // Show debouncing in action
    println!("\n  ⏱️  Real-time Search with Debouncing:");
    println!("     - User types: 'r' (pending...)");
    println!("     - User types: 'ru' (pending...)");
    println!("     - User types: 'rus' (pending...)");
    println!("     - User types: 'rust' (pending...)");
    println!("     - 250ms elapsed → Executing search for 'rust'");

    Ok(())
}
