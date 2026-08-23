// SPDX-License-Identifier: Apache-2.0

use crate::YamlParser;

/// The scalar values of a stream, in event order.
fn scalar_values(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    for event in YamlParser::parse_to_events(input).unwrap() {
        if let crate::YamlEvent::Scalar(_, _, v, _, _, _) = event {
            out.push(v);
        }
    }
    out
}

#[test]
fn test_explicit_mapping_keys() {
    // `? key` with the value on the next `: ` line.
    let input = "? a\n: 1.3\nfifteen: d\n";
    assert_eq!(scalar_values(input), vec!["a", "1.3", "fifteen", "d"]);
    // Explicit key with an empty value.
    let input = "? explicit key # Empty value\n";
    assert_eq!(scalar_values(input), vec!["explicit key", ""]);
    // `? : x`: the key is the compact single-pair map `{: x}` and the
    // explicit value is empty (yaml-test-suite:
    // question-mark-edge-cases/00).
    let input = "- ? : x\n";
    assert_eq!(scalar_values(input), vec!["", "x", ""]);
    // Tagged explicit key.
    let input = "? !!str a\n: !!int 47\n";
    let events = YamlParser::parse_to_events(input).unwrap();
    let tags: Vec<Option<String>> = events
        .iter()
        .filter_map(|e| match e {
            crate::YamlEvent::Scalar(_, tag, _, _, _, _) => Some(tag.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        tags,
        vec![
            Some("<tag:yaml.org,2002:str>".into()),
            Some("<tag:yaml.org,2002:int>".into()),
        ]
    );
}

#[test]
fn test_flow_explicit_and_empty_keys() {
    // Explicit entry with an empty value plus an empty-key entry
    // (YAML 1.2.2 SPEC, 7.4.2, productions [143] and [146]).
    let input = "{ ? foo :, : bar }";
    assert_eq!(scalar_values(input), vec!["foo", "", "", "bar"]);
    // A tag decorating an empty flow mapping value.
    let events = YamlParser::parse_to_events("{ foo : !!str }").unwrap();
    let tagged: Vec<(Option<String>, String)> = events
        .iter()
        .filter_map(|e| match e {
            crate::YamlEvent::Scalar(_, tag, v, _, _, _) => {
                Some((tag.clone(), v.clone()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        tagged,
        vec![
            (None, "foo".to_string()),
            (Some("<tag:yaml.org,2002:str>".into()), String::new()),
        ]
    );
    // Empty key with a value.
    assert_eq!(scalar_values("{ : bar }"), vec!["", "bar"]);
    // A lone `?` is an explicit entry with empty key and empty value.
    assert_eq!(scalar_values("{ ? }"), vec!["", ""]);
    assert_eq!(scalar_values("{ ? , a: b }"), vec!["", "", "a", "b"]);
    // Entries with an omitted value.
    assert_eq!(scalar_values("{ ? foo }"), vec!["foo", ""]);
    assert_eq!(scalar_values("{a:}"), vec!["a", ""]);
    // A `:` followed by a plain-safe character stays in the key scalar.
    assert_eq!(scalar_values("{a:1}"), vec!["a:1", ""]);
    // Empty key and explicit entries in single-pair flow sequences.
    assert_eq!(scalar_values("[ : v ]"), vec!["", "v"]);
    assert_eq!(scalar_values("[ ? ]"), vec!["", ""]);
    assert_eq!(scalar_values("[ ? k ]"), vec!["k", ""]);
    assert_eq!(scalar_values("[ ? k : v ]"), vec!["k", "v"]);
}

#[test]
fn test_flow_empty_entry_rejected() {
    // A bare `,` starts no entry: an entry must begin with a `?`
    // indicator, a key node or a `:` (empty key) (YAML 1.2.2 SPEC,
    // 7.4.2, productions [138], [142] and [144]).
    assert!(YamlParser::parse_to_events("{ , }").is_err());
    assert!(YamlParser::parse_to_events("{,}").is_err());
}

#[test]
fn test_flow_explicit_and_empty_key_values() {
    use crate::{Value, from_str};
    let value: Value = from_str("{ ? foo :, : bar }").unwrap();
    let map = value.as_mapping().unwrap();
    assert_eq!(map.len(), 2);
    let foo_key: Value = from_str("foo\n").unwrap();
    let bar: Value = from_str("bar\n").unwrap();
    // The empty-key entry.
    assert_eq!(map.get(&Value::default()), Some(&bar));
    // The explicit entry with an empty value.
    assert!(map.get(&foo_key).unwrap().is_null());
}

#[test]
fn test_anchors_on_empty_scalars() {
    let input = "- &a\n- a\n-\n  &c : &a\n";
    let values = scalar_values(input);
    assert_eq!(values, vec!["", "a", "", ""]);
    let events = YamlParser::parse_to_events(input).unwrap();
    let anchors: Vec<Option<String>> = events
        .iter()
        .filter_map(|e| match e {
            crate::YamlEvent::Scalar(a, _, _, _, _, _) => Some(a.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        anchors,
        vec![Some("a".into()), None, Some("c".into()), Some("a".into())]
    );
}

#[test]
fn test_quoted_mapping_keys() {
    let input = "\"foo\": 23\n'x': 24\n";
    assert_eq!(scalar_values(input), vec!["foo", "23", "x", "24"]);
    // Escaped newlines inside a double-quoted key are allowed.
    let input = "\"a\\nb\": 1\n";
    assert_eq!(scalar_values(input), vec!["a\nb", "1"]);
    // But a key spanning real source lines is not.
    assert!(YamlParser::parse_to_events("\"a\n b\": 1\n").is_err());
}

#[test]
fn test_tagged_mapping_keys() {
    let input = "!!str : value\n";
    assert_eq!(scalar_values(input), vec!["", "value"]);
}

#[test]
fn test_empty_documents_and_streams() {
    // An empty stream has no documents.
    assert_eq!(YamlParser::parse_to_events("").unwrap().len(), 2);
    // `---` alone is an empty document with an empty scalar.
    let values = scalar_values("---\n");
    assert_eq!(values, vec![""]);
    // `...` alone is an empty stream.
    assert_eq!(YamlParser::parse_to_events("...\n").unwrap().len(), 2);
}

#[test]
fn test_flow_collection_trailing_commas() {
    // A trailing comma is allowed.
    let input = "[ a, b, ]\n";
    assert_eq!(scalar_values(input), vec!["a", "b"]);
    // A double comma is not.
    assert!(YamlParser::parse_to_events("[ a, b, , ]\n").is_err());
    // A comment directly after a comma needs a space.
    assert!(YamlParser::parse_to_events("[ a,#c\n]\n").is_err());
}

#[test]
fn test_document_markers_in_quoted_scalars() {
    // A quoted scalar may not span a document marker.
    assert!(YamlParser::parse_to_events("\"\n---\n\"\n").is_err());
    assert!(YamlParser::parse_to_events("'\n...\n'\n").is_err());
}

#[test]
fn test_block_mapping_after_document_start_marker() {
    // `--- a: b` is an error; the mapping must start on its own line.
    assert!(YamlParser::parse_to_events("--- a: b\n").is_err());
    assert!(YamlParser::parse_to_events("---\na: b\n").is_ok());
}

#[test]
fn test_wrong_indented_sequence_item() {
    assert!(YamlParser::parse_to_events("- key: value\n - item1\n").is_err());
    // Same-indent entries are fine.
    assert!(YamlParser::parse_to_events("- a\n- b\n").is_ok());
}

#[test]
fn test_multiple_documents_rejected() {
    // Multi-document streams are not supported (see the
    // "Limitations" section in README.md); parsing one must fail
    // with `NoSupportMultipleDocuments` instead of silently
    // returning only the first document.
    use crate::{ErrorKind, Value, from_str};
    let err = from_str::<Value>("a: 1\n...\nb: 2\n").unwrap_err();
    assert_eq!(err.kind(), ErrorKind::NoSupportMultipleDocuments);
    // A single-document stream still works.
    let value = from_str::<Value>("- 1\n- 2\n").unwrap();
    assert_eq!(value, from_str::<Value>("[1, 2]").unwrap());
}

#[test]
fn test_double_quoted_escaped_line_break() {
    // `\` + line break is a line continuation (removed).
    assert_eq!(scalar_values("\"a\\\nb\"\n"), vec!["ab"]);
    // `\ ` is a space escape.
    assert_eq!(scalar_values("\"a\\ \nb\"\n"), vec!["a b"]);
}
