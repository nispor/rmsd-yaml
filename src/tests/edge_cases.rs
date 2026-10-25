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
fn test_lone_explicit_key_indicator() {
    // A `?` indicator alone on its line: the key is empty and the
    // value follows on the `: ` line at the mapping's key column.
    let input = "?\n: x\n";
    assert_eq!(scalar_values(input), vec!["", "x"]);
    // Comment and empty lines between the indicator and the value.
    let input = "? # c\n: v\n";
    assert_eq!(scalar_values(input), vec!["", "v"]);
    let input = "?\n\n: v\n";
    assert_eq!(scalar_values(input), vec!["", "v"]);
    // A nested lone `?` keeps the outer value on its own line.
    let input = "? ? \n  : x\n:\n";
    assert_eq!(scalar_values(input), vec!["", "x", ""]);
    // A deeper block node after a lone `?` is the key node.
    let input = "?\n  k: v\n: y\n";
    assert_eq!(scalar_values(input), vec!["k", "v", "y"]);
    let input = "? ?\n    k: v\n: y\n";
    assert_eq!(scalar_values(input), vec!["k", "v", "", "y"]);
    // A block sequence at the mapping's indentation is the key
    // (yaml-test-suite:
    // zero-indented-sequences-in-explicit-mapping-keys).
    let input = "?\n- a\n- b\n:\n- c\n- d\n";
    assert_eq!(scalar_values(input), vec!["a", "b", "c", "d"]);
    // Same inside a block sequence entry.
    let input = "- ? \n  : v\n";
    assert_eq!(scalar_values(input), vec!["", "v"]);
    // Anchors and tags keep decorating the empty key.
    let input = "? &a\n: v\n";
    assert_eq!(scalar_values(input), vec!["", "v"]);
    let input = "? !!str\n: v\n";
    assert_eq!(scalar_values(input), vec!["", "v"]);
    // A lone `?` at end of input is a complete empty entry.
    assert_eq!(scalar_values("?\n"), vec!["", ""]);
}

#[test]
fn test_explicit_key_dump_roundtrip() {
    use crate::{Value, from_str};
    // The value dumper renders null-keyed maps as explicit entries;
    // the dumped form must re-parse to the same value.
    for input in ["? : x\n", "[ : v ]\n", "? []: x\n", "? ? a\n: b\n"] {
        let value: Value = from_str(input).unwrap();
        let dumped = value.to_string().unwrap();
        assert_eq!(
            from_str::<Value>(&dumped).unwrap(),
            value,
            "round-trip of {input:?} via {dumped:?}"
        );
    }
}

#[test]
fn test_implicit_key_length_limit() {
    // An implicit key must not span more than 1024 Unicode characters
    // (YAML 1.2.2 SPEC, productions [154]/[155], Example 7.22).
    let key_1024 = "a".repeat(1024);
    let key_1025 = "a".repeat(1025);
    // Block mapping keys (plain, quoted, anchored).
    assert!(YamlParser::parse_to_events(&format!("{key_1024}: v\n")).is_ok());
    assert!(YamlParser::parse_to_events(&format!("{key_1025}: v\n")).is_err());
    assert!(YamlParser::parse_to_events(&format!("{key_1025}:\n")).is_err());
    assert!(
        YamlParser::parse_to_events(&format!("\"{}\": v\n", "a".repeat(1022)))
            .is_ok()
    );
    assert!(
        YamlParser::parse_to_events(&format!("\"{}\": v\n", "a".repeat(1023)))
            .is_err()
    );
    assert!(
        YamlParser::parse_to_events(&format!(
            "&k {key_1023}: v\n",
            key_1023 = "a".repeat(1023)
        ))
        .is_err()
    );
    // Indentation does not count toward the limit.
    assert!(
        YamlParser::parse_to_events(&format!("k:\n  {key_1024}: v\n")).is_ok()
    );
    assert!(
        YamlParser::parse_to_events(&format!("k:\n  {key_1025}: v\n")).is_err()
    );
    // Flow mapping keys.
    assert!(YamlParser::parse_to_events(&format!("{{{key_1024}: v}}")).is_ok());
    assert!(
        YamlParser::parse_to_events(&format!("{{{key_1025}: v}}")).is_err()
    );
    // Flow sequence single-pair keys.
    assert!(YamlParser::parse_to_events(&format!("[ {key_1024}: v ]")).is_ok());
    assert!(
        YamlParser::parse_to_events(&format!("[ {key_1025}: v ]")).is_err()
    );
    // Multi-byte characters count as characters, not bytes.
    let mb_1024 = "\u{e9}".repeat(1024);
    let mb_1025 = "\u{e9}".repeat(1025);
    assert!(YamlParser::parse_to_events(&format!("{mb_1024}: v\n")).is_ok());
    assert!(YamlParser::parse_to_events(&format!("{mb_1025}: v\n")).is_err());
    // Explicit keys are exempt from the limit.
    assert!(
        YamlParser::parse_to_events(&format!("? {key_1025}\n: v\n")).is_ok()
    );
    assert!(
        YamlParser::parse_to_events(&format!("{{ ? {key_1025} : v }}")).is_ok()
    );
    // A long scalar that is not a key and a long value stay valid.
    assert!(
        YamlParser::parse_to_events(&format!("{}\n", "a".repeat(2000))).is_ok()
    );
    assert!(
        YamlParser::parse_to_events(&format!("k: {}\n", "a".repeat(2000)))
            .is_ok()
    );
}

