//! Structured error types for the Forme rendering engine.
//!
//! Three variants cover the real error sources: JSON parsing, font loading,
//! and layout/PDF generation failures.

use std::fmt;

/// The unified error type returned by all public Forme API functions.
#[derive(Debug)]
pub enum FormeError {
    /// JSON input failed to parse as a valid Forme document.
    ParseError {
        source: serde_json::Error,
        hint: String,
    },
    /// A font could not be loaded, parsed, or embedded.
    FontError(String),
    /// Layout or PDF generation failed.
    RenderError(String),
}

impl fmt::Display for FormeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormeError::ParseError { source, hint } => {
                write!(f, "Failed to parse document: {}", source)?;
                if !hint.is_empty() {
                    write!(f, "\n  Hint: {}", hint)?;
                }
                Ok(())
            }
            FormeError::FontError(msg) => write!(f, "Font error: {}", msg),
            FormeError::RenderError(msg) => write!(f, "Render error: {}", msg),
        }
    }
}

impl std::error::Error for FormeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FormeError::ParseError { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for FormeError {
    fn from(e: serde_json::Error) -> Self {
        let hint = match e.classify() {
            serde_json::error::Category::Syntax => {
                "Check for trailing commas, missing quotes, or unescaped characters.".to_string()
            }
            serde_json::error::Category::Data => {
                "The JSON is valid but doesn't match the Forme document schema. Check field names and types.".to_string()
            }
            serde_json::error::Category::Eof => {
                "Unexpected end of input — is the JSON truncated?".to_string()
            }
            serde_json::error::Category::Io => String::new(),
        };
        FormeError::ParseError { source: e, hint }
    }
}
