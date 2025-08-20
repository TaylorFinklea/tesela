use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn tesela_binary() -> Command {
    let cmd = Command::new(env!("CARGO_BIN_EXE_tesela"));
    cmd
}

#[test]
fn test_help_command() {
    let output = tesela_binary()
        .arg("--help")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("A keyboard-first, file-based note-taking system"));
    assert!(stdout.contains("Commands:"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("new"));
    assert!(stdout.contains("list"));
}

#[test]
fn test_version_command() {
    let output = tesela_binary()
        .arg("--version")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tesela 0.1.0"));
}

#[test]
fn test_no_command() {
    let output = tesela_binary().output().expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Tesela - Build your knowledge mosaic"));
    assert!(stdout.contains("Run 'tesela --help' to see available commands"));
}

#[test]
fn test_init_command() {
    let temp_dir = TempDir::new().unwrap();
    let mosaic_path = temp_dir.path().join("my-mosaic");

    let output = tesela_binary()
        .arg("init")
        .arg(&mosaic_path)
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Initializing Tesela mosaic"));
    assert!(stdout.contains("Your knowledge mosaic is ready!"));

    // Check that files were created
    assert!(mosaic_path.join("tesela.toml").exists());
    assert!(mosaic_path.join("notes").exists());
    assert!(mosaic_path.join("attachments").exists());

    // Check config file content
    let config_content = fs::read_to_string(mosaic_path.join("tesela.toml")).unwrap();
    assert!(config_content.contains("[mosaic]"));
    assert!(config_content.contains("name = \"My Knowledge Mosaic\""));
    assert!(config_content.contains("[settings]"));
}

#[test]
fn test_init_current_directory() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    let output = tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    assert!(Path::new("tesela.toml").exists());
    assert!(Path::new("notes").exists());
    assert!(Path::new("attachments").exists());

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_new_command_without_mosaic() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    let output = tesela_binary()
        .arg("new")
        .arg("My Note")
        .output()
        .expect("Failed to execute tesela");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No mosaic found"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_new_command_with_mosaic() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    // First init
    tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    // Then create note
    let output = tesela_binary()
        .arg("new")
        .arg("My Test Note")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Creating new note: 'My Test Note'"));
    assert!(stdout.contains("Created note: notes/my-test-note.md"));

    // Check note file
    let note_path = Path::new("notes/my-test-note.md");
    assert!(note_path.exists());
    let content = fs::read_to_string(note_path).unwrap();
    assert!(content.contains("title: \"My Test Note\""));
    assert!(content.contains("-"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_list_command_without_mosaic() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    let output = tesela_binary()
        .arg("list")
        .output()
        .expect("Failed to execute tesela");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No mosaic found"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_list_command_empty_mosaic() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    // First init
    tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    // Then list
    let output = tesela_binary()
        .arg("list")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Recent notes:"));
    assert!(stdout.contains("No notes found in this mosaic"));
    assert!(stdout.contains("Create your first note with: tesela new"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_list_command_with_notes() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    // Init and create notes
    tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    tesela_binary()
        .arg("new")
        .arg("First Note")
        .output()
        .expect("Failed to execute tesela");

    tesela_binary()
        .arg("new")
        .arg("Second Note")
        .output()
        .expect("Failed to execute tesela");

    // List notes
    let output = tesela_binary()
        .arg("list")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Recent notes:"));
    assert!(stdout.contains("Second Note"));
    assert!(stdout.contains("First Note"));
    assert!(stdout.contains("[second-note.md]"));
    assert!(stdout.contains("[first-note.md]"));

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_note_title_with_special_characters() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    // Init
    tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    // Create note with special characters
    let output = tesela_binary()
        .arg("new")
        .arg("My Note: With Special Characters! & More?")
        .output()
        .expect("Failed to execute tesela");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created note: notes/my-note_-with-special-characters_-_-more_.md"));

    // Check the file exists with safe filename
    assert!(Path::new("notes/my-note_-with-special-characters_-_-more_.md").exists());

    env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_multiple_notes_ordering() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = env::current_dir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();

    // Init
    tesela_binary()
        .arg("init")
        .arg(".")
        .output()
        .expect("Failed to execute tesela");

    // Create notes with small delays to ensure different timestamps
    for i in 1..=5 {
        tesela_binary()
            .arg("new")
            .arg(format!("Note {}", i))
            .output()
            .expect("Failed to execute tesela");

        // Small delay to ensure different modification times
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // List notes
    let output = tesela_binary()
        .arg("list")
        .output()
        .expect("Failed to execute tesela");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Find positions of notes in output
    let note_positions: Vec<_> = (1..=5)
        .map(|i| stdout.find(&format!("Note {}", i)).unwrap_or(usize::MAX))
        .collect();

    // Verify they appear in reverse order (newest first)
    for i in 0..4 {
        assert!(
            note_positions[i] > note_positions[i + 1],
            "Notes not in correct order: Note {} appears after Note {}",
            i + 1,
            i + 2
        );
    }

    env::set_current_dir(original_dir).unwrap();
}