#[test]
fn test_line_break_normalization_in_scalars() {
    // YAML 1.2.2 SPEC, 5.4 (productions [28]/[29]): CRLF and CR
    // inside scalar content are normalized to LF before line folding.
    assert_eq!(scalar_values("k: 'a\r\n b'\n"), vec!["k", "a b"]);
    assert_eq!(scalar_values("k: 'a\r b'\n"), vec!["k", "a b"]);
    assert_eq!(scalar_values("[ a\r\nb ]\n"), vec!["a b"]);
    assert_eq!(scalar_values("[ a\rb ]\n"), vec!["a b"]);
    // Two normalized breaks fold to a line feed.
    assert_eq!(scalar_values("k: 'a\r\n\r\n b'\n"), vec!["k", "a\nb"]);
    assert_eq!(scalar_values("[ a\r\n\r\nb ]\n"), vec!["a\nb"]);
    // A continuation line of a double- or single-quoted scalar in block
    // context must be indented one space past the parent collection,
    // for every line break flavor (YAML 1.2.2 SPEC, 7.3: both styles
    // route continuation lines through `s-flow-folded(n)` [74] ->
    // `s-flow-line-prefix(n)` [69], with `n` from production [197];
    // yaml-test-suite: wrong-indented-multiline-quoted-scalar).
    // libyaml accepts these, folding them to `a b`; that is a libyaml
    // bug and this rejection is an intentional, spec-mandated
    // divergence.
    for src in [
        "k: \"a\nb\"\n",
        "k: \"a\r\nb\"\n",
        "k: \"a\rb\"\n",
        "k: 'a\nb'\n",
        "k: 'a\r\nb'\n",
        "k: 'a\rb'\n",
        "---\nquoted: \"a\nb\nc\"\n",
        "---\nquoted: 'a\nb\nc'\n",
        "- \"a\nb\"\n",
        "- 'a\nb'\n",
    ] {
        assert!(
            YamlParser::parse_to_events(src).is_err(),
            "expected an error for {src:?}"
        );
    }
    // Indented continuation lines stay valid for both styles and all
    // break flavors; a whitespace-only line is exempt from the
    // indentation floor (production [70]).
    assert_eq!(scalar_values("k: \"a\n b\"\n"), vec!["k", "a b"]);
    assert_eq!(scalar_values("k: 'a\n b'\n"), vec!["k", "a b"]);
    assert_eq!(scalar_values("k: \"a\r\n b\"\n"), vec!["k", "a b"]);
    assert_eq!(scalar_values("k: 'a\n\n b'\n"), vec!["k", "a\nb"]);
}

