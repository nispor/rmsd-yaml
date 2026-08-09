// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

use crate::{
    YamlSerializeOption, from_str, to_string, to_string_with_opt, to_value,
};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum E {
    A(u32),
    B(u32, u32),
    C { x: u32, y: u32 },
    D,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct S {
    a: u32,
    b: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct N(u32);

fn round_trip<T>(value: &T, expected: &str)
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let s = to_string(value).unwrap();
    assert_eq!(s, expected, "serialized form");
    let back: T = from_str(&s).unwrap();
    assert_eq!(&back, value, "round trip of {s:?}");
}

#[test]
fn test_sequences_and_maps() {
    round_trip(&vec![1u32, 2, 3], "- 1\n- 2\n- 3\n");
    round_trip(
        &vec![vec![1u32, 2], vec![3, 4]],
        "- - 1\n  - 2\n- - 3\n  - 4\n",
    );
    round_trip(
        &std::collections::BTreeMap::from([
            ("a".to_string(), 1u32),
            ("b".to_string(), 2u32),
        ]),
        "a: 1\nb: 2\n",
    );
    round_trip(
        &std::collections::BTreeMap::from([("a".to_string(), vec![1u32, 2])]),
        "a:\n  - 1\n  - 2\n",
    );
}

#[test]
fn test_enum_variants() {
    round_trip(&E::A(42), "!A 42\n");
    round_trip(&E::B(1, 2), "!B\n- 1\n- 2\n");
    round_trip(&E::C { x: 1, y: 2 }, "!C\nx: 1\ny: 2\n");
    round_trip(&E::D, "D\n");
    round_trip(&vec![E::A(1), E::D], "- !A 1\n- D\n");
    round_trip(
        &std::collections::BTreeMap::from([("k".to_string(), E::B(1, 2))]),
        "k: !B\n  - 1\n  - 2\n",
    );
}

#[test]
fn test_tagged_scalar_then_collection() {
    // A tagged scalar followed by another collection must not leak the
    // pending tag into the following collection's layout.
    round_trip(&(E::A(1), vec![2u32, 3]), "- !A 1\n- - 2\n  - 3\n");
    round_trip(&(vec![1u32, 2], E::A(3)), "- - 1\n  - 2\n- !A 3\n");
}

#[test]
fn test_structs() {
    round_trip(
        &S {
            a: 1,
            b: "hi".into(),
        },
        "a: 1\nb: hi\n",
    );
    round_trip(&N(7), "7\n");
}

#[test]
fn test_option_and_unit() {
    round_trip(&Some(5u32), "5\n");
    round_trip(&None::<u32>, "null\n");
    round_trip(&(), "null\n");
    round_trip(&"a\nb".to_string(), "\"a\\nb\"\n");
}

/// Wrapper that serializes through `serializer.serialize_bytes`, the
/// only way serde's data model reaches the byte-buffer path.
struct Bytes<'a>(&'a [u8]);
impl Serialize for Bytes<'_> {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

#[test]
fn test_bytes_use_binary_tag() {
    let bytes = Bytes(&[0u8, 1, 2, 255]);
    let s = to_string(&bytes).unwrap();
    assert_eq!(s, "!!binary AAEC/w==\n");
    let empty = Bytes(&[]);
    assert_eq!(to_string(&empty).unwrap(), "!!binary \n");
    // `CString` deserializes through `deserialize_byte_buf`.
    let back: std::ffi::CString = from_str("!!binary aGVsbG8=").unwrap();
    assert_eq!(back, std::ffi::CString::new("hello").unwrap());
    assert_eq!(
        from_str::<std::ffi::CString>("!!binary ").unwrap(),
        std::ffi::CString::new("").unwrap()
    );
}

#[test]
fn test_binary_tag_deserialize_error() {
    let e = from_str::<std::ffi::CString>("!!binary Zm9v!").unwrap_err();
    assert_eq!(e.kind(), crate::ErrorKind::InvalidNumber);
    let e = from_str::<std::ffi::CString>("plain").unwrap_err();
    assert_eq!(e.kind(), crate::ErrorKind::BytesUnsupported);
}

#[test]
fn test_long_line_is_folded() {
    let long = "word ".repeat(30) + "end";
    let s = to_string(&long).unwrap();
    assert!(s.starts_with('"'), "double quoted: {s:?}");
    // The long line is folded with a real line break inside the double
    // quotes; flow folding turns it back into a space on parsing.
    assert!(s.contains('\n'), "folds: {s:?}");
    let back: String = from_str(&s).unwrap();
    assert_eq!(back, long);
}

#[test]
fn test_indent_too_small() {
    let opt = YamlSerializeOption {
        indent_count: 1,
        ..Default::default()
    };
    let e = to_string_with_opt(&"abc", opt).unwrap_err();
    assert_eq!(e.kind(), crate::ErrorKind::IndentTooSmall);
}

#[test]
fn test_leading_start_indicator() {
    let opt = YamlSerializeOption {
        leading_start_indicator: true,
        ..Default::default()
    };
    let s = to_string_with_opt(&vec![1u32, 2], opt).unwrap();
    assert!(s.starts_with("---\n"), "got {s:?}");
    let back: Vec<u32> = from_str(&s).unwrap();
    assert_eq!(back, vec![1, 2]);
}

#[test]
fn test_to_value() {
    let v = to_value("- 1\n- 2\n").unwrap();
    assert!(matches!(v.data, crate::YamlValueData::Array(_)));
}
