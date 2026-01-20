// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::str::FromStr;

use serde::{
    Deserialize,
    de::{Deserializer, Visitor},
};

use crate::{
    ErrorKind, YamlError, YamlValue, YamlValueData, YamlValueEnumAccess,
    YamlValueMapAccess, YamlValueSeqAccess,
};

#[derive(Debug, Default)]
pub struct YamlDeserializer {
    pub(crate) parsed: YamlValue,
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T, YamlError>
where
    T: Deserialize<'a>,
{
    let parsed = YamlValue::from_str(s)?;
    let mut deserializer = YamlDeserializer { parsed };

    T::deserialize(&mut deserializer)
}

pub fn to_value(input: &str) -> Result<YamlValue, YamlError> {
    YamlValue::from_str(input)
}

impl<'de> Deserializer<'de> for &mut YamlDeserializer {
    type Error = YamlError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.parsed.data {
            YamlValueData::String(_) => {
                if self.parsed.is_bool() {
                    self.deserialize_bool(visitor)
                } else if self.parsed.is_integer() {
                    self.deserialize_u64(visitor)
                } else if self.parsed.is_signed_integer() {
                    self.deserialize_i64(visitor)
                } else {
                    self.deserialize_str(visitor)
                }
            }
            YamlValueData::Array(_) => self.deserialize_seq(visitor),
            YamlValueData::Map(_) => self.deserialize_map(visitor),
            YamlValueData::Tag(_) => {
                let access = YamlValueEnumAccess::new(self.parsed.clone());
                visitor.visit_enum(access)
            }
            v => Err(YamlError::new(
                ErrorKind::Bug,
                format!("deserialize_any() got unexpected data {v:?}"),
                self.parsed.start,
                self.parsed.end,
            )),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parsed.as_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.parsed.as_i8()?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(self.parsed.as_i16()?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(self.parsed.as_i32()?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(self.parsed.as_i64()?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.parsed.as_u8()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(self.parsed.as_u16()?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(self.parsed.as_u32()?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(self.parsed.as_u64()?)
    }

    fn deserialize_f32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_f64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_char(self.parsed.as_char()?)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.parsed.as_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parsed.as_str()?.to_string())
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_byte_buf<V>(
        self,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.data {
            YamlValueData::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let YamlValueData::Array(v) = &self.parsed.data {
            // TODO: We cannot move data output of `&mut self`, so we use
            // to_vec() to clone here. Maybe should use `Option<YamlValue>` for
            // Self::parsed, where we can use `Option::take()` to move data out.
            let access = YamlValueSeqAccess::new(v.to_vec());
            visitor.visit_seq(access)
        } else if let YamlValueData::Tag(tag) = &self.parsed.data {
            if let YamlValueData::Array(v) = &tag.data {
                let access = YamlValueSeqAccess::new(v.to_vec());
                visitor.visit_seq(access)
            } else {
                Err(YamlError::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!(
                        "Expecting a sequence in tag, got {}",
                        self.parsed.data
                    ),
                    self.parsed.start,
                    self.parsed.end,
                ))
            }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a sequence, got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_tuple<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let YamlValueData::Map(v) = &self.parsed.data {
            // TODO: We cannot move data output of `&mut self`, so we use clone
            // here. Maybe should use `Option<YamlValue>` for Self::parsed,
            // where we can use `Option::take()` to move data out.
            let access = YamlValueMapAccess::new(*v.clone());
            visitor.visit_map(access)
        } else if let YamlValueData::Null = &self.parsed.data {
            let access = YamlValueMapAccess::new(Default::default());
            visitor.visit_map(access)
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a map, got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // TODO: We cannot move data output of `&mut self`, so we use clone
        // here. Maybe should use `Option<YamlValue>` for Self::parsed,
        // where we can use `Option::take()` to move data out.
        let access = YamlValueEnumAccess::new(self.parsed.clone());

        visitor.visit_enum(access)
    }

    fn deserialize_identifier<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use serde::{Deserialize, Serialize};

    use crate::YamlError;

    #[test]
    fn test_de_char() -> Result<(), YamlError> {
        assert_eq!(crate::from_str::<char>("q")?, 'q');
        Ok(())
    }

    #[test]
    fn test_de_simple_str() -> Result<(), YamlError> {
        let yaml_str = r#"
        "acb"
    "#;

        let abc: String = crate::from_str(yaml_str)?;
        assert_eq!(abc, "acb");
        Ok(())
    }

    #[test]
    fn test_de_bool() -> Result<(), YamlError> {
        assert!(!crate::from_str("false")?);

        assert!(crate::from_str("true")?);
        Ok(())
    }

    #[test]
    fn test_de_unsign_number() -> Result<(), YamlError> {
        assert_eq!(123114u32, crate::from_str("\n---\n123114")?);

        assert_eq!(1234u16, crate::from_str("+1234")?);

        assert_eq!(0x123123u64, crate::from_str("0x123123")?);
        assert_eq!(0o123u16, crate::from_str("0o123")?);
        assert_eq!(0b1001u8, crate::from_str("0b1001")?);

        Ok(())
    }

    #[test]
    fn test_de_simple_struct() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: u32,
            str_b: String,
            bar: BarTest,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct BarTest {
            data: bool,
        }

        crate::testlib::init_logger();

        let yaml_str = r#"---
            uint_a: 500
            str_b: "abc"
            bar:
              data: false"#;

        let foo_test: FooTest = crate::from_str(yaml_str)?;

        assert_eq!(
            foo_test,
            FooTest {
                uint_a: 500,
                str_b: "abc".to_string(),
                bar: BarTest { data: false }
            }
        );
        Ok(())
    }

    #[test]
    fn test_de_simple_array() -> Result<(), YamlError> {
        crate::testlib::init_logger();

        let yaml_str = r#"
        ---
        - 500
        - 600
        - 0xfe
    "#;

        let value: Vec<u32> = crate::from_str(yaml_str)?;

        assert_eq!(value, vec![500u32, 600, 0xfe,]);
        Ok(())
    }

    #[test]
    fn test_de_tuple() -> Result<(), YamlError> {
        crate::testlib::init_logger();

        let yaml_str = r#"
        ---
        - 500
        - 0xff
    "#;

        let value: (u32, u32) = crate::from_str(yaml_str)?;

        assert_eq!(value, (500u32, 0xff));
        Ok(())
    }

    #[test]
    fn test_de_tuple_of_struct() -> Result<(), YamlError> {
        crate::testlib::init_logger();

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: u32,
            str_b: String,
        }

        let yaml_str = r#"
        ---
        - uint_a: 36
          str_b: item1
        - uint_a: 37
          str_b: item2
    "#;

        let value: (FooTest, FooTest) = crate::from_str(yaml_str)?;

        assert_eq!(
            value,
            (
                FooTest {
                    uint_a: 36,
                    str_b: "item1".to_string(),
                },
                FooTest {
                    uint_a: 37,
                    str_b: "item2".to_string(),
                },
            )
        );
        Ok(())
    }

    #[test]
    fn test_de_array_of_struct() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: u32,
            str_b: String,
        }

        let yaml_str = r#"
        ---
        - uint_a: 36
          str_b: item1
        - uint_a: 37
          str_b: item2"#;

        crate::testlib::init_logger();

        let value: Vec<FooTest> = crate::from_str(yaml_str)?;

        assert_eq!(
            value,
            vec![
                FooTest {
                    uint_a: 36,
                    str_b: "item1".to_string(),
                },
                FooTest {
                    uint_a: 37,
                    str_b: "item2".to_string(),
                },
            ]
        );
        Ok(())
    }

    #[test]
    fn test_de_new_type_struct() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest(String);

        let yaml_str = r#"
        ---
        abcedfo
    "#;

        let value: FooTest = crate::from_str(yaml_str)?;

        assert_eq!(value, FooTest("abcedfo".to_string()));
        Ok(())
    }

    #[test]
    fn test_de_enum() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        enum FooTest {
            Abc,
            Abd,
            Abe,
        }

        assert_eq!(FooTest::Abe, crate::from_str("Abe")?);

        Ok(())
    }