#[test]
fn test_unicode_nfc_nfd_preserved() {
    use crate::{Value, from_str};
    // YAML performs no Unicode normalization: NFC and NFD forms of the
    // same grapheme stay distinct keys (YAML 1.2.2 SPEC, 3.2.1.3:
    // scalar equality is character-by-character on the canonical
    // form).
    let value: Value = from_str("caf\u{e9}: 1\ncafe\u{301}: 2\n").unwrap();
    let map = value.as_mapping().unwrap();
    assert_eq!(map.len(), 2);
    // Content is preserved verbatim.
    let nfc: Value = from_str("caf\u{e9}\n").unwrap();
    let nfd: Value = from_str("cafe\u{301}\n").unwrap();
    assert_eq!(nfc.as_str().unwrap(), "caf\u{e9}");
    assert_eq!(nfd.as_str().unwrap(), "cafe\u{301}");
    assert_ne!(nfc, nfd);
    // Round-trip keeps the exact code points.
    let dumped = nfd.to_string().unwrap();
    assert_eq!(from_str::<Value>(&dumped).unwrap(), nfd);
    // NEL, LS and PS are YAML 1.2.2 content characters, not line
    // breaks (5.1, production [27]).
    assert_eq!(scalar_values("k: a\u{85}b\n"), vec!["k", "a\u{85}b"]);
    assert_eq!(scalar_values("k: a\u{2028}b\n"), vec!["k", "a\u{2028}b"]);
    assert_eq!(scalar_values("k: a\u{2029}b\n"), vec!["k", "a\u{2029}b"]);
    // Surrogate escapes are not valid Unicode scalar values.
    assert!(YamlParser::parse_to_events("k: \"\\uD800\"\n").is_err());
    assert!(YamlParser::parse_to_events("k: \"\\uD83D\\uDE00\"\n").is_err());
}

#[test]
fn test_nested_explicit_mapping_keys() {
    // A nested `?` starts a block mapping whose entries sit at the
    // nested indicator's column; the outer value follows on its own
    // `: ` line (yaml-test-suite style, libyaml compatible).
    let input = "? ? a\n: b\n";
    assert_eq!(scalar_values(input), vec!["a", "", "b"]);
    // The nested entry takes its own value line at the nested column.
    let input = "? ? a\n  : b\n: c\n";
    assert_eq!(scalar_values(input), vec!["a", "b", "c"]);
    // Same inside a block sequence entry.
    let input = "- ? ? a\n    : b\n  : c\n";
    assert_eq!(scalar_values(input), vec!["a", "b", "c"]);
    // A nested compact mapping as the nested explicit key.
    let input = "? ? []: y\n: z\n";
    assert_eq!(scalar_values(input), vec!["y", "", "z"]);
    // A same-line nested compact mapping.
    let input = "? ? a : b\n";
    assert_eq!(scalar_values(input), vec!["a", "b", "", ""]);
}

#[test]
fn test_explicit_value_line_indentation() {
    // The explicit value line must sit at the mapping's key column
    // (YAML 1.2.2 SPEC, 8.2.2, production [191]).
    assert!(YamlParser::parse_to_events("? a\n  : b\n").is_err());
    assert!(YamlParser::parse_to_events("? k\n : v\n").is_err());
    assert!(YamlParser::parse_to_events("- ? a\n: b\n").is_err());
    // Correct indentation keeps working.
    assert!(YamlParser::parse_to_events("- ? a\n  : b\n").is_ok());
    assert!(YamlParser::parse_to_events("? a\n: b\n").is_ok());
}

#[test]
fn test_explicit_key_same_line_value_is_compact_mapping() {
    // A same-line `: value` after a quoted key forms a compact
    // single-pair mapping used as the entry key (YAML 1.2.2 SPEC,
    // 8.2.2, Example 8.19); the entry value stays empty.
    let input = "? \"q\" : v\n";
    assert_eq!(scalar_values(input), vec!["q", "v", ""]);
    // The entry's own value may follow on a `: ` line.
    let input = "? \"q\" : v\n: w\n";
    assert_eq!(scalar_values(input), vec!["q", "v", "w"]);
    // Same for an alias or a trailing-colon plain key.
    let input = "? *a : v\n";
    let events = YamlParser::parse_to_events(input).unwrap();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, crate::YamlEvent::Alias(a, _) if a == "a"))
    );
    let input = "? a:\n";
    assert_eq!(scalar_values(input), vec!["a", "", ""]);
    // A comment between the key and the value line keeps the regular
    // explicit value semantics.
    let input = "? a: # c\n: v\n";
    assert_eq!(scalar_values(input), vec!["a", "", "v"]);
    let input = "? \"q\" # c\n: v\n";
    assert_eq!(scalar_values(input), vec!["q", "v"]);
    // A block scalar key keeps the regular explicit value.
    let input = "? |-\n  k\n: v\n";
    assert_eq!(scalar_values(input), vec!["k", "v"]);
}

