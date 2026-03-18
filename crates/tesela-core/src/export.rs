//! Note export functionality for Tesela

use crate::note::Note;
use pulldown_cmark::{html, Parser};

pub enum ExportFormat {
    Html,
    PlainText,
    Markdown,
}

/// Export a note to the given format
pub fn export_note(note: &Note, format: ExportFormat) -> String {
    match format {
        ExportFormat::Html => export_to_html(note),
        ExportFormat::PlainText => export_to_text(note),
        ExportFormat::Markdown => note.content.clone(),
    }
}

fn export_to_html(note: &Note) -> String {
    let parser = Parser::new(&note.body);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    format!(
        "<!DOCTYPE html>\n<html>\n<head><title>{}</title></head>\n<body>\n{}</body>\n</html>",
        note.title, html_output
    )
}

fn export_to_text(note: &Note) -> String {
    note.body.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{Note, NoteId, NoteMetadata};
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_test_note() -> Note {
        Note {
            id: NoteId::new("test"),
            title: "Test Note".to_string(),
            content: "---\ntitle: Test Note\n---\n\nHello **world**".to_string(),
            body: "Hello **world**".to_string(),
            metadata: NoteMetadata::default(),
            path: PathBuf::from("notes/test.md"),
            checksum: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        }
    }

    #[test]
    fn test_export_html() {
        let note = make_test_note();
        let html = export_note(&note, ExportFormat::Html);
        assert!(html.contains("<title>Test Note</title>"));
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_export_plain_text() {
        let note = make_test_note();
        let text = export_note(&note, ExportFormat::PlainText);
        assert_eq!(text, "Hello **world**");
    }

    #[test]
    fn test_export_markdown() {
        let note = make_test_note();
        let md = export_note(&note, ExportFormat::Markdown);
        assert_eq!(md, note.content);
    }
}
