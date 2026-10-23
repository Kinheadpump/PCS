//! Helpers for the content loaders of the sim crate (TECHNICAL_DESIGN §10).
//!
//! Each loader reads a RON text into a raw struct, checks it, and links the names to
//! indices. A problem names the file and the entry. The loaders never panic on bad data.

use base::content::{ContentError, ContentIssue, MaterialDef};
use serde::de::DeserializeOwned;

/// Read a RON text into a raw struct.
pub(crate) fn parse<T: DeserializeOwned>(file: &str, text: &str) -> Result<T, ContentError> {
    ron::from_str(text).map_err(|e| ContentError {
        file: file.to_string(),
        issues: vec![ContentIssue {
            entry: None,
            message: format!("parse error: {e}"),
        }],
    })
}

/// The problems found in one file.
pub(crate) struct Issues {
    file: String,
    list: Vec<ContentIssue>,
}

impl Issues {
    /// An empty list for a file.
    pub(crate) fn new(file: &str) -> Self {
        Self {
            file: file.to_string(),
            list: Vec::new(),
        }
    }

    /// Add a problem of one entry.
    pub(crate) fn add(&mut self, entry: &str, message: &str) {
        self.list.push(ContentIssue {
            entry: Some(entry.to_string()),
            message: message.to_string(),
        });
    }

    /// Link a material key to its index. A missing key adds a problem.
    pub(crate) fn material(&mut self, materials: &[MaterialDef], key: &str, what: &str) -> u16 {
        match materials.iter().position(|m| m.key == key) {
            Some(i) => u16::try_from(i).unwrap_or(u16::MAX),
            None => {
                self.add(key, &format!("{what} uses an unknown material"));
                u16::MAX
            }
        }
    }

    /// Give the value, or the error if there is a problem.
    pub(crate) fn finish<T>(self, value: T) -> Result<T, ContentError> {
        if self.list.is_empty() {
            Ok(value)
        } else {
            Err(ContentError {
                file: self.file,
                issues: self.list,
            })
        }
    }
}
