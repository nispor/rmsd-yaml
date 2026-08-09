// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use pretty_assertions::assert_eq;

use crate::YamlParser;

const TEST_DATA_FOLDER_PATH: &str = "yaml-test-suit-data/name";
const DESCRIPTION_FILE_NAME: &str = "===";
const INPUT_YAML_FILE_NAME: &str = "in.yaml";
const TEST_EVENT_FILE_NAME: &str = "test.event";
const OUT_YAML_FILE_NAME: &str = "out.yaml";

/// The test cases enabled for the `out.yaml` test, grown one case at a
/// time (enable a case, run the test, fix failures as they appear).
///
/// The test uses the serde_yaml workflow: parse `in.yaml` into a
/// [`YamlValue`](crate::YamlValue), dump it back with
/// `YamlValue::to_string()`, and compare byte-identically with
/// `out.yaml`.
///
/// A `YamlValue` tree cannot reproduce every `out.yaml` (see
/// `DESIGNS.md`): anchors/aliases, the explicit `---`/`...` document
/// markers, block scalar (`|`/`>`) and quoted styles, and multiple
/// documents are lost by the value-based dump, and a number of
/// `out.yaml` files were hand-tuned with conventions that do not
/// round-trip. Only the cases that can be reproduced byte-identically
/// are enabled here.
const SUPPORTED_OUT_YAML_TEST: &[&str] = &[
    "allowed-characters-in-keys",
    "allowed-characters-in-plain-scalars",
    "backslashes-in-singlequotes",
    "blank-lines",
    "block-mapping-with-missing-values",
    "block-mapping-with-multiline-scalars",
    "block-mappings-in-block-sequence",
    "block-sequence-in-block-mapping",
    "colon-at-the-beginning-of-adjacent-flow-scalar",
    "comment-and-document-end-marker",
    "document-end-marker",
    "empty-implicit-key-in-single-pair-flow-sequences",
    "empty-lines-between-mapping-elements",
    "flow-mapping",
    "flow-mapping-edge-cases",
    "flow-mapping-in-block-sequence",
    "flow-sequence",
    "flow-sequence-in-block-mapping",
    "flow-sequence-in-flow-mapping",
    "flow-sequence-in-flow-sequence",
    "implicit-flow-mapping-key-on-one-line",
    "inline-tabs-in-double-quoted/01",
    "inline-tabs-in-double-quoted/02",
    "legal-tab-after-indentation",
    "mixed-block-mapping-explicit-to-implicit",
    "mixed-block-mapping-implicit-to-explicit",
    "multiline-scalar-in-mapping",
    "nested-top-level-flow-mapping",
    "plain-url-in-flow-mapping",
    "question-mark-edge-cases/00",
    "question-mark-edge-cases/01",
    "question-marks-in-scalars",
    "scalars-in-flow-start-with-syntax-char/00",
    "scalars-in-flow-start-with-syntax-char/01",
    "sequence-entry-that-looks-like-two-with-wrong-indentation",
    "sequence-indent",
    "single-character-streams/00",
    "single-character-streams/01",
    "spec-example-2-11-mapping-between-sequences",
    "spec-example-2-17-quoted-scalars",
    "spec-example-2-18-multi-line-flow-scalars",
    "spec-example-2-2-mapping-scalars-to-scalars",
    "spec-example-2-3-mapping-scalars-to-sequences",
    "spec-example-2-4-sequence-of-mappings",
    "spec-example-2-5-sequence-of-sequences",
    "spec-example-2-6-mapping-of-mappings",
    "spec-example-5-3-block-structure-indicators",
    "spec-example-5-4-flow-collection-indicators",
    "spec-example-5-5-comment-indicator",
    "spec-example-6-10-comment-lines",
    "spec-example-6-11-multi-line-comments",
    "spec-example-6-12-separation-spaces",
    "spec-example-6-2-indentation-indicators",
    "spec-example-6-24-verbatim-tags",
    "spec-example-6-3-separation-spaces",
    "spec-example-6-7-block-folding",
    "spec-example-6-8-flow-folding",
    "spec-example-6-8-flow-folding-1-3",
    "spec-example-6-9-separated-comment",
    "spec-example-7-11-plain-implicit-keys",
    "spec-example-7-13-flow-sequence",
    "spec-example-7-15-flow-mappings",
    "spec-example-7-16-flow-mapping-entries",
    "spec-example-7-19-single-pair-flow-mappings",
    "spec-example-7-2-empty-content",
    "spec-example-7-20-single-pair-explicit-entry",
    "spec-example-7-5-double-quoted-line-breaks",
    "spec-example-7-5-double-quoted-line-breaks-1-3",
    "spec-example-7-6-double-quoted-lines",
    "spec-example-7-6-double-quoted-lines-1-3",
    "spec-example-8-14-block-sequence",
    "spec-example-8-16-block-mappings",
    "spec-example-8-22-block-collection-nodes",
    "spec-example-8-7-literal-scalar-1-3",
    "spec-example-8-8-literal-content",
    "spec-example-8-8-literal-content-1-3",
    "tab-at-beginning-of-line-followed-by-a-flow-mapping",
    "tabs-in-various-contexts/002",
    "tabs-in-various-contexts/010",
    "tags-for-block-objects",
    "tags-for-flow-objects",
    "tags-in-block-sequence",
    "tags-in-explicit-mapping",
    "tags-in-implicit-mapping",
    "tags-on-empty-scalars",
    "three-dashes-and-content-without-space",
    "three-dashes-and-content-without-space-1-3",
    "trailing-spaces-after-flow-collection",
    "trailing-tabs-in-double-quoted/00",
    "trailing-tabs-in-double-quoted/01",
    "trailing-tabs-in-double-quoted/02",
    "trailing-tabs-in-double-quoted/03",
    "various-trailing-tabs",
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

/// Run the `out.yaml` comparison for the tests in `SUPPORTED_OUT_YAML_TEST`.
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
        if !SUPPORTED_OUT_YAML_TEST.iter().any(|t| {
            test_path_str.as_str() == *t
                || test_path_str.starts_with(&format!("{}/", t))
        }) {
            continue;
        }

        if !test_path.join(OUT_YAML_FILE_NAME).exists() {
            log::warn!(
                "Skipping test {test_path_str}: no {OUT_YAML_FILE_NAME}"
            );
            continue;
        }
        if test_path.join("error").exists() {
            // The test expects a parse error (an `error` file); there
            // is nothing to dump.
            log::warn!("Skipping test {test_path_str}: expects a parse error");
            continue;
        }

        log::trace!("====== {} ======", test_path_str);

        // The serde_yaml workflow: parse `in.yaml` into a `YamlValue`,
        // then dump it back to YAML and compare with `out.yaml`.
        // `to_value` is used instead of `from_str::<YamlValue>` because
        // the latter visits the value through serde, losing the raw
        // text of number-like scalars (e.g. `0x10`, `1e5`, `.inf`).
        let input_yaml = read_file(&test_path.join(INPUT_YAML_FILE_NAME));
        let expected_out = read_file(&test_path.join(OUT_YAML_FILE_NAME));
        let value = crate::to_value(&input_yaml).unwrap();
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
