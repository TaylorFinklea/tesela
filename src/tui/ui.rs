//! UI rendering module for the TUI

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem as RatatuiListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{App, BacklinkItem, InputMode, InputType, ListType, ListingMode, Mode, ViewMode};
use super::power_search::PowerSearchMode;
use std::path::Path;

/// Main draw function that renders the entire UI
pub fn draw(app: &mut App, frame: &mut Frame) {
    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(frame.area());

    // Draw header
    draw_header(frame, chunks[0]);

    // Draw content based on mode
    match &app.mode {
        Mode::MainMenu => draw_main_menu(app, frame, chunks[1]),
        Mode::Input(input_mode) => draw_input(input_mode, frame, chunks[1]),
        Mode::Listing(listing_mode) => draw_listing(listing_mode, frame, chunks[1]),
        Mode::PowerSearch(power_search) => draw_power_search(power_search, frame, chunks[1]),
        Mode::Message(message, _) => draw_message(message, frame, chunks[1]),
        Mode::Help(help_mode) => draw_help(help_mode, frame, chunks[1]),
    }

    // Draw footer
    draw_footer(&app.mode, frame, chunks[2]);

    // Draw error popup if there's a recent error
    if let Mode::Message(msg, _) = &app.mode {
        if msg.starts_with("❌") {
            draw_popup(msg, frame);
        }
    }
}

/// Draw the header bar
fn draw_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled("🗿 ", Style::default().fg(Color::Cyan)),
        Span::styled(
            "Tesela",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" - "),
        Span::styled("Interactive Note Manager", Style::default().fg(Color::Gray)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    )
    .alignment(Alignment::Center);

    frame.render_widget(header, area);
}

/// Draw the footer with context-sensitive help
fn draw_footer(mode: &Mode, frame: &mut Frame, area: Rect) {
    let help_text = match mode {
        Mode::MainMenu => "S: Search/Create | E: Edit | L: List | D: Daily | Q: Quit",
        Mode::Input(_) => "Tab: Complete | Enter: Confirm | Esc: Cancel",
        Mode::Listing(_) => {
            "↑↓/jk: Navigate | Enter: Open | G: Toggle Graph | PgUp/PgDn: Scroll | Esc: Back"
        }
        Mode::PowerSearch(_) => {
            "Tab: Switch sections | ↑↓/jk: Navigate | Enter: Select | /: Filters | Esc: Cancel"
        }
        Mode::Message(_, _) => "Press any key to continue...",
        Mode::Help(_) => "↑↓: Scroll | Esc/?: Close Help",
    };

    let footer = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);

    frame.render_widget(footer, area);
}