#[test]
fn test_properties_on_explicit_compact_key() {
    use crate::YamlEvent;
    // Node properties written on the `?` indicator line decorate the
    // first key node of the compact mapping that the key content forms,
    // not the compact mapping itself: production [202]
    // (`ns-l-compact-mapping`) has no node properties slot, so `? &a k
    // : v` anchors the `k` scalar (libyaml compatible) and an alias
    // `*a` refers to `k`, not to the `{k: v}` mapping used as the key.
    let map_anchors = |input: &str| -> Vec<Option<String>> {
        YamlParser::parse_to_events(input)
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                YamlEvent::MapStart(anchor, _, _, _) => Some(anchor),
                _ => None,
            })
            .collect()
    };
    let scalars = |input: &str| -> Vec<(Option<String>, String)> {
        YamlParser::parse_to_events(input)
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                YamlEvent::Scalar(anchor, _, value, _, _, _) => {
                    Some((anchor, value))
                }
                _ => None,
            })
            .collect()
    };
    // The anchor belongs to the plain-scalar key.
    assert_eq!(map_anchors("? &a k : v\n"), vec![None, None]);
    assert_eq!(
        scalars("? &a k : v\n"),
        vec![
            (Some("a".to_string()), "k".to_string()),
            (None, "v".to_string()),
            (None, String::new()),
        ]
    );
    // A tag behaves like an anchor.
    let tags: Vec<Option<String>> =
        YamlParser::parse_to_events("? !!str k : v\n")
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                YamlEvent::MapStart(tag, _, _, _)
                | YamlEvent::Scalar(_, tag, _, _, _, _) => Some(tag),
                _ => None,
            })
            .collect();
    assert_eq!(
        tags,
        vec![
            None,
            None,
            Some("<tag:yaml.org,2002:str>".to_string()),
            None,
            None,
        ]
    );
    // A flow collection key carries the properties itself.
    let seq_anchors = |input: &str| -> Vec<Option<String>> {
        YamlParser::parse_to_events(input)
            .unwrap()
            .into_iter()
            .filter_map(|e| match e {
                YamlEvent::SequenceStart(anchor, _, _, _) => Some(anchor),
                _ => None,
            })
            .collect()
    };
    assert_eq!(map_anchors("? &a [x] : v\n"), vec![None, None]);
    assert_eq!(seq_anchors("? &a [x] : v\n"), vec![Some("a".to_string())]);
    assert_eq!(
        map_anchors("? &a {x: y} : v\n"),
        vec![None, None, Some("a".to_string())]
    );
    // The empty key of a compact mapping is anchored too.
    assert_eq!(
        scalars("? &a : x\n"),
        vec![
            (Some("a".to_string()), String::new()),
            (None, "x".to_string()),
            (None, String::new()),
        ]
    );
    // The same rules apply inside block sequences and nested explicit
    // keys.
    assert_eq!(
        scalars("- ? &a k : v\n  : w\n"),
        vec![
            (Some("a".to_string()), "k".to_string()),
            (None, "v".to_string()),
            (None, "w".to_string()),
        ]
    );
    assert_eq!(
        scalars("? ? &a k: v\n: w\n"),
        vec![
            (Some("a".to_string()), "k".to_string()),
            (None, "v".to_string()),
            (None, String::new()),
            (None, "w".to_string()),
        ]
    );
    // The alias resolves to the anchored scalar `k`.
    let value: crate::Value = crate::from_str("? &a k : v\n: *a\n").unwrap();
    let map = value.as_mapping().unwrap();
    let (key, val) = map.iter().next().unwrap();
    assert_eq!(key.as_mapping().unwrap().len(), 1);
    assert_eq!(val.as_str().unwrap(), "k");
}

