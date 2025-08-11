//! Tesela - A keyboard-first, file-based note-taking system
//!
//! This library provides the core functionality for the Tesela note-taking application.

pub mod commands;

// Re-export commonly used items
pub use commands::{create_note, init_mosaic, list_notes};

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn test_init_creates_directory_structure() {
        let temp_dir = TempDir::new().unwrap();
        let mosaic_path = temp_dir.path().join("test-mosaic");

        // Initialize mosaic
        init_mosaic(mosaic_path.to_str().unwrap()).unwrap();

        // Check all expected directories and files exist
        assert!(mosaic_path.exists());
        assert!(mosaic_path.join("notes").exists());
        assert!(mosaic_path.join("attachments").exists());
        assert!(mosaic_path.join("tesela.toml").exists());
    }

    #[test]
    fn test_config_file_content() {
        let temp_dir = TempDir::new().unwrap();
        let mosaic_path = temp_dir.path().join("test-mosaic");

        init_mosaic(mosaic_path.to_str().unwrap()).unwrap();

        let config_content = fs::read_to_string(mosaic_path.join("tesela.toml")).unwrap();
        assert!(config_content.contains("[mosaic]"));
        assert!(config_content.contains("name = \"My Knowledge Mosaic\""));
        assert!(config_content.contains("[settings]"));
        assert!(config_content.contains("editor = \"default\""));
        assert!(config_content.contains("auto_save = 30"));
        assert!(config_content.contains("daily_notes = true"));
    }

    #[test]
    fn test_create_note_requires_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        // Try to create note without mosaic
        let result = create_note("Test Note");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No mosaic found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_create_note_with_valid_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        // Initialize mosaic first
        init_mosaic(".").unwrap();

        // Create note
        create_note("My Test Note").unwrap();

        // Check note exists
        let note_path = Path::new("notes/my-test-note.md");
        assert!(note_path.exists());

        // Check note content
        let content = fs::read_to_string(note_path).unwrap();
        assert!(content.contains("title: \"My Test Note\""));
        assert!(content.contains("# My Test Note"));
        assert!(content.contains("tags: []"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_filename_sanitization() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        init_mosaic(".").unwrap();

        // Create note with special characters
        create_note("My Note: With Special/Characters!").unwrap();

        // Check that file was created with safe name
        let expected_path = Path::new("notes/my-note_-with-special_characters_.md");
        assert!(expected_path.exists());

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_list_notes_requires_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let result = list_notes();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No mosaic found"));

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_list_notes_with_empty_mosaic() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        init_mosaic(".").unwrap();

        // Should succeed even with no notes
        list_notes().unwrap();

        env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn test_list_notes_missing_notes_directory() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = env::current_dir().unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        init_mosaic(".").unwrap();

        // Remove notes directory
        fs::remove_dir("notes").unwrap();

        // Should still succeed
        list_notes().unwrap();

        env::set_current_dir(original_dir).unwrap();
    }
}
