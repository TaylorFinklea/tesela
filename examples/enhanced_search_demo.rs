//! Enhanced Search Demo
//!
//! This example demonstrates the enhanced FTS5 search capabilities of Tesela,
//! including ranked results, snippets, and various search operators.

use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tesela::core::database::{Database, DatabaseConfig};
use tesela::core::search::{SearchConfig, SearchEngine};
use tesela::core::storage::{Note, NoteMetadata};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🚀 Tesela Enhanced Search Demo\n");
    println!("This demo showcases the FTS5 search capabilities.\n");

    // Create temporary directory for the demo
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("demo.db");

    // Initialize database with FTS5
    let db_config = DatabaseConfig {
        db_path: db_path.clone(),
        max_connections: 5,
        connect_timeout: 30,
        enable_wal: true,
        enable_foreign_keys: true,
    };

    let database = Arc::new(Database::new(db_config).await?);
    database.initialize().await?;
    println!("✅ Database initialized with FTS5 support\n");

    // Create sample notes
    let sample_notes = vec![
        (
            "Getting Started with Rust",
            "# Getting Started with Rust\n\n\
            Rust is a systems programming language that runs blazingly fast, \
            prevents segfaults, and guarantees thread safety. This guide will \
            help you get started with Rust programming.\n\n\
            ## Installation\n\n\
            To install Rust, use rustup: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`\n\n\
            ## Your First Program\n\n\
            Create a new project with `cargo new hello_world` and write your first Rust program.",
            vec!["rust", "programming", "tutorial"],
        ),
        (
            "Advanced Rust Patterns",
            "# Advanced Rust Patterns\n\n\
            Once you've mastered the basics of Rust, it's time to explore advanced patterns.\n\n\
            ## Ownership and Borrowing\n\n\
            Rust's ownership system is unique. Every value has a single owner, and when \
            the owner goes out of scope, the value is dropped.\n\n\
            ## Lifetimes\n\n\
            Lifetimes ensure that references are valid. They prevent dangling pointers \
            and are a key part of Rust's memory safety guarantees.",
            vec!["rust", "advanced", "patterns"],
        ),
        (
            "Python vs Rust Performance",
            "# Python vs Rust Performance Comparison\n\n\
            When it comes to performance, Rust significantly outperforms Python in most scenarios.\n\n\
            ## CPU-Intensive Tasks\n\n\
            Rust can be 10-100x faster than Python for CPU-bound operations like number crunching.\n\n\
            ## Memory Usage\n\n\
            Rust programs typically use less memory due to zero-cost abstractions and \
            no garbage collector overhead.",
            vec!["rust", "python", "performance", "comparison"],
        ),
        (
            "Building Web APIs with Actix",
            "# Building Web APIs with Actix\n\n\
            Actix-web is a powerful, pragmatic, and extremely fast web framework for Rust.\n\n\
            ## Getting Started\n\n\
            Add actix-web to your Cargo.toml: `actix-web = \"4.0\"`\n\n\
            ## Creating Your First Endpoint\n\n\
            ```rust\n\
            use actix_web::{web, App, HttpServer, Responder};\n\n\
            async fn hello() -> impl Responder {\n\
                \"Hello, World!\"\n\
            }\n\
            ```",
            vec!["rust", "web", "actix", "api"],
        ),
        (
            "Memory Safety in Systems Programming",
            "# Memory Safety in Systems Programming\n\n\
            Memory safety is crucial in systems programming to prevent bugs and security vulnerabilities.\n\n\
            ## Common Memory Issues\n\n\
            - Buffer overflows\n\
            - Use after free\n\
            - Double free\n\
            - Null pointer dereferences\n\n\
            ## How Rust Helps\n\n\
            Rust's ownership system and borrow checker prevent these issues at compile time.",
            vec!["memory", "safety", "systems", "programming"],
        ),
    ];

    // Insert sample notes into database
    println!("📝 Inserting sample notes...");
    for (i, (title, content, tags)) in sample_notes.iter().enumerate() {
        let note = Note {
            id: format!("note_{}", i),
            title: title.to_string(),
            content: content.to_string(),
            body: content.to_string(),
            metadata: NoteMetadata {
                title: Some(title.to_string()),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                aliases: Vec::new(),
                custom: HashMap::new(),
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
            },
            path: PathBuf::from(format!("{}.md", title.to_lowercase().replace(' ', "-"))),
            checksum: format!("{:x}", md5::compute(content)),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        database.upsert_note(&note).await?;
        println!("  ✓ {}", title);
    }
    println!("\n✅ {} notes indexed\n", sample_notes.len());

    // Initialize search engine
    let search_config = SearchConfig {
        max_results: 10,
        context_lines: 2,
        fuzzy_search: false,
        fuzzy_threshold: 0.8,
        highlight_matches: true,
        title_boost: 1.5,
        recency_boost: 0.2,
        max_suggestions: 5,
        enable_suggestions: true,
    };

    let search_engine = SearchEngine::new(database.clone(), search_config);

    // Demonstrate different types of searches
    println!("🔍 SEARCH DEMONSTRATIONS\n");
    println!("{}", "=".repeat(60));

    // 1. Simple keyword search
    println!("\n1️⃣  Simple Keyword Search: 'rust'\n");
    let results = database.search_notes_with_snippets("rust", 5, 0).await?;
    display_results(&results);

    // 2. Phrase search
    println!("\n2️⃣  Phrase Search: '\"systems programming\"'\n");
    let results = database
        .search_notes_with_snippets("\"systems programming\"", 5, 0)
        .await?;
    display_results(&results);

    // 3. Boolean AND search
    println!("\n3️⃣  Boolean AND Search: 'rust AND performance'\n");
    let results = database
        .search_notes_with_snippets("rust AND performance", 5, 0)
        .await?;
    display_results(&results);

    // 4. Boolean OR search
    println!("\n4️⃣  Boolean OR Search: 'python OR actix'\n");
    let results = database
        .search_notes_with_snippets("python OR actix", 5, 0)
        .await?;
    display_results(&results);

    // 5. Prefix search
    println!("\n5️⃣  Prefix Search: 'prog*'\n");
    let results = database.search_notes_with_snippets("prog*", 5, 0).await?;
    display_results(&results);

    // 6. Complex query
    println!("\n6️⃣  Complex Query: 'rust AND (web OR api)'\n");
    let results = database
        .search_notes_with_snippets("rust AND (web OR api)", 5, 0)
        .await?;
    display_results(&results);

    // Demonstrate search suggestions
    println!("\n💡 SEARCH SUGGESTIONS\n");
    println!("{}", "=".repeat(60));

    let suggestions = search_engine.get_suggestions("rus").await?;
    println!("\nSuggestions for 'rus':");
    for (i, suggestion) in suggestions.iter().enumerate() {
        println!(
            "  {}. {} (confidence: {:.2})",
            i + 1,
            suggestion.suggestion,
            suggestion.confidence
        );
    }

    // Demonstrate tag-based search
    println!("\n🏷️  TAG-BASED SEARCH\n");
    println!("{}", "=".repeat(60));

    let tag_results = database.get_notes_by_tag("rust").await?;
    println!("\nNotes tagged with 'rust':");
    for note in tag_results.iter() {
        println!("  • {}", note.title);
    }

    // Show FTS5 search performance
    println!("\n⚡ PERFORMANCE TEST\n");
    println!("{}", "=".repeat(60));

    let start = std::time::Instant::now();
    let _ = database.search_notes("rust programming", 100, 0).await?;
    let duration = start.elapsed();
    println!("\nSearch completed in: {:?}", duration);

    // Cleanup
    println!("\n✅ Demo completed successfully!");

    Ok(())
}

/// Helper function to display search results with snippets
fn display_results(results: &[(Note, String, String)]) {
    if results.is_empty() {
        println!("  No results found.");
        return;
    }

    for (i, (note, title_snippet, body_snippet)) in results.iter().enumerate() {
        println!("  {}. {}", i + 1, note.title);

        // Show title snippet if it contains matches
        if title_snippet.contains("<mark>") {
            let clean_title = title_snippet
                .replace("<mark>", "【")
                .replace("</mark>", "】");
            println!("     Title: {}", clean_title);
        }

        // Show body snippet
        if body_snippet.contains("<mark>") {
            let clean_body = body_snippet
                .replace("<mark>", "【")
                .replace("</mark>", "】")
                .replace('\n', " ");
            println!("     Match: ...{}...", clean_body);
        }

        println!("     Tags: {}", note.metadata.tags.join(", "));
        println!();
    }

    println!("  Found {} result(s)", results.len());
}
