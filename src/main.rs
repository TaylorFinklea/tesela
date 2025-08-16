use clap::{Parser, Subcommand};

use tesela::{
    attach_file, backup_mosaic, benchmark_performance, cat_note, create_note, daily_note,
    export_note, generate_completions, import_notes, init_mosaic, interactive_mode, link_notes,
    list_notes, search_notes, show_graph,
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
    Init {
        /// Path where to create the mosaic
        #[arg(default_value = ".")]
        path: String,
    },
    /// Create a new note
    New {
        /// Title of the note
        title: String,
    },
    /// List recent notes
    List,
    /// Display a note's content
    Cat {
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
    Search {
        /// Search query
        query: String,
    },
    /// Create a link between two notes
    Link {
        /// Source note identifier
        from: String,
        /// Target note identifier
        to: String,
    },
    /// Show connections for a note
    Graph {
        /// Note identifier to show connections for
        note: String,
    },
    /// Open today's daily note
    Daily,
    /// Create a backup of the mosaic
    Backup,
    /// Import notes from external sources
    Import {
        /// Path to file or directory to import
        path: String,
    },
    /// Start interactive mode
    Interactive,
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
            if let Err(e) = daily_note() {
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
        Some(Commands::Interactive) => {
            if let Err(e) = interactive_mode() {
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
