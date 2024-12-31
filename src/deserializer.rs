// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use serde::de::{Deserializer, Visitor};
use serde::Deserialize;

use crate::{
    RmsdError, YamlValue, YamlValueData, YamlValueEnumAccess,
    YamlValueMapAccess, YamlValueSeqAccess,
};

#[derive(Debug, Default)]
pub struct RmsdDeserializer {
    pub(crate) parsed: YamlValue,
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T, RmsdError>
where
    T: Deserialize<'a>,
{
    let parsed = YamlValue::from_str(s)?;
    println!("HAHA {:?}", parsed);
    let mut deserializer = RmsdDeserializer { parsed };

    T::deserialize(&mut deserializer)
}

pub fn to_value(input: &str) -> Result<YamlValue, RmsdError> {
    YamlValue::from_str(input)
}

impl<'de> Deserializer<'de> for &mut RmsdDeserializer {
    type Error = RmsdError;

    // Look at the input data to decide what Serde data model type to
    // deserialize as. Not all data formats are able to support this operation.
    // Formats that support `deserialize_any` are known as self-describing.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.parsed.data {
            YamlValueData::Scalar(_) => self.deserialize_str(visitor),
            YamlValueData::Sequence(_) => self.deserialize_seq(visitor),
            YamlValueData::Map(_) => self.deserialize_map(visitor),
            v => Err(RmsdError::bug(
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

    fn deserialize_i8<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i16<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i32<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_i64<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
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

    fn deserialize_option<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        todo!()
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
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
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
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
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
}
