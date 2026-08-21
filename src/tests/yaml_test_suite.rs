// SPDX-License-Identifier: Apache-2.0

use std::{path::Path, str::FromStr};

use pretty_assertions::assert_eq;

use crate::YamlParser;

const TEST_DATA_FOLDER_PATH: &str = "yaml-test-suit-data/name";
const DESCRIPTION_FILE_NAME: &str = "===";
const INPUT_YAML_FILE_NAME: &str = "in.yaml";
const TEST_EVENT_FILE_NAME: &str = "test.event";
const OUT_YAML_FILE_NAME: &str = "out.yaml";
const IN_JSON_FILE_NAME: &str = "in.json";

/// The test cases skipped for the `out.yaml` test. The test runs over
/// *every* case with an `out.yaml` and a successful parse, and only
/// these known-failing cases are skipped.
///
/// The test uses the serde_yaml workflow: parse `in.yaml` into a
/// [`Value`](crate::Value), dump it back with
/// `Value::to_string()`, and compare byte-identically with
/// `out.yaml`.
///
/// The skipped cases are the ones a `Value` tree cannot reproduce
/// (see `DESIGNS.md`): multi-document streams, and `out.yaml` files
/// hand-tuned with conventions that deviate from the canonical events
/// (extra blank lines, `---`/`...` not in the input, re-derived styles,
/// rejected tabs, the `-1-3` YAML-1.3 variants, block-scalar
/// indentation-hint deviations).
const SKIPPED_OUT_YAML_TEST: &[&str] = &[
    // multi-doc stream: compose() rejects >1 document
    "bare-document-after-document-end-marker",
    // out.yaml drops the input's `---` (hand-tuned)
    "block-scalar-keep",
    // out.yaml re-derives quoted keys to plain
    "colon-at-the-beginning-of-adjacent-flow-scalar",
    // multi-doc stream: compose() rejects >1 document
    "document-start-on-last-line",
    // out.yaml adds a `---` the input lacks
    "flow-collections-over-many-lines/01",
    // out.yaml renders empty values as `null` (contradicts
    // block-mapping-with-missing-values)
    "flow-mapping-separate-values",
    "literal-scalars", // out.yaml adds a `---` the input lacks
    // out.yaml has an extra blank line the events do not
    "multiline-plain-scalar-with-empty-line",
    // out.yaml has an extra blank line the events do not
    "multiline-plain-value-with-tabs-on-empty-lines",
    // out.yaml has an extra blank line (does not round-trip)
    "multiline-scalar-at-top-level",
    // out.yaml has an extra blank line (does not round-trip)
    "multiline-scalar-at-top-level-1-3",
    // out.yaml appends a `...` the input lacks
    "multiline-scalar-that-looks-like-a-yaml-directive",
    // out.yaml adds a `---` the input lacks
    "question-mark-at-start-of-flow-key",
    // out.yaml re-derives a double-quoted scalar to plain
    "scalar-doc-with-in-content/00",
    // multi-doc stream: compose() rejects >1 document
    "scalars-on-line",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-2-28-log-file",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-2-7-two-documents-in-a-stream",
    // 1.3 variant keeps `---` on its own line (not inline)
    "spec-example-6-13-reserved-directives-1-3",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-6-18-primary-tag-handle",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-6-18-primary-tag-handle-1-3",
    // 1.3 variant drops the input's `---`
    "spec-example-6-8-flow-folding-1-3",
    // out.yaml has an extra blank line the events do not
    "spec-example-7-12-plain-lines",
    // 1.3 variant drops the input's `---`
    "spec-example-7-5-double-quoted-line-breaks-1-3",
    // 1.3 variant drops the input's `---`
    "spec-example-7-6-double-quoted-lines-1-3",
    // out.yaml has an extra blank line the events do not
    "spec-example-7-9-single-quoted-lines",
    // 1.3 variant: extra blank line and dropped `---`
    "spec-example-7-9-single-quoted-lines-1-3",
    // out.yaml drops the indentation hint for a leading break
    "spec-example-8-10-folded-lines-8-13-final-empty-lines",
    // 1.3 variant: drops `---` and double-quotes tab content
    "spec-example-8-7-literal-scalar-1-3",
    // 1.3 variant drops the input's `---`
    "spec-example-8-8-literal-content-1-3",
    // 1.3 variant drops the input's `---`
    "spec-example-8-9-folded-scalar-1-3",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-9-4-explicit-documents",
    // multi-doc stream: compose() rejects >1 document
    "spec-example-9-5-directives-documents",
    // out.yaml appends a `...` the input lacks
    "tab-after-document-header",
    // parser rejects the tab in flow context
    "tabs-in-various-contexts/003",
    // parser rejects the tab after the block indicator
    "tabs-in-various-contexts/004",
    // parser rejects the tab after the block indicator
    "tabs-in-various-contexts/005",
    // parser rejects the tab after `?`
    "tabs-in-various-contexts/006",
    // parser rejects the tab after `?`
    "tabs-in-various-contexts/007",
    // parser rejects the tab after `?`
    "tabs-in-various-contexts/008",
    // parser rejects the tab after `?`
    "tabs-in-various-contexts/009",
    // multi-doc stream: compose() rejects >1 document
    "tags-for-root-objects",
    // 1.3 variant drops the input's `---`
    "three-dashes-and-content-without-space-1-3",
    // out.yaml drops the indentation hint for a leading break
    "trailing-whitespace-in-streams/00",
    // out.yaml drops the indentation hint for a leading break
    "trailing-whitespace-in-streams/01",
    // out.yaml drops the indentation hint for a leading break
    "trailing-whitespace-in-streams/02",
    // multi-doc stream: compose() rejects >1 document
    "two-document-start-markers",
    // multi-doc stream: compose() rejects >1 document
    "two-scalar-docs-with-trailing-comments",
    // multi-doc stream: compose() rejects >1 document
    "various-combinations-of-tags-and-anchors",
];

