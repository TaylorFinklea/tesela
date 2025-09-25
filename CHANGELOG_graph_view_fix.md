# Graph View Fix - August 28, 2025

## Fixed Issues

### 1. Backlinks Not Selectable
**Problem:** In Graph View (when pressing 'G' in list mode), backlinks were displayed but not selectable/interactive
**Solution:** Added `selected_backlink` tracking to `ListingMode` and implemented navigation for backlinks
**Result:** Users can now navigate through backlinks with arrow keys and open them with Enter

### 2. Context Display Issues
**Problem:** Backlinks showed multiple lines of context which was unnecessary and cluttered
**Solution:** Modified `get_link_context` to return only the single line containing the link
**Result:** Clean, focused display showing just the relevant line with the backlink

### 3. Visual Feedback Missing
**Problem:** No visual indication of which backlink was selected
**Solution:** Converted graph view from Paragraph to List widget with selection highlighting
**Result:** Selected backlink is highlighted in yellow with a selection indicator

## Implementation Details

### Files Modified

#### `src/tui/app.rs`
- Added `selected_backlink: usize` field to `ListingMode` struct to track selection
- Modified `handle_listing` to handle Up/Down navigation differently in Graph mode vs Preview mode
- Added navigation logic for backlinks (Up/Down keys navigate through backlinks when in Graph mode)
- Added `open_note` method to open backlink source files in the editor
- Modified `get_link_context` to return only the single line containing the link
- Updated Enter key handling to open selected backlink when in Graph mode

#### `src/tui/ui.rs`
- Modified `draw_graph_pane` to accept `selected_index` parameter
- Converted backlinks display from `Paragraph` widget to `List` widget for selection support
- Added highlighting for selected backlink (yellow color with bold modifier)
- Simplified backlink display format to show:
  - Note title
  - Single line of context containing the link
  - File path

## User Experience Improvements

### Before
- Backlinks were displayed as static text
- No way to navigate or select backlinks
- Context showed multiple lines (before/after the link)
- No visual feedback for interaction

### After
- Backlinks are fully interactive
- Navigate with Up/Down arrows or j/k keys
- Press Enter to open the source file containing the backlink
- Clean single-line context display
- Selected backlink is highlighted in yellow
- Smooth navigation between Preview and Graph modes with 'G' key

## Navigation Summary

### Graph View Controls
- **G**: Toggle between Preview and Graph view
- **↑/k**: Select previous backlink
- **↓/j**: Select next backlink  
- **Enter**: Open the selected backlink's source file
- **Esc**: Return to main menu

## Technical Notes

### Selection State Management
- Each `ListingMode` instance now tracks both:
  - `selected`: Index of selected note in the main list
  - `selected_backlink`: Index of selected backlink in Graph view
- Selection state is preserved when toggling between Preview and Graph modes

### Widget Change
- Changed from `Paragraph` widget (static text) to `List` widget (interactive)
- List widget provides built-in selection highlighting and scrolling
- More efficient rendering for large numbers of backlinks

## Testing Recommendations

1. Open a note with backlinks (e.g., `@savanne` which has links from daily notes)
2. Press 'L' to list notes
3. Navigate to a note with backlinks
4. Press 'G' to switch to Graph View
5. Use arrow keys to navigate through backlinks
6. Press Enter to open a backlink source
7. Verify the editor opens with the correct file

## Future Enhancements

Potential improvements for future iterations:
- Add line number in the opened file when jumping to backlink
- Show preview of backlink source on selection
- Add filtering/search within backlinks
- Group backlinks by source directory (notes vs dailies)
- Add bidirectional link navigation