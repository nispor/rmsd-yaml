// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use pretty_assertions::assert_eq;
use serde::Deserialize;

use crate::{
    ErrorKind, Value, YamlCollectionStyle, YamlEvent, YamlParseOption,
    YamlParser, YamlPosition, YamlScalarStyle, from_reader_with_opt, from_str,
    from_str_with_opt,
};

#[test]
fn test_map_of_plain_scalar() {
    super::testlib::init_logger();
    assert_eq!(
        YamlParser::parse_to_events("a: 1\nb: 2\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "a".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 1)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "1".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 4),
                YamlPosition::new(1, 4)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "b".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 1),
                YamlPosition::new(2, 1)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "2".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 4),
                YamlPosition::new(2, 4)
            ),
            YamlEvent::MapEnd(YamlPosition::new(2, 5)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(2, 5)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_map_of_plain_scalar_in_two_lines() {
    assert_eq!(
        YamlParser::parse_to_events("a:\n  b\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "a".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 1)
            ),
            YamlEvent::Scalar(
                None,
                None,
                "b".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 3)
            ),
            YamlEvent::MapEnd(YamlPosition::new(2, 4)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(2, 4)),
            YamlEvent::StreamEnd,
        ]
    )
}

#[test]
fn test_deserialize_bool() {
    assert_eq!(from_str::<bool>("true").unwrap(), true);
    assert_eq!(from_str::<bool>("false").unwrap(), false);
    assert_eq!(
        from_str::<bool>("yes").unwrap_err().kind(),
        ErrorKind::InvalidBool
    );
}

#[test]
fn test_all_yaml_core_bool_spellings() {
    // YAML 1.2 Core Schema (10.3.1) recognizes all six of
    // `true|True|TRUE` and `false|False|FALSE` as booleans. The old
    // `as_bool` only accepted lowercase, so e.g. `TRUE` failed to
    // deserialize into a `bool` field and also leaked into
    // `deserialize_any` as the plain string `"TRUE"`.
    for (w, expected) in [
        ("true", true),
        ("True", true),
        ("TRUE", true),
        ("false", false),
        ("False", false),
        ("FALSE", false),
    ] {
        assert_eq!(
            from_str::<bool>(w).unwrap(),
            expected,
            "{w:?} should deserialize to {expected}"
        );
        let got: serde_json::Value = from_str(w).unwrap();
        assert_eq!(
            got,
            serde_json::json!(expected),
            "{w:?} should be a JSON bool, not a string"
        );
    }
    // The YAML 1.1 words (`yes`/`no`/`on`/`off`) and single digits
    // must *not* be booleans under YAML 1.2; they stay ordinary
    // strings / integers.
    for w in ["yes", "no", "on", "off"] {
        assert!(from_str::<bool>(w).is_err(), "{w:?} is not a YAML 1.2 bool");
        let got: serde_json::Value = from_str(w).unwrap();
        assert_eq!(got, serde_json::json!(w), "{w:?} must stay a string");
    }
    // A quoted scalar is never a bool, even when its content is a
    // valid bool spelling (already covered by
    // `test_quoted_numeric_and_bool_like_scalars_are_not_coerced` but
    // restated here since the bool set just widened).
    assert!(from_str::<bool>("\"True\"").is_err());
}

#[test]
fn test_deserialize_char() {
    assert_eq!(from_str::<char>("a").unwrap(), 'a');
    assert_eq!(
        from_str::<char>("ab").unwrap_err().kind(),
        ErrorKind::UnexpectedYamlNodeType
    );
    // Null / integer / bool scalars must not be coerced to their
    // single-character text (`~` -> '~', `0` -> '0'). `as_char` must
    // respect the same null/number/bool gates as `as_str`, `as_bool`,
    // `as_f64`, keeping type-safety consistent.
    for c in ["~", "null", "Null", "NULL", "0", "1", "42", "true", "FALSE"] {
        assert!(
            from_str::<char>(c).is_err(),
            "{c:?} is not a valid YAML char"
        );
    }
    // A *quoted* single-character scalar is an ordinary string and does
    // resolve to a char (its content is not a number / bool / null).
    assert_eq!(from_str::<char>("\"0\"").unwrap(), '0');
    assert_eq!(from_str::<char>("\"~\"").unwrap(), '~');
    assert_eq!(from_str::<char>("'a'").unwrap(), 'a');
}

