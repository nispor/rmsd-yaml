// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::str::FromStr;

use serde::de::{Deserializer, Visitor};
use serde::Deserialize;

use crate::{
    YamlError, YamlValue, YamlValueData, YamlValueEnumAccess,
    YamlValueMapAccess, YamlValueSeqAccess,
};

#[derive(Debug, Default)]
pub struct RmsdDeserializer {
    pub(crate) parsed: YamlValue,
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T, YamlError>
where
    T: Deserialize<'a>,
{
    let parsed = YamlValue::from_str(s)?;
    let mut deserializer = RmsdDeserializer { parsed };

    T::deserialize(&mut deserializer)
}

pub fn to_value(input: &str) -> Result<YamlValue, YamlError> {
    YamlValue::from_str(input)
}

impl<'de> Deserializer<'de> for &mut RmsdDeserializer {
    type Error = YamlError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.parsed.data {
            YamlValueData::Scalar(_) => {
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
            YamlValueData::Sequence(_) => self.deserialize_seq(visitor),
            YamlValueData::Map(_) => self.deserialize_map(visitor),
            v => Err(YamlError::bug(
                format!("deserialize_any() got unexpected data {v:?}"),
                self.parsed.start,
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
        if let YamlValueData::Sequence(v) = &self.parsed.data {
            // TODO: We cannot move data output of `&mut self`, so we use
            // to_vec() to clone here. Maybe should use `Option<YamlValue>` for
            // Self::parsed, where we can use `Option::take()` to move data out.
            let access = YamlValueSeqAccess::new(v.to_vec());
            visitor.visit_seq(access)
        } else if let YamlValueData::Tag(tag) = &self.parsed.data {
            if let YamlValueData::Sequence(v) = &tag.data {
                let access = YamlValueSeqAccess::new(v.to_vec());
                visitor.visit_seq(access)
            } else {
                Err(YamlError::unexpected_yaml_node_type(
                    format!(
                        "Expecting a sequence in tag, got {}",
                        self.parsed.data
                    ),
                    self.parsed.start,
                ))
            }
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a sequence, got {}", self.parsed.data),
                self.parsed.start,
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
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a map, got {}", self.parsed.data),
                self.parsed.start,
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
