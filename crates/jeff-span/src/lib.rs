//! Source spans and diagnostics.
//!
//! Authority: CLAUDE.md APPENDIX H.1 (Diagnostic structure), R37 (every IR node /
//! certificate / diagnostic preserves a source span), R38 (user-input errors are
//! `Result`/diagnostics, never panics), R36 (secret values are never printed —
//! only names/types).
//!
//! This is a small foundational crate (deviation note, DR6): the constitution's
//! crate layout (PART 5.2) does not list a dedicated span crate, but `Span` and
//! `Diagnostic` are referenced by nearly every crate (jeff-cert IrRef has a span;
//! diagnostics are produced by syntax/types/verify/...). Hoisting them here keeps
//! the dependency graph acyclic and lets every crate own one shared definition
//! (R39: own core data structures).

use serde::{Deserialize, Serialize};

/// A byte/line/column source location range.
///
/// `lo`/`hi` are byte offsets into the source; `line`/`col` are 1-based and refer
/// to `lo` for human rendering (APPENDIX H.1). `file` indexes a source map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file: u32,
    pub lo: u32,
    pub hi: u32,
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub const fn new(file: u32, lo: u32, hi: u32, line: u32, col: u32) -> Self {
        Span {
            file,
            lo,
            hi,
            line,
            col,
        }
    }

    /// A synthetic span for compiler-generated nodes (collapsed forms). Still
    /// carries the originating file so diagnostics point somewhere useful (R37).
    pub const fn synthetic(file: u32) -> Self {
        Span {
            file,
            lo: 0,
            hi: 0,
            line: 0,
            col: 0,
        }
    }

    pub const fn dummy() -> Self {
        Span::synthetic(0)
    }

    /// Smallest span covering both `self` and `other` (same file assumed).
    pub fn merge(self, other: Span) -> Span {
        let (lo, line, col) = if self.lo <= other.lo {
            (self.lo, self.line, self.col)
        } else {
            (other.lo, other.line, other.col)
        };
        Span {
            file: self.file,
            lo,
            hi: self.hi.max(other.hi),
            line,
            col,
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Diagnostic severity (APPENDIX H.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// Stable diagnostic codes (APPENDIX H.5). Codes are part of the public contract
/// (R12) and are matched by tests (e.g. E0301 for secret-dependent branch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagCode {
    // E01xx parse / lex
    Lex,
    Parse,
    Layout,
    // E02xx type / kind / arity
    TypeMismatch,
    Kind,
    Arity,
    UnknownName,
    // E03xx secret-taint (R6)
    SecretBranch,      // E0301 [SECRET-IF]
    SecretIndex,       // E0302 [SECRET-IDX]
    IllegalDeclassify, // E0303
    // E04xx termination (R34, Total mode)
    Termination,            // E0401
    GuardedCorecursion,     // E0402
    // E05xx linearity (T3)
    UseAfterMove,    // E05xx
    UnconsumedLinear,
    // E06xx collapse
    CollapseRequiredButBarrier, // E0601
    // E07xx refinement
    RefinementFailed, // E07xx (often Warning: runtime-check fallback)
    // E09xx license / attributes
    LicenseViolation,      // E0901
    UnrecognizedAttribute, // W09xx
}

impl DiagCode {
    /// Render the stable `Exxxx`/`Wxxxx` code string (APPENDIX H.5).
    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::Lex => "E0101",
            DiagCode::Parse => "E0102",
            DiagCode::Layout => "E0103",
            DiagCode::TypeMismatch => "E0201",
            DiagCode::Kind => "E0202",
            DiagCode::Arity => "E0203",
            DiagCode::UnknownName => "E0204",
            DiagCode::SecretBranch => "E0301",
            DiagCode::SecretIndex => "E0302",
            DiagCode::IllegalDeclassify => "E0303",
            DiagCode::Termination => "E0401",
            DiagCode::GuardedCorecursion => "E0402",
            DiagCode::UseAfterMove => "E0501",
            DiagCode::UnconsumedLinear => "E0502",
            DiagCode::CollapseRequiredButBarrier => "E0601",
            DiagCode::RefinementFailed => "E0701",
            DiagCode::LicenseViolation => "E0901",
            DiagCode::UnrecognizedAttribute => "W0901",
        }
    }
}

/// A user-facing diagnostic: (1) what happened, (2) why (notes), (3) what the user
/// can do (help). PART 13 / APPENDIX H.1. R27: readable; R36: never embed secret
/// *values* — only names/types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub span: Span,
    pub message: String,
    pub notes: Vec<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: DiagCode, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            span,
            message: message.into(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn warning(code: DiagCode, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code,
            span,
            message: message.into(),
            notes: Vec::new(),
            help: None,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error)
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        write!(f, "{}[{}]: {}", sev, self.code.as_str(), self.message)?;
        write!(f, "\n  --> {}", self.span)?;
        for n in &self.notes {
            write!(f, "\n  = note: {n}")?;
        }
        if let Some(h) = &self.help {
            write!(f, "\n  = help: {h}")?;
        }
        Ok(())
    }
}

/// Collect diagnostics without stopping at the first error (R20: parsers/checkers
/// gather as many diagnostics as possible).
#[derive(Clone, Debug, Default)]
pub struct DiagSink {
    pub diags: Vec<Diagnostic>,
}

impl DiagSink {
    pub fn new() -> Self {
        DiagSink::default()
    }
    pub fn push(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(Diagnostic::is_error)
    }
    pub fn into_result<T>(self, ok: T) -> Result<T, Vec<Diagnostic>> {
        if self.has_errors() {
            Err(self.diags)
        } else {
            Ok(ok)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_merge_covers_both() {
        let a = Span::new(0, 2, 5, 1, 3);
        let b = Span::new(0, 7, 10, 1, 8);
        let m = a.merge(b);
        assert_eq!(m.lo, 2);
        assert_eq!(m.hi, 10);
        assert_eq!((m.line, m.col), (1, 3));
    }

    #[test]
    fn diag_codes_are_stable() {
        assert_eq!(DiagCode::SecretBranch.as_str(), "E0301");
        assert_eq!(DiagCode::SecretIndex.as_str(), "E0302");
        assert_eq!(DiagCode::Termination.as_str(), "E0401");
        assert_eq!(DiagCode::CollapseRequiredButBarrier.as_str(), "E0601");
    }

    #[test]
    fn sink_reports_errors() {
        let mut s = DiagSink::new();
        assert!(!s.has_errors());
        s.push(Diagnostic::error(DiagCode::Parse, Span::dummy(), "boom"));
        assert!(s.has_errors());
        let r: Result<(), _> = s.into_result(());
        assert!(r.is_err());
    }
}