#[test]
fn test_deserialize_integers() {
    assert_eq!(from_str::<u64>("42").unwrap(), 42);
    assert_eq!(from_str::<u64>("0xfa").unwrap(), 0xfa);
    assert_eq!(from_str::<u64>("0o20").unwrap(), 16);
    assert_eq!(from_str::<u64>("0b10").unwrap(), 2);
    assert_eq!(from_str::<i64>("-42").unwrap(), -42);
    assert_eq!(from_str::<i64>("+42").unwrap(), 42);
    assert_eq!(
        from_str::<u8>("300").unwrap_err().kind(),
        ErrorKind::NumberOverflow
    );
    assert_eq!(
        from_str::<u64>("abc").unwrap_err().kind(),
        ErrorKind::InvalidNumber
    );
}

#[test]
fn test_i64_min_and_boundaries() {
    // `i64::MIN` has a magnitude (`2^63`) that does not fit in an `i64`,
    // so the old parse-by-unsigned-magnitude-then-negate logic rejected
    // it. Every base spelling of `i64::MIN` must deserialize to the
    // minimum value instead (matching `serde_yaml`, which parses the
    // whole signed token with `i64::from_str`).
    let mag: i128 = 1 << 63;
    assert_eq!(from_str::<i64>("-9223372036854775808").unwrap(), i64::MIN);
    assert_eq!(from_str::<i64>(&format!("-0x{:x}", mag)).unwrap(), i64::MIN);
    assert_eq!(from_str::<i64>(&format!("-0o{:o}", mag)).unwrap(), i64::MIN);
    assert_eq!(from_str::<i64>(&format!("-0b{:b}", mag)).unwrap(), i64::MIN);
    // `i64::MAX` still parses, and `one-past` either end is rejected as
    // out of range rather than silently wrapping.
    assert_eq!(from_str::<i64>("9223372036854775807").unwrap(), i64::MAX);
    assert!(from_str::<i64>("9223372036854775808").is_err());
    assert!(from_str::<i64>("-9223372036854775809").is_err());
    // A negative plain integer and the narrower signed targets still work.
    assert_eq!(from_str::<i64>("-42").unwrap(), -42);
    assert_eq!(from_str::<i32>("-2147483648").unwrap(), i32::MIN);
    // The generic `serde_json::Value` path carries the integer too.
    let got: serde_json::Value = from_str("-9223372036854775808").unwrap();
    assert_eq!(got, serde_json::Value::from(i64::MIN));
}

