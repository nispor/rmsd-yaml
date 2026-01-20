// SPDX-License-Identifier: Apache-2.0

use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, VariantAccess, Visitor,
    value::StrDeserializer,
};

use crate::{
    ErrorKind, YamlDeserializer, YamlError, YamlValue,
    YamlValueData,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlValueEnumAccess {
    value: YamlValue,
}

impl YamlValueEnumAccess {
    pub(crate) fn new(value: YamlValue) -> Self {
        Self { value }
    }
}

impl<'de> VariantAccess<'de> for YamlValueEnumAccess {
    type Error = YamlError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        if matches!(self.value.data, YamlValueData::String(_)) {
            Ok(())
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!(
                    "Expecting enum/variant string, but got {}",
                    self.value.data
                ),
                self.value.start,
                self.value.end,
            ))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if let YamlValueData::Tag(tag) = self.value.data {
            let value = YamlValue {
                start: self.value.start,
                end: self.value.end,
                data: tag.data,
            };
            seed.deserialize(&mut YamlDeserializer { parsed: value })
        } else {
            seed.deserialize(&mut YamlDeserializer { parsed: self.value })
        }
    }

    fn tuple_variant<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        YamlDeserializer {
            parsed: self.value.clone(),
        }
        .deserialize_seq(visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        YamlDeserializer {
            parsed: self.value.clone(),
        }
        .deserialize_map(visitor)
    }
}

impl<'de> EnumAccess<'de> for YamlValueEnumAccess {
    type Error = YamlError;
    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        if let YamlValueData::Tag(tag) = self.value.data {
            let tag_name =
                StrDeserializer::<Self::Error>::new(tag.name.as_str());
            Ok((
                seed.deserialize(tag_name)?,
                Self {
                    value: YamlValue {
                        data: tag.data.clone(),
                        start: self.value.start,
                        end: self.value.end,
                    },
                },
            ))
        } else {
            Ok((
                seed.deserialize(&mut YamlDeserializer {
                    parsed: self.value.clone(),
                })?,
                self,
            ))
        }
    }
}
