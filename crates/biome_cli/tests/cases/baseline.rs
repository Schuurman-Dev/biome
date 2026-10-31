use crate::TestArgs as Args;
use crate::run_cli_with_dyn_fs;
use crate::snap_test::{CliRunResult, SnapshotPayload, assert_cli_snapshot, message_to_string};
use biome_console::BufferConsole;
use biome_fs::TemporaryFs;

const CONFIG: &str = r#"{
    "linter": {
        "rules": {
            "preset": "none",
            "suspicious": { "noDebugger": "error", "noDoubleEquals": "warn" },
            "complexity": { "noUselessCatch": "error" }
        }
    }
}"#;

const TWO_DEBUGGERS: &str = "debugger;\ndebugger;\n";

// The workspace reads the baseline file from disk, so these tests use a real
// directory rather than a `MemoryFileSystem`.
fn setup(name: &str, files: &[(&str, &str)]) -> TemporaryFs {
    let mut fs = TemporaryFs::new(name);
    fs.create_file("biome.json", CONFIG);
    for (path, content) in files {
        fs.create_file(path, content);
    }
    fs
}

fn read_baseline(fs: &TemporaryFs) -> String {
    std::fs::read_to_string(fs.working_directory.join("biome-baseline.json")).unwrap()
}

fn run(fs: &TemporaryFs, console: &mut BufferConsole, args: &[&str]) -> CliRunResult {
    let path = fs.cli_path();
    let mut args = args.to_vec();
    args.push(path);
    run_cli_with_dyn_fs(
        Box::new(fs.create_os()),
        console,
        Args::from(args.as_slice()),
    )
}