    #[test]
    fn test_de_enum_new_type_struct() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        enum FooTest {
            Abc(u32),
            Abd(u64),
            Abe(u8),
        }

        assert_eq!(FooTest::Abe(128), crate::from_str::<FooTest>("!Abe 128")?);

        Ok(())
    }

    #[test]
    fn test_de_enum_new_type_struct_string() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        enum FooTest {
            Abc(String),
            Abd(String),
            Abe(String),
        }

        assert_eq!(
            FooTest::Abe("128".into()),
            crate::from_str::<FooTest>("!Abe 128")?
        );

        Ok(())
    }

    #[test]
    fn test_de_array_of_enum_of_struct() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: u32,
            str_b: String,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct BarTest {
            uint_b: u32,
            str_c: String,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        enum EnumTest {
            Foo(FooTest),
            Bar(BarTest),
        }

        crate::testlib::init_logger();

        assert_eq!(
            vec![
                EnumTest::Foo(FooTest {
                    uint_a: 128,
                    str_b: "foo".into(),
                }),
                EnumTest::Bar(BarTest {
                    uint_b: 129,
                    str_c: "bar".into(),
                }),
            ],
            crate::from_str::<Vec<EnumTest>>(
                r#"
            ---
            - !Foo
              uint_a: 128
              str_b: foo
            - !Bar
              uint_b: 129
              str_c: bar
            "#
            )?
        );

        Ok(())
    }

    #[test]
    fn test_de_struct_with_enum_member() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: Option<EnumTest>,
        }

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        enum EnumTest {
            Bar(BarTest),
        }

        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct BarTest {
            uint_b: u32,
        }

        crate::testlib::init_logger();

        assert_eq!(
            FooTest {
                uint_a: Some(EnumTest::Bar(BarTest { uint_b: 32 }))
            },
            crate::from_str::<FooTest>(
                r#"
            ---
            uint_a:
              !Bar
              uint_b: 32
            "#
            )?
        );

        Ok(())
    }

    #[test]
    fn test_signed_interger() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            uint_a: i32,
        }

        assert_eq!(
            FooTest { uint_a: -128 },
            crate::from_str::<FooTest>(
                r#"
            ---
            uint_a: -128
            "#
            )?
        );
        Ok(())
    }

    #[test]
    fn test_empty_input() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            #[serde(default)]
            uint_a: u32,
        }

        assert_eq!(FooTest { uint_a: 0 }, crate::from_str::<FooTest>("")?);
        Ok(())
    }

    /*
    #[test]
    fn test_line_folding() -> Result<(), YamlError> {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
        struct FooTest {
            base: String,
        }

        let yaml_str = r#"
        base: >-
            interfaces.name==
            capture.default-gw.routes.running.0.next-hop-interface
            "#;

        assert_eq!(
            FooTest {
                base: "interfaces.name== \
                       capture.default-gw.routes.running.0.next-hop-interface"
                    .into(),
            },
            crate::from_str::<FooTest>(yaml_str)?
        );
        Ok(())
    }
    */
}
