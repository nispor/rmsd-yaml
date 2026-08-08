// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;
use serde::Deserialize;

use crate::{
    ErrorKind, YamlEvent, YamlParser, YamlPosition, YamlScalarStyle, from_str,
    to_value,
};

#[test]
fn test_map_of_plain_scalar() {
    super::testlib::init_logger();

    assert_eq!(
        YamlParser::parse_to_events("a: 1\nb: 2\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(None, None, YamlPosition::new(1, 1)),
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
            YamlEvent::MapStart(None, None, YamlPosition::new(1, 1)),
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
    let value = to_value("a: &x 1\nb: *x\n").unwrap();
    let mut map = match value.data {
        crate::YamlValueData::Map(m) => *m,
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
        to_value("b: *nope").unwrap_err().kind(),
        ErrorKind::UnknownAlias
    );
}
