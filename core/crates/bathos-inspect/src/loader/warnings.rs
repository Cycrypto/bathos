//! `ParseWarning` — lenient parse warning collector (CR-3/CF-0.5).
//!
//! Every loader stage (manifest/enums/audit) loads a warning into this collector
//! and continues instead of crashing. "A partial failure is a warning, an absence is
//! normal" is the principle governing this whole crate.
//! [Source: project-context-kr.md §3 CR-3, api-contracts-kr.md §B ParseWarning]

use serde::Serialize;

/// A single warning entry. Feeds both `--json` output and the dashboard banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseWarning {
    /// Stable code (e.g. `W-ENUM-UNKNOWN`, `W-MANIFEST-DESCRIPTIVE`). [Source: exceptions-kr.md]
    pub code: String,
    /// Human-readable description.
    pub message: String,
    /// Where it occurred (e.g. `_state/manifest.json#waves[2]`, `audit-log.jsonl:14`).
    pub location: String,
}

impl ParseWarning {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: location.into(),
        }
    }
}

/// The warning accumulator shared by every loader stage.
#[derive(Debug, Default, Clone)]
pub struct WarningSink {
    warnings: Vec<ParseWarning>,
}

impl WarningSink {
    /// Load one warning. Continuing without crashing is this crate's contract.
    pub fn push(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        location: impl Into<String>,
    ) {
        self.warnings
            .push(ParseWarning::new(code, message, location));
    }

    /// Number of warnings accumulated so far.
    pub fn len(&self) -> usize {
        self.warnings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Finish collecting and consume into `Vec<ParseWarning>`.
    pub fn into_vec(self) -> Vec<ParseWarning> {
        self.warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_accumulates_in_order() {
        let mut sink = WarningSink::default();
        sink.push("W-A", "first", "loc-1");
        sink.push("W-B", "second", "loc-2");
        assert_eq!(sink.len(), 2);
        let v = sink.into_vec();
        assert_eq!(v[0].code, "W-A");
        assert_eq!(v[1].message, "second");
    }
}