/// Draw the main menu
fn draw_main_menu(_app: &App, frame: &mut Frame, area: Rect) {
    // Check mosaic status
    let mosaic_status = if Path::new("tesela.toml").exists() {
        Line::from(vec![
            Span::raw("📚 "),
            Span::styled("Mosaic Ready", Style::default().fg(Color::Green)),
        ])
    } else {
        Line::from(vec![
            Span::raw("⚠️  "),
            Span::styled(
                "No Mosaic Found (run: tesela init)",
                Style::default().fg(Color::Yellow),
            ),
        ])
    };

    // Create menu content
    let menu_items = vec![
        Line::from(""),
        mosaic_status,
        Line::from(""),
        Line::from(vec![Span::styled(
            "Quick Commands",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [S]", Style::default().fg(Color::Cyan)),
            Span::raw("  🔍 Search/Create notes (Power Search)"),
        ]),
        Line::from(vec![
            Span::styled("  [E]", Style::default().fg(Color::Cyan)),
            Span::raw("  ✏️ Edit existing note"),
        ]),
        Line::from(vec![
            Span::styled("  [L]", Style::default().fg(Color::Cyan)),
            Span::raw("  📚 List all notes"),
        ]),
        Line::from(vec![
            Span::styled("  [D]", Style::default().fg(Color::Cyan)),
            Span::raw("  📅 Open daily note"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Q]", Style::default().fg(Color::Red)),
            Span::raw("  🚪 Quit"),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Features",
            Style::default().add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("  • Outliner format with block inheritance"),
        Line::from("  • Cross-directory support (notes/ & dailies/)"),
        Line::from("  • Smart autocomplete with Tab cycling"),
        Line::from("  • Full-text search with context"),
        Line::from("  • Vim integration for editing"),
    ];

    let menu = Paragraph::new(menu_items)
        .block(
            Block::default()
                .title(" Main Menu ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(menu, area);
}

/// Draw input mode with suggestions
fn draw_input(input_mode: &InputMode, frame: &mut Frame, area: Rect) {
    // Split area for input and suggestions
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input field
            Constraint::Min(0),    // Suggestions
        ])
        .split(area);

    // Draw input field
    let input_widget = Paragraph::new(format!("{} {}", input_mode.prompt, input_mode.input))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(input_widget, chunks[0]);

    // Set cursor position
    frame.set_cursor_position((
        chunks[0].x + input_mode.prompt.len() as u16 + 1 + input_mode.cursor_position as u16,
        chunks[0].y + 1,
    ));

    // Draw suggestions if available
    if !input_mode.suggestions.is_empty() {
        let suggestion_title = match input_mode.input_type {
            InputType::NewNote => " Power Search (redirects) ",
            InputType::EditNote => " Available Notes ",
            InputType::SearchQuery => " Search Suggestions ",
        };

        let items: Vec<RatatuiListItem> = input_mode
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, suggestion)| {
                let style = if Some(i) == input_mode.suggestion_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                };
                RatatuiListItem::new(format!("  {}", suggestion)).style(style)
            })
            .collect();

        let suggestions_list = List::new(items)
            .block(
                Block::default()
                    .title(suggestion_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(suggestions_list, chunks[1]);
    }
}

/// Draw power search mode with sections
fn draw_power_search(power_search: &PowerSearchMode, frame: &mut Frame, area: Rect) {
    // Split area into input, filters, and sections
    let has_filters = power_search.filters.is_active || power_search.filter_mode;

    let mut constraints = vec![Constraint::Length(3)]; // Search input

    if has_filters {
        constraints.push(Constraint::Length(3)); // Filter display
    }

    constraints.push(Constraint::Min(0)); // Sections

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut current_chunk = 0;

    // Draw search/filter input box
    let input_text = if power_search.filter_mode {
        vec![
            Span::styled("🏷️  Filters: ", Style::default().fg(Color::Magenta)),
            Span::raw(&power_search.filters.filter_string),
            Span::styled(
                "▊",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    } else {
        vec![
            Span::styled("", Style::default().fg(Color::Cyan)),
            Span::raw(&power_search.query),
            Span::styled(
                "▊",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    };

    let title = if power_search.filter_mode {
        " Filter Mode (ESC to exit, Enter to apply) "
    } else {
        " Power Search (Tab: sections, /: filters, ?: help) "
    };

    let input = Paragraph::new(Line::from(input_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if power_search.filter_mode {
                Color::Magenta
            } else {
                Color::Cyan
            }))
            .title(title),
    );

    frame.render_widget(input, chunks[current_chunk]);
    current_chunk += 1;

    // Draw active filters if any
    if has_filters {
        let filter_display = if power_search.filters.is_active {
            format!("📌 Active Filters: {}", power_search.filters.description())
        } else if power_search.filter_mode {
            "💡 Filter syntax: tag:name after:date before:date type:daily|notes since:7d"
                .to_string()
        } else {
            "No active filters".to_string()
        };

        let filters = Paragraph::new(filter_display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray));

        frame.render_widget(filters, chunks[current_chunk]);
        current_chunk += 1;
    }

    // Draw sections (Create, Notes, Tiles, Recents)
    let sections_chunk = chunks[current_chunk];

    if power_search.sections.is_empty() {
        let no_results = if power_search.query.is_empty() {
            "Start typing to search..."
        } else if power_search.is_searching {
            "Searching..."
        } else {
            "No matches found"
        };

        let empty = Paragraph::new(no_results)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(empty, sections_chunk);
    } else {
        // Calculate section heights based on content
        let mut section_constraints = Vec::new();
        for section in &power_search.sections {
            // Tiles section needs more height for snippets
            let height = match section.section_type {
                crate::tui::power_search::SectionType::Tiles => {
                    // Each tile item takes about 3 lines with spacing, plus 2 for borders
                    ((section.items.len() * 3) + 2).min(20) as u16
                }
                _ => {
                    // Other sections: 1 line per item, plus 2 for borders, max 10
                    (section.items.len() + 2).min(10) as u16
                }
            };
            section_constraints.push(Constraint::Length(height));
        }

        // Add remaining space for scrolling if needed
        if section_constraints.len() < power_search.sections.len() {
            section_constraints.push(Constraint::Min(0));
        }

        let section_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(section_constraints)
            .split(sections_chunk);

        // Render each section
        for (i, (section, chunk)) in power_search
            .sections
            .iter()
            .zip(section_chunks.iter())
            .enumerate()
        {
            let is_selected_section = i == power_search.selected_section;

            // Create list items for this section
            let items: Vec<RatatuiListItem> = section
                .items
                .iter()
                .enumerate()
                .map(|(j, item)| {
                    let is_selected = is_selected_section && j == power_search.selected_item;

                    // Format the item based on section type
                    let display = match section.section_type {
                        crate::tui::power_search::SectionType::Create => item.title.clone(),
                        crate::tui::power_search::SectionType::Notes => {
                            format!("📝 {} • {}", item.title, item.metadata)
                        }
                        crate::tui::power_search::SectionType::Tiles => {
                            if let Some(ref snippet) = item.snippet {
                                // Show only the line containing the match
                                let clean_snippet = snippet
                                    .replace("<mark>", "【")
                                    .replace("</mark>", "】")
                                    .lines()
                                    .find(|line| line.contains("【"))
                                    .unwrap_or("")
                                    .trim()
                                    .chars()
                                    .take(100)
                                    .collect::<String>();
                                format!("📄 {}\n    {}", item.title, clean_snippet)
                            } else {
                                format!("📄 {}", item.title)
                            }
                        }
                        crate::tui::power_search::SectionType::Recents => {
                            format!("⏱️  {} • {}", item.title, item.metadata)
                        }
                    };

                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    RatatuiListItem::new(display).style(style)
                })
                .collect();

            let border_style = if is_selected_section {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let section_list = List::new(items).block(
                Block::default()
                    .title(format!(" {} ", section.title))
                    .borders(Borders::ALL)
                    .border_style(border_style),
            );

            frame.render_widget(section_list, *chunk);
        }
    }
}

/// Draw help mode with keyboard shortcuts
fn draw_help(help_mode: &crate::tui::app::HelpMode, frame: &mut Frame, area: Rect) {
    use crate::tui::shortcuts;

    // Get shortcuts for the current context
    let shortcuts = shortcuts::get_shortcuts_for_context(help_mode.context.clone());

    // Create help text lines
    let mut lines = vec![
        Line::from(vec![Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
    ];

    // Group shortcuts by category if available
    for shortcut in shortcuts {
        let key_span = Span::styled(
            format!("{:<15}", shortcut.keys),
            Style::default().fg(Color::Yellow),
        );
        let desc_span = Span::raw(shortcut.description.clone());
        lines.push(Line::from(vec![key_span, desc_span]));
    }

    // Add global shortcuts at the bottom
    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "Global Shortcuts",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<15}", "?"), Style::default().fg(Color::Yellow)),
        Span::raw("Toggle this help"),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!("{:<15}", "Esc"), Style::default().fg(Color::Yellow)),
        Span::raw("Close help / Go back"),
    ]));

    // Create scrollable paragraph
    let help_text = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((help_mode.scroll_offset, 0))
        .wrap(Wrap { trim: false });

    frame.render_widget(help_text, area);
}

/// Draw search mode with live results
// // Legacy draw_search function removed - using draw_power_search instead
// fn draw_search_legacy(search_mode: &(), frame: &mut Frame, area: Rect) {
//     // Split area into input and results
//     let chunks = Layout::default()
//         .direction(Direction::Vertical)
//         .constraints([
//             Constraint::Length(3), // Search input
//             Constraint::Min(0),    // Results
//         ])
//         .split(area);
//
//     // Draw search input box
//     let input_text = vec![
//         Span::styled("🔍 Search: ", Style::default().fg(Color::Cyan)),
//         Span::raw(&search_mode.query),
//         Span::styled(
//             "▊",
//             Style::default()
//                 .fg(Color::Yellow)
//                 .add_modifier(Modifier::BOLD),
//         ),
//     ];
//
//     let input = Paragraph::new(Line::from(input_text)).block(
//         Block::default()
//             .borders(Borders::ALL)
//             .border_style(Style::default().fg(Color::Cyan))
//             .title(" Live Search (type to filter) "),
//     );
//
//     frame.render_widget(input, chunks[0]);
//
//     // Draw search results
//     if search_mode.results.is_empty() {
//         let no_results = if search_mode.query.is_empty() {
//             "Start typing to search..."
//         } else {
//             "No matches found"
//         };
//
//         let empty_msg = Paragraph::new(no_results)
//             .block(
//                 Block::default()
//                     .borders(Borders::ALL)
//                     .border_style(Style::default().fg(Color::DarkGray)),
//             )
//             .style(Style::default().fg(Color::Gray))
//             .alignment(Alignment::Center);
//
//         frame.render_widget(empty_msg, chunks[1]);
//     } else {
//         // Create list items with context
//         let items: Vec<RatatuiListItem> = search_mode
//             .results
//             .iter()
//             .enumerate()
//             .map(|(i, item)| {
//                 let is_selected = i == search_mode.selected_result;
//
//                 // Build the content lines
//                 let mut lines = vec![];
//
//                 // First line: title and metadata
//                 let main_line = format!("🔍 {} • {}", item.title, item.metadata);
//                 lines.push(Line::from(main_line));
//
//                 // Second line: context with highlighted matches
//                 if let Some(ref context) = item.context {
//                     let mut context_spans = vec![Span::styled(
//                         "    └─ ",
//                         Style::default().fg(Color::DarkGray),
//                     )];
//
//                     // If we have match indices, highlight them
//                     if !item.match_indices.is_empty() {
//                         let mut last_end = 0;
//                         for (start, end) in &item.match_indices {
//                             // Add non-matched text before this match
//                             if last_end < *start {
//                                 context_spans.push(Span::styled(
//                                     &context[last_end..*start],
//                                     Style::default()
//                                         .fg(Color::Gray)
//                                         .add_modifier(Modifier::ITALIC),
//                                 ));
//                             }
//                             // Add matched text with highlighting
//                             if *start < context.len() && *end <= context.len() {
//                                 context_spans.push(Span::styled(
//                                     &context[*start..*end],
//                                     Style::default()
//                                         .fg(Color::Yellow)
//                                         .add_modifier(Modifier::BOLD | Modifier::ITALIC),
//                                 ));
//                             }
//                             last_end = *end;
//                         }
//                         // Add any remaining non-matched text
//                         if last_end < context.len() {
//                             context_spans.push(Span::styled(
//                                 &context[last_end..],
//                                 Style::default()
//                                     .fg(Color::Gray)
//                                     .add_modifier(Modifier::ITALIC),
//                             ));
//                         }
//                     } else {
//                         // No match indices, show plain context
//                         context_spans.push(Span::styled(
//                             context,
//                             Style::default()
//                                 .fg(Color::Gray)
//                                 .add_modifier(Modifier::ITALIC),
//                         ));
//                     }
//
//                     lines.push(Line::from(context_spans));
//                 }
//
//                 let style = if is_selected {
//                     Style::default()
//                         .bg(Color::DarkGray)
//                         .fg(Color::White)
//                         .add_modifier(Modifier::BOLD)
//                 } else {
//                     Style::default().fg(Color::White)
//                 };
//
//                 RatatuiListItem::new(lines).style(style)
//             })
//             .collect();
//
//         let results_title = format!(
//             " {} result{} for '{}' ",
//             search_mode.results.len(),
//             if search_mode.results.len() == 1 {
//                 ""
//             } else {
//                 "s"
//             },
//             search_mode.query
//         );
//
//         let list = List::new(items)
//             .block(
//                 Block::default()
//                     .title(results_title)
//                     .borders(Borders::ALL)
//                     .border_style(Style::default().fg(Color::DarkGray)),
//             )
//             .highlight_style(
//                 Style::default()
//                     .bg(Color::DarkGray)
//                     .add_modifier(Modifier::BOLD),
//             );
//
//         frame.render_widget(list, chunks[1]);
//     }
// }

/// Draw listing mode
fn draw_listing(listing_mode: &ListingMode, frame: &mut Frame, area: Rect) {
    if listing_mode.items.is_empty() {
        let empty_msg = Paragraph::new("No items to display")
            .block(
                Block::default()
                    .title(format!(" {} ", listing_mode.title))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(empty_msg, area);
        return;
    }

    // Split area into list and preview/graph panes
    let should_split = listing_mode.preview_content.is_some() || !listing_mode.backlinks.is_empty();
    let (list_area, right_pane_area) = if should_split {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40), // List takes 40% width
                Constraint::Percentage(60), // Preview/Graph takes 60% width
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    // Create list items
    let items: Vec<RatatuiListItem> = listing_mode
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == listing_mode.selected;

            let prefix = match listing_mode.list_type {
                ListType::Notes => "📄",
                ListType::SearchResults => "🔍",
            };

            // Build the content lines
            let mut lines = vec![];

            // First line: title and metadata
            let main_line = if item.metadata.is_empty() {
                format!("{} {}", prefix, item.title)
            } else {
                format!("{} {} • {}", prefix, item.title, item.metadata)
            };
            lines.push(Line::from(main_line));

            // Second line: context (for search results)
            if let Some(ref context) = item.context {
                // Build spans with highlighted matches
                let mut context_spans = vec![Span::styled(
                    "    └─ ",
                    Style::default().fg(Color::DarkGray),
                )];

                // If we have match indices, highlight them
                if !item.match_indices.is_empty() {
                    let mut last_end = 0;
                    for (start, end) in &item.match_indices {
                        // Add non-matched text before this match
                        if last_end < *start {
                            context_spans.push(Span::styled(
                                &context[last_end..*start],
                                Style::default()
                                    .fg(Color::Gray)
                                    .add_modifier(Modifier::ITALIC),
                            ));
                        }
                        // Add matched text with highlighting
                        if *start < context.len() && *end <= context.len() {
                            context_spans.push(Span::styled(
                                &context[*start..*end],
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
                            ));
                        }
                        last_end = *end;
                    }
                    // Add any remaining non-matched text
                    if last_end < context.len() {
                        context_spans.push(Span::styled(
                            &context[last_end..],
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                } else {
                    // No match indices, show plain context
                    context_spans.push(Span::styled(
                        context,
                        Style::default()
                            .fg(Color::Gray)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }

                lines.push(Line::from(context_spans));
            }

            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            RatatuiListItem::new(lines).style(style)
        })
        .collect();

    // Add view mode indicator to title
    let list_title = if listing_mode.view_mode == ViewMode::Graph {
        format!(" {} [Graph View] ", listing_mode.title)
    } else {
        format!(" {} ", listing_mode.title)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, list_area);

    // Draw right pane based on view mode
    if let Some(right_area) = right_pane_area {
        match listing_mode.view_mode {
            ViewMode::Graph => {
                // Draw backlinks/graph view
                draw_graph_pane(
                    &listing_mode.backlinks,
                    listing_mode.selected_backlink,
                    frame,
                    right_area,
                );
            }
            ViewMode::Preview => {
                // Draw preview pane if content is available
                if let Some(preview_content) = &listing_mode.preview_content {
                    let selected_item = &listing_mode.items[listing_mode.selected];

                    // Format the preview content with syntax highlighting if possible
                    let preview_lines: Vec<Line> = preview_content
                        .lines()
                        .map(|line| {
                            // Simple markdown syntax highlighting
                            if line.starts_with("# ") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                )])
                            } else if line.starts_with("## ") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default()
                                        .fg(Color::Blue)
                                        .add_modifier(Modifier::BOLD),
                                )])
                            } else if line.starts_with("### ") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default()
                                        .fg(Color::Green)
                                        .add_modifier(Modifier::BOLD),
                                )])
                            } else if line.starts_with("- ")
                                || line.starts_with("* ")
                                || line.starts_with("+ ")
                            {
                                Line::from(vec![
                                    Span::styled(&line[..2], Style::default().fg(Color::Yellow)),
                                    Span::raw(&line[2..]),
                                ])
                            } else if line.starts_with("> ") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default()
                                        .fg(Color::Gray)
                                        .add_modifier(Modifier::ITALIC),
                                )])
                            } else if line.contains("```") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default().fg(Color::Magenta),
                                )])
                            } else if line.starts_with("[[") && line.ends_with("]]") {
                                Line::from(vec![Span::styled(
                                    line,
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::UNDERLINED),
                                )])
                            } else {
                                Line::raw(line)
                            }
                        })
                        .collect();

                    let preview = Paragraph::new(preview_lines)
                        .block(
                            Block::default()
                                .title(format!(
                                    " Preview: {} [PgUp/PgDn to scroll] ",
                                    selected_item.title
                                ))
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(Color::DarkGray)),
                        )
                        .wrap(Wrap { trim: false })
                        .scroll((listing_mode.preview_scroll, 0));

                    frame.render_widget(preview, right_area);
                }
            }
        }
    }
}

