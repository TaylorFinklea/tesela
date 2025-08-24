//! UI rendering module for the TUI

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem as RatatuiListItem, Paragraph, Wrap},
    Frame,
};

use super::app::{
    App, BacklinkItem, InputMode, InputType, ListType, ListingMode, Mode, SearchMode, ViewMode,
};
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
        Mode::Search(search_mode) => draw_search(search_mode, frame, chunks[1]),
        Mode::Message(message, _) => draw_message(message, frame, chunks[1]),
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
        Mode::MainMenu => "N: New | E: Edit | S: Search | L: List | D: Daily | Q: Quit",
        Mode::Input(_) => "Tab: Complete | Enter: Confirm | Esc: Cancel",
        Mode::Listing(_) => {
            "↑↓/jk: Navigate | Enter: Open | G: Toggle Graph | PgUp/PgDn: Scroll | Esc: Back"
        }
        Mode::Search(_) => "Type to search | ↑↓/jk: Navigate | Enter: Open | Esc: Cancel",
        Mode::Message(_, _) => "Press any key to continue...",
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
            Span::styled("  [N]", Style::default().fg(Color::Cyan)),
            Span::raw("  📝 Create new note"),
        ]),
        Line::from(vec![
            Span::styled("  [E]", Style::default().fg(Color::Cyan)),
            Span::raw("  ✏️ Edit existing note"),
        ]),
        Line::from(vec![
            Span::styled("  [S]", Style::default().fg(Color::Cyan)),
            Span::raw("  🔍 Search notes"),
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
            InputType::NewNote => " Similar Notes ",
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

/// Draw search mode with live results
fn draw_search(search_mode: &SearchMode, frame: &mut Frame, area: Rect) {
    // Split area into input and results
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(0),    // Results
        ])
        .split(area);

    // Draw search input box
    let input_text = vec![
        Span::styled("🔍 Search: ", Style::default().fg(Color::Cyan)),
        Span::raw(&search_mode.query),
        Span::styled(
            "▊",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    let input = Paragraph::new(Line::from(input_text)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Live Search (type to filter) "),
    );

    frame.render_widget(input, chunks[0]);

    // Draw search results
    if search_mode.results.is_empty() {
        let no_results = if search_mode.query.is_empty() {
            "Start typing to search..."
        } else {
            "No matches found"
        };

        let empty_msg = Paragraph::new(no_results)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

        frame.render_widget(empty_msg, chunks[1]);
    } else {
        // Create list items with context
        let items: Vec<RatatuiListItem> = search_mode
            .results
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == search_mode.selected_result;

                // Build the content lines
                let mut lines = vec![];

                // First line: title and metadata
                let main_line = format!("🔍 {} • {}", item.title, item.metadata);
                lines.push(Line::from(main_line));

                // Second line: context with highlighted matches
                if let Some(ref context) = item.context {
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

        let results_title = format!(
            " {} result{} for '{}' ",
            search_mode.results.len(),
            if search_mode.results.len() == 1 {
                ""
            } else {
                "s"
            },
            search_mode.query
        );

        let list = List::new(items)
            .block(
                Block::default()
                    .title(results_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(list, chunks[1]);
    }
}

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
                draw_graph_pane(&listing_mode.backlinks, frame, right_area);
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
fn draw_graph_pane(backlinks: &[BacklinkItem], frame: &mut Frame, area: Rect) {
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

    // Create content for backlinks display
    let mut lines = vec![];

    for (idx, backlink) in backlinks.iter().enumerate() {
        if idx > 0 {
            lines.push(Line::from("")); // Add spacing between backlinks
        }

        // Source note title
        lines.push(Line::from(vec![
            Span::styled("📄 ", Style::default().fg(Color::Cyan)),
            Span::styled(
                &backlink.source_title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" (line {})", backlink.line_number),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Context with link highlighted
        for context_line in backlink.context.lines() {
            if context_line.starts_with("→") {
                // This is the main line with the link
                lines.push(Line::from(vec![Span::styled(
                    context_line,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::ITALIC),
                )]));
            } else {
                // Context lines
                lines.push(Line::from(vec![Span::styled(
                    context_line,
                    Style::default().fg(Color::Gray),
                )]));
            }
        }

        // Source file path
        lines.push(Line::from(vec![
            Span::styled("   📁 ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &backlink.source_path,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    let backlinks_widget = Paragraph::new(lines)
        .block(
            Block::default()
                .title(format!(" 🔗 {} Backlinks ", backlinks.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(backlinks_widget, area);
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
