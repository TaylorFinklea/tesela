//! Simulate TUI PowerSearch behavior to diagnose issues
//!
//! This simulates exactly what the TUI should be doing when you type "dawg"

use anyhow::Result;
use std::time::{Duration, Instant};
use tesela::commands;
use tesela::tui::async_runtime::AsyncRuntime;
use tesela::tui::power_search::PowerSearchMode;

fn main() -> Result<()> {
    println!("🎮 TUI PowerSearch Simulation\n");
    println!("==============================\n");

    // Check we're in a mosaic
    if !std::path::Path::new("tesela.toml").exists() {
        println!("❌ No tesela.toml found. Run from a directory with a tesela mosaic.");
        return Ok(());
    }

    // Step 1: Initialize like TUI does
    println!("1️⃣ Initializing AsyncRuntime (like TUI does)...");
    let async_runtime = match AsyncRuntime::new() {
        Ok(runtime) => {
            println!("   ✅ AsyncRuntime initialized successfully");
            runtime
        }
        Err(e) => {
            println!("   ❌ Failed to initialize AsyncRuntime: {}", e);
            return Err(e);
        }
    };
    println!();

    // Step 2: Create PowerSearch mode
    println!("2️⃣ Creating PowerSearch mode (like pressing 'S' in TUI)...");
    let mut power_search = PowerSearchMode::new();
    println!("   ✅ PowerSearch created");
    println!("   Initial state:");
    println!("     - Query: '{}'", power_search.query);
    println!("     - Sections: {}", power_search.sections.len());
    println!("     - Pending query: {:?}", power_search.pending_query);
    println!();

    // Step 3: Simulate typing "dawg" letter by letter
    println!("3️⃣ Simulating typing 'd-a-w-g'...\n");

    let letters = ['d', 'a', 'w', 'g'];
    for (i, letter) in letters.iter().enumerate() {
        println!("   Typing '{}'...", letter);

        // Simulate character input
        power_search.query.push(*letter);
        power_search.cursor_position = power_search.query.len();

        // Set pending query (like TUI does)
        power_search.pending_query = Some(power_search.query.clone());
        power_search.last_query_time = Instant::now();

        println!("     Query now: '{}'", power_search.query);
        println!("     Pending: {:?}", power_search.pending_query);

        // Don't search yet - wait for debounce
        if i < letters.len() - 1 {
            println!("     (waiting for more input...)\n");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    println!();

    // Step 4: Wait for debounce period
    println!("4️⃣ Waiting 250ms for debounce (like TUI does)...");
    std::thread::sleep(Duration::from_millis(260));

    // Check if we should search now
    if let Some(ref pending) = power_search.pending_query {
        if power_search.last_query_time.elapsed() >= Duration::from_millis(250) {
            println!("   ✅ Debounce period elapsed, triggering search");

            // Step 5: Perform the search (exactly like TUI)
            println!("\n5️⃣ Performing search (exactly like TUI does)...");

            power_search.is_searching = true;
            let query = pending.clone();

            // Get existing notes from filesystem
            println!("   Getting existing notes from filesystem...");
            let existing_notes: Vec<(String, String)> = commands::get_notes_with_paths()
                .unwrap_or_default()
                .into_iter()
                .map(|(path, title, _)| (path, title))
                .collect();

            println!("     Found {} notes:", existing_notes.len());
            for (path, title) in &existing_notes {
                println!("       - {} -> {}", title, path);
            }

            // Perform content search
            println!("\n   Searching database for '{}'...", query);
            let content_results = match async_runtime.search_notes(&query) {
                Ok(results) => {
                    println!("     ✅ Search returned {} results:", results.len());
                    for result in &results {
                        println!("       - {} (score: {:.2})", result.title, result.rank);
                        if let Some(ref snippet) = result.snippet {
                            println!("         Snippet: {}", snippet);
                        }
                    }
                    results
                }
                Err(e) => {
                    println!("     ❌ Search failed: {}", e);
                    vec![]
                }
            };

            // Update PowerSearch with results
            println!("\n   Updating PowerSearch with results...");
            power_search.update_results(&query, existing_notes, content_results);
            power_search.pending_query = None;
            power_search.is_searching = false;

            println!("     ✅ Update complete");
        } else {
            println!("   ⏳ Still within debounce period");
        }
    } else {
        println!("   ❌ No pending query!");
    }

    // Step 6: Show final state
    println!("\n6️⃣ Final PowerSearch state:");
    println!("   Query: '{}'", power_search.query);
    println!("   Sections: {}", power_search.sections.len());

    for section in &power_search.sections {
        println!("\n   📁 {} ({} items):", section.title, section.items.len());
        for (i, item) in section.items.iter().enumerate() {
            if i >= 3 {
                println!("       ... and {} more", section.items.len() - 3);
                break;
            }
            println!("       - {}", item.title);
            if let Some(ref snippet) = item.snippet {
                let clean = snippet
                    .replace("<mark>", "【")
                    .replace("</mark>", "】")
                    .chars()
                    .take(60)
                    .collect::<String>();
                println!("         {}", clean);
            }
        }
    }

    // Diagnosis
    println!("\n🔍 Diagnosis:");
    if power_search.sections.is_empty() {
        println!("   ❌ No sections created - this is the problem!");
        println!("   Possible causes:");
        println!("   1. AsyncRuntime not initialized properly");
        println!("   2. Database not accessible");
        println!("   3. Search query not being executed");
        println!("   4. Results not being passed to update_results");
    } else {
        println!("   ✅ PowerSearch is working correctly!");
        println!("   If you don't see these sections in the TUI, the issue might be:");
        println!("   1. UI rendering problem");
        println!("   2. Event loop not processing updates");
        println!("   3. Mode not being set correctly");
    }

    // Test what happens with an empty query too
    println!("\n7️⃣ Testing empty query (should show recents if any)...");
    let mut ps2 = PowerSearchMode::new();
    ps2.update_results("", vec![], vec![]);
    println!("   Empty query creates {} sections", ps2.sections.len());
    for section in &ps2.sections {
        println!("     - {} ({} items)", section.title, section.items.len());
    }

    println!("\n✅ Simulation complete!");

    Ok(())
}
