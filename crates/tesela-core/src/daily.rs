//! Daily note generation for Tesela

use chrono::NaiveDate;

pub struct DailyNoteConfig {
    pub template: Option<String>,
    pub date_format: String, // e.g. "%Y-%m-%d"
}

impl Default for DailyNoteConfig {
    fn default() -> Self {
        Self {
            template: None,
            date_format: "%Y-%m-%d".to_string(),
        }
    }
}

/// Generate the title for a daily note
pub fn daily_note_title(date: NaiveDate, config: &DailyNoteConfig) -> String {
    date.format(&config.date_format).to_string()
}

/// Generate the filename for a daily note
pub fn daily_note_filename(date: NaiveDate, config: &DailyNoteConfig) -> String {
    format!("{}.md", daily_note_title(date, config))
}

/// Generate content for a new daily note
pub fn daily_note_content(date: NaiveDate, config: &DailyNoteConfig) -> String {
    if let Some(template) = &config.template {
        template.replace("{{date}}", &date.format(&config.date_format).to_string())
    } else {
        let title = daily_note_title(date, config);
        format!(
            "---\ntitle: {}\ntags: [daily]\ncreated: {}\n---\n\n# {}\n\n",
            title,
            date.format("%Y-%m-%dT00:00:00Z"),
            title
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_daily_note_title_default_format() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig::default();
        assert_eq!(daily_note_title(date, &config), "2026-03-18");
    }

    #[test]
    fn test_daily_note_title_custom_format() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig {
            template: None,
            date_format: "%d/%m/%Y".to_string(),
        };
        assert_eq!(daily_note_title(date, &config), "18/03/2026");
    }

    #[test]
    fn test_daily_note_filename() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig::default();
        assert_eq!(daily_note_filename(date, &config), "2026-03-18.md");
    }

    #[test]
    fn test_daily_note_content_default() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig::default();
        let content = daily_note_content(date, &config);
        assert!(content.contains("title: 2026-03-18"));
        assert!(content.contains("tags: [daily]"));
        assert!(content.contains("# 2026-03-18"));
    }

    #[test]
    fn test_daily_note_content_with_template() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig {
            template: Some("# Journal for {{date}}\n\n## Tasks\n\n".to_string()),
            date_format: "%Y-%m-%d".to_string(),
        };
        let content = daily_note_content(date, &config);
        assert_eq!(content, "# Journal for 2026-03-18\n\n## Tasks\n\n");
    }
}
