# Tesela TUI Fixes Summary - August 28, 2025

## Overview
This document summarizes all fixes applied to the Tesela TUI's PowerSearch and Graph View features, addressing critical issues with search functionality, UI display, and database operations.

## 1. PowerSearch Fixes

### Issue 1: Create Section Duplicate Text
**Problem:** The Create section showed duplicate text: "Create page — Create page called 'dawg'"
**Root Cause:** Redundant formatting in title string
**Fix:** 
- File: `src/tui/power_search.rs:160`
- Changed from: `format!("Create page — Create page called '{}'", query)`
- Changed to: `format!("Create page called '{}'", query)`

### Issue 2: Content Search Returning Zero Results
**Problem:** FTS5 search returned 0 results even when content existed
**Root Cause:** Multiple issues:
1. Only "notes" directory was being indexed, not "dailies"
2. Database connection string was incorrect
3. FTS5 snippet function using wrong column indexes

**Fixes:**
1. **Directory Indexing** (`src/tui/async_runtime.rs`):
   - Modified `index_existing_notes` to index both "notes" AND "dailies" directories
   - Modified `reindex_all` to include both directories
   
2. **Database Connection** (`src/core/database.rs`):
   - Fixed SQLite connection string format from `"tesela.db"` to `"sqlite:tesela.db"`
   - Used absolute path for database connection in `AsyncRuntime`

3. **FTS5 Column Indexes** (`src/core/database.rs`):
   - Fixed snippet column indexes: Column 0=id, Column 1=title, Column 2=body
   - Changed from `snippet(notes_fts, 1, ...)` to `snippet(notes_fts, 2, ...)` for body content

### Issue 3: UI Formatting Issues
**Problem:** Text appeared garbled in search results
**Fixes:**
- Removed double formatting in `src/tui/ui.rs:387`
- Simplified Create section display to avoid redundant formatting

### Issue 4: Snippet Display Issues
**Problem:** 
1. Only showing first two results, third was cut off
2. Showing entire file content instead of just matching line
3. Unnecessary "..." decorations

**Fixes** (`src/tui/ui.rs`):
- Adjusted height calculation for Tiles section (3 lines per item)
- Modified snippet display to show only the line containing the match
- Removed "..." decorations
- Reduced FTS5 snippet window from 30-64 tokens to 10 tokens

### Issue 5: Debug Message Cleanup
**Problem:** Debug messages were cluttering the TUI interface
**Fix:** Removed all `eprintln!` debug statements from:
- `src/core/database.rs` (4 messages)
- `src/tui/power_search.rs` (9 messages)
- `src/tui/app.rs` (13 messages)
- `src/tui/async_runtime.rs` (warning messages)

## 2. Graph View Fixes

### Issue 1: Backlinks Not Selectable
**Problem:** Backlinks in Graph View were static text, not interactive
**Solution:** 
- Added `selected_backlink: usize` field to `ListingMode` struct
- Implemented Up/Down navigation for backlinks
- Added Enter key handling to open selected backlink source files

### Issue 2: Context Display
**Problem:** Backlinks showed multiple lines of context, cluttering the display
**Solution:**
- Modified `get_link_context` to return only the single line containing the link
- Removed multi-line context display

### Issue 3: Visual Feedback
**Problem:** No visual indication of which backlink was selected
**Solution:**
- Converted from `Paragraph` widget to `List` widget
- Added yellow highlighting for selected backlink
- Added selection indicator (▶)

## 3. Database Initialization Fix

### Problem
Database file was created but remained empty (0 bytes), causing all database operations to fail silently.

### Root Cause
SQLite connection string format was incorrect - `SqliteConnectOptions::from_str()` expects `"sqlite:path"` format, not just `"path"`.

### Solution
Modified `src/core/database.rs`:
```rust
let db_path = config.db_path.to_str().unwrap_or("tesela.db");
let connection_string = format!("sqlite:{}", db_path);
```

## Results After Fixes

### PowerSearch
- ✅ Create section shows clean "Create page called 'query'" format
- ✅ Content search returns results from both notes and dailies directories
- ✅ All search results are visible with proper context snippets
- ✅ Snippets show only the matching line with highlighted terms
- ✅ Both "Pages" (title matches) and "Tiles" (content matches) sections work correctly
- ✅ Clean UI without debug messages

### Graph View
- ✅ Backlinks are fully interactive and selectable
- ✅ Navigate through backlinks with arrow keys (↑↓ or j/k)
- ✅ Press Enter to open backlink source files
- ✅ Clean single-line context display
- ✅ Visual highlighting for selected backlink

### Database
- ✅ Properly initialized with all tables
- ✅ Indexes both notes and dailies directories
- ✅ FTS5 search works correctly with prefix matching
- ✅ Snippets show correct content with highlighted matches

## Testing Verification

### Test Setup
Created test mosaic with:
- `foobar.md` - Note with "foobar" in title
- `another.md` - Note with "foo" in content
- `daily-2025-08-28.md` - Daily note with "foo" in content
- `unrelated.md` - Note without matches

### Expected Behavior (Verified)
When searching for "foo":
1. **Pages section** shows: `foobar.md` (title match)
2. **Tiles section** shows all three notes with content matches
3. Each result displays with single-line context snippet
4. All results are visible without scrolling issues

## Files Modified

1. `src/tui/power_search.rs` - Fixed duplicate text, improved result handling
2. `src/tui/ui.rs` - Fixed display formatting, height calculations, graph view
3. `src/tui/async_runtime.rs` - Fixed directory indexing, database initialization
4. `src/tui/app.rs` - Added backlink selection, removed debug messages
5. `src/core/database.rs` - Fixed FTS5 column indexes, connection string

## User Impact

Users now have a fully functional PowerSearch that:
- Correctly searches across all notes including daily notes
- Shows both title matches and content matches in separate sections
- Displays clean, readable snippets with context
- Provides interactive Graph View with selectable backlinks
- Works reliably with proper database initialization

The TUI is now production-ready with these core features working as expected.