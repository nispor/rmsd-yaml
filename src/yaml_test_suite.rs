// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use pretty_assertions::assert_eq;

use crate::YamlParser;

const TEST_DATA_FOLDER_PATH: &str = "yaml-test-suit-data/name";
const DESCRIPTION_FILE_NAME: &str = "===";
const INPUT_YAML_FILE_NAME: &str = "in.yaml";
const TEST_EVENT_FILE_NAME: &str = "test.event";

#[test]
fn yaml_test_suit() {
    crate::testlib::init_logger();

    #[rustfmt::skip]
        let supported_tests: &[&str] = &[
//            "aliases-in-block-sequence",
//            "aliases-in-explicit-block-mapping",
//            "aliases-in-flow-objects",
//            "aliases-in-implicit-block-mapping",
//            "allowed-characters-in-alias",
//            "allowed-characters-in-keys",
//            "allowed-characters-in-plain-scalars",
//            "allowed-characters-in-quoted-mapping-key",
//            "anchor-and-alias-as-mapping-key",
//            "anchor-before-sequence-entry-on-same-line",
//            "anchor-before-zero-indented-sequence",
//            "anchor-for-empty-node",
//            "anchor-plus-alias",
//            "anchor-with-colon-in-the-middle",
//            "anchor-with-unicode-character",
//            "anchors-and-tags",
//            "anchors-in-mapping",
//            "anchors-on-empty-scalars",
//            "anchors-with-colon-in-name",
//            "backslashes-in-singlequotes",
//            "bad-indentation-in-mapping",
//            "bad-indentation-in-mapping-2",
//            "bare-document-after-document-end-marker",
            "blank-lines",
//            "block-mapping-with-missing-keys",
//            "block-mapping-with-missing-values",
//            "block-mapping-with-multiline-scalars",
//            "block-mappings-in-block-sequence",
//            "block-scalar-indicator-order",
//            "block-scalar-keep",
//            "block-scalar-strip",
//            "block-scalar-strip-1-3",
//            "block-scalar-with-more-spaces-than-first-content-line",
//            "block-scalar-with-wrong-indented-line-after-spaces-only",
//            "block-sequence-in-block-mapping",
//            "block-sequence-in-block-sequence",
//            "block-sequence-indentation",
//            "block-submapping",
//            "colon-and-adjacent-value-after-comment-on-next-line",
//            "colon-and-adjacent-value-on-next-line",
//            "colon-at-the-beginning-of-adjacent-flow-scalar",
//            "colon-followed-by-comma",
//            "colon-in-double-quoted-string",
//            "comment-and-document-end-marker",
//            "comment-between-plain-scalar-lines",
//            "comment-in-flow-sequence-before-comma",
//            "comment-in-plain-multiline-value",
//            "comment-that-looks-like-a-mapping-key",
//            "comment-without-whitespace-after-block-scalar-indicator",
//            "comment-without-whitespace-after-doublequoted-scalar",
//            "construct-binary",
//            "dash-in-flow-sequence",
//            "directive-by-itself-with-no-document",
//            "directive-variants",
//            "directive-without-document",
//            "document-end-marker",
//            "document-start-on-last-line",
//            "document-with-footer",
//            "double-quoted-scalar-with-escaped-single-quote",
//            "double-quoted-string-without-closing-quote",
//            "doublequoted-scalar-starting-with-a-tab",
//            "duplicate-yaml-directive",
//            "empty-flow-collections",
//            "empty-implicit-key-in-single-pair-flow-sequences",
//            "empty-keys-in-block-and-flow-mapping",
//            "empty-lines-at-end-of-document",
//            "empty-lines-between-mapping-elements",
//            "empty-stream",
//            "escaped-slash-in-double-quotes",
//            "explicit-key-and-value-seperated-by-comment",
//            "explicit-non-specific-tag",
//            "explicit-non-specific-tag-1-3",
//            "extra-words-on-yaml-directive",
//            "flow-collections-over-many-lines",
//            "flow-mapping",
//            "flow-mapping-colon-on-line-after-key",
//            "flow-mapping-edge-cases",
//            "flow-mapping-in-block-sequence",
//            "flow-mapping-key-on-two-lines",
//            "flow-mapping-missing-a-separating-comma",
//            "flow-mapping-separate-values",
//            "flow-sequence",
//            "flow-sequence-in-block-mapping",
//            "flow-sequence-in-flow-mapping",
//            "flow-sequence-in-flow-sequence",
//            "flow-sequence-with-invalid-comma-at-the-beginning",
//            "folded-block-scalar",
//            "flow-sequence-with-invalid-extra-closing-bracket",
//            "flow-sequence-with-invalid-extra-comma",
//            "flow-sequence-without-closing-bracket",
//            "folded-block-scalar-1-3",
//            "implicit-flow-mapping-key-on-one-line",
//            "implicit-key-followed-by-newline",
//            "implicit-key-followed-by-newline-and-adjacent-value",
//            "inline-tabs-in-double-quoted",
//            "invalid-anchor-in-zero-indented-sequence",
//            "invalid-block-mapping-key-on-same-line-as-previous-key",
//            "invalid-comma-in-tag",
//            "invalid-comment-after-comma",
//            "invalid-comment-after-end-of-flow-sequence",
//            "invalid-content-after-document-end-marker",
//            "invalid-document-end-marker-in-single-quoted-string",
//            "invalid-document-markers-in-flow-style",
//            "invalid-document-start-marker-in-doublequoted-tring",
//            "invalid-escape-in-double-quoted-string",
//            "invalid-item-after-end-of-flow-sequence",
//            "invalid-mapping-after-sequence",
//            "invalid-mapping-in-plain-multiline",
//            "invalid-mapping-in-plain-scalar",
//            "invalid-mapping-in-plain-single-line-value",
//            "invalid-nested-mapping",
//            "invalid-scalar-after-sequence",
//            "invalid-scalar-at-the-end-of-mapping",
//            "invalid-scalar-at-the-end-of-sequence",
//            "invalid-sequene-item-on-same-line-as-previous-item",
//            "invalid-tabs-as-indendation-in-a-mapping",
//            "invalid-tag",
//            "invalid-text-after-block-scalar-indicator",
//            "invalid-value-after-mapping",
//            "key-with-anchor-after-missing-explicit-mapping-value",
//            "leading-tab-content-in-literals",
//            "leading-tabs-in-double-quoted",
//            "legal-tab-after-indentation",
//            "literal-block-scalar",
//            "literal-block-scalar-with-more-spaces-in-first-line",
//            "literal-modifers",
//            "literal-scalars",
//            "literal-unicode",
//            "lookahead-test-cases",
//            "mapping-key-and-flow-sequence-item-anchors",
//            "mapping-starting-at-line",
//            "mapping-with-anchor-on-document-start-line",
//            "missing-colon",
//            "missing-comma-in-flow",
//            "missing-document-end-marker-before-directive",
//            "mixed-block-mapping-explicit-to-implicit",
//            "mixed-block-mapping-implicit-to-explicit",
//            "more-indented-lines-at-the-beginning-of-folded-block-scalars",
//            "multi-level-mapping-indent",
//            "multiline-double-quoted-flow-mapping-key",
//            "multiline-double-quoted-implicit-keys",
//            "multiline-doublequoted-flow-mapping-key-without-value",
//            "multiline-implicit-keys",
//            "multiline-plain-flow-mapping-key",
//            "multiline-plain-flow-mapping-key-without-value",
//            "multiline-plain-scalar-with-empty-line",
//            "multiline-plain-value-with-tabs-on-empty-lines",
//            "multiline-scalar-at-top-level",
//            "multiline-scalar-at-top-level-1-3",
//            "multiline-scalar-in-mapping",
//            "multiline-scalar-that-looks-like-a-yaml-directive",
//            "multiline-single-quoted-implicit-keys",
//            "multiline-unidented-double-quoted-block-key",
            "multiple-entry-block-sequence",
            "multiple-pair-block-mapping",
//            "need-document-footer-before-directives",
//            "nested-flow-collections",
//            "nested-flow-collections-on-one-line",
//            "nested-flow-mapping-sequence-and-mappings",
//            "nested-implicit-complex-keys",
//            "nested-top-level-flow-mapping",
//            "node-anchor-and-tag-on-seperate-lines",
//            "node-anchor-in-sequence",
//            "node-anchor-not-indented",
//            "node-and-mapping-key-anchors",
//            "node-and-mapping-key-anchors-1-3",
//            "non-specific-tags-on-scalars",
//            "scalars-on-line",
//            "plain-dashes-in-flow-sequence",
//            "plain-mapping-key-ending-with-colon",
//            "plain-scalar-looking-like-key-comment-anchor-and-tag",
//            "plain-scalar-with-backslashes",
//            "plain-url-in-flow-mapping",
//            "question-mark-at-start-of-flow-key",
//            "question-mark-edge-cases",
//            "question-marks-in-scalars",
//            "scalar-doc-with-in-content",
//            "scalar-value-with-two-anchors",
//            "scalars-in-flow-start-with-syntax-char",
//            "sequence-entry-that-looks-like-two-with-wrong-indentation",
//            "sequence-indent",
//            "sequence-on-same-line-as-mapping-key",
//            "sequence-with-same-indentation-as-parent-mapping",
            "simple-mapping-indent",
//            "single-block-sequence-with-anchor",
//            "single-block-sequence-with-anchor-and-explicit-document-start",
            "single-character-streams",
//            "single-entry-block-sequence",
//            "single-pair-block-mapping",
//            "single-pair-implicit-entries",
//            "spec-example-2-1-sequence-of-scalars",
//            "spec-example-2-10-node-for-sammy-sosa-appears-twice-\
//                in-this-document",
//            "spec-example-2-11-mapping-between-sequences",
//            "spec-example-2-12-compact-nested-mapping",
//            "spec-example-2-13-in-literals-newlines-are-preserved",
//            "spec-example-2-14-in-the-folded-scalars-newlines-become-spaces",
//            "spec-example-2-15-folded-newlines-are-preserved-for-\
//                more-indented-and-blank-lines",
//            "spec-example-2-16-indentation-determines-scope",
//            "spec-example-2-17-quoted-scalars",
//            "spec-example-2-18-multi-line-flow-scalars",
//            "spec-example-2-2-mapping-scalars-to-scalars",
//            "spec-example-2-24-global-tags",
//            "spec-example-2-25-unordered-sets",
//            "spec-example-2-26-ordered-mappings",
//            "spec-example-2-27-invoice",
//            "spec-example-2-28-log-file",
//            "spec-example-2-3-mapping-scalars-to-sequences",
//            "spec-example-2-4-sequence-of-mappings",
//            "spec-example-2-5-sequence-of-sequences",
//            "spec-example-2-6-mapping-of-mappings",
//            "spec-example-2-7-two-documents-in-a-stream",
//            "spec-example-2-8-play-by-play-feed-from-a-game",
//            "spec-example-2-9-single-document-with-two-comments",
//            "spec-example-5-12-tabs-and-spaces",
//            "spec-example-5-3-block-structure-indicators",
//            "spec-example-5-4-flow-collection-indicators",
//            "spec-example-5-5-comment-indicator",
//            "spec-example-5-6-node-property-indicators",
//            "spec-example-5-7-block-scalar-indicators",
//            "spec-example-5-8-quoted-scalar-indicators",
//            "spec-example-5-9-directive-indicator",
//            "spec-example-6-1-indentation-spaces",
//            "spec-example-6-10-comment-lines",
//            "spec-example-6-11-multi-line-comments",
//            "spec-example-6-12-separation-spaces",
//            "spec-example-6-13-reserved-directives",
//            "spec-example-6-13-reserved-directives-1-3",
//            "spec-example-6-14-yaml-directive",
//            "spec-example-6-16-tag-directive",
//            "spec-example-6-18-primary-tag-handle",
//            "spec-example-6-18-primary-tag-handle-1-3",
//            "spec-example-6-19-secondary-tag-handle",
//            "spec-example-6-2-indentation-indicators",
//            "spec-example-6-20-tag-handles",
//            "spec-example-6-21-local-tag-prefix",
//            "spec-example-6-22-global-tag-prefix",
//            "spec-example-6-23-node-properties",
//            "spec-example-6-24-verbatim-tags",
//            "spec-example-6-26-tag-shorthands",
//            "spec-example-6-28-non-specific-tags",
//            "spec-example-6-29-node-anchors",
//            "spec-example-6-3-separation-spaces",
//            "spec-example-6-4-line-prefixes",
//            "spec-example-6-5-empty-lines",
//            "spec-example-6-5-empty-lines-1-3",
//            "spec-example-6-6-line-folding",
//            "spec-example-6-6-line-folding-1-3",
//            "spec-example-6-7-block-folding",
//            "spec-example-6-8-flow-folding",
//            "spec-example-6-8-flow-folding-1-3",
//            "spec-example-6-9-separated-comment",
//            "spec-example-7-1-alias-nodes",
//            "spec-example-7-10-plain-characters",
//            "spec-example-7-11-plain-implicit-keys",
//            "spec-example-7-12-plain-lines",
//            "spec-example-7-13-flow-sequence",
//            "spec-example-7-14-flow-sequence-entries",
//            "spec-example-7-15-flow-mappings",
//            "spec-example-7-16-flow-mapping-entries",
//            "spec-example-7-18-flow-mapping-adjacent-values",
//            "spec-example-7-19-single-pair-flow-mappings",
//            "spec-example-7-2-empty-content",
//            "spec-example-7-20-single-pair-explicit-entry",
//            "spec-example-7-23-flow-content",
//            "spec-example-7-24-flow-nodes",
//            "spec-example-7-3-completely-empty-flow-nodes",
//            "spec-example-7-4-double-quoted-implicit-keys",
//            "spec-example-7-5-double-quoted-line-breaks",
//            "spec-example-7-5-double-quoted-line-breaks-1-3",
//            "spec-example-7-6-double-quoted-lines",
//            "spec-example-7-6-double-quoted-lines-1-3",
//            "spec-example-7-7-single-quoted-characters",
//            "spec-example-7-7-single-quoted-characters-1-3",
//            "spec-example-7-8-single-quoted-implicit-keys",
//            "spec-example-7-9-single-quoted-lines",
//            "spec-example-7-9-single-quoted-lines-1-3",
//            "spec-example-8-1-block-scalar-header",
//            "spec-example-8-10-folded-lines-8-13-final-empty-lines",
//            "spec-example-8-14-block-sequence",
//            "spec-example-8-15-block-sequence-entry-types",
//            "spec-example-8-16-block-mappings",
//            "spec-example-8-17-explicit-block-mapping-entries",
//            "spec-example-8-18-implicit-block-mapping-entries",
//            "spec-example-8-19-compact-block-mappings",
//            "spec-example-8-2-block-indentation-indicator",
//            "spec-example-8-2-block-indentation-indicator-1-3",
//            "spec-example-8-20-block-node-types",
//            "spec-example-8-21-block-scalar-nodes",
//            "spec-example-8-21-block-scalar-nodes-1-3",
//            "spec-example-8-22-block-collection-nodes",
//            "spec-example-8-4-chomping-final-line-break",
//            "spec-example-8-5-chomping-trailing-lines",
//            "spec-example-8-6-empty-scalar-chomping",
//            "spec-example-8-7-literal-scalar",
//            "spec-example-8-7-literal-scalar-1-3",
//            "spec-example-8-8-literal-content",
//            "spec-example-8-8-literal-content-1-3",
//            "spec-example-8-9-folded-scalar",
//            "spec-example-8-9-folded-scalar-1-3",
//            "spec-example-9-2-document-markers",
//            "spec-example-9-3-bare-documents",
//            "spec-example-9-4-explicit-documents",
//            "spec-example-9-5-directives-documents",
//            "spec-example-9-6-stream",
//            "spec-example-9-6-stream-1-3",
//            "syntax-character-edge-cases",
//            "tab-after-document-header",
//            "tab-at-beginning-of-line-followed-by-a-flow-mapping",
//            "tab-indented-top-flow",
//            "tabs-in-various-contexts",
//            "tabs-that-look-like-indentation",
//            "tag-shorthand-used-in-documents-but-only-defined-in-the-first",
//            "tags-for-block-objects",
//            "tags-for-flow-objects",
//            "tags-for-root-objects",
//            "tags-in-block-sequence",
//            "tags-in-explicit-mapping",
//            "tags-in-implicit-mapping",
//            "tags-on-empty-scalars",
//            "three-dashes-and-content-without-space",
//            "three-dashes-and-content-without-space-1-3",
//            "three-explicit-integers-in-a-block-sequence",
//            "trailing-comment-in-multiline-plain-scalar",
//            "trailing-line-of-spaces",
//            "trailing-content-after-quoted-value",
//            "trailing-content-that-looks-like-a-mapping",
//            "trailing-spaces-after-flow-collection",
//            "trailing-tabs-in-double-quoted",
//            "trailing-whitespace-in-streams",
//            "two-document-start-markers",
//            "two-scalar-docs-with-trailing-comments",
//            "various-combinations-of-explicit-block-mappings",
//            "various-combinations-of-tags-and-anchors",
//            "various-empty-or-newline-only-quoted-strings",
//            "various-location-of-anchors-in-flow-sequence",
//            "various-trailing-comments",
//            "various-trailing-comments-1-3",
//            "various-trailing-tabs",
//            "whitespace-after-scalars-in-flow",
//            "whitespace-around-colon-in-mappings",
//            "wrong-indendation-in-map",
//            "wrong-indendation-in-mapping",
//            "wrong-indendation-in-sequence",
//            "wrong-indented-flow-sequence",
//            "wrong-indented-multiline-quoted-scalar",
//            "wrong-indented-sequence-item",
//            "yaml-directive-without-document-end-marker",
//            "zero-indented-block-scalar",
//            "zero-indented-block-scalar-with-line-that-looks-like-a-comment",
//            "zero-indented-sequences-in-explicit-mapping-keys",
        ];

    let test_data_dir =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(TEST_DATA_FOLDER_PATH);

    let mut test_paths: Vec<std::path::PathBuf> = Vec::new();

    for entry in std::fs::read_dir(&test_data_dir).unwrap() {
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
    let total_test_count = test_paths.len();
    let mut tested = 0;

    for test_path in test_paths.into_iter() {
        let test_path_str = test_path
            .strip_prefix(&test_data_dir)
            .unwrap()
            .display()
            .to_string();
        if !supported_tests.iter().any(|t| {
            test_path_str.as_str() == *t
                || test_path_str.starts_with(&format!("{}/", t))
        }) {
            log::warn!("Skipping test {test_path_str}");
            continue;
        }

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
