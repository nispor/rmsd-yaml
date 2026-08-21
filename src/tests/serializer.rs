// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    Value, YamlSerializeOption, from_str, to_string, to_string_with_opt,
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
    // A block sequence as a mapping value is written indentless,
    // matching serde_yaml.
    round_trip(
        &std::collections::BTreeMap::from([("a".to_string(), vec![1u32, 2])]),
        "a:\n- 1\n- 2\n",
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
        "k: !B\n- 1\n- 2\n",
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

#[test]
fn test_whitespace_edge_strings_are_quoted() {
    // A leading or trailing YAML blank cannot be part of a plain scalar
    // (the scanner trims/folds it), so either edge must be double
    // quoted or the value corrupts on re-parse. Regression: "trailing "
    // used to serialize unquoted and come back as "trailing".
    round_trip(&"trailing ".to_string(), "\"trailing \"\n");
    round_trip(&" leading".to_string(), "\" leading\"\n");
    round_trip(&"a b ".to_string(), "\"a b \"\n");
    round_trip(&"a\tb ".to_string(), "\"a\\tb \"\n");
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
    let v = Value::from_str("- 1\n- 2\n").unwrap();
    assert!(matches!(v.data, crate::ValueData::Array(_)));
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Bridge {
    name: String,
    port: Vec<Port>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Port {
    name: String,
    vlan: Vlan,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Vlan {
    mode: String,
    trunk_tags: Vec<TrunkTag>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TrunkTag {
    id: u32,
}

#[test]
fn test_map_value_sequence_is_indentless() {
    // A block sequence used as a mapping value is written indentless
    // (items at the key's own column), matching serde_yaml and the
    // yaml-test-suite `out.yaml` convention
    // (`key:\n- item1`), even when nested several levels deep.
    let bridge = Bridge {
        name: "br0".to_string(),
        port: vec![Port {
            name: "eth1".to_string(),
            vlan: Vlan {
                mode: "access".to_string(),
                trunk_tags: vec![TrunkTag { id: 101 }],
            },
        }],
    };
    round_trip(
        &bridge,
        "name: br0\nport:\n- name: eth1\n  vlan:\n    mode: access\n    \
         trunk_tags:\n    - id: 101\n",
    );
}

#[test]
fn test_empty_collections_serialized_explicitly() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Empty {}
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct S {
        routes: Empty,
        address: Vec<String>,
        items: Vec<Empty>,
    }
    // Empty map/struct and sequence values are rendered explicitly
    // (`{}` / `[]`), matching serde_yaml, instead of a bare `key:`.
    round_trip(
        &S {
            routes: Empty {},
            address: vec![],
            items: vec![],
        },
        "routes: {}\naddress: []\nitems: []\n",
    );
    round_trip(&Empty {}, "{}\n");
    round_trip(&vec![Empty {}, Empty {}], "- {}\n- {}\n");
}

#[test]
fn test_non_finite_floats_use_yaml_special_scalars() {
    // The old code emitted Rust's `Display` forms (`inf`, `-inf`,
    // `NaN`), which are not valid YAML floats and no longer deserialize
    // back as numbers. Both the serialized shape and the re-parse must
    // use the YAML 1.2 Core Schema special scalars (YAML 1.2.2 SPEC,
    // 10.3.2), matching `serde_yaml`.
    let pos = f64::INFINITY;
    assert_eq!(to_string(&pos).unwrap(), ".inf\n");
    assert_eq!(from_str::<f64>(".inf").unwrap(), pos);

    let neg = -pos;
    assert_eq!(to_string(&neg).unwrap(), "-.inf\n");
    assert_eq!(from_str::<f64>("-.inf").unwrap(), neg);

    let nan = f64::NAN;
    assert_eq!(to_string(&nan).unwrap(), ".nan\n");
    assert!(from_str::<f64>(".nan").unwrap().is_nan());

    // The same path is exercised by `f32` (it up-converts to `f64`).
    assert_eq!(to_string(&f32::INFINITY).unwrap(), ".inf\n");
    assert_eq!(to_string(&f32::NEG_INFINITY).unwrap(), "-.inf\n");
    assert!(from_str::<f32>(".nan").unwrap().is_nan());

    // A nested structure with non-finite floats must round-trip as a
    // whole.
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Inner {
        limit: f64,
        score: f64,
        err: f64,
    }
    let v = Inner {
        limit: pos,
        score: neg,
        err: nan,
    };
    let s = to_string(&v).unwrap();
    assert_eq!(s, "limit: .inf\nscore: -.inf\nerr: .nan\n");
    // Compare field-by-field (NaN is not `PartialEq`).
    let got: Inner = from_str(&s).unwrap();
    assert_eq!(got.limit, pos);
    assert_eq!(got.score, neg);
    assert!(got.err.is_nan());
}

#[test]
fn test_negative_zero_round_trips_as_float() {
    // Rust's `Display` prints `-0.0` as `-0`, which the YAML Core
    // Schema resolves as an integer; the serializer must emit `-0.0`
    // so the float type and its sign survive a round trip (matching
    // `serde_yaml`).
    let neg_zero = -0.0_f64;
    assert_eq!(to_string(&neg_zero).unwrap(), "-0.0\n");
    let got: f64 = from_str("-0.0").unwrap();
    assert!(got == 0.0 && got.is_sign_negative());

    // `f32` routes through `serialize_f64` and keeps the sign.
    assert_eq!(to_string(&-0.0_f32).unwrap(), "-0.0\n");
    let got32: f32 = from_str("-0.0").unwrap();
    assert!(got32 == 0.0 && got32.is_sign_negative());

    // A nested structure round-trips the value as a whole.
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Inner {
        neg_zero: f64,
    }
    let s = to_string(&Inner { neg_zero }).unwrap();
    assert_eq!(s, "neg_zero: -0.0\n");
    let got_inner: Inner = from_str(&s).unwrap();
    assert!(got_inner.neg_zero == 0.0 && got_inner.neg_zero.is_sign_negative());
}
