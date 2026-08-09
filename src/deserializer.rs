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
                } else if self.parsed.is_float() {
                    self.deserialize_f64(visitor)
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

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(self.parsed.as_f64()?)
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

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Only `!!binary` tagged scalars carry binary data; plain
        // scalars and other tags are rejected like serde_yaml.
        if let YamlValueData::Tag(tag) = &self.parsed.data
            && tag.name == "<tag:yaml.org,2002:binary>"
            && let YamlValueData::String(s) = &tag.data
        {
            let bytes = crate::base64::decode(s).map_err(|e| {
                YamlError::new(
                    ErrorKind::InvalidNumber,
                    format!("Invalid base64 in !!binary tag: {e}"),
                    self.parsed.start,
                    self.parsed.end,
                )
            })?;
            return visitor.visit_byte_buf(bytes);
        }
        Err(YamlError::new(
            ErrorKind::BytesUnsupported,
            "Deserializing bytes is not supported (expected !!binary tag)"
                .to_string(),
            self.parsed.start,
            self.parsed.end,
        ))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.parsed.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.parsed.is_null() {
            visitor.visit_unit()
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting null, but got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
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