#[test]
fn test_comment_directly_after_colon() {
    // An inline comment right after `: ` ends the value: a sibling
    // key on the next line is not swallowed as the value.
    let input = "a: # c\nb: 2\n";
    assert_eq!(scalar_values(input), vec!["a", "", "b", "2"]);
    // A deeper-indented node after the comment is the value.
    let input = "a: # c\n  b: 2\n";
    assert_eq!(scalar_values(input), vec!["a", "b", "2"]);
    // A zero-indented block sequence is a valid value.
    let input = "a: # c\n- x\n";
    assert_eq!(scalar_values(input), vec!["a", "x"]);
    // Node properties before the comment keep their anchor.
    let input = "a: &x # c\nb: 2\n";
    assert_eq!(scalar_values(input), vec!["a", "", "b", "2"]);
    let input = "a: &x # c\n  v\n";
    assert_eq!(scalar_values(input), vec!["a", "v"]);
}

#[test]
fn test_double_quoted_escaped_line_break() {
    // `\` + line break is a line continuation (removed).
    assert_eq!(scalar_values("\"a\\\nb\"\n"), vec!["ab"]);
    // `\ ` is a space escape.
    assert_eq!(scalar_values("\"a\\ \nb\"\n"), vec!["a b"]);
}

#[test]
fn test_document_marker_only_at_line_start() {
    // An indented `...` is block scalar content, not a document end
    // marker: the kernel stack traces stored in the CVE records have a
    // `...` line followed by trace lines starting with `[`.
    let input = "description: |-\n  ...\n  [  134.222]  foo+0x64\n";
    assert_eq!(
        scalar_values(input),
        vec!["description", "...\n[  134.222]  foo+0x64"]
    );
    // Same for a block scalar in a sequence and for the folded style.
    let input = "- |-\n  x\n  ...\n  y\n";
    assert_eq!(scalar_values(input), vec!["x\n...\ny"]);
    let input = "k: |\n  x\n  ---\n  y\n";
    assert_eq!(scalar_values(input), vec!["k", "x\n---\ny\n"]);
    // An indented `...` continues a plain scalar as content.
    let input = "k: a\n  ...\n  b\n";
    assert_eq!(scalar_values(input), vec!["k", "a ... b"]);
    let input = "key:\n  ...\n";
    assert_eq!(scalar_values(input), vec!["key", "..."]);
    // A scalar that starts mid-line is never a marker.
    let input = "key: ...\n";
    assert_eq!(scalar_values(input), vec!["key", "..."]);
}

#[test]
fn test_document_marker_at_line_start_ends_document() {
    // A `...` at column 0 still ends the document, even when it
    // follows an open block scalar.
    let value: crate::Value = crate::from_str("k: |-\n  x\n...\n").unwrap();
    assert_eq!(value.get("k").unwrap().as_str().unwrap(), "x");
    // An empty document terminated by `...` stays empty.
    let value: crate::Value = crate::from_str("---\n...\n").unwrap();
    assert!(value.data == crate::ValueData::Null);
}

#[test]
fn test_flow_mapping_value_indicator_before_collection() {
    // A `:` in a flow collection separates the key from the value when
    // it is followed by any flow indicator, not only `,`/`]`/`}`. This
    // keeps JSON-like `{a:[1, 2]}` and `{a:{b: c}}` parseable (YAML
    // 1.2.2 SPEC, production [147]: `c-ns-flow-map-separate-value`
    // requires the character after `:` to not be `ns-plain-safe`, and
    // `ns-plain-safe(FLOW-IN)` excludes `[` and `{`).
    let value: crate::Value = crate::from_str("{a:[1, 2]}").unwrap();
    let a = value.get("a").unwrap();
    assert_eq!(
        a.as_sequence().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>(),
        vec!["1".to_string(), "2".to_string()]
    );

    let value: crate::Value = crate::from_str("{a:{b: c}}").unwrap();
    let a = value.get("a").unwrap();
    assert_eq!(
        a.as_mapping().unwrap().get(&"b".into()).unwrap().as_str().unwrap(),
        "c"
    );

    // Same for the flow-sequence single-pair form.
    let value: crate::Value = crate::from_str("[a:{b: c}]").unwrap();
    let entry = &value.as_sequence().unwrap()[0];
    assert_eq!(
        entry.as_mapping().unwrap().get(&"a".into()).unwrap().as_mapping().unwrap()
            .get(&"b".into()).unwrap().as_str().unwrap(),
        "c"
    );
}

