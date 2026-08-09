// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlParser};

/// Scalar events of a single-document stream, `(tag, value)` pairs.
fn scalar_events(input: &str) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    for event in YamlParser::parse_to_events(input).unwrap() {
        if let crate::YamlEvent::Scalar(_, tag, value, _, _, _) = event {
            out.push((tag, value));
        }
    }
    out
}

#[test]
fn test_default_tag_shorthands() {
    // `!!str` -> global tag; `!foo` -> local tag.
    let events = scalar_events("- !!str hello\n- !foo bar\n");
    assert_eq!(
        events,
        vec![
            (Some("<tag:yaml.org,2002:str>".into()), "hello".into()),
            (Some("<!foo>".into()), "bar".into()),
        ]
    );
}

#[test]
fn test_tag_directive_handle_resolution() {
    let input = "%TAG !e! tag:example.com,2000:app/\n---\n!e!foo \"bar\"\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/foo>".into()), "bar".into())]
    );
}

#[test]
fn test_tag_directive_primary_and_secondary() {
    let input = "%TAG ! tag:example.com,2000:app/\n%TAG !! tag:example.org,2000:app/\n---\n- !foo \"a\"\n- !!bar \"b\"\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![
            (Some("<tag:example.com,2000:app/foo>".into()), "a".into()),
            (Some("<tag:example.org,2000:app/bar>".into()), "b".into()),
        ]
    );
}

#[test]
fn test_tag_directive_scope_resets_between_documents() {
    // `%TAG` declarations only apply to the following document; a
    // named handle used in a later document without re-declaration is
    // an error.
    let input = "%TAG !prefix! tag:example.com,2011:\n---\n!prefix!A x: 1\n---\n!prefix!B y: 2\n";
    assert!(YamlParser::parse_to_events(input).is_err());
}

#[test]
fn test_verbatim_tags() {
    let events = scalar_events("!<tag:yaml.org,2002:str> foo\n");
    assert_eq!(
        events,
        vec![(Some("<tag:yaml.org,2002:str>".into()), "foo".into())]
    );
    assert!(YamlParser::parse_to_events("!<> foo\n").is_err());
}

#[test]
fn test_percent_escaped_suffix() {
    let input = "%TAG !e! tag:example.com,2000:app/\n---\n!e!tag%21 \"baz\"\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/tag!>".into()), "baz".into())]
    );
}

#[test]
fn test_invalid_tags() {
    // Flow indicators are not allowed in tag suffixes.
    assert!(
        YamlParser::parse_to_events("---\n!invalid{}tag scalar\n").is_err()
    );
    // A named handle without a %TAG declaration is an error.
    assert!(YamlParser::parse_to_events("!h!foo bar\n").is_err());
}

#[test]
fn test_yaml_directive_validation() {
    // Valid versions are accepted; a comment is allowed.
    let events = scalar_events("%YAML 1.3 # comment\n---\nfoo\n");
    assert_eq!(events, vec![(None, "foo".into())]);
    // Duplicate %YAML is an error.
    let e = YamlParser::parse_to_events("%YAML 1.2\n%YAML 1.2\n---\nfoo\n")
        .unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidDirective);
    // Extra words after the version are an error.
    let e =
        YamlParser::parse_to_events("%YAML 1.2 foo\n---\nfoo\n").unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidDirective);
}

#[test]
fn test_directives_require_document() {
    let e = YamlParser::parse_to_events("%YAML 1.2\n").unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidDirective);
    let e = YamlParser::parse_to_events("%YAML 1.2\n...\n").unwrap_err();
    assert_eq!(e.kind(), ErrorKind::InvalidDirective);
}

#[test]
fn test_reserved_directives_ignored() {
    let events = scalar_events("%FOO bar baz\n--- \"foo\"\n");
    assert_eq!(events, vec![(None, "foo".into())]);
}

#[test]
fn test_multiple_documents_with_directives() {
    let input =
        "%YAML 1.2\n--- |\n%!PS-Adobe-2.0\n...\n%YAML 1.2\n---\n# Empty\n...\n";
    let events = YamlParser::parse_to_events(input).unwrap();
    let scalars: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            crate::YamlEvent::Scalar(_, _, v, _, _, _) => Some(v.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(scalars, vec!["%!PS-Adobe-2.0\n".to_string(), String::new()]);
}

#[test]
fn test_document_end_marker_with_comment() {
    let input = "%YAML 1.2\n---\nDocument\n... # Suffix\n";
    let events = YamlParser::parse_to_events(input).unwrap();
    let s: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            crate::YamlEvent::Scalar(_, _, v, _, _, _) => Some(v.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(s, vec!["Document".to_string()]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, crate::YamlEvent::DocumentEnd(true, _)))
    );
}