#[test]
fn test_deserialize_floats() {
    assert_eq!(from_str::<f64>("1.5").unwrap(), 1.5);
    assert_eq!(from_str::<f64>("-2.25").unwrap(), -2.25);
    assert_eq!(from_str::<f64>("1e3").unwrap(), 1000.0);
    assert_eq!(from_str::<f64>(".inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("-.inf").unwrap(), f64::NEG_INFINITY);
    assert!(from_str::<f64>(".nan").unwrap().is_nan());
    assert_eq!(from_str::<f32>("1.5").unwrap(), 1.5_f32);
}

#[test]
fn test_bare_inf_nan_infinity_are_strings_not_floats() {
    // Per the YAML 1.2 Core Schema (10.3), only the dot forms `.inf`,
    // `+.inf`, `-.inf` and `.nan` resolve to real numbers. The bare
    // words `inf`, `infinity` and `nan`, whatever their sign and/or
    // casing, are ordinary strings. The bug they used to trigger was a
    // straight copy-paste of Rust's `f64::FromStr` semantics (which
    // does accept `inf`/`nan`/`infinity` in any case), leaking into
    // both the generic `deserialize_any` path (they became `null` when
    // fed into `serde_json::Value`) and the `as_f64` path (they parsed
    // to `-inf`/`inf`/`NaN`).
    //
    // Generic / serde_json target: each bare word stays a string.
    for w in [
        "inf",
        "-inf",
        "+inf",
        "INF",
        "Inf",
        "infinity",
        "-infinity",
        "INFINITY",
        "Infinity",
        "nan",
        "NaN",
        "Nan",
        "-nan",
        "+nan",
    ] {
        let got: serde_json::Value = from_str(w).unwrap();
        assert_eq!(
            got,
            serde_json::json!(w),
            "bare word {w:?} should deserialize to a string"
        );
    }
    // Generic / String target: identity round trip.
    for w in ["inf", "-inf", "infinity", "nan", "-nan"] {
        assert_eq!(from_str::<String>(w).unwrap(), w);
    }
    // Deserializing into a concrete numeric target is a type error,
    // matching serde_yaml ("invalid type: string `inf`, expected f64").
    assert!(from_str::<f64>("inf").is_err());
    assert!(from_str::<f64>("NaN").is_err());
    assert!(from_str::<f64>("-infinity").is_err());
    let err = from_str::<f64>("inf").unwrap_err();
    assert!(
        err.msg()
            .contains("invalid type: string \"inf\", expected f64"),
        "got: {err}"
    );
    // The dot forms and genuine YAML numeric floats must still be
    // floats, so the fix does not over-reject.
    assert_eq!(from_str::<f64>(".inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("+.inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("-.inf").unwrap(), f64::NEG_INFINITY);
    assert!(from_str::<f64>(".nan").unwrap().is_nan());
    assert_eq!(from_str::<f64>("1e400").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("-1e400").unwrap(), f64::NEG_INFINITY);
    assert_eq!(from_str::<f64>("5.").unwrap(), 5.0);
    assert_eq!(from_str::<f64>(".5").unwrap(), 0.5);
    // And `Value`-level type predicates report "not a float" too.
    for w in ["inf", "nan", "infinity"] {
        let v = crate::Value::from_str(w).unwrap();
        assert!(!v.is_float(), "{w:?} is not a YAML float");
        assert!(v.as_f64().is_err());
        assert_eq!(v.as_str().unwrap(), w);
    }
}

#[test]
fn test_deserialize_string() {
    assert_eq!(from_str::<String>("hello").unwrap(), "hello");
    assert_eq!(from_str::<String>("").unwrap(), "");
}

#[test]
fn test_deserialize_unit_and_option() {
    assert_eq!(from_str::<()>("").unwrap(), ());
    assert_eq!(from_str::<Option<u32>>("").unwrap(), None);
    assert_eq!(from_str::<Option<u32>>("42").unwrap(), Some(42));
}

#[test]
fn test_deserialize_bytes_unsupported() {
    #[derive(Debug)]
    struct BytesProbe;

    struct BytesProbeVisitor;

    impl<'de> serde::de::Visitor<'de> for BytesProbeVisitor {
        type Value = ();

        fn expecting(
            &self,
            formatter: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            formatter.write_str("bytes")
        }
    }

    impl<'de> Deserialize<'de> for BytesProbe {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_bytes(BytesProbeVisitor)?;
            Ok(Self)
        }
    }

    assert_eq!(
        from_str::<BytesProbe>("abc").unwrap_err().kind(),
        ErrorKind::BytesUnsupported
    );
}

#[test]
fn test_deserialize_enum() {
    #[derive(Debug, PartialEq, Deserialize)]
    enum UnitEnum {
        Alpha,
        Beta,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    enum NewTypeEnum {
        Number(u32),
    }

    assert_eq!(from_str::<UnitEnum>("!Alpha").unwrap(), UnitEnum::Alpha);
    assert_eq!(
        from_str::<NewTypeEnum>("!Number 5").unwrap(),
        NewTypeEnum::Number(5)
    );
}

#[test]
fn test_alias_resolution() {
    let value = Value::from_str("a: &x 1\nb: *x\n").unwrap();
    let mut map = match value.data {
        crate::ValueData::Map(m) => *m,
        d => panic!("Expecting a map, but got {d:?}"),
    };
    let mut got = Vec::new();
    while let Some((k, v)) = map.pop() {
        got.push((
            k.as_str().unwrap().to_string(),
            v.as_str().unwrap().to_string(),
        ));
    }
    got.sort();
    assert_eq!(
        got,
        vec![
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "1".to_string())
        ]
    );

    assert_eq!(
        Value::from_str("b: *nope").unwrap_err().kind(),
        ErrorKind::UnknownAlias
    );
}

#[test]
fn test_indented_block_map_key() {
    // A document whose root block mapping is indented must not keep the
    // leading whitespace in its keys.
    let value =
        Value::from_str("        interfaces:\n          - a\n").unwrap();
    let map = match value.data {
        crate::ValueData::Map(m) => *m,
        d => panic!("Expecting a map, but got {d:?}"),
    };
    assert_eq!(map.len(), 1);
    for (k, v) in map.iter() {
        assert_eq!(k.as_str().unwrap(), "interfaces");
        assert!(matches!(v.data, crate::ValueData::Array(_)));
    }
}

#[test]
fn test_seq_entry_map_keys_after_nested_seq_value() {
    // A block mapping used as a block sequence entry whose value is a
    // nested block sequence: the following keys of the entry map sit at
    // the key's own column, which is one deeper than the sequence
    // indentation (`- bridge:` starts the key at column 4 while the
    // sequence itself is at column 2). Sibling keys like `mac-address`
    // and `name` must not be confused with a continuation of the value
    // scalar, and must not keep leading whitespace.
    let value = Value::from_str(
        "interfaces:\n  - bridge:\n      port:\n        - name: eth0\n    \
         mac-address: 00:00:5E:00:00:01\n    name: br1\n",
    )
    .unwrap();
    let map = match value.data {
        crate::ValueData::Map(m) => *m,
        d => panic!("Expecting a map, but got {d:?}"),
    };
    let interfaces = map.get(&Value::from("interfaces")).unwrap();
    let seq = match &interfaces.data {
        crate::ValueData::Array(seq) => seq,
        d => panic!("Expecting an array, but got {d:?}"),
    };
    assert_eq!(seq.len(), 1);
    let entry = match &seq[0].data {
        crate::ValueData::Map(m) => m,
        d => panic!("Expecting a map, but got {d:?}"),
    };
    assert_eq!(entry.len(), 3);
    for key in ["bridge", "mac-address", "name"] {
        assert!(
            entry.contains_key(&Value::from(key)),
            "missing key {key:?}: {entry:?}"
        );
    }
    assert_eq!(
        entry.get(&Value::from("mac-address")).unwrap().as_str(),
        Ok("00:00:5E:00:00:01")
    );
}

#[derive(Deserialize, PartialEq, Debug)]
struct NullableScalar {
    #[serde(rename = "a")]
    a: Option<String>,
}

#[test]
fn test_quoted_null_like_scalars_are_not_null() {
    // serde_yaml treats only plain `null`, `Null`, `NULL`, `~` and an
    // empty value as null; quoted scalars such as `"null"`, `"~"` and
    // `""` are ordinary strings and must deserialize to Some(...).
    for (input, expected) in [
        ("a: null\n", None),
        ("a: NULL\n", None),
        ("a: Null\n", None),
        ("a: ~\n", None),
        ("a: \n", None),
        ("a: \"null\"\n", Some("null".to_string())),
        ("a: \"~\"\n", Some("~".to_string())),
        ("a: \"\"\n", Some(String::new())),
    ] {
        let got: NullableScalar = from_str(input).unwrap();
        assert_eq!(got.a, expected, "input: {input:?}");
    }
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct CwndRoute {
    #[serde(rename = "cwnd")]
    cwnd: Option<u32>,
    config: Option<Vec<CwndRoute>>,
}

#[test]
fn test_invalid_type_error_matches_serde_yaml() {
    // serde_yaml reports a type mismatch as
    // `{path}: invalid type: {actual}, expected {expected}` so that
    // users can locate the offending field, e.g.
    // `cwnd: invalid type: integer `-20`, expected u32`.
    let err = from_str::<CwndRoute>("cwnd: -20\n").unwrap_err();
    assert!(err.kind() == ErrorKind::InvalidNumber);
    assert!(
        err.msg()
            .contains("cwnd: invalid type: integer `-20`, expected u32"),
        "got: {}",
        err
    );
    // The path accumulates through nested maps and sequences, e.g.
    // `config[0].cwnd`.
    let err = from_str::<CwndRoute>("config:\n  - cwnd: abc\n").unwrap_err();
    assert!(
        err.msg().contains("config[0].cwnd: invalid type"),
        "got: {}",
        err
    );
}

#[test]
fn test_deserialize_empty_value_as_empty_map_and_seq() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Inner {
        #[serde(default)]
        items: Vec<i32>,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        a: Inner,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct T {
        x: Vec<i32>,
    }
    // `a:` and `x:` with empty values deserialize into empty
    // collections, matching serde_yaml.
    let s: S = from_str("a:\nb: 1\n").unwrap();
    assert_eq!(
        s,
        S {
            a: Inner { items: vec![] }
        }
    );
    let t: T = from_str("x:\n").unwrap();
    assert_eq!(t, T { x: vec![] });
}

#[test]
fn test_deserialize_any_null_scalar_is_null() {
    // `deserialize_any` (used e.g. by `serde_json::Value`) checked
    // `is_bool`/`is_integer`/`is_float` before falling back to a
    // plain string, but never checked `is_null`, so a null scalar
    // used to deserialize into the empty string `""` instead of
    // `Value::Null`.
    let got: serde_json::Value = from_str("a: null\nb: ~\nc:\n").unwrap();
    assert_eq!(got, serde_json::json!({"a": null, "b": null, "c": null}),);
}

#[test]
fn test_deserialize_any_resolves_tags_transparently() {
    // `deserialize_any` used to treat *every* tagged node as an enum
    // representation (`ValueEnumAccess`), which fails for generic
    // targets like `serde_json::Value` since there is no enum to
    // decode into. Per the YAML Core Schema, only `!!str`/`!`,
    // `!!int`, `!!float`, `!!bool` and `!!null` force a scalar type;
    // every other tag (custom application tags, `!!seq`, `!!map`,
    // `!!set`, `!!omap`, `!!binary`, ...) resolves transparently to
    // its underlying, untagged data.
    let got: serde_json::Value = from_str(
        "a: !!str 23\nb: !!int \"23\"\nc: !!float \"1.5\"\nd: !!bool \
         \"true\"\ne: !!null ~\nf: ! 12\ng: !circle\n  center: 0\n  radius: \
         1\n",
    )
    .unwrap();
    assert_eq!(
        got,
        serde_json::json!({
            "a": "23",
            "b": 23,
            "c": 1.5,
            "d": true,
            "e": null,
            "f": "12",
            "g": {"center": 0, "radius": 1},
        }),
    );
}

#[test]
fn test_tagged_scalar_as_string_uses_content_not_tag_name() {
    // `Value::as_str()` used to return the tag's own name (e.g.
    // `"<tag:yaml.org,2002:str>"`) instead of the tagged scalar's
    // content for any tagged value, which corrupted map keys and
    // string fields built from a tagged scalar.
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        key: String,
    }
    let got: S = from_str("key: !!str value\n").unwrap();
    assert_eq!(
        got,
        S {
            key: "value".to_string()
        }
    );

    let got: std::collections::BTreeMap<String, i32> =
        from_str("!!str a: 1\nb: 2\n").unwrap();
    assert_eq!(got.get("a"), Some(&1));
    assert_eq!(got.get("b"), Some(&2));
}

#[test]
fn test_quoted_numeric_and_bool_like_scalars_are_not_coerced() {
    // `is_bool`/`is_integer`/`is_signed_integer`/`is_float` (and the
    // `as_bool`/`as_i64`/`as_u64`/`as_f64` parsers `deserialize_any`,
    // `deserialize_bool` etc. use) never checked `scalar_style`, so a
    // quoted scalar like `"true"` or `"42"` was auto-detected as a
    // bool/number instead of staying a string, unlike `is_null` which
    // already required a plain scalar. This matches `serde_yaml`:
    // quoted scalars are always plain strings when deserialized
    // generically, ...
    let got: serde_json::Value =
        from_str("a: \"true\"\nb: \"42\"\nc: \"1.5\"\n").unwrap();
    assert_eq!(got, serde_json::json!({"a": "true", "b": "42", "c": "1.5"}),);

    // ... and error rather than silently coerce when the target field
    // has a concrete, incompatible type.
    #[derive(Deserialize, Debug)]
    struct Flag {
        #[allow(dead_code)]
        a: bool,
    }
    assert!(from_str::<Flag>("a: \"true\"\n").is_err());

    #[derive(Deserialize, Debug)]
    struct Num {
        #[allow(dead_code)]
        b: i64,
    }
    assert!(from_str::<Num>("b: \"42\"\n").is_err());
}

#[test]
fn test_yaml_parse_option_defaults() {
    // `YamlParseOption::default()` must match the hardcoded resource
    // limits used by `from_str`/`from_reader`. The node cap is kept
    // deliberately low enough that a near-limit anchor/alias document
    // cannot make the composer spend seconds materializing and cloning
    // hundreds of thousands of nodes.
    let option = YamlParseOption::default();
    assert_eq!(option.max_depth, 128);
    assert_eq!(option.max_nodes, 100_000);
}

#[test]
fn test_from_str_with_opt_respects_custom_max_depth() {
    // A `max_depth` tighter than the default must reject input that
    // would otherwise parse fine, proving the option is actually
    // threaded into both the parser's and the composer's depth checks
    // instead of the option being silently ignored.
    let option = YamlParseOption {
        max_depth: 2,
        ..Default::default()
    };
    let err = from_str_with_opt::<Value>("[[[[1]]]]", option).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::RecursionLimitExceeded);
}

#[test]
fn test_from_str_with_opt_custom_max_depth_allows_shallow_input() {
    // The same tight `max_depth` must still accept input nested within
    // that limit, proving it is not simply always rejecting.
    let option = YamlParseOption {
        max_depth: 2,
        ..Default::default()
    };
    let got: Value = from_str_with_opt("[1, 2]", option).unwrap();
    assert_eq!(got, from_str::<Value>("[1, 2]").unwrap());
}

#[test]
fn test_from_str_with_opt_respects_custom_max_nodes() {
    // A `max_nodes` tighter than the default must reject a document
    // that composes more nodes than the budget allows.
    let option = YamlParseOption {
        max_nodes: 2,
        ..Default::default()
    };
    let err = from_str_with_opt::<Value>("[1, 2, 3]", option).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::AliasExpansionLimitExceeded);
}

