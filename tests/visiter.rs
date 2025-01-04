// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use rmsd_yaml::RmsdError;
use serde::{de, de::Visitor, Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct FooTest {
    #[serde(deserialize_with = "int_or_string")]
    kind: FooKind,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
enum FooKind {
    A,
    B,
    C,
    D,
    E,
}

pub(crate) fn int_or_string<'de, D>(
    deserializer: D,
) -> Result<FooKind, D::Error>
where
    D: Deserializer<'de>,
{
    struct IntegerOrString(PhantomData<fn() -> FooKind>);

    impl Visitor<'_> for IntegerOrString {
        type Value = FooKind;

        fn expecting(
            &self,
            formatter: &mut std::fmt::Formatter,
        ) -> std::fmt::Result {
            formatter
                .write_str("integer 1|2|-3 or string A|B|C|D|E or true|false")
        }

        fn visit_str<E>(self, value: &str) -> Result<FooKind, E>
        where
            E: de::Error,
        {
            match value {
                "A" => Ok(FooKind::A),
                "B" => Ok(FooKind::B),
                "C" => Ok(FooKind::C),
                _ => Err(de::Error::custom(format!("Invalid Fookind {value}"))),
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<FooKind, E>
        where
            E: de::Error,
        {
            match value {
                1 => Ok(FooKind::A),
                2 => Ok(FooKind::B),
                _ => Err(de::Error::custom(format!("Invalid Fookind {value}"))),
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<FooKind, E>
        where
            E: de::Error,
        {
            match value {
                -3 => Ok(FooKind::C),
                _ => Err(de::Error::custom(format!("Invalid Fookind {value}"))),
            }
        }

        fn visit_bool<E>(self, value: bool) -> Result<FooKind, E>
        where
            E: de::Error,
        {
            if value {
                Ok(FooKind::D)
            } else {
                Ok(FooKind::E)
            }
        }
    }

    deserializer.deserialize_any(IntegerOrString(PhantomData))
}

#[test]
fn test_deserialize_any() -> Result<(), RmsdError> {
    assert_eq!(
        vec![
            FooTest { kind: FooKind::A },
            FooTest { kind: FooKind::B },
            FooTest { kind: FooKind::C },
            FooTest { kind: FooKind::D },
            FooTest { kind: FooKind::E },
        ],
        rmsd_yaml::from_str::<Vec<FooTest>>(
            r#"
            ---
            - kind: A
            - kind: 2
            - kind: -3
            - kind: true
            - kind: false
            "#
        )?
    );
    Ok(())
}