/// Draw a message
fn draw_message(message: &str, frame: &mut Frame, area: Rect) {
    let color = if message.starts_with("✅") {
        Color::Green
    } else if message.starts_with("❌") {
        Color::Red
    } else {
        Color::Yellow
    };

    let msg = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color)),
        )
        .style(Style::default().fg(color))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(msg, area);
}

/// Draw graph pane showing backlinks
fn draw_graph_pane(
    backlinks: &[BacklinkItem],
    selected_index: usize,
    frame: &mut Frame,
    area: Rect,
) {
    if backlinks.is_empty() {
        let empty_msg = Paragraph::new("No backlinks found for this note")
            .block(
                Block::default()
                    .title(" 🔗 Backlinks ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(empty_msg, area);
        return;
    }

    // Create list items for backlinks display
    let items: Vec<RatatuiListItem> = backlinks
        .iter()
        .enumerate()
        .map(|(idx, backlink)| {
            let is_selected = idx == selected_index;

            // Format the backlink display
            let mut content = vec![format!("📄 {}", backlink.source_title)];

            // Add the context line (just the line with the link)
            if !backlink.context.is_empty() {
                content.push(format!("    {}", backlink.context));
            }

            // Add the file path
            content.push(format!("    📁 {}", backlink.source_path));

            let display = content.join("\n");

            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            RatatuiListItem::new(display).style(style)
        })
        .collect();

    let backlinks_list = List::new(items)
        .block(
            Block::default()
                .title(format!(" 🔗 {} Backlinks ", backlinks.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default())
        .highlight_symbol("▶ ");

    frame.render_widget(backlinks_list, area);
}

/// Draw a popup message
fn draw_popup(message: &str, frame: &mut Frame) {
    let popup_area = centered_rect(60, 20, frame.area());

    // Clear the background
    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(message)
        .block(
            Block::default()
                .title(" Error ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(popup, popup_area);
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Setup terminal for TUI
pub fn setup_terminal(
) -> Result<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, anyhow::Error> {
    use crossterm::{
        execute,
        terminal::{enable_raw_mode, EnterAlternateScreen},
    };
    use std::io;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let terminal = ratatui::Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal after TUI
pub fn restore_terminal(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
) -> Result<(), anyhow::Error> {
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, LeaveAlternateScreen},
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
