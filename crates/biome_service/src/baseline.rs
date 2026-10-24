//! Bulk suppressions ("the baseline").
//!
//! A baseline file records, for each file and each lint rule, how many
//! error-level violations that file is allowed to keep. It lets a project
//! promote a rule to `error` without fixing or comment-suppressing every
//! existing violation first: the recorded violations are hidden, and any new
//! one makes the rule report again.
//!
//! Counts can't tell an old violation from a new one, so a rule is handled per
//! file as a whole:
//! - violations `<=` the recorded count: all of them are suppressed;
//! - violations `>` the recorded count: all of them are reported;
//! - violations `<` the recorded count: the difference is reported as unused,
//!   so the baseline only ever shrinks.

use std::collections::BTreeMap;

use biome_diagnostics::{Diagnostic, Severity};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::workspace::BaselineMode;

/// The name of the baseline file, looked up next to the root configuration file.
pub const BASELINE_FILE_NAME: &str = "biome-baseline.json";

/// Allowed violations per rule, keyed by diagnostic category (`lint/suspicious/noExplicitAny`).
pub type RuleCounts = BTreeMap<String, u32>;

/// The contents of a baseline file.
///
/// Paths are relative to the project root and always use `/`. `BTreeMap` keeps
/// keys sorted so the serialized file is stable and diffs stay small.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Baseline {
    files: BTreeMap<String, BTreeMap<String, BaselineEntry>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    expecting = r#"an entry such as `{ "count": 2 }`"#
)]
struct BaselineEntry {
    count: u32,
}

/// What applying the baseline to one file decided.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct BaselineOutcome {
    /// Rules whose diagnostics must be hidden for this file.
    pub suppressed: Vec<String>,
    /// Rules with recorded violations that no longer occur, and how many.
    pub unused: RuleCounts,
}

impl Baseline {
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(content)
    }

    /// Serializes the baseline with two-space indentation and a trailing newline.
    pub fn to_json(&self) -> String {
        let mut json =
            serde_json::to_string_pretty(self).expect("a baseline is always serializable");
        json.push('\n');
        json
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn contains_file(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// Decides which rules to suppress in `path`, given the error-level
    /// violations it has now.
    pub fn evaluate(&self, path: &str, violations: &RuleCounts) -> BaselineOutcome {
        let mut outcome = BaselineOutcome::default();
        let Some(entries) = self.files.get(path) else {
            return outcome;
        };

        for (rule, entry) in entries {
            let found = violations.get(rule).copied().unwrap_or(0);
            if found > 0 && found <= entry.count {
                outcome.suppressed.push(rule.clone());
            }
            if found < entry.count {
                outcome.unused.insert(rule.clone(), entry.count - found);
            }
        }

        outcome
    }

    /// Makes the recorded counts of `path` match its current violations. When
    /// `rules` is given, only those rules are touched; otherwise every rule is,
    /// including recorded rules that no longer have violations.
    pub fn record(&mut self, path: &str, violations: &RuleCounts, rules: Option<&[String]>) {
        let in_scope = |rule: &String| rules.is_none_or(|rules| rules.contains(rule));
        let entries = self.files.entry(path.to_string()).or_default();

        entries.retain(|rule, _| !in_scope(rule));
        for (rule, &count) in violations {
            if count > 0 && in_scope(rule) {
                entries.insert(rule.clone(), BaselineEntry { count });
            }
        }

        if entries.is_empty() {
            self.files.remove(path);
        }
    }

    /// Lowers the recorded counts of `path` to its current violations, and
    /// removes rules (and the file) that no longer have any.
    pub fn prune(&mut self, path: &str, violations: &RuleCounts) {
        let Some(entries) = self.files.get_mut(path) else {
            return;
        };
        entries.retain(|rule, entry| {
            let found = violations.get(rule).copied().unwrap_or(0);
            entry.count = entry.count.min(found);
            entry.count > 0
        });
        if entries.is_empty() {
            self.files.remove(path);
        }
    }

    /// Like [Self::evaluate], but against the baseline as `mode` will leave it,
    /// so that the output of a run that rewrites the baseline already matches
    /// the new file.
    pub fn evaluate_with_mode(
        &self,
        path: &str,
        violations: &RuleCounts,
        mode: &BaselineMode,
    ) -> BaselineOutcome {
        let mut file = Self {
            files: self
                .files
                .get_key_value(path)
                .map(|(path, entries)| BTreeMap::from([(path.clone(), entries.clone())]))
                .unwrap_or_default(),
        };
        match mode {
            BaselineMode::Apply => {}
            BaselineMode::Record { rules } => file.record(path, violations, rules.as_deref()),
            BaselineMode::Prune => file.prune(path, violations),
        }
        file.evaluate(path, violations)
    }

    /// Removes files for which `keep` returns `false`, e.g. files that were deleted.
    pub fn retain_files(&mut self, mut keep: impl FnMut(&str) -> bool) {
        self.files.retain(|path, _| keep(path));
    }
}

/// A project's baseline, with the directory its paths are relative to.
#[derive(Debug)]
pub struct ProjectBaseline {
    pub root: Utf8PathBuf,
    pub baseline: Baseline,
}

impl ProjectBaseline {
    /// The key `path` has in the baseline file, if it is inside the root.
    pub fn relative_path(&self, path: &Utf8Path) -> Option<String> {
        relative_path(&self.root, path)
    }
}

/// The key `path` has in a baseline file whose paths are relative to `root`.
pub fn relative_path(root: &Utf8Path, path: &Utf8Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let key = relative
        .components()
        .map(|component| component.as_str())
        .collect::<Vec<_>>()
        .join("/");
    Some(key)
}

/// Counts the error-level lint violations per rule in `diagnostics`.
///
/// Warnings are never baselined: a rule has to be an error for its violations
/// to be worth recording.
pub fn count_violations<'a, D: Diagnostic + 'a>(
    diagnostics: impl IntoIterator<Item = &'a D>,
) -> RuleCounts {
    let mut counts = RuleCounts::new();
    for diagnostic in diagnostics {
        if let Some(rule) = baselined_rule(diagnostic) {
            *counts.entry(rule.to_string()).or_default() += 1;
        }
    }
    counts
}

/// The rule a diagnostic counts against, if the baseline applies to it.
pub fn baselined_rule(diagnostic: &impl Diagnostic) -> Option<&'static str> {
    let category = diagnostic.category()?.name();
    (category.starts_with("lint/") && diagnostic.severity() >= Severity::Error).then_some(category)
}

#[cfg(test)]
#[path = "baseline.tests.rs"]
mod tests;
