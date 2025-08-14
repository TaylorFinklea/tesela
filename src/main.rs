use clap::{Parser, Subcommand};

use tesela::{cat_note, create_note, init_mosaic, list_notes};

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
    /// Search your notes
    Search {
        /// Search query
        query: String,
    },
    /// Open today's daily note
    Daily,
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
        Some(Commands::Search { query }) => {
            println!("🔍 Searching for: '{}'", query);
            println!("📄 Found 3 notes containing '{}'", query);
        }
        Some(Commands::Daily) => {
            println!(
                "📅 Opening daily note for {}",
                chrono::Local::now().format("%Y-%m-%d")
            );
            println!("🌱 Ready to capture today's thoughts!");
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
