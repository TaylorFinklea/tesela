# PowerSearch Fix - August 28, 2025

## Fixed Issues

### 1. Create Section Display Bug
**Problem:** The Create section was showing duplicate text: "Create page — Create page called 'dawg'"
**Solution:** Removed duplicate prefix in `src/tui/power_search.rs` line 160
**Result:** Now displays clean "Create page called 'dawg'" format

### 2. Content Search Returning Zero Results
**Problem:** FTS5 search was returning 0 results even when content existed in daily notes
**Root Cause:** Only the "notes" directory was being indexed, not the "dailies" directory
**Solution:** Modified both `index_existing_notes` and `reindex_all` functions in `src/tui/async_runtime.rs` to index both directories
**Result:** Search now properly finds content in both regular notes and daily notes

### 3. UI Text Formatting Issues
**Problem:** Text appeared garbled (showing "Crea e page called 'd wg'" instead of "Create page called 'dawg'")
**Solution:** Removed double formatting in `src/tui/ui.rs` line 387 for Create section items
**Result:** Clean, properly formatted text display

### 4. Database Indexing on Startup
**Problem:** FTS5 index wasn't always up-to-date
**Solution:** Added automatic `rebuild_index()` call after database initialization
**Result:** Search index is always current when TUI starts

## Files Modified

- `src/tui/power_search.rs` - Fixed duplicate text in Create section
- `src/tui/ui.rs` - Removed double formatting for Create section display
- `src/tui/async_runtime.rs` - Added dailies directory indexing and FTS5 rebuild
- `src/tui/app.rs` - Removed orphaned code fragment causing build error
- `src/core/database.rs` - Added debug logging for search troubleshooting

## Technical Details

### Database Changes
- Extended indexing to include both `notes/` and `dailies/` directories
- Added automatic FTS5 index rebuild on startup
- Both `index_existing_notes` and `reindex_all` now handle multiple directories

### Debug Improvements
- Added comprehensive debug logging to trace search flow
- Added FTS query logging to understand search behavior
- Debug output now shows query transformation and result counts

## Testing Results

After fixes:
- Database properly indexes all 5 notes (2 from notes/, 3 from dailies/)
- FTS5 search for "dawg" correctly returns 3 matches from daily notes
- Create section displays properly formatted text
- No UI formatting issues or text corruption
- Search results display with proper context and snippets

## User Impact

Users can now:
- Successfully search for content across all notes including daily notes
- See properly formatted Create suggestions
- Experience responsive and accurate search results
- Use PowerSearch as the unified interface for both creating and finding notes

## Debug Cleanup (Final Fix)

### Problem
Debug messages (`eprintln!` statements) were cluttering the TUI interface and breaking the visual formatting.

### Solution
Removed all debug statements from:
- `src/core/database.rs` - Removed 4 debug messages
- `src/tui/power_search.rs` - Removed 9 debug messages  
- `src/tui/app.rs` - Removed 13 debug messages
- `src/tui/async_runtime.rs` - Commented out warning messages and success notifications

### Result
- Clean TUI interface without debug output
- Proper visual formatting of PowerSearch sections
- Professional user experience without development artifacts