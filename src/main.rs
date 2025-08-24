use clap::{Parser, Subcommand};

use tesela::{
    attach_file, autocomplete_suggestions, backup_mosaic, benchmark_performance, cat_note,
    create_note, daily_note_and_edit, export_note, generate_completions, import_notes, init_mosaic,
    link_notes, list_notes, open_note_in_editor, search_notes, show_graph,
};

/// Main CLI structure for Tesela.
///
/// This struct defines the top-level CLI interface using clap's derive macros.
/// It contains an optional subcommand - if no subcommand is provided, it shows
/// a welcome message with usage hints.
#[derive(Parser)]
#[command(name = "tesela")]
#[command(
    about = "A keyboard-first, file-based note-taking system for building lasting knowledge mosaics"
)]
#[command(version = "0.1.0")]
struct Cli {
    /// Open today's daily note in editor
    #[arg(short = 'd', long = "daily")]
    daily: bool,

    /// List recent notes
    #[arg(short = 'l', long = "list")]
    list: bool,

    /// Create a backup of the mosaic
    #[arg(short = 'b', long = "backup")]
    backup: bool,

    /// Create a new note
    #[arg(short = 'n', long = "new")]
    new: Option<String>,

    /// Search your notes
    #[arg(short = 's', long = "search")]
    search: Option<String>,

    /// Edit a note in external editor
    #[arg(short = 'e', long = "edit")]
    edit: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Available CLI subcommands for Tesela.
///
/// Each variant represents a different operation that can be performed.
/// The documentation comments (///) become the help text shown to users.
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new mosaic (knowledge base)
    #[command(alias = "m")]
    Init {
        /// Path where to create the mosaic
        #[arg(default_value = ".")]
        path: String,
    },
    /// Create a new note
    #[command(alias = "n")]
    New {
        /// Title of the note
        title: String,
    },
    /// List recent notes
    #[command(alias = "l")]
    List,
    /// Display a note's content
    Cat {
        /// Note identifier (filename or partial name)
        note: String,
    },
    /// Edit a note in external editor
    #[command(alias = "e")]
    Edit {
        /// Note identifier (filename or partial name)
        note: String,
    },
    /// Attach a file to a note
    Attach {
        /// Note identifier (filename or partial name)
        note: String,
        /// Path to the file to attach
        file: String,
    },
    /// Export a note to different formats
    Export {
        /// Note identifier (filename or partial name)
        note: String,
        /// Export format (html, markdown, txt)
        format: String,
    },
    /// Search your notes
    #[command(alias = "s")]
    Search {
        /// Search query
        query: String,
    },
    /// Create a link between two notes
    #[command(alias = "k")]
    Link {
        /// Source note identifier
        from: String,
        /// Target note identifier
        to: String,
    },
    /// Show connections for a note
    #[command(alias = "g")]
    Graph {
        /// Note identifier to show connections for
        note: String,
    },
    /// Open today's daily note in editor
    #[command(alias = "d")]
    Daily,
    /// Create a backup of the mosaic
    #[command(alias = "b")]
    Backup,
    /// Import notes from external sources
    #[command(alias = "i")]
    Import {
        /// Path to file or directory to import
        path: String,
    },
    /// Start TUI (Terminal User Interface) mode
    Tui,
    /// Get autocomplete suggestions
    Autocomplete {
        /// Partial text to autocomplete
        partial: String,
        /// Completion type (notes, search, all)
        #[arg(short = 't', long = "type", default_value = "notes")]
        completion_type: String,
    },
    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: String,
    },
    /// Run performance benchmarks
    Benchmark,
}

/// Main entry point for the Tesela CLI application.
///
/// Parses command-line arguments and dispatches to the appropriate command handler.
/// Error handling is done at this level - if a command returns an error, it's
/// displayed to stderr and the program exits with status code 1.
fn main() {
    let cli = Cli::parse();

    // Handle global flags first
    if cli.daily {
        if let Err(e) = daily_note_and_edit() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    if cli.list {
        if let Err(e) = list_notes() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    if cli.backup {
        if let Err(e) = backup_mosaic() {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Some(title) = cli.new {
        if let Err(e) = create_note(&title) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Some(query) = cli.search {
        if let Err(e) = search_notes(&query) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Some(note) = cli.edit {
        if let Err(e) = open_note_in_editor(&note) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }

    match &cli.command {
        Some(Commands::Init { path }) => {
            if let Err(e) = init_mosaic(path) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::New { title }) => {
            if let Err(e) = create_note(title) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::List) => {
            if let Err(e) = list_notes() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Cat { note }) => {
            if let Err(e) = cat_note(note) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Edit { note }) => {
            if let Err(e) = open_note_in_editor(note) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Attach { note, file }) => {
            if let Err(e) = attach_file(note, file) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Export { note, format }) => {
            if let Err(e) = export_note(note, format) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Search { query }) => {
            if let Err(e) = search_notes(query) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Link { from, to }) => {
            if let Err(e) = link_notes(from, to) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Graph { note }) => {
            if let Err(e) = show_graph(note) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Daily) => {
            if let Err(e) = daily_note_and_edit() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Backup) => {
            if let Err(e) = backup_mosaic() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Import { path }) => {
            if let Err(e) = import_notes(path) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Tui) => {
            if let Err(e) = tesela::tui::run() {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Autocomplete {
            partial,
            completion_type,
        }) => {
            if let Err(e) = autocomplete_suggestions(partial, completion_type) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Completions { shell }) => {
            if let Err(e) = generate_completions(shell) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Benchmark) => {
            if let Err(e) = benchmark_performance() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        None => {
            println!("🗿 Tesela - Build your knowledge mosaic");
            println!("📝 A keyboard-first note-taking system");
            println!("");
            println!("Run 'tesela --help' to see available commands");
            println!("Start with 'tesela init' to create your first mosaic");
        }
    }
}