#[test]
fn test_document_end_marker_with_trailing_whitespace() {
    // A document end marker may be followed by separation spaces (and
    // an optional comment) before the line break (YAML 1.2.2 SPEC,
    // productions [204]/[205]). Trailing spaces used to make `...`
    // parse as a plain scalar.
    let value: crate::Value = crate::from_str("...   \n").unwrap();
    assert!(value.data == crate::ValueData::Null);

    let value: crate::Value = crate::from_str("a: 1\n...\t\n").unwrap();
    assert_eq!(value.get("a").unwrap().as_str().unwrap(), "1");

    // The marker also terminates an open block scalar.
    let value: crate::Value = crate::from_str("k: |-\n  x\n...   \n").unwrap();
    assert_eq!(value.get("k").unwrap().as_str().unwrap(), "x");
}

#[test]
fn test_document_start_marker_only_at_column_zero() {
    // A `---` placed inside a line is ordinary content, not a
    // document marker (YAML 1.2.2 SPEC, 9.1.2).
    assert_eq!(scalar_values("a --- b\n"), vec!["a --- b"]);
    assert_eq!(
        scalar_values("key: --- value\n"),
        vec!["key", "--- value"]
    );

    // `---#` has no separation after the marker, so it is a plain
    // scalar (production [206] only forbids a marker followed by
    // white space, a line break or end of input).
    assert_eq!(scalar_values("---#x\n"), vec!["---#x"]);
    assert_eq!(scalar_values("a\n---#x\n"), vec!["a ---#x"]);
    assert_eq!(scalar_values("a\n\n---#x\n"), vec!["a\n---#x"]);
}

#[test]
fn test_indented_directive_is_rejected() {
    // Directives must sit on a non-indented line (YAML 1.2.2 SPEC,
    // 6.8); an indented `%YAML` used to be accepted as a directive.
    for input in ["  %YAML 1.2\n---\nx\n", "\t%YAML 1.2\n---\nx\n"] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), crate::ErrorKind::InvalidDirective);
    }
    // A directive at column 0 still works.
    let value: crate::Value = crate::from_str("%YAML 1.2\n---\nx\n").unwrap();
    assert_eq!(value.as_str().unwrap(), "x");
}

#[test]
fn test_yaml_directive_major_version() {
    // A version 1.2 processor must reject documents declaring a
    // different major version (YAML 1.2.2 SPEC, 6.8.1). Higher minor
    // versions and 1.1 must still be accepted.
    let err = YamlParser::parse_to_events("%YAML 2.0\n---\nx\n").unwrap_err();
    assert_eq!(err.kind(), crate::ErrorKind::InvalidDirective);
    let err = YamlParser::parse_to_events("%YAML 0.9\n---\nx\n").unwrap_err();
    assert_eq!(err.kind(), crate::ErrorKind::InvalidDirective);

    for version in ["1.1", "1.2", "1.3"] {
        let input = format!("%YAML {version}\n---\nx\n");
        let value: crate::Value = crate::from_str(&input).unwrap();
        assert_eq!(value.as_str().unwrap(), "x");
    }
}

#[test]
fn test_block_alias_cannot_carry_node_properties() {
    // An alias denotes a reference and may not be anchored or tagged
    // (YAML 1.2.2 SPEC, 6.9.2). The block path used to silently drop
    // the properties.
    for input in [
        "a: &x 1\nb: !t *x\n",
        "x: &a 1\ny: !<tag:yaml.org,2002:str> *a\n",
        "!t *b\n",
    ] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), crate::ErrorKind::InvalidAnchor, "{input:?}");
    }
}

