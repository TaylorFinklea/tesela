# Tesela Improvement Plan

## Overview
This plan focuses on practical, incremental improvements to the existing Tesela codebase rather than a complete rewrite. We'll prioritize features that provide immediate value to users while maintaining the current architecture.

## Current State Assessment
- ✅ Working TUI with navigation and search
- ✅ Basic note management (create, edit, list)
- ✅ Simple search functionality
- ✅ Daily notes feature
- ⚠️ Search is basic (no full-text indexing)
- ⚠️ No file watching (manual refresh needed)
- ⚠️ Limited test coverage
- ⚠️ No CI/CD pipeline

## Priority Improvements

### Phase 1: Core Functionality ✅ COMPLETED (Dec 2024)

#### 1.1 Enhance Existing FTS5 Search ✅ COMPLETED
**Why**: FTS5 is already set up but not fully utilized. Need to improve search UX.
**Current State**: SQLite with FTS5 tables and triggers already exist in `database.rs`
**Tasks**:
- [x] Fix FTS search to actually use the notes_fts table properly
- [x] Add ranked search results using FTS5 bm25() function
- [x] Support search operators (AND, OR, phrase search, prefix search)
- [x] Add search highlighting in results using snippet() function
- [x] Implement incremental indexing on note changes
- [ ] Add search result caching for repeated queries (deferred)

**Files modified**:
- ✅ `src/core/database.rs` - Added `search_notes_with_snippets`, `prepare_fts_query`, `delete_note_by_path`
- ✅ `src/core/search.rs` - Updated to use FTS5 snippets for highlighting
- ✅ Created `examples/enhanced_search_demo.rs` - Demonstrates all search features

#### 1.2 Implement File Watcher ✅ COMPLETED
**Why**: Auto-index changes made outside the app, keep index fresh.
**Current State**: `notify` crate is already a dependency but not implemented
**Tasks**:
- [x] Create `src/core/watcher.rs` module using existing notify dependency
- [x] Watch both `notes/` and `dailies/` directories
- [x] Implement 250ms debounce for rapid changes
- [x] Queue reindexing on file changes via channel
- [x] Handle create/modify/delete/rename events
- [x] Add status tracking with WatcherStatus enum
- [x] Gracefully handle watch errors (permissions, etc.)

**Files created**:
- ✅ `src/core/watcher.rs` - Complete file watcher implementation with FileWatcher struct
- ✅ Added to `src/core/mod.rs` - Module exported

### Phase 2: User Experience (Week 2-3)

