use crate::CliDiagnostic;
use biome_configuration::analyzer::AnalyzerSelector;
use biome_diagnostics::Category;
use biome_fs::{FileSystem, OpenOptions};
use biome_service::WorkspaceError;
use biome_service::baseline::{BASELINE_FILE_NAME, Baseline, RuleCounts, relative_path};
use biome_service::diagnostics::BaselineNotReadable;
use biome_service::workspace::BaselineMode;
use bpaf::Bpaf;
use camino::{Utf8Path, Utf8PathBuf};

#[derive(Debug, Default, Clone, Bpaf)]
pub struct BaselineOptions {
    /// Records the current error-level lint violations in `biome-baseline.json`, at the root of
    /// the project, so that they are no longer reported. A file that gets more violations of a
    /// rule than recorded reports all of them again.
    #[bpaf(long("baseline"), switch)]
    pub baseline: bool,

    /// Like `--baseline`, but records the violations of the given rule only. It can be repeated.
    ///
    /// Example:
    ///
    /// ```shell
    /// biome lint --baseline-rule=suspicious/noExplicitAny --baseline-rule=style/useConst
    /// ```
    #[bpaf(long("baseline-rule"), argument("GROUP/RULE"))]
    pub baseline_rule: Vec<String>,

    /// Lowers the counts in `biome-baseline.json` to the violations that remain, and removes
    /// entries for violations and files that no longer exist.
    #[bpaf(long("prune-baseline"), switch)]
    pub prune_baseline: bool,
}

impl BaselineOptions {
    /// The [BaselineMode] these options ask for, validating them against the
    /// rest of the command.
    pub(crate) fn mode(
        &self,
        only: &[AnalyzerSelector],
        skip: &[AnalyzerSelector],
        stdin_file_path: Option<&str>,
    ) -> Result<BaselineMode, CliDiagnostic> {
        let mode = match (
            self.baseline,
            self.baseline_rule.is_empty(),
            self.prune_baseline,
        ) {
            (false, true, false) => return Ok(BaselineMode::Apply),
            (true, true, false) => BaselineMode::Record { rules: None },
            (false, false, false) => BaselineMode::Record {
                rules: Some(
                    self.baseline_rule
                        .iter()
                        .map(|rule| rule_category(rule))
                        .collect::<Result<_, _>>()?,
                ),
            },
            (false, true, true) => BaselineMode::Prune,
            (true, _, true) | (_, false, true) => {
                return Err(CliDiagnostic::incompatible_arguments(
                    self.writing_flag(),
                    "--prune-baseline",
                    "Recording and pruning the baseline can't be done at the same time.",
                ));
            }
            (true, false, false) => {
                return Err(CliDiagnostic::incompatible_arguments(
                    "--baseline",
                    "--baseline-rule",
                    "`--baseline` records every rule; use `--baseline-rule` alone to record some of them.",
                ));
            }
        };

        let flag = self.writing_flag();
        if !only.is_empty() || !skip.is_empty() {
            return Err(CliDiagnostic::incompatible_arguments(
                flag,
                if only.is_empty() { "--skip" } else { "--only" },
                "The baseline needs the violations of every rule; use `--baseline-rule` to record some of them.",
            ));
        }
        if stdin_file_path.is_some() {
            return Err(CliDiagnostic::incompatible_arguments(
                flag,
                "--stdin-file-path",
                "The baseline can only be updated from files on disk.",
            ));
        }

        Ok(mode)
    }

    /// The flag that writes the baseline, for diagnostics.
    pub(crate) fn writing_flag(&self) -> &'static str {
        if self.baseline {
            "--baseline"
        } else if !self.baseline_rule.is_empty() {
            "--baseline-rule"
        } else {
            "--prune-baseline"
        }
    }

    pub(crate) fn writes_baseline(&self) -> bool {
        self.baseline || !self.baseline_rule.is_empty() || self.prune_baseline
    }
}

/// Turns `group/rule` (or `lint/group/rule`) into the rule's diagnostic
/// category, and `plugin/name` into the name a plugin's violations are
/// recorded under (`lint/plugin/name`).
fn rule_category(rule: &str) -> Result<String, CliDiagnostic> {
    let name = format!("lint/{}", rule.strip_prefix("lint/").unwrap_or(rule));
    if let Some(plugin) = name.strip_prefix("lint/plugin/")
        && !plugin.is_empty()
        && !plugin.contains('/')
    {
        return Ok(name);
    }
    match name.parse::<&'static Category>() {
        Ok(category) if name.matches('/').count() == 2 => Ok(category.name().to_string()),
        _ => Err(CliDiagnostic::parse_error(format!(
            "`{rule}` isn't a lint rule. `--baseline-rule` expects a rule as `group/rule`, e.g. `suspicious/noExplicitAny`, or a plugin as `plugin/name`."
        ))),
    }
}

/// Rewrites the baseline file of `project_dir` from the violations found in
/// this run, as `--baseline`, `--baseline-rule` and `--prune-baseline` ask.
///
/// Only the files that were linted are updated. A prune also drops files that
/// no longer exist.
pub(crate) fn update_baseline_file(
    fs: &dyn FileSystem,
    project_dir: &Utf8Path,
    mode: &BaselineMode,
    violations: &[(Utf8PathBuf, RuleCounts)],
) -> Result<(), CliDiagnostic> {
    let path = project_dir.join(BASELINE_FILE_NAME);
    let mut baseline = if fs.path_is_file(&path) {
        let content = fs
            .read_file_from_path(&path)
            .map_err(WorkspaceError::from)?;
        Baseline::parse(&content).map_err(|error| BaselineNotReadable {
            path: path.to_string(),
            reason: error.to_string(),
        })?
    } else {
        Baseline::default()
    };

    for (file, violations) in violations {
        let Some(key) = relative_path(project_dir, file) else {
            continue;
        };
        match mode {
            BaselineMode::Apply => return Ok(()),
            BaselineMode::Record { rules } => baseline.record(&key, violations, rules.as_deref()),
            BaselineMode::Prune => baseline.prune(&key, violations),
        }
    }

    if *mode == BaselineMode::Prune {
        baseline.retain_files(|key| fs.path_is_file(&project_dir.join(key)));
    }

    let mut file = fs.open_with_options(&path, OpenOptions::default().write(true).create(true))?;
    file.set_content(baseline.to_json().as_bytes())?;
    Ok(())
}
