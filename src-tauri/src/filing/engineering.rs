//! Software-engineering profile: classification of local automation, QA, and
//! build artifacts by filename.
//!
//! Rules first, AI never (for this layer): a pure function of the file *name*
//! and extension. No network, no model, no IO, no file-content reads. This helps
//! engineers and automation teams preview how recurring reports/logs would be
//! filed before the Organizer performs any approved move.

use serde::{Deserialize, Serialize};

/// A software-engineering artifact Ghost recognizes by filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineeringKind {
    TestReport,
    CoverageReport,
    BuildLog,
    ReleaseNote,
    IncidentReport,
    Runbook,
    Benchmark,
    Screenshot,
    Trace,
    Config,
    /// Recognized as an engineering artifact, but the specific type is unclear.
    Generic,
}

impl EngineeringKind {
    pub fn folder_name(self) -> &'static str {
        match self {
            EngineeringKind::TestReport => "Test Reports",
            EngineeringKind::CoverageReport => "Coverage Reports",
            EngineeringKind::BuildLog => "Build Logs",
            EngineeringKind::ReleaseNote => "Release Notes",
            EngineeringKind::IncidentReport => "Incidents",
            EngineeringKind::Runbook => "Runbooks",
            EngineeringKind::Benchmark => "Benchmarks",
            EngineeringKind::Screenshot => "Screenshots",
            EngineeringKind::Trace => "Traces",
            EngineeringKind::Config => "Configs",
            EngineeringKind::Generic => "Engineering Artifacts",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EngineeringKind::TestReport => "Test Report",
            EngineeringKind::CoverageReport => "Coverage Report",
            EngineeringKind::BuildLog => "Build Log",
            EngineeringKind::ReleaseNote => "Release Note",
            EngineeringKind::IncidentReport => "Incident Report",
            EngineeringKind::Runbook => "Runbook",
            EngineeringKind::Benchmark => "Benchmark",
            EngineeringKind::Screenshot => "Screenshot",
            EngineeringKind::Trace => "Trace",
            EngineeringKind::Config => "Config",
            EngineeringKind::Generic => "Engineering Artifact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringClassification {
    pub kind: EngineeringKind,
    pub confidence: f32,
    pub reason: String,
}

pub const LOW_CONFIDENCE: f32 = 0.5;
const SPECIFIC_CONFIDENCE: f32 = 0.9;
const GENERIC_CONFIDENCE: f32 = 0.55;

const ENGINEERING_EXTS: &[&str] = &[
    "txt", "log", "md", "json", "xml", "html", "htm", "csv", "tsv", "pdf", "png", "jpg", "jpeg",
    "webp", "zip", "tar", "gz", "yaml", "yml", "toml", "ini", "sarif", "har",
];

#[allow(clippy::type_complexity)]
fn keyword_table() -> &'static [(&'static [&'static str], EngineeringKind)] {
    &[
        (
            &["coverage", "lcov", "cobertura"],
            EngineeringKind::CoverageReport,
        ),
        (
            &[
                "test report",
                "junit",
                "pytest",
                "playwright",
                "cypress",
                "test-results",
                "test_results",
            ],
            EngineeringKind::TestReport,
        ),
        (
            &["build log", "ci log", "compile", "pipeline", "workflow run"],
            EngineeringKind::BuildLog,
        ),
        (
            &["release notes", "changelog", "release"],
            EngineeringKind::ReleaseNote,
        ),
        (
            &["incident", "postmortem", "retro", "outage"],
            EngineeringKind::IncidentReport,
        ),
        (
            &["runbook", "playbook", "ops guide"],
            EngineeringKind::Runbook,
        ),
        (
            &["benchmark", "perf", "performance", "load test"],
            EngineeringKind::Benchmark,
        ),
        (
            &["screenshot", "snapshot", "visual diff"],
            EngineeringKind::Screenshot,
        ),
        (
            &["trace", "har", "profile", "flamegraph"],
            EngineeringKind::Trace,
        ),
        (
            &["config", "settings", "env", "manifest"],
            EngineeringKind::Config,
        ),
    ]
}

fn extension(name: &str) -> Option<String> {
    std::path::Path::new(name)
        .extension()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
}

fn normalized(name: &str) -> String {
    name.to_ascii_lowercase().replace(['_', '-'], " ")
}

pub fn classify_artifact(file_name: &str) -> Option<EngineeringClassification> {
    let ext = extension(file_name)?;
    if !ENGINEERING_EXTS.contains(&ext.as_str()) {
        return None;
    }

    let haystack = normalized(file_name);
    for (keywords, kind) in keyword_table() {
        if let Some(keyword) = keywords.iter().find(|keyword| haystack.contains(**keyword)) {
            return Some(EngineeringClassification {
                kind: *kind,
                confidence: SPECIFIC_CONFIDENCE,
                reason: format!("filename contains engineering keyword '{keyword}'"),
            });
        }
    }

    if matches!(ext.as_str(), "log" | "sarif" | "har") {
        return Some(EngineeringClassification {
            kind: EngineeringKind::Generic,
            confidence: GENERIC_CONFIDENCE,
            reason: format!(".{ext} is a common engineering artifact extension"),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_test_reports() {
        let c = classify_artifact("junit-test-results-2026-07.xml").unwrap();
        assert_eq!(c.kind, EngineeringKind::TestReport);
        assert!(c.confidence > LOW_CONFIDENCE);
    }

    #[test]
    fn rejects_unrelated_images() {
        assert!(classify_artifact("vacation.jpg").is_none());
    }
}