#[test]
fn baseline_records_error_violations() {
    let fs = setup(
        "baseline_baseline_records_error_violations",
        &[
            ("a.js", TWO_DEBUGGERS),
            ("b.js", "if (a == b) {}\ntry {} catch (e) { throw e; }\n"),
            ("clean.js", "export const a = 1;\n"),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint", "--baseline"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
    assert_eq!(
        read_baseline(&fs),
        r#"{
  "a.js": {
    "lint/suspicious/noDebugger": {
      "count": 2
    }
  },
  "b.js": {
    "lint/complexity/noUselessCatch": {
      "count": 1
    }
  }
}
"#
    );

    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "baseline_records_error_violations",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn baseline_suppresses_recorded_violations() {
    let fs = setup(
        "baseline_baseline_suppresses_recorded_violations",
        &[
            ("a.js", TWO_DEBUGGERS),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": { "count": 2 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "baseline_suppresses_recorded_violations",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn baseline_reports_every_violation_when_count_is_exceeded() {
    let fs = setup(
        "baseline_baseline_reports_every_violation_when_count_is_exceeded",
        &[
            ("a.js", TWO_DEBUGGERS),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": { "count": 1 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "baseline_reports_every_violation_when_count_is_exceeded",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn baseline_reports_unused_entries() {
    let fs = setup(
        "baseline_baseline_reports_unused_entries",
        &[
            ("a.js", "debugger;\n"),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": { "count": 2 }, "lint/complexity/noUselessCatch": { "count": 1 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "baseline_reports_unused_entries",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn baseline_doesnt_report_unused_entries_with_only() {
    let fs = setup(
        "baseline_baseline_doesnt_report_unused_entries_with_only",
        &[
            ("a.js", "debugger;\n"),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": { "count": 1 }, "lint/complexity/noUselessCatch": { "count": 1 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint", "--only=suspicious/noDebugger"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
}

#[test]
fn prune_baseline_lowers_counts_and_drops_missing_files() {
    let fs = setup(
        "baseline_prune_baseline_lowers_counts_and_drops_missing_files",
        &[
            ("a.js", "debugger;\n"),
            (
                "biome-baseline.json",
                r#"{
                "a.js": { "lint/suspicious/noDebugger": { "count": 3 }, "lint/complexity/noUselessCatch": { "count": 1 } },
                "deleted.js": { "lint/suspicious/noDebugger": { "count": 1 } }
            }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint", "--prune-baseline"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
    assert_eq!(
        read_baseline(&fs),
        r#"{
  "a.js": {
    "lint/suspicious/noDebugger": {
      "count": 1
    }
  }
}
"#
    );
}

#[test]
fn baseline_rule_records_only_that_rule() {
    let fs = setup(
        "baseline_baseline_rule_records_only_that_rule",
        &[("a.js", "debugger;\ntry {} catch (e) { throw e; }\n")],
    );
    let mut console = BufferConsole::default();

    let result = run(
        &fs,
        &mut console,
        &["lint", "--baseline-rule=suspicious/noDebugger"],
    );

    // noUselessCatch isn't recorded, so it still fails the run.
    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_eq!(
        read_baseline(&fs),
        r#"{
  "a.js": {
    "lint/suspicious/noDebugger": {
      "count": 1
    }
  }
}
"#
    );
}

#[test]
fn check_applies_the_baseline() {
    let fs = setup(
        "baseline_check_applies_the_baseline",
        &[
            ("a.js", TWO_DEBUGGERS),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": { "count": 2 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["check", "--formatter-enabled=false"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
}

#[test]
fn err_when_baseline_and_prune_baseline_are_passed() {
    let fs = setup(
        "baseline_err_when_baseline_and_prune_baseline_are_passed",
        &[("a.js", TWO_DEBUGGERS)],
    );
    let mut console = BufferConsole::default();

    let result = run(
        &fs,
        &mut console,
        &["lint", "--baseline", "--prune-baseline"],
    );

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "err_when_baseline_and_prune_baseline_are_passed",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn err_when_baseline_and_only_are_passed() {
    let fs = setup(
        "baseline_err_when_baseline_and_only_are_passed",
        &[("a.js", TWO_DEBUGGERS)],
    );
    let mut console = BufferConsole::default();

    let result = run(
        &fs,
        &mut console,
        &["lint", "--baseline", "--only=suspicious"],
    );

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "err_when_baseline_and_only_are_passed",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn err_when_baseline_rule_is_unknown() {
    let fs = setup(
        "baseline_err_when_baseline_rule_is_unknown",
        &[("a.js", TWO_DEBUGGERS)],
    );
    let mut console = BufferConsole::default();

    let result = run(
        &fs,
        &mut console,
        &["lint", "--baseline-rule=suspicious/notARule"],
    );

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "err_when_baseline_rule_is_unknown",
        fs.create_mem(),
        console,
        result,
    ));
}

#[test]
fn err_when_baseline_is_malformed() {
    let fs = setup(
        "baseline_err_when_baseline_is_malformed",
        &[
            ("a.js", TWO_DEBUGGERS),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/suspicious/noDebugger": 2 } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_cli_snapshot(SnapshotPayload::new(
        module_path!(),
        "err_when_baseline_is_malformed",
        fs.create_mem(),
        console,
        result,
    ));
}

// Two GritQL plugins that report errors, so their violations can be told
// apart in the baseline.
const PLUGIN_CONFIG: &str = r#"{
    "plugins": ["./no-debugger.grit", "./no-alert.grit"],
    "linter": { "rules": { "preset": "none" } }
}"#;

const NO_DEBUGGER_PLUGIN: &str = r#"language js
`debugger` as $d where { register_diagnostic(span=$d, message="No debugger.", severity="error") }
"#;

const NO_ALERT_PLUGIN: &str = r#"language js
`alert($x)` as $a where { register_diagnostic(span=$a, message="No alert.", severity="error") }
"#;

fn setup_plugins(name: &str, files: &[(&str, &str)]) -> TemporaryFs {
    let mut fs = TemporaryFs::new(name);
    fs.create_file("biome.json", PLUGIN_CONFIG);
    fs.create_file("no-debugger.grit", NO_DEBUGGER_PLUGIN);
    fs.create_file("no-alert.grit", NO_ALERT_PLUGIN);
    for (path, content) in files {
        fs.create_file(path, content);
    }
    fs
}

#[test]
fn baseline_records_plugin_violations_by_plugin() {
    let fs = setup_plugins(
        "baseline_baseline_records_plugin_violations_by_plugin",
        &[("a.js", "debugger;\ndebugger;\nalert(1);\n")],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint", "--baseline"]);

    assert!(result.is_ok(), "run_cli returned {result:?}");
    assert_eq!(
        read_baseline(&fs),
        r#"{
  "a.js": {
    "lint/plugin/no-alert": {
      "count": 1
    },
    "lint/plugin/no-debugger": {
      "count": 2
    }
  }
}
"#
    );
}

#[test]
fn baseline_suppresses_recorded_plugin_violations() {
    let fs = setup_plugins(
        "baseline_baseline_suppresses_recorded_plugin_violations",
        &[
            ("a.js", "debugger;\nalert(1);\n"),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/plugin/no-debugger": { "count": 1 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    // Only no-alert is reported: the entry for no-debugger doesn't cover it.
    assert!(result.is_err(), "run_cli returned {result:?}");
    let output: String = console.out_buffer.iter().map(message_to_string).collect();
    assert!(output.contains("No alert."), "{output}");
    assert!(!output.contains("No debugger."), "{output}");
}

#[test]
fn baseline_reports_plugin_violations_over_count() {
    let fs = setup_plugins(
        "baseline_baseline_reports_plugin_violations_over_count",
        &[
            ("a.js", "debugger;\ndebugger;\n"),
            (
                "biome-baseline.json",
                r#"{ "a.js": { "lint/plugin/no-debugger": { "count": 1 } } }"#,
            ),
        ],
    );
    let mut console = BufferConsole::default();

    let result = run(&fs, &mut console, &["lint"]);

    assert!(result.is_err(), "run_cli returned {result:?}");
}

#[test]
fn baseline_rule_records_only_that_plugin() {
    let fs = setup_plugins(
        "baseline_baseline_rule_records_only_that_plugin",
        &[("a.js", "debugger;\nalert(1);\n")],
    );
    let mut console = BufferConsole::default();

    let result = run(
        &fs,
        &mut console,
        &["lint", "--baseline-rule=plugin/no-debugger"],
    );

    // no-alert isn't recorded, so it still fails the run.
    assert!(result.is_err(), "run_cli returned {result:?}");
    assert_eq!(
        read_baseline(&fs),
        r#"{
  "a.js": {
    "lint/plugin/no-debugger": {
      "count": 1
    }
  }
}
"#
    );
}
