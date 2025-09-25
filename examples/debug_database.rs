//! Debug script to check database indexing status
//!
//! This script helps diagnose why PowerSearch might not be showing content results

use anyhow::Result;
use std::path::PathBuf;
use tesela::core::{Database, Storage, StorageConfig};

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔍 Database Indexing Debug\n");
    println!("========================\n");

    // Initialize database
    let db = Database::new("tesela.db").await?;
    println!("✅ Connected to database\n");

    // Check if notes table has data
    let all_notes = db.get_all_notes().await?;
    println!("📚 Notes in database: {}", all_notes.len());

    if all_notes.is_empty() {
        println!("❌ No notes found in database! This is likely the issue.\n");
        println!("Let's check the filesystem and reindex...\n");

        // Check filesystem
        let config = StorageConfig {
            mosaic_root: PathBuf::from("."),
            notes_dir: "notes".to_string(),
            attachments_dir: "attachments".to_string(),
            note_extensions: vec!["md".to_string()],
            max_attachment_size: 10 * 1024 * 1024,
        };

        let storage = Storage::new(config);
        let fs_notes = storage.list_notes()?;
        println!("📁 Notes found in filesystem: {}", fs_notes.len());

        for (i, note) in fs_notes.iter().enumerate().take(5) {
            println!("  {}. {} ({})", i + 1, note.title, note.path.display());
        }

        if fs_notes.len() > 5 {
            println!("  ... and {} more", fs_notes.len() - 5);
        }

        if !fs_notes.is_empty() {
            println!("\n🔄 Reindexing notes...");

            // Clear existing notes
            db.clear_all_notes().await?;

            // Reindex all notes
            for note in &fs_notes {
                db.upsert_note(note).await?;
            }

            println!("✅ Reindexed {} notes\n", fs_notes.len());

            // Verify reindexing
            let reindexed_notes = db.get_all_notes().await?;
            println!("📚 Notes now in database: {}", reindexed_notes.len());
        }
    } else {
        println!("📝 Sample notes in database:");
        for (i, note) in all_notes.iter().enumerate().take(5) {
            println!("  {}. {} ({})", i + 1, note.title, note.path.display());
        }
        if all_notes.len() > 5 {
            println!("  ... and {} more", all_notes.len() - 5);
        }
    }

    println!("\n🔍 Testing search for 'dawg'...");
    let dawg_results = db.search_notes_with_snippets("dawg").await?;
    println!("Found {} results for 'dawg':", dawg_results.len());

    for (i, result) in dawg_results.iter().enumerate() {
        println!("  {}. {} (score: {:.2})", i + 1, result.title, result.rank);
        println!("     Path: {}", result.path);
        if let Some(ref snippet) = result.snippet {
            println!("     Snippet: {}", snippet);
        }
        println!("     Tags: {:?}", result.tags);
        println!();
    }

    if dawg_results.is_empty() {
        println!("❌ No results found for 'dawg'");
        println!("\nLet's try a broader search...");

        // Try searching for common words
        for word in &["the", "and", "a", "is", "to"] {
            let results = db.search_notes_with_snippets(word).await?;
            if !results.is_empty() {
                println!("✅ Found {} results for '{}'", results.len(), word);
                break;
            }
        }
    }

    // Check FTS table directly
    println!("\n🔧 Checking FTS table status...");
    match sqlx::query("SELECT COUNT(*) as count FROM notes_fts")
        .fetch_one(&db.pool)
        .await
    {
        Ok(row) => {
            let count: i64 = row.get("count");
            println!("📊 FTS table has {} entries", count);
        }
        Err(e) => {
            println!("❌ Error checking FTS table: {}", e);
        }
    }

    // Show some raw FTS entries
    match sqlx::query("SELECT title, snippet(notes_fts, 1, '<mark>', '</mark>', '...', 50) as snippet FROM notes_fts LIMIT 3")
        .fetch_all(&db.pool)
        .await
    {
        Ok(rows) => {
            println!("\n📝 Sample FTS entries:");
            for row in rows {
                let title: String = row.get("title");
                let snippet: String = row.get("snippet");
                println!("  • {}: {}", title, snippet);
            }
        }
        Err(e) => {
            println!("❌ Error fetching FTS samples: {}", e);
        }
    }

    println!("\n✅ Debug complete!");

    if dawg_results.is_empty() {
        println!("\n💡 Recommendations:");
        println!("1. Make sure you have notes containing 'dawg' in your notes/ directory");
        println!("2. Try running the TUI again - it should work now if we reindexed");
        println!("3. The database should now be properly indexed for PowerSearch");
    }

    Ok(())
}
