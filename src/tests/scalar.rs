// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;

use crate::{YamlEvent, YamlParser, YamlPosition, YamlScalarStyle};

#[test]
fn test_block_scalar_literal_block_clip_auto() {
    super::testlib::init_logger();

    assert_eq!(
        YamlParser::parse_to_events("--- |\n abc \n def\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "abc \ndef\n".to_string(),
                YamlScalarStyle::Literal,
                YamlPosition::new(2, 2),
                YamlPosition::new(3, 5)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(3, 5)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_block_scalar_literal_block_clip_fixed_ident() {
    assert_eq!(
        YamlParser::parse_to_events("--- |3\n    abc \n    def\n   \n  \n")
            .unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                " abc \n def\n".to_string(),
                YamlScalarStyle::Literal,
                YamlPosition::new(2, 4),
                YamlPosition::new(5, 3),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(5, 3)),
            YamlEvent::StreamEnd,
        ]
    );
}

#[test]
fn test_block_scalar_literal_indentation_indicator_nine() {
    // Regression: the indentation indicator was matched with the
    // pattern `'1'..'9'`, an *exclusive* range in Rust, so `9` never
    // matched and was rejected with `ExpectingCommentOrLineBreak`
    // instead of being accepted as an explicit indentation of 9 (YAML
    // 1.2.2 SPEC 8.1.1.1 allows indentation indicators 1-9 inclusive).
    assert_eq!(
        YamlParser::parse_to_events("--- |9\n          abc\n          def\n")
            .unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                " abc\n def\n".to_string(),
                YamlScalarStyle::Literal,
                YamlPosition::new(2, 10),
                YamlPosition::new(3, 14),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(3, 14)),
            YamlEvent::StreamEnd,
        ]
    );
}

#[test]
fn test_block_scalar_literal_block_strip_fixed_ident() {
    let expected = vec![
        YamlEvent::StreamStart,
        YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
        YamlEvent::Scalar(
            None,
            None,
            " abc \n def".to_string(),
            YamlScalarStyle::Literal,
            YamlPosition::new(2, 4),
            YamlPosition::new(3, 8),
        ),
        YamlEvent::DocumentEnd(false, YamlPosition::new(3, 8)),
        YamlEvent::StreamEnd,
    ];
    assert_eq!(
        YamlParser::parse_to_events("--- |3-\n    abc \n    def\n").unwrap(),
        expected
    );
    assert_eq!(
        YamlParser::parse_to_events("--- |-3\n    abc \n    def\n").unwrap(),
        expected
    );
}

#[test]
fn test_block_scalar_literal_block_keep_fixed_ident() {
    let expected = vec![
        YamlEvent::StreamStart,
        YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
        YamlEvent::Scalar(
            None,
            None,
            " abc \n def  \n\n\n".to_string(),
            YamlScalarStyle::Literal,
            YamlPosition::new(2, 4),
            YamlPosition::new(5, 1),
        ),
        YamlEvent::DocumentEnd(false, YamlPosition::new(5, 1)),
        YamlEvent::StreamEnd,
    ];
    assert_eq!(
        YamlParser::parse_to_events("--- |3+\n    abc \n    def  \n   \n\n")
            .unwrap(),
        expected
    );
    assert_eq!(
        YamlParser::parse_to_events("--- |+3\n    abc \n    def  \n   \n\n")
            .unwrap(),
        expected
    );
}

#[test]
fn test_block_scalar_literal_all_indented() {
    assert_eq!(
        YamlParser::parse_to_events("---\n   |\n   abc\n   def\n\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "abc\ndef\n".to_string(),
                YamlScalarStyle::Literal,
                YamlPosition::new(3, 4),
                YamlPosition::new(5, 1)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(5, 1)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_plain_scalar_folding() {
    assert_eq!(
        YamlParser::parse_to_events(
            "1st non-empty\n\n 2nd non-empty \n\t3rd non-empty"
        )
        .unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "1st non-empty\n2nd non-empty 3rd non-empty".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(4, 14)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(4, 14)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_double_quoted_scalar() {
    assert_eq!(
        YamlParser::parse_to_events("\"\n  foo \n \n  \tbar\n\n  baz\n \"")
            .unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                " foo\nbar\nbaz ".to_string(),
                YamlScalarStyle::DoubleQuoted,
                YamlPosition::new(1, 1),
                YamlPosition::new(7, 2)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(7, 2)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_block_folding_scalar_simple() {
    assert_eq!(
        YamlParser::parse_to_events(">\n folded\n text\n\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "folded text\n".to_string(),
                YamlScalarStyle::Folded,
                YamlPosition::new(2, 1),
                YamlPosition::new(4, 1)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(4, 1)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_block_folding_scalar_more_indented() {
    assert_eq!(
        YamlParser::parse_to_events(">\n  foo \n \n  \t bar\n\n  baz\n")
            .unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "foo \n\n\t bar\n\nbaz\n".to_string(),
                YamlScalarStyle::Folded,
                YamlPosition::new(2, 1),
                YamlPosition::new(6, 6)
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(6, 6)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn crlf_block_scalar() {
    let ev =
        YamlParser::parse_to_events("a: |\r\n  line1\r\n  line2\r\n").unwrap();
    let scalars: Vec<String> = ev
        .iter()
        .filter_map(|e| match e {
            YamlEvent::Scalar(_, _, v, ..) => Some(v.clone()),
            _ => None,
        })
        .collect();
    let scalar = &scalars[1];
    assert_eq!(scalar, "line1\nline2\n", "got {scalar:?}");
    let ev2 =
        YamlParser::parse_to_events("a: >\r\n  line1\r\n  line2\r\n").unwrap();
    let s2v: Vec<String> = ev2
        .iter()
        .filter_map(|e| match e {
            YamlEvent::Scalar(_, _, v, ..) => Some(v.clone()),
            _ => None,
        })
        .collect();
    let s2 = &s2v[1];
    assert_eq!(s2, "line1 line2\n", "got {s2:?}");
}
