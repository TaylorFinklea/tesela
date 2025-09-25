//! Direct test of PowerSearch functionality without TUI
//!
//! This tests PowerSearch programmatically to diagnose why search results aren't showing

use anyhow::Result;
use tesela::commands;
use tesela::tui::async_runtime::AsyncRuntime;
use tesela::tui::power_search::PowerSearchMode;

fn main() -> Result<()> {
    println!("🔍 PowerSearch Direct Test\n");
    println!("==========================\n");

    // Check we're in a mosaic
    if !std::path::Path::new("tesela.toml").exists() {
        println!("❌ No tesela.toml found. Run from a directory with a tesela mosaic.");
        return Ok(());
    }

    // Initialize async runtime (this should index notes if needed)
    println!("1️⃣ Initializing AsyncRuntime...");
    let async_runtime = AsyncRuntime::new()?;
    println!("   ✅ AsyncRuntime initialized\n");

    // Get existing notes from filesystem
    println!("2️⃣ Getting notes from filesystem...");
    let fs_notes = commands::get_notes_with_paths().unwrap_or_default();
    let existing_notes: Vec<(String, String)> = fs_notes
        .into_iter()
        .map(|(path, title, _)| (path, title))
        .collect();

    println!("   📚 Found {} notes in filesystem:", existing_notes.len());
    for (path, title) in existing_notes.iter() {
        println!("      - {} -> {}", title, path);
    }
    println!();

    // Test search for "dawg"
    let query = "dawg";
    println!("3️⃣ Searching database for '{}'...", query);

    let content_results = match async_runtime.search_notes(query) {
        Ok(results) => {
            println!("   ✅ Search completed, found {} results:", results.len());
            for result in &results {
                println!("      - {} (score: {:.2})", result.title, result.rank);
                if let Some(ref snippet) = result.snippet {
                    println!("        Snippet: {}", snippet);
                }
            }
            results
        }
        Err(e) => {
            println!("   ❌ Search failed: {}", e);
            vec![]
        }
    };
    println!();

    // Create PowerSearch and update it
    println!("4️⃣ Testing PowerSearch update_results...");
    let mut power_search = PowerSearchMode::new();

    println!("   Calling update_results with:");
    println!("   - Query: '{}'", query);
    println!("   - {} existing notes", existing_notes.len());
    println!("   - {} content results", content_results.len());

    power_search.update_results(query, existing_notes.clone(), content_results.clone());

    println!(
        "\n   📊 PowerSearch created {} sections:",
        power_search.sections.len()
    );
    for section in &power_search.sections {
        println!("\n   📁 {} ({} items):", section.title, section.items.len());
        for item in section.items.iter().take(3) {
            println!("      - {}", item.title);
            if let Some(ref snippet) = item.snippet {
                let clean_snippet = snippet
                    .replace("<mark>", "【")
                    .replace("</mark>", "】")
                    .chars()
                    .take(80)
                    .collect::<String>();
                println!("        {}", clean_snippet);
            }
        }
        if section.items.len() > 3 {
            println!("      ... and {} more", section.items.len() - 3);
        }
    }

    if power_search.sections.is_empty() {
        println!("\n   ⚠️ No sections created!");
        println!("   This explains why you see no results in the TUI.");
    }

    // Test with a non-existing note name
    println!("\n5️⃣ Testing with non-existing note 'my-new-note'...");
    power_search.update_results("my-new-note", existing_notes.clone(), vec![]);

    println!(
        "   📊 PowerSearch created {} sections:",
        power_search.sections.len()
    );
    for section in &power_search.sections {
        println!("   - {} ({} items)", section.title, section.items.len());
    }

    // Check if Create section appears correctly
    let has_create = power_search
        .sections
        .iter()
        .any(|s| s.section_type == tesela::tui::power_search::SectionType::Create);

    if has_create {
        println!("   ✅ Create section appears for non-existing note");
    } else {
        println!("   ❌ Create section missing for non-existing note");
    }

    println!("\n✅ Test complete!");

    println!("\n📋 Summary:");
    println!(
        "- Database search: {}",
        if content_results.is_empty() {
            "❌ No results"
        } else {
            "✅ Working"
        }
    );
    println!(
        "- PowerSearch sections: {}",
        if power_search.sections.is_empty() {
            "❌ Not created"
        } else {
            "✅ Created"
        }
    );
    println!(
        "- Create section logic: {}",
        if has_create {
            "✅ Working"
        } else {
            "❓ Needs testing with non-existing note"
        }
    );

    Ok(())
}
