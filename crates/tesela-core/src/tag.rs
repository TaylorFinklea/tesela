//! Tag validation and types for Tesela

use serde::{Deserialize, Serialize};

use crate::error::TeselaError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag(String);

impl Tag {
    pub fn new(s: impl Into<String>) -> crate::error::Result<Self> {
        let s = s.into();
        if s.is_empty() {
            return Err(TeselaError::Validation {
                message: "Tag cannot be empty".to_string(),
            });
        }
        if !s
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '/' || c == '_')
        {
            return Err(TeselaError::Validation {
                message: format!(
                    "Invalid tag '{}': only alphanumeric, hyphens, slashes, underscores allowed",
                    s
                ),
            });
        }
        Ok(Tag(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_tag() {
        let tag = Tag::new("rust").unwrap();
        assert_eq!(tag.as_str(), "rust");
    }

    #[test]
    fn test_valid_tag_with_hyphens() {
        let tag = Tag::new("my-tag").unwrap();
        assert_eq!(tag.as_str(), "my-tag");
    }

    #[test]
    fn test_valid_tag_with_underscores() {
        let tag = Tag::new("my_tag").unwrap();
        assert_eq!(tag.as_str(), "my_tag");
    }

    #[test]
    fn test_valid_hierarchical_tag() {
        let tag = Tag::new("projects/tesela/core").unwrap();
        assert_eq!(tag.as_str(), "projects/tesela/core");
    }

    #[test]
    fn test_invalid_empty_tag() {
        let result = Tag::new("");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("empty"));
    }

    #[test]
    fn test_invalid_tag_with_spaces() {
        let result = Tag::new("my tag");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid tag"));
    }

    #[test]
    fn test_invalid_tag_with_special_chars() {
        assert!(Tag::new("tag@home").is_err());
        assert!(Tag::new("tag!").is_err());
        assert!(Tag::new("tag#1").is_err());
        assert!(Tag::new("tag$").is_err());
    }

    #[test]
    fn test_valid_tag_with_numbers() {
        let tag = Tag::new("v2").unwrap();
        assert_eq!(tag.as_str(), "v2");
    }
}