#### 2.0a Power Search Implementation ✅ COMPLETED (December 2024)
**Why**: Current "create new note" overwrites existing notes - DANGEROUS! Need a unified search/create interface like Logseq.
**Current Problem**: `create_note` command doesn't check if note exists, causing data loss
**Tasks**:
- [x] Fix immediate bug: Check if note exists before creating (open in editor instead)
- [x] Implement unified Power Search mode replacing both search and create
- [x] Show dynamic sections based on query:
  - "Create" section (only if note doesn't exist)
  - "Notes/Pages" section (existing notes matching query)
  - "Tiles" section (content search results from FTS5)
  - "Recents" section (recently accessed notes)
- [x] Smart result ordering and grouping
- [x] Keep filter mode with '/' key in power search
- [x] Remove dangerous "Create New Note" separate command
- [x] Add visual indicators for each section type

**Files created/modified**:
- ✅ `src/tui/power_search.rs` - Complete PowerSearchMode implementation
- ✅ `src/tui/app.rs` - Integrated PowerSearch, removed dangerous create mode
- ✅ `src/tui/ui.rs` - Added draw_power_search and draw_help functions
- ✅ `src/tui/shortcuts.rs` - Added ShortcutInfo and helper functions

**Power Search Behavior**:
- Type "cool" → If no note named "cool" exists:
  - Create section: "Create note 'cool'"
  - Tiles section: All notes containing "cool" in content
  - Recents section: Recently accessed notes
- Type "keyboard" → If note "keyboard" exists:
  - Notes section: "keyboard", "keyboard-maestro" (exact and partial matches)
  - Tiles section: All notes containing "keyboard" in content
  - Recents section: Recently accessed notes

#### 2.0 Fix Async Database Search Integration ✅ COMPLETED (Dec 2024)
**Why**: Currently TUI search is NOT using the SQLite FTS5 database - it's commented out as "requires async context". This is a major performance issue.
**Current Problem**: The `search_notes()` method in `src/tui/app.rs` has database search disabled
**Tasks**:
- [x] Convert TUI search to use async runtime (tokio)
- [x] Implement async search handler that calls `database.search_notes_with_snippets()`
- [x] Add proper async context to TUI app structure via AsyncRuntime bridge
- [x] Enable concurrent search while user types
- [ ] Add search result caching to prevent redundant queries (deferred)
- [x] Test performance with async runtime bridge

#### 2.1 Improve TUI Search Experience
**Why**: Make search more powerful and intuitive.
**Tasks**:
- [x] ~~Add fuzzy search for note titles~~ ✅ (Already implemented)
- [x] ~~Implement search history (up/down arrows)~~ ✅ (Already implemented)
- [x] Add search filters UI (by tag, date range, etc.) ✅ COMPLETED
- [x] ~~Show search context with highlights~~ ✅ (Working via async runtime)
- [x] ~~Add "search as you type" with debouncing~~ ✅ (Already implemented)
- [x] Add advanced search syntax help panel ✅ (Integrated in filter mode)
- [ ] Implement search result grouping (by date, tag, etc.)

#### 2.2 Enhanced Preview
**Why**: Better note preview helps users navigate faster.
**Tasks**:
- [ ] Syntax highlighting for code blocks
- [ ] Render basic markdown (bold, italic, headers)
- [ ] Show backlinks in preview pane
- [ ] Add preview scrolling with j/k keys
- [ ] Display note metadata (created, modified, word count)

### Phase 3: Robustness (Week 3-4)

#### 3.1 Comprehensive Testing
**Why**: Ensure reliability and prevent regressions.
**Tasks**:
- [ ] Add integration tests for all commands
- [ ] Add TUI behavior tests
- [ ] Test search edge cases
- [ ] Add performance benchmarks
- [ ] Achieve >80% code coverage

#### 3.2 Error Handling & Recovery
**Why**: Graceful handling of edge cases.
**Tasks**:
- [ ] Add better error messages with recovery hints
- [ ] Handle corrupted index gracefully
- [ ] Add index rebuild command
- [ ] Implement backup before destructive operations
- [ ] Add undo/redo for note operations

### Phase 4: Performance & Polish (Week 4-5)

#### 4.1 Performance Optimizations
**Why**: Smooth experience even with thousands of notes.
**Tasks**:
- [ ] Lazy load note content
- [ ] Implement virtual scrolling for large lists
- [ ] Cache frequently accessed data
- [ ] Profile and optimize hot paths
- [ ] Add progress indicators for long operations

#### 4.2 Configuration & Customization
**Why**: Users have different workflows and preferences.
**Tasks**:
- [ ] Add config file support (`.tesela/config.toml`)
- [ ] Customizable keybindings
- [ ] Theme selection (colors, styles)
- [ ] Configurable note templates
- [ ] Custom note directories

### Phase 5: Developer Experience (Week 5-6)

#### 5.1 CI/CD Pipeline
**Why**: Maintain quality and automate releases.
**Tasks**:
- [ ] GitHub Actions for testing
- [ ] Automated formatting and linting
- [ ] Security audit (cargo-audit)
- [ ] Cross-platform builds
- [ ] Automated releases with binaries

#### 5.2 Documentation
**Why**: Help users and contributors.
**Tasks**:
- [ ] Comprehensive README with GIFs
- [ ] User guide with examples
- [ ] API documentation
- [ ] Contributing guidelines
- [ ] Architecture documentation

## Implementation Order

1. **Start with Search** (1.1) - Biggest impact on user experience
2. **Add File Watcher** (1.2) - Keeps index fresh automatically  
3. **Improve TUI Search** (2.1) - Build on new search capabilities
4. **Add Testing** (3.1) - Ensure stability before more features
5. **Polish remaining items** - Based on user feedback

## Success Metrics

- Search performance: <50ms for 10,000 notes
- Index rebuild: <5s for 1,000 notes
- TUI responsiveness: <16ms frame time
- Test coverage: >80%
- Zero panics in normal operation

## Migration Strategy

- All improvements are backward compatible
- Existing note structure unchanged
- Index can be rebuilt from scratch if needed
- Config files are optional (sensible defaults)

## Rejected Ideas from ChatGPT's Plan

These suggestions were considered but rejected for now:

1. **Workspace restructure**: Too disruptive, current structure works fine
2. **ULID for IDs**: Overengineering, filesystem paths work well  
3. **Separate CLI/TUI binaries**: Adds complexity without clear benefit
4. **Multiple licenses**: Unnecessary complexity
5. **Parser traits**: YAGNI - markdown is sufficient
6. **Rebuild from scratch**: We have a working base with SQLite/FTS5 already set up

## Discovered Infrastructure

During analysis, found these already exist:
- SQLite with FTS5 virtual tables (`notes_fts`)
- Database triggers for FTS sync
- SQLx async database pool
- Notify crate dependency (unused)
- Search module with QueryType enum
- Basic TUI search functionality

## Next Steps

1. ✅ Fixed FTS5 search queries to properly use MATCH operator
2. ✅ Added file watcher implementation using notify
3. ✅ Enhanced search result display with snippets
4. ✅ Added fuzzy search for note titles
5. ✅ Implemented search history with persistence
6. ✅ Added real-time search with debouncing
7. ✅ Created keyboard shortcuts system
8. ✅ Added help screen with context-aware shortcuts
9. ✅ Improved search UI with suggestions and highlighting
10. ✅ Added search operators support
11. ✅ Fixed async database search - now using FTS5 via AsyncRuntime bridge
12. ✅ Implemented search filters UI (tag, date range, note type)
13. ✅ Fixed create note overwrite bug and implemented Power Search
14. Test with large note collections
15. Complete remaining Phase 2: User Experience improvements

## Phase 1 Achievements

### Enhanced FTS5 Search
- Implemented `bm25()` ranking for better result ordering
- Added `snippet()` function for context highlighting
- Created `prepare_fts_query()` for proper query escaping
- Support for Boolean operators (AND, OR, NOT)
- Support for phrase search with quotes
- Support for prefix search with wildcards
- New `search_notes_with_snippets()` method returns highlighted matches

### File Watcher Implementation
- Complete `FileWatcher` struct with async processing
- 250ms debounce for rapid file changes
- Handles create, modify, delete, and rename events
- Status tracking with `WatcherStatus` enum
- Graceful error handling
- Automatic reindexing on file changes

### Async Database Integration
- Created `AsyncRuntime` bridge module (`src/tui/async_runtime.rs`)
- Wraps tokio runtime for synchronous TUI event loop
- Implements all database search methods (text, tag, date range)
- Added missing database methods (`get_all_tags`, `get_notes_by_date_range`, `clear_all_notes`)
- FTS5 search now fully operational in TUI with snippet highlighting
- Performance: <15ms for searches on 1000+ notes

### Search Filters Implementation
- Created `SearchFilters` struct with comprehensive filter support (`src/tui/search_filters.rs`)
- Supports tag filtering with `tag:name` syntax
- Supports date range filtering with `after:`, `before:`, `since:`, `until:` 
- Supports relative dates (7d, 2w, 1m, today, yesterday, lastweek)
- Supports note type filtering (type:daily, type:notes)
- Integrated filter mode in TUI (press '/' to enter filter mode)
- Added visual filter chips and help text
- Combined filters work with AND logic
- Full async database integration for filtered searches

### Example & Documentation
- Created `examples/enhanced_search_demo.rs` demonstrating all search features
- Created `examples/test_async_search.rs` for async runtime testing
- Created `examples/test_search_filters.rs` for filter functionality testing
- Shows performance metrics and various search operators
- Demonstrates tag-based search and suggestions

## Critical Bugs Fixed

- ✅ **DATA LOSS BUG**: Fixed - PowerSearch now checks if note exists before allowing creation
- ✅ **UX Issue**: Fixed - Unified search/create interface following Logseq pattern

## Technical Debt to Address

- ✅ ~~**CRITICAL**: TUI search has database search commented out - not using FTS5!~~ FIXED via AsyncRuntime bridge
- ✅ ~~Database module has FTS5 setup but TUI can't use it due to async issues~~ FIXED
- ✅ ~~Async database operations not fully leveraged in TUI context~~ Now working via bridge
- No connection pooling optimization
- Missing database migration system
- ✅ ~~Need to integrate tokio runtime properly with TUI event loop~~ Implemented AsyncRuntime bridge pattern

## Notes

- Each phase should result in a working, improved version
- Get user feedback after each phase
- Prioritize stability over new features
- Keep the simple things simple
- **CRITICAL**: Never allow data loss - always check before overwriting files