#[test]
fn test_from_str_with_opt_custom_max_nodes_allows_small_input() {
    let option = YamlParseOption {
        max_nodes: 2,
        ..Default::default()
    };
    let got: Value = from_str_with_opt("1", option).unwrap();
    assert_eq!(got, from_str::<Value>("1").unwrap());
}

#[test]
fn test_from_reader_with_opt_respects_custom_max_input_bytes() {
    // A reader delivering more than `max_input_bytes` must be rejected
    // with `InputTooLarge` before parsing is even attempted, instead
    // of being buffered into memory in full.
    let option = YamlParseOption {
        max_input_bytes: 4,
        ..Default::default()
    };
    let err = from_reader_with_opt::<_, Value>("12345".as_bytes(), option)
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InputTooLarge);
}

#[test]
fn test_from_reader_with_opt_max_input_bytes_allows_input_at_the_limit() {
    // Input of exactly `max_input_bytes` must still be accepted (the
    // limit is inclusive), proving the one-extra-byte read used to
    // detect an overlong stream does not off-by-one reject it.
    let option = YamlParseOption {
        max_input_bytes: 4,
        ..Default::default()
    };
    let got: Value = from_reader_with_opt("1234".as_bytes(), option).unwrap();
    assert_eq!(got, from_str::<Value>("1234").unwrap());
}

