use super::*;

const RULE_A: &str = "lint/suspicious/noExplicitAny";
const RULE_B: &str = "lint/style/useConst";

fn counts(pairs: &[(&str, u32)]) -> RuleCounts {
    pairs
        .iter()
        .map(|(rule, count)| ((*rule).to_string(), *count))
        .collect()
}

fn baseline(json: &str) -> Baseline {
    Baseline::parse(json).unwrap()
}

#[test]
fn parses_and_serializes_sorted() {
    let baseline = baseline(
        r#"{ "src/z.ts": { "lint/b": { "count": 1 } }, "src/a.ts": { "lint/a": { "count": 2 } } }"#,
    );
    assert_eq!(
        baseline.to_json(),
        r#"{
  "src/a.ts": {
    "lint/a": {
      "count": 2
    }
  },
  "src/z.ts": {
    "lint/b": {
      "count": 1
    }
  }
}
"#
    );
}

#[test]
fn rejects_unknown_entry_fields() {
    assert!(Baseline::parse(r#"{ "a.ts": { "lint/a": { "count": 1, "extra": 1 } } }"#).is_err());
}

#[test]
fn suppresses_when_within_count() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    let outcome = baseline.evaluate("a.ts", &counts(&[(RULE_A, 3)]));
    assert_eq!(outcome.suppressed, vec![RULE_A.to_string()]);
    assert!(outcome.unused.is_empty());
}

#[test]
fn reports_all_when_over_count() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    let outcome = baseline.evaluate("a.ts", &counts(&[(RULE_A, 4)]));
    assert!(outcome.suppressed.is_empty());
    assert!(outcome.unused.is_empty());
}

#[test]
fn reports_unused_when_under_count() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    let outcome = baseline.evaluate("a.ts", &counts(&[(RULE_A, 1)]));
    assert_eq!(outcome.suppressed, vec![RULE_A.to_string()]);
    assert_eq!(outcome.unused, counts(&[(RULE_A, 2)]));
}

#[test]
fn reports_unused_when_rule_is_gone() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    let outcome = baseline.evaluate("a.ts", &RuleCounts::new());
    assert!(outcome.suppressed.is_empty());
    assert_eq!(outcome.unused, counts(&[(RULE_A, 3)]));
}

#[test]
fn ignores_files_and_rules_not_in_baseline() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    assert_eq!(
        baseline.evaluate("b.ts", &counts(&[(RULE_A, 1)])),
        BaselineOutcome::default()
    );
    let outcome = baseline.evaluate("a.ts", &counts(&[(RULE_A, 3), (RULE_B, 5)]));
    assert_eq!(outcome.suppressed, vec![RULE_A.to_string()]);
}

#[test]
fn record_matches_current_violations() {
    let mut baseline = baseline(
        r#"{
            "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 }, "lint/style/useConst": { "count": 1 } },
            "c.ts": { "lint/style/useConst": { "count": 4 } }
        }"#,
    );

    baseline.record("a.ts", &counts(&[(RULE_A, 5)]), None);
    baseline.record("b.ts", &counts(&[(RULE_B, 2)]), None);

    assert_eq!(
        baseline,
        self::baseline(
            r#"{
                "a.ts": { "lint/suspicious/noExplicitAny": { "count": 5 } },
                "b.ts": { "lint/style/useConst": { "count": 2 } },
                "c.ts": { "lint/style/useConst": { "count": 4 } }
            }"#
        )
    );
}

#[test]
fn record_removes_file_without_violations() {
    let mut baseline = baseline(r#"{ "a.ts": { "lint/style/useConst": { "count": 1 } } }"#);

    baseline.record("a.ts", &RuleCounts::new(), None);

    assert!(baseline.is_empty());
}

#[test]
fn record_only_given_rules() {
    let mut baseline = baseline(
        r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 }, "lint/style/useConst": { "count": 1 } } }"#,
    );

    baseline.record("a.ts", &counts(&[(RULE_A, 5)]), Some(&[RULE_B.to_string()]));

    assert_eq!(
        baseline,
        self::baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#)
    );
}

#[test]
fn prune_lowers_counts_and_drops_empty_entries() {
    let mut baseline = baseline(
        r#"{
            "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 }, "lint/style/useConst": { "count": 1 } },
            "b.ts": { "lint/style/useConst": { "count": 2 } }
        }"#,
    );

    baseline.prune("a.ts", &counts(&[(RULE_A, 1)]));
    baseline.prune("b.ts", &RuleCounts::new());

    assert_eq!(
        baseline,
        self::baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 1 } } }"#)
    );
}

#[test]
fn prune_never_raises_counts() {
    let mut baseline =
        baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 2 } } }"#);

    baseline.prune("a.ts", &counts(&[(RULE_A, 5)]));

    assert_eq!(
        baseline,
        self::baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 2 } } }"#)
    );
}

#[test]
fn evaluate_with_record_suppresses_everything_recorded() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 1 } } }"#);
    let violations = counts(&[(RULE_A, 4), (RULE_B, 2)]);

    let outcome =
        baseline.evaluate_with_mode("a.ts", &violations, &BaselineMode::Record { rules: None });
    assert_eq!(
        outcome.suppressed,
        vec![RULE_B.to_string(), RULE_A.to_string()]
    );
    assert!(outcome.unused.is_empty());

    let outcome = baseline.evaluate_with_mode(
        "a.ts",
        &violations,
        &BaselineMode::Record {
            rules: Some(vec![RULE_B.to_string()]),
        },
    );
    assert_eq!(outcome.suppressed, vec![RULE_B.to_string()]);
}

#[test]
fn evaluate_with_prune_reports_no_unused() {
    let baseline = baseline(r#"{ "a.ts": { "lint/suspicious/noExplicitAny": { "count": 3 } } }"#);

    let outcome =
        baseline.evaluate_with_mode("a.ts", &counts(&[(RULE_A, 1)]), &BaselineMode::Prune);
    assert_eq!(outcome.suppressed, vec![RULE_A.to_string()]);
    assert!(outcome.unused.is_empty());
}

#[test]
fn relative_paths_use_forward_slashes() {
    assert_eq!(
        relative_path(Utf8Path::new("/repo"), Utf8Path::new("/repo/src/a.ts")).as_deref(),
        Some("src/a.ts")
    );
    assert_eq!(
        relative_path(Utf8Path::new("/repo"), Utf8Path::new("/other/a.ts")),
        None
    );
}
