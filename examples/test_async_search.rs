//! Test example for async database search in TUI
//!
//! This example demonstrates that the async runtime bridge is working
//! and that we can perform FTS5 database searches from synchronous code.

use anyhow::Result;
use std::time::Instant;
use tesela::tui::async_runtime::AsyncRuntime;

fn main() -> Result<()> {
    println!("🧪 Testing Async Database Search Integration\n");
    println!("{}", "=".repeat(60));

    // Create async runtime
    println!("\n📦 Initializing async runtime...");
    let runtime = AsyncRuntime::new()?;
    println!("✅ Async runtime initialized");

    // Test 1: Full-text search
    println!("\n🔍 Test 1: Full-text search for 'rust'");
    let start = Instant::now();
    let results = runtime.search_notes("rust")?;
    let elapsed = start.elapsed();

    println!("  Found {} results in {:?}", results.len(), elapsed);
    for (i, result) in results.iter().take(3).enumerate() {
        println!("  {}. {}", i + 1, result.title);
        if let Some(ref snippet) = result.snippet {
            let clean_snippet = snippet.replace("<mark>", "**").replace("</mark>", "**");
            println!("     Snippet: {}", clean_snippet);
        }
        println!("     Tags: {:?}", result.tags);
    }

    // Test 2: Search by tag
    println!("\n🏷️  Test 2: Search by tag 'daily'");
    let start = Instant::now();
    let results = runtime.search_by_tag("daily")?;
    let elapsed = start.elapsed();

    println!("  Found {} results in {:?}", results.len(), elapsed);
    for (i, result) in results.iter().take(3).enumerate() {
        println!("  {}. {} ({})", i + 1, result.title, result.path);
    }

    // Test 3: Get all tags
    println!("\n🏷️  Test 3: Get all unique tags");
    let start = Instant::now();
    let tags = runtime.get_all_tags()?;
    let elapsed = start.elapsed();

    println!("  Found {} unique tags in {:?}", tags.len(), elapsed);
    if !tags.is_empty() {
        println!("  Tags: {:?}", &tags[..tags.len().min(10)]);
    }

    // Test 4: Date range search (last 7 days)
    println!("\n📅 Test 4: Search notes from last 7 days");
    let from = Some(chrono::Utc::now() - chrono::Duration::days(7));
    let to = Some(chrono::Utc::now());

    let start = Instant::now();
    let results = runtime.search_by_date_range(from, to)?;
    let elapsed = start.elapsed();

    println!("  Found {} results in {:?}", results.len(), elapsed);
    for (i, result) in results.iter().take(3).enumerate() {
        println!("  {}. {} ({})", i + 1, result.title, result.path);
    }

    // Test 5: Combined search with filters
    println!("\n🔍 Test 5: Combined search with text and tags");
    let start = Instant::now();
    let results =
        runtime.search_with_filters(Some("test"), vec!["daily".to_string()], None, None)?;
    let elapsed = start.elapsed();

    println!("  Found {} results in {:?}", results.len(), elapsed);
    for (i, result) in results.iter().take(3).enumerate() {
        println!("  {}. {} (rank: {:.2})", i + 1, result.title, result.rank);
    }

    // Performance summary
    println!("\n📊 Performance Summary:");
    println!("  ✅ Async runtime bridge is working");
    println!("  ✅ FTS5 search is accessible from TUI");
    println!("  ✅ All database operations functional");

    if results.is_empty() && tags.is_empty() {
        println!("\n⚠️  Note: No data found. Make sure to:");
        println!("  1. Run 'tesela init' to create a mosaic");
        println!("  2. Create some notes with 'tesela new'");
        println!("  3. Run this test again");
    } else {
        println!("\n✨ All tests passed! Database search is ready for TUI integration.");
    }

    Ok(())
}