/// The test cases skipped for the `in.json` test. The test runs over
/// every case with an `in.json` and a successful parse, deserializing
/// `in.yaml` into a `serde_json::Value` and comparing it against the
/// JSON parsed from `in.json`.
///
/// The skipped cases all have an `in.json` that is not a single valid
/// JSON value, for one of two reasons:
///
/// * The case is a multi-document YAML stream, so `in.json` is several JSON
///   values concatenated back to back (one per document). This mirrors the
///   multi-doc entries already in `SKIPPED_OUT_YAML_TEST`, since `compose()`
///   likewise rejects more than one document.
/// * The case's `in.yaml` produces no document at all (e.g. an empty stream, or
///   a stream containing only comments/directives), so `in.json` is an empty
///   file with nothing to parse or compare.
const SKIPPED_IN_JSON_TEST: &[&str] = &[
    // multi-doc stream: in.json is several JSON values concatenated
    "bare-document-after-document-end-marker",
    "document-start-on-last-line",
    "scalars-on-line",
    "spec-example-2-28-log-file",
    "spec-example-2-7-two-documents-in-a-stream",
    "spec-example-2-8-play-by-play-feed-from-a-game",
    "spec-example-6-18-primary-tag-handle",
    "spec-example-6-18-primary-tag-handle-1-3",
    "spec-example-6-21-local-tag-prefix",
    "spec-example-9-3-bare-documents",
    "spec-example-9-4-explicit-documents",
    "spec-example-9-5-directives-documents",
    "spec-example-9-6-stream",
    "spec-example-9-6-stream-1-3",
    "tags-for-root-objects",
    "two-document-start-markers",
    "two-scalar-docs-with-trailing-comments",
    "various-combinations-of-tags-and-anchors",
    // empty stream: in.json is empty, there is no document to compare
    "comment-and-document-end-marker",
    "document-end-marker",
    "empty-stream",
    "spec-example-5-5-comment-indicator",
    "spec-example-6-10-comment-lines",
];

/// Collect the test directories under `test_data_dir`. A test directory is
/// either a direct child with a `===` description file, or a subdirectory
/// (for tests with multiple subtests).
fn discover_test_paths(
    test_data_dir: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let mut test_paths: Vec<std::path::PathBuf> = Vec::new();

    for entry in std::fs::read_dir(test_data_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if path.join(DESCRIPTION_FILE_NAME).exists() {
                test_paths.push(path);
            } else {
                for dir_entry in std::fs::read_dir(&path).unwrap() {
                    let entry = dir_entry.unwrap();
                    let path = entry.path();
                    if path.join(DESCRIPTION_FILE_NAME).exists() {
                        test_paths.push(path);
                    }
                }
            }
        }
    }
    test_paths.sort_unstable();
    test_paths
}

/// Run the `test.event` comparison for every test case in the suite.
#[test]
fn yaml_test_suit_test_event() {
    super::testlib::init_logger();

    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(TEST_DATA_FOLDER_PATH);

    let test_paths = discover_test_paths(&test_data_dir);
    let total_test_count = test_paths.len();
    let mut tested = 0;

    for test_path in test_paths.into_iter() {
        let test_path_str = test_path
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();

        let input_yaml = read_file(&test_path.join(INPUT_YAML_FILE_NAME));
        let expected_events = read_file(&test_path.join(TEST_EVENT_FILE_NAME));

        log::trace!(
            "====== {:03}/{total_test_count:03} {} ======",
            tested + 1,
            test_path_str,
        );
        run_event_parser_test(
            &input_yaml,
            &expected_events,
            test_path.join("error").exists(),
        );
        tested += 1;
    }
    log::info!("Tested {tested}/{total_test_count}");
}