#[test]
fn test_from_reader_default_has_no_input_size_cap() {
    // `from_reader` (without `_with_opt`) must keep its historical,
    // unbounded behavior: `YamlParseOption::default().max_input_bytes`
    // is 0, meaning no limit.
    assert_eq!(YamlParseOption::default().max_input_bytes, 0);
    let long_input = "a".repeat(10_000);
    let got: String = crate::from_reader(long_input.as_bytes()).unwrap();
    assert_eq!(got, long_input);
}

#[test]
fn test_leading_bom_is_stripped_before_the_first_token() {
    // A leading UTF-8 BOM (`U+FEFF`) is not part of the document's
    // content (YAML 1.2.2 SPEC, 2.1.1). Before the fix, it merged into
    // the first token, turning the key `a` of `\u{feff}a: 1` into the
    // spurious key `"\u{feff}a"`.
    let bom = '\u{feff}';

    // The BOM on a block-map key must not leak into the key.
    let map = format!("{bom}a: 1");
    let v: Value = from_str(&map).unwrap();
    assert_eq!(v, from_str::<Value>("a: 1").unwrap());
    assert!(
        v.get("\u{feff}a").is_none(),
        "BOM must not leak into the key"
    );
    assert!(v.get("a").is_some(), "key must be the bare `a`");

    // The BOM on a plain scalar must not corrupt the value.
    let scalar = format!("{bom}42");
    let v2: Value = from_str(&scalar).unwrap();
    assert_eq!(v2.as_i64().unwrap(), 42);

    // A BOM *inside* a value is ordinary content, not a stream BOM.
    let mid = format!("a: {bom}b");
    let v3: Value = from_str(&mid).unwrap();
    assert_eq!(v3.get("a").unwrap().as_str().unwrap(), "\u{feff}b");
}
