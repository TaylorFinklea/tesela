//! Simple debug script to check PowerSearch functionality
//!
//! This script uses AsyncRuntime to check why content search might not be working

use anyhow::Result;
use tesela::tui::async_runtime::AsyncRuntime;

fn main() -> Result<()> {
    println!("🔍 PowerSearch Debug Tool\n");
    println!("========================\n");

    // Initialize async runtime (same as TUI does)
    let async_runtime = AsyncRuntime::new()?;
    println!("✅ AsyncRuntime initialized\n");

    // Test 1: Search for "dawg"
    println!("🔍 Testing search for 'dawg'...");
    let dawg_results = async_runtime.search_notes("dawg")?;
    println!("Found {} results for 'dawg':", dawg_results.len());

    if dawg_results.is_empty() {
        println!("❌ No results found for 'dawg'\n");
    } else {
        for (i, result) in dawg_results.iter().enumerate() {
            println!("  {}. {} (score: {:.2})", i + 1, result.title, result.rank);
            println!("     Path: {}", result.path);
            if let Some(ref snippet) = result.snippet {
                println!("     Snippet: {}", snippet);
            }
            println!(
                "     Content preview: {}",
                result
                    .content
                    .chars()
                    .take(100)
                    .collect::<String>()
                    .replace('\n', " ")
            );
            println!();
        }
    }

    // Test 2: Try some common words
    println!("🔍 Testing search for common words...");
    for word in &["the", "a", "is", "to", "and"] {
        let results = async_runtime.search_notes(word)?;
        if !results.is_empty() {
            println!("✅ Found {} results for '{}'", results.len(), word);

            // Show first result as example
            if let Some(first) = results.first() {
                println!(
                    "   Example: {} - {}",
                    first.title,
                    first
                        .content
                        .chars()
                        .take(80)
                        .collect::<String>()
                        .replace('\n', " ")
                );
            }
            break;
        } else {
            println!("❌ No results for '{}'", word);
        }
    }

    // Test 3: Try searching for something that should definitely exist
    println!("\n🔍 Testing if database has any indexed content...");

    // Try a wildcard search or empty search to see if anything is indexed
    let all_results = async_runtime.search_notes("*")?;
    if all_results.is_empty() {
        let all_results = async_runtime.search_notes("")?;
    }

    if all_results.is_empty() {
        println!("❌ No content found in database - this is the problem!");
        println!("\n💡 The issue is likely that your notes aren't indexed in the database.");
        println!("   This can happen if:");
        println!("   1. This is a fresh install and notes haven't been indexed yet");
        println!("   2. The database file is missing or corrupted");
        println!("   3. The file watcher hasn't indexed your existing notes");
        println!("\n🔧 Try this fix:");
        println!("   1. Quit the TUI if it's running");
        println!("   2. Delete the tesela.db file (if it exists)");
        println!("   3. Restart the TUI - it should reindex all notes automatically");
    } else {
        println!("✅ Database has {} indexed notes", all_results.len());
        println!("\nSample indexed notes:");
        for (i, result) in all_results.iter().take(5).enumerate() {
            println!("  {}. {} - {}", i + 1, result.title, result.path);
        }

        if all_results.len() > 5 {
            println!("  ... and {} more", all_results.len() - 5);
        }

        println!("\n🤔 Notes are indexed but 'dawg' search failed.");
        println!("   This means 'dawg' might not be in any of your indexed notes,");
        println!("   or there might be a search query issue.");
    }

    // Test 4: Check tags
    println!("\n🏷️  Checking tags...");
    let tags = async_runtime.get_all_tags()?;
    if tags.is_empty() {
        println!("📝 No tags found in database");
    } else {
        println!("📝 Found {} tags: {:?}", tags.len(), tags);
    }

    println!("\n✅ Debug complete!");
    println!("\nNext steps:");
    println!("1. If no notes were found, delete tesela.db and restart TUI");
    println!("2. Make sure you have .md files in the notes/ directory");
    println!("3. Try searching for words you know are in your notes");
    println!("4. The PowerSearch should work once notes are properly indexed");

    Ok(())
}
