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
    "aliases-in-explicit-block-mapping",
    "aliases-in-flow-objects",
    "aliases-in-implicit-block-mapping",
    "allowed-characters-in-keys",
    "allowed-characters-in-plain-scalars",
    "allowed-characters-in-quoted-mapping-key",
    "anchor-before-zero-indented-sequence",
    "anchor-for-empty-node",
    "anchor-with-colon-in-the-middle",
    "anchor-with-unicode-character",
    "anchors-and-tags",
    "anchors-on-empty-scalars",
    "anchors-with-colon-in-name",
    "backslashes-in-singlequotes",
    "blank-lines",
    "block-mapping-with-missing-values",
    "block-mapping-with-multiline-scalars",
    "block-mappings-in-block-sequence",
    "block-scalar-indicator-order",
    "block-sequence-in-block-mapping",
    "block-sequence-indentation",
    "colon-and-adjacent-value-after-comment-on-next-line",
    "colon-and-adjacent-value-on-next-line",
    "colon-followed-by-comma",
    "comment-and-document-end-marker",
    "comment-in-flow-sequence-before-comma",
    "directive-variants/02",
    "directive-variants/03",
    "directive-variants/04",
    "directive-variants/05",
    "directive-variants/06",
    "document-end-marker",
    "doublequoted-scalar-starting-with-a-tab",
    "empty-flow-collections",
    "empty-implicit-key-in-single-pair-flow-sequences",
    "empty-lines-between-mapping-elements",
    "escaped-slash-in-double-quotes",
    "explicit-key-and-value-seperated-by-comment",
    "flow-mapping",
    "flow-mapping-edge-cases",
    "flow-mapping-in-block-sequence",
    "flow-sequence",
    "flow-sequence-in-block-mapping",
    "flow-sequence-in-flow-mapping",
    "flow-sequence-in-flow-sequence",
    "folded-block-scalar",
    "implicit-flow-mapping-key-on-one-line",
    "inline-tabs-in-double-quoted/01",
    "inline-tabs-in-double-quoted/02",
    "key-with-anchor-after-missing-explicit-mapping-value",
    "leading-tab-content-in-literals/00",
    "leading-tab-content-in-literals/01",
    "legal-tab-after-indentation",
    "literal-block-scalar",
    "literal-unicode",
    "mixed-block-mapping-explicit-to-implicit",
    "mixed-block-mapping-implicit-to-explicit",
    "multiline-double-quoted-flow-mapping-key",
    "multiline-doublequoted-flow-mapping-key-without-value",
    "multiline-plain-flow-mapping-key",
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