#[test]
fn test_inline_comments_do_not_affect_block_structure() {
    // An inline comment is a presentation detail: a `: ` inside it
    // must not turn a scalar into a mapping or a mapping value into a
    // nested mapping (YAML 1.2.2 SPEC, 7.1).
    assert_eq!(scalar_values("a # b: c\n"), vec!["a"]);
    assert_eq!(scalar_values("- a # b: c\n"), vec!["a"]);
    assert_eq!(scalar_values("a: 1 # b: c\n"), vec!["a", "1"]);
    assert_eq!(
        scalar_values("a: > # b: c\n  text\n"),
        vec!["a", "text\n"]
    );
    assert_eq!(scalar_values("? a # b: c\n: 1\n"), vec!["a", "1"]);
    assert_eq!(scalar_values("--- a # b: c\n"), vec!["a"]);

    // A `#` inside a quoted scalar is content, not a comment.
    assert_eq!(scalar_values("'a # b': c\n"), vec!["a # b", "c"]);
    assert_eq!(scalar_values("\"a # b\": c\n"), vec!["a # b", "c"]);
}

#[test]
fn test_indented_plain_scalar_indicator_start() {
    // A plain scalar may be indented at the document root; the
    // indicator lookahead must use the scalar's first character, not
    // the indentation spaces (YAML 1.2.2 SPEC, 7.3.3, `ns-plain-first`).
    assert_eq!(scalar_values("  :foo\n"), vec![":foo"]);
    assert_eq!(scalar_values("  ?foo\n"), vec!["?foo"]);
    assert_eq!(scalar_values("  -foo\n"), vec!["-foo"]);
    assert_eq!(scalar_values("  ---\n"), vec!["---"]);
    assert_eq!(scalar_values("  --- text\n"), vec!["--- text"]);

    // A real `: ` after the indentation still starts a mapping with an
    // empty key.
    let value: crate::Value = "  : foo\n".parse().unwrap();
    let map = value.as_mapping().unwrap();
    assert_eq!(map.len(), 1);
    assert_eq!(map.iter().next().unwrap().1.as_str().unwrap(), "foo");
}

#[test]
fn test_raw_non_printable_characters_are_rejected() {
    // YAML streams may only contain the printable character subset
    // (YAML 1.2.2 SPEC, 5.1); raw C0/C1 controls (other than
    // tab/LF/CR/NEL), DEL, U+FFFE and U+FFFF must be rejected.
    for input in [
        "a: \u{0}\n",
        "a: \u{1}\n",
        "a: \u{7f}\n",
        "a: \u{80}\n",
        "a: \u{9f}\n",
        "a: \u{fffe}\n",
        "a: \u{ffff}\n",
    ] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), crate::ErrorKind::InvalidCharacter, "{input:?}");
    }

    // Printable non-ASCII, NEL, tab and astral characters stay
    // accepted.
    for input in [
        "a: \t\n",
        "a: \u{85}\n",
        "a: \u{a0}\n",
        "a: \u{1F600}\n",
    ] {
        assert!(YamlParser::parse_to_events(input).is_ok(), "{input:?}");
    }

    // A raw forbidden character can still be expressed with an escape
    // inside a quoted scalar.
    let value: crate::Value = "\"\\x01\"\n".parse().unwrap();
    assert_eq!(value.as_str().unwrap(), "\u{1}");

    // A leading stream BOM is not content, so it must not shift the
    // reported error column.
    let err = YamlParser::parse_to_events("\u{feff}a: \u{0}\n").unwrap_err();
    assert_eq!(err.kind(), crate::ErrorKind::InvalidCharacter);
    assert_eq!(err.start_pos(), crate::YamlPosition::new(1, 4));
}

#[test]
fn test_block_scalar_header_accepts_tab_separation() {
    // The block scalar header is followed by `s-b-comment`, whose
    // separation is `s-separate-in-line` (spaces or tabs), so a tab
    // before the line break or a comment is valid (YAML 1.2.2 SPEC,
    // productions [66]/[77]/[162]).
    assert_eq!(scalar_values("|\t\n text\n"), vec!["text\n"]);
    assert_eq!(scalar_values("| \t# comment\n text\n"), vec!["text\n"]);
    assert_eq!(scalar_values("|2\t# comment\n  text\n"), vec!["text\n"]);
}
