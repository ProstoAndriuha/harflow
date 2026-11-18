use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Current canonical schema version shared across `APIModel`, project manifests,
/// graph view models, and analysis reports.
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Standardized CLI and application exit codes specified by SRS Section 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    RuntimeError = 1,
    InvalidArgs = 2,
    InvalidSource = 3,
    AnalysisError = 4,
    GenerationError = 5,
    ValidationError = 6,
    SecurityViolation = 7,
}

impl ExitCode {
    /// Returns the numeric process exit code.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Typed domain errors across the HARFlow core workspace.
#[derive(Debug, Error)]
pub enum HarflowError {
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),

    #[error("invalid source input: {0}")]
    InvalidSource(String),

    #[error("analysis error: {0}")]
    Analysis(String),

    #[error("generation error: {0}")]
    Generation(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("security policy violation: {0}")]
    SecurityViolation(String),

    #[error("storage or I/O error: {0}")]
    Storage(String),
}

impl HarflowError {
    /// Maps each domain error to its canonical process exit code.
    #[must_use]
    pub const fn exit_code(&self) -> ExitCode {
        match self {
            Self::InvalidArgs(_) => ExitCode::InvalidArgs,
            Self::InvalidSource(_) => ExitCode::InvalidSource,
            Self::Analysis(_) => ExitCode::AnalysisError,
            Self::Generation(_) => ExitCode::GenerationError,
            Self::Validation(_) => ExitCode::ValidationError,
            Self::SecurityViolation(_) => ExitCode::SecurityViolation,
            Self::Storage(_) => ExitCode::RuntimeError,
        }
    }
}

impl From<std::io::Error> for HarflowError {
    fn from(err: std::io::Error) -> Self {
        Self::Storage(err.to_string())
    }
}

/// Preserves the M03 distinction between an absent field, explicit `null`, and a present value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "presence", content = "value", rename_all = "snake_case")]
pub enum FieldPresence<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> FieldPresence<T> {
    #[must_use]
    pub fn from_option(opt: Option<Option<T>>) -> Self {
        match opt {
            None => Self::Missing,
            Some(None) => Self::Null,
            Some(Some(value)) => Self::Value(value),
        }
    }

    #[must_use]
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_errors_to_spec_exit_codes() {
        assert_eq!(
            HarflowError::InvalidArgs("x".into()).exit_code().as_i32(),
            2
        );
        assert_eq!(
            HarflowError::InvalidSource("x".into()).exit_code().as_i32(),
            3
        );
        assert_eq!(HarflowError::Analysis("x".into()).exit_code().as_i32(), 4);
        assert_eq!(HarflowError::Generation("x".into()).exit_code().as_i32(), 5);
        assert_eq!(HarflowError::Validation("x".into()).exit_code().as_i32(), 6);
        assert_eq!(
            HarflowError::SecurityViolation("x".into())
                .exit_code()
                .as_i32(),
            7
        );
        assert_eq!(HarflowError::Storage("x".into()).exit_code().as_i32(), 1);
    }

    #[test]
    fn distinguishes_missing_null_and_empty_string() {
        let missing: FieldPresence<String> = FieldPresence::from_option(None);
        let null_val: FieldPresence<String> = FieldPresence::from_option(Some(None));
        let empty_val: FieldPresence<String> =
            FieldPresence::from_option(Some(Some(String::new())));

        assert!(missing.is_missing());
        assert!(null_val.is_null());
        assert_eq!(empty_val, FieldPresence::Value(String::new()));
    }
}