/// Run the `out.yaml` comparison for every case except
/// `SKIPPED_OUT_YAML_TEST`.
#[test]
fn yaml_test_suit_out_yaml() {
    super::testlib::init_logger();

    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(TEST_DATA_FOLDER_PATH);

    let test_paths = discover_test_paths(&test_data_dir);
    let mut tested = 0;

    for test_path in test_paths.into_iter() {
        let test_path_str = test_path
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();
        if SKIPPED_OUT_YAML_TEST.iter().any(|t| {
            test_path_str.as_str() == *t
                || test_path_str.starts_with(&format!("{}/", t))
        }) {
            continue;
        }

        if !test_path.join(OUT_YAML_FILE_NAME).exists() {
            log::info!(
                "Skipping test {test_path_str}: no {OUT_YAML_FILE_NAME}"
            );
            continue;
        }
        if test_path.join("error").exists() {
            // The test expects a parse error (an `error` file); there
            // is nothing to dump.
            log::info!("Skipping test {test_path_str}: expects a parse error");
            continue;
        }

        log::trace!("====== {} ======", test_path_str);

        // The serde_yaml workflow: parse `in.yaml` into a `Value`,
        // then dump it back to YAML and compare with `out.yaml`.
        // `Value::from_str` is used instead of `from_str::<Value>` because
        // the latter visits the value through serde, losing the raw
        // text of number-like scalars (e.g. `0x10`, `1e5`, `.inf`).
        let input_yaml = read_file(&test_path.join(INPUT_YAML_FILE_NAME));
        let expected_out = read_file(&test_path.join(OUT_YAML_FILE_NAME));
        let value = crate::Value::from_str(&input_yaml).unwrap();
        let got_out = value.to_string().unwrap();
        pretty_assertions::assert_eq!(
            expected_out,
            got_out,
            "out.yaml mismatch for {test_path_str}",
        );

        tested += 1;
    }
    log::info!("Tested {tested} {OUT_YAML_FILE_NAME} tests");
}

/// Run the `in.json` comparison for every case except
/// `SKIPPED_IN_JSON_TEST`.
#[test]
fn yaml_test_suit_in_json() {
    super::testlib::init_logger();

    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(TEST_DATA_FOLDER_PATH);

    let test_paths = discover_test_paths(&test_data_dir);
    let mut tested = 0;

    for test_path in test_paths.into_iter() {
        let test_path_str = test_path
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();
        if SKIPPED_IN_JSON_TEST.iter().any(|t| {
            test_path_str.as_str() == *t
                || test_path_str.starts_with(&format!("{}/", t))
        }) {
            continue;
        }

        if !test_path.join(IN_JSON_FILE_NAME).exists() {
            log::info!("Skipping test {test_path_str}: no {IN_JSON_FILE_NAME}");
            continue;
        }
        if test_path.join("error").exists() {
            // The test expects a parse error (an `error` file); there
            // is nothing to deserialize.
            log::info!("Skipping test {test_path_str}: expects a parse error");
            continue;
        }

        log::trace!("====== {} ======", test_path_str);

        // Deserialize `in.yaml` through serde into a `serde_json::Value`
        // (exercising the real `Deserializer` impl and its type
        // inference), then compare against the JSON parsed from
        // `in.json`.
        let input_yaml = read_file(&test_path.join(INPUT_YAML_FILE_NAME));
        let expected_json = read_file(&test_path.join(IN_JSON_FILE_NAME));
        let expected: serde_json::Value = serde_json::from_str(&expected_json)
            .unwrap_or_else(|e| {
                panic!(
                    "{test_path_str}: {IN_JSON_FILE_NAME} is not valid JSON: \
                     {e}"
                )
            });
        let got: serde_json::Value = crate::from_str(&input_yaml)
            .unwrap_or_else(|e| {
                panic!("{test_path_str}: failed to deserialize in.yaml: {e}")
            });
        assert!(
            json_value_eq(&expected, &got),
            "in.json mismatch for {test_path_str}: expected {expected:?}, got \
             {got:?}",
        );

        tested += 1;
    }
    log::info!("Tested {tested} {IN_JSON_FILE_NAME} tests");
}

/// Deep-compare two JSON values, treating numbers as equal whenever
/// their `f64` representation matches. This is needed because
/// `serde_json::Value`'s derived `PartialEq` distinguishes an
/// integer-shaped `Number` from a float-shaped one even when they are
/// mathematically equal (e.g. `450 != 450.0`), while a YAML float
/// scalar like `450.00` is only guaranteed to round-trip to the
/// correct magnitude, not to a specific JSON number representation.
fn json_value_eq(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value;

    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter().zip(b.iter()).all(|(a, b)| json_value_eq(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, v)| {
                    b.get(k).is_some_and(|bv| json_value_eq(v, bv))
                })
        }
        (a, b) => a == b,
    }
}

fn run_event_parser_test(
    input_yaml: &str,
    expected_events: &str,
    is_error: bool,
) {
    let result = YamlParser::parse_to_events(input_yaml);

    log::trace!("Input YAML:\n{}", input_yaml);

    if is_error {
        assert!(result.is_err());
    } else {
        log::trace!("Expected events:\n{}", expected_events);
        let mut events_str = String::new();
        for event in result.unwrap() {
            events_str.push_str(&event.to_string());
            events_str.push('\n');
        }
        log::trace!("Parsed events:\n{}", events_str);
        assert_eq!(expected_events, events_str);
    }
}

fn read_file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}
