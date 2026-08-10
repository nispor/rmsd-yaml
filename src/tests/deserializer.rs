// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use pretty_assertions::assert_eq;
use serde::Deserialize;

use crate::{
    ErrorKind, Value, YamlCollectionStyle, YamlEvent, YamlParser, YamlPosition,
    YamlScalarStyle, from_str,
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
fn test_deserialize_char() {
    assert_eq!(from_str::<char>("a").unwrap(), 'a');
    assert_eq!(
        from_str::<char>("ab").unwrap_err().kind(),
        ErrorKind::UnexpectedYamlNodeType
    );
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
