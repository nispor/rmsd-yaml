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
    let input = "%TAG ! tag:example.com,2000:app/\n%TAG !! \
                 tag:example.org,2000:app/\n---\n- !foo \"a\"\n- !!bar \"b\"\n";
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
    let input = "%TAG !prefix! tag:example.com,2011:\n---\n!prefix!A x: \
                 1\n---\n!prefix!B y: 2\n";
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
fn test_percent_escaped_suffix_multibyte_utf8() {
    // Regression: `decode_percent()` used to cast each decoded byte to
    // `char` individually (`byte as char`), which is only correct for
    // ASCII. A multi-byte UTF-8 character split across several `%XX`
    // escapes (`%C3%A9` is the 2-byte encoding of `é`) decoded to
    // mojibake (two separate Latin-1 code points) instead of the
    // intended single character.
    let input = "%TAG !e! tag:example.com,2000:app/\n---\n!e!caf%C3%A9 \"a\"\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/café>".into()), "a".into())]
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

#[test]
fn test_tag_directive_with_trailing_comment() {
    // A `%TAG` directive line may end with a comment (YAML 1.2.2 SPEC,
    // production [82] followed by `s-l-comments`). The comment used to
    // be parsed as extra parameters and rejected.
    let input =
        "%TAG !e! tag:example.com,2000:app/ # comment\n---\n!e!foo bar\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/foo>".into()), "bar".into())]
    );

    // A `#` that is not preceded by separation belongs to the URI
    // prefix, not to a comment.
    let input = "%TAG !e! tag:example.com,2000:app/#frag\n---\n!e!foo bar\n";
    let events = scalar_events(input);
    assert_eq!(
        events,
        vec![(
            Some("<tag:example.com,2000:app/#fragfoo>".into()),
            "bar".into()
        )]
    );
}

#[test]
fn test_reserved_directive_requires_a_name() {
    // `%` must be followed by a directive name (`ns-char+`, YAML
    // 1.2.2 SPEC, production [84]); a bare `%` is not a directive.
    for input in ["%\n---\nx\n", "% \n---\nx\n", "%\t\n---\nx\n"] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidDirective, "{input:?}");
    }
    // A named reserved directive is ignored.
    assert_eq!(
        scalar_events("%FOO bar baz\n--- \"foo\"\n"),
        vec![(None, "foo".into())]
    );
}

#[test]
fn test_tag_handle_name_restricted_to_word_characters() {
    // A named handle is `!` `ns-word-char+` `!` (YAML 1.2.2 SPEC,
    // production [97]); `.` and an inner `!` are not word characters.
    for input in [
        "%TAG !a.b! tag:example.com,2000:app/\n---\n!a.b!s v\n",
        "%TAG !a!b! tag:example.com,2000:app/\n---\n!a!b!s v\n",
    ] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidDirective, "{input:?}");
    }

    // Letters, digits and `-` are valid handle-name characters.
    let events = scalar_events(
        "%TAG !a-b2! tag:example.com,2000:app/\n---\n!a-b2!s v\n",
    );
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/s>".into()), "v".into())]
    );
}

#[test]
fn test_invalid_tag_suffix_characters() {
    // A tag shorthand suffix may only contain `ns-tag-char`
    // (YAML 1.2.2 SPEC, productions [39]/[40]).
    for input in [
        "!foo\"bar\" baz\n",
        "!foo{bar} baz\n",
        "!foo<bar baz\n",
        "!foo%zz baz\n",
        "!foo%2 baz\n",
    ] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidTag, "{input:?}");
    }

    // URI escapes and the other allowed punctuation still work.
    let events = scalar_events("!foo%21 baz\n");
    assert_eq!(events, vec![(Some("<!foo!>".into()), "baz".into())]);
    let events = scalar_events("!a.b~c*d/e?f:g@h&i=j+k$l_m#n;o'p(q) baz\n");
    assert_eq!(
        events,
        vec![(
            Some("<!a.b~c*d/e?f:g@h&i=j+k$l_m#n;o'p(q)>".into()),
            "baz".into()
        )]
    );
}

#[test]
fn test_tag_prefix_syntax() {
    // `ns-tag-prefix` is either `!` plus URI characters or an
    // `ns-tag-char` followed by URI characters (YAML 1.2.2 SPEC,
    // productions [101]-[103]); a prefix starting with a flow
    // indicator is invalid.
    for input in [
        "%TAG !e! ,bad\n---\n!e!s v\n",
        "%TAG !e! [bad]\n---\n!e!s v\n",
        "%TAG !e! {bad}\n---\n!e!s v\n",
        "%TAG !e! %zz\n---\n!e!s v\n",
    ] {
        let err = YamlParser::parse_to_events(input).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidDirective, "{input:?}");
    }

    // A local prefix (starting with `!`) may contain flow indicators
    // and `!`; a global prefix may not start with them.
    let events = scalar_events("%TAG !e! !local\n---\n!e!s v\n");
    assert_eq!(events, vec![(Some("<!locals>".into()), "v".into())]);
    let events =
        scalar_events("%TAG !e! tag:example.com,2000:app/\n---\n!e!s v\n");
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/s>".into()), "v".into())]
    );

    // `ns-tag-char` is a single `ns-uri-char`, so a global prefix may
    // start with a complete `%XX` escape (productions [39]/[40]). The
    // escape is decoded during tag resolution.
    let events =
        scalar_events("%TAG !e! %74ag:example.com,2000:app/\n---\n!e!s v\n");
    assert_eq!(
        events,
        vec![(Some("<tag:example.com,2000:app/s>".into()), "v".into())]
    );
    let events = scalar_events("%TAG !e! !%74local\n---\n!e!s v\n");
    assert_eq!(events, vec![(Some("<!tlocals>".into()), "v".into())]);
}
