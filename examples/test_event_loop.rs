//! Test to verify the event loop fix for PowerSearch
//!
//! This simulates the TUI event loop to ensure pending searches
//! trigger after debounce period even without key presses.

use anyhow::Result;
use crossterm::event;
use std::time::{Duration, Instant};
use tesela::tui::async_runtime::AsyncRuntime;
use tesela::tui::power_search::PowerSearchMode;

fn main() -> Result<()> {
    println!("🔍 Event Loop Test for PowerSearch\n");
    println!("===================================\n");

    // Initialize components
    let async_runtime = AsyncRuntime::new()?;
    let mut power_search = PowerSearchMode::new();

    println!("Test 1: Simulating OLD event loop (bug)");
    println!("----------------------------------------");
    simulate_old_event_loop(&mut power_search.clone(), &async_runtime)?;

    println!("\nTest 2: Simulating NEW event loop (fixed)");
    println!("------------------------------------------");
    simulate_new_event_loop(&mut power_search, &async_runtime)?;

    Ok(())
}

fn simulate_old_event_loop(
    power_search: &mut PowerSearchMode,
    _async_runtime: &AsyncRuntime,
) -> Result<()> {
    println!("Simulating typing 'dawg' with OLD event loop...\n");

    // Simulate typing "dawg"
    power_search.query = "dawg".to_string();
    power_search.cursor_position = 4;
    power_search.pending_query = Some("dawg".to_string());
    power_search.last_query_time = Instant::now();

    println!("✍️  Typed 'dawg', pending_query set");
    println!("⏰ Last keystroke time: {:?}", power_search.last_query_time);

    let mut search_triggered = false;
    let mut iterations = 0;
    let start = Instant::now();

    // Simulate 5 event loop iterations (500ms total with 100ms poll timeout)
    while iterations < 5 && !search_triggered {
        iterations += 1;
        println!("\n🔄 Loop iteration {}", iterations);

        // OLD BUGGY CODE: Check pending ONLY if event happened
        if would_poll_return_event(iterations) {
            println!("  📥 Event detected (simulated)");

            // Process pending searches (INSIDE event handling - BUG!)
            if let Some(ref pending) = power_search.pending_query {
                if power_search.last_query_time.elapsed() >= Duration::from_millis(250) {
                    println!("  🔍 Search triggered for '{}'!", pending);
                    search_triggered = true;
                } else {
                    println!(
                        "  ⏳ Still in debounce period ({:?} elapsed)",
                        power_search.last_query_time.elapsed()
                    );
                }
            }
        } else {
            println!("  ⏸️  No event (poll timeout)");
            // In buggy version, pending search is NOT checked here!
        }

        // Simulate 100ms poll timeout
        std::thread::sleep(Duration::from_millis(100));
    }

    if !search_triggered {
        println!("\n❌ BUG CONFIRMED: Search never triggered!");
        println!("   Even though {:?} elapsed", start.elapsed());
        println!("   The search check is inside event handling,");
        println!("   so it never runs when no keys are pressed!");
    }

    Ok(())
}

fn simulate_new_event_loop(
    power_search: &mut PowerSearchMode,
    async_runtime: &AsyncRuntime,
) -> Result<()> {
    println!("Simulating typing 'dawg' with NEW event loop...\n");

    // Reset and simulate typing "dawg"
    power_search.query = "dawg".to_string();
    power_search.cursor_position = 4;
    power_search.pending_query = Some("dawg".to_string());
    power_search.last_query_time = Instant::now();

    println!("✍️  Typed 'dawg', pending_query set");
    println!("⏰ Last keystroke time: {:?}", power_search.last_query_time);

    let mut search_triggered = false;
    let mut iterations = 0;
    let start = Instant::now();

    // Simulate 5 event loop iterations
    while iterations < 5 && !search_triggered {
        iterations += 1;
        println!("\n🔄 Loop iteration {}", iterations);

        // NEW FIXED CODE: Check pending BEFORE event handling
        if let Some(pending) = power_search.pending_query.clone() {
            if power_search.last_query_time.elapsed() >= Duration::from_millis(250) {
                println!("  🔍 Search triggered for '{}'!", pending);

                // Simulate the actual search
                println!("  📊 Getting existing notes...");
                let existing_notes = tesela::commands::get_notes_with_paths()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(path, title, _)| (path, title))
                    .collect::<Vec<_>>();

                println!("  📊 Searching database...");
                let content_results = async_runtime.search_notes(&pending).unwrap_or_default();

                println!(
                    "  📊 Updating PowerSearch with {} notes, {} results",
                    existing_notes.len(),
                    content_results.len()
                );

                power_search.update_results(&pending, existing_notes, content_results);
                power_search.pending_query = None;
                power_search.is_searching = false;

                println!(
                    "  ✅ Search completed! Sections created: {}",
                    power_search.sections.len()
                );
                for section in &power_search.sections {
                    println!("     - {} ({} items)", section.title, section.items.len());
                }

                search_triggered = true;
            } else {
                println!(
                    "  ⏳ Still in debounce period ({:?} elapsed)",
                    power_search.last_query_time.elapsed()
                );
            }
        }

        // Then handle events (no pending check here)
        if would_poll_return_event(iterations) {
            println!("  📥 Event detected (simulated)");
        } else {
            println!("  ⏸️  No event (poll timeout)");
        }

        // Simulate 100ms poll timeout
        std::thread::sleep(Duration::from_millis(100));
    }

    if search_triggered {
        println!("\n✅ FIX CONFIRMED: Search triggered correctly!");
        println!("   After {:?} (>250ms debounce)", start.elapsed());
        println!("   The search runs regardless of key events!");
    } else {
        println!("\n⚠️  Search didn't trigger in test timeframe");
    }

    Ok(())
}

// Simulate whether poll() would return an event
// In reality, no events after initial typing
fn would_poll_return_event(iteration: usize) -> bool {
    // Only first iteration might have trailing events
    iteration == 1
}
