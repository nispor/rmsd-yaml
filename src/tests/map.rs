// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use pretty_assertions::assert_eq;

use crate::{
    YamlCollectionStyle, YamlEvent, YamlParser, YamlPosition, YamlScalarStyle,
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
fn test_value_get_and_as_mapping() {
    let value: crate::Value =
        crate::Value::from_str("interfaces:\n  - name: eth1\n    mtu: 1500\n")
            .unwrap();
    let ifaces = value
        .get("interfaces")
        .and_then(crate::Value::as_sequence)
        .unwrap();
    assert_eq!(ifaces.len(), 1);
    let iface = &ifaces[0];
    assert_eq!(iface.get("name").unwrap().as_str().unwrap(), "eth1");
    assert_eq!(iface.get("mtu").unwrap().as_str().unwrap(), "1500");
    assert!(iface.get("absent-key").is_none());
    assert!(value.as_mapping().is_some());
    assert!(iface.as_mapping().is_some());
    assert!(value.as_sequence().is_none());
    assert!(value.get("interfaces").unwrap().as_mapping().is_none());
}

#[test]
fn test_empty_value_followed_by_sibling_key() {
    // `a:` with no value and a sibling key at the same indentation:
    // the value of `a` is empty and `b` is a new entry.
    let value: crate::Value = crate::Value::from_str("a:\nb: 1\n").unwrap();
    assert!(value.get("a").unwrap().is_null());
    assert_eq!(value.get("b").unwrap().as_str().unwrap(), "1");
    assert_eq!(value.as_mapping().unwrap().len(), 2);
}

#[test]
fn test_empty_value_followed_by_dedent() {
    // `a:` inside a nested block with a dedent after it: the value of
    // `a` is empty and the dedented line closes the inner block.
    let value: crate::Value =
        crate::Value::from_str("x:\n  a:\nb: 1\n").unwrap();
    assert!(value.get("x").unwrap().get("a").unwrap().is_null());
    assert_eq!(value.get("b").unwrap().as_str().unwrap(), "1");
}

#[test]
fn test_zero_indent_sequence_value_still_valid() {
    // A zero-indented block sequence stays the value of the key
    // (YAML 1.2.2 SPEC, 8.2.2), not an empty value + sibling.
    let value: crate::Value = crate::Value::from_str("a:\n- 1\n- 2\n").unwrap();
    assert_eq!(value.get("a").unwrap().as_sequence().unwrap().len(), 2);
}
