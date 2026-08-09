// SPDX-License-Identifier: Apache-2.0

use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, VariantAccess, Visitor,
    value::StrDeserializer,
};

use crate::{Error, ErrorKind, Value, ValueData, YamlDeserializer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValueEnumAccess {
    value: Value,
}

impl ValueEnumAccess {
    pub(crate) fn new(value: Value) -> Self {
        Self { value }
    }
}

impl<'de> VariantAccess<'de> for ValueEnumAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        if matches!(self.value.data, ValueData::String(_)) {
            Ok(())
        } else {
            Err(Error::new(
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
        if let ValueData::Tag(tag) = self.value.data {
            let value = Value {
                start: self.value.start,
                end: self.value.end,
                data: tag.data,
                ..Default::default()
            };
            seed.deserialize(&mut YamlDeserializer {
                parsed: value,
                ..Default::default()
            })
        } else {
            seed.deserialize(&mut YamlDeserializer {
                parsed: self.value,
                ..Default::default()
            })
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
            ..Default::default()
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
            ..Default::default()
        }
        .deserialize_map(visitor)
    }
}

impl<'de> EnumAccess<'de> for ValueEnumAccess {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        if let ValueData::Tag(tag) = self.value.data {
            let tag_name = StrDeserializer::<Self::Error>::new(
                variant_name_from_tag(tag.name.as_str()),
            );
            Ok((
                seed.deserialize(tag_name)?,
                Self {
                    value: Value {
                        data: tag.data.clone(),
                        start: self.value.start,
                        end: self.value.end,
                        ..Default::default()
                    },
                },
            ))
        } else {
            Ok((
                seed.deserialize(&mut YamlDeserializer {
                    parsed: self.value.clone(),
                    ..Default::default()
                })?,
                self,
            ))
        }
    }
}

/// Extract the enum variant name from a tag string. The serializer
/// renders enum variants as local tags (e.g. `!Variant`), which the
/// parser stores as `<!Variant>` or `<tag:yaml.org,2002:Variant>`.
fn variant_name_from_tag(tag_name: &str) -> &str {
    let mut ret = tag_name;
    if let Some(stripped) = ret.strip_prefix('<')
        && let Some(stripped) = stripped.strip_suffix('>')
    {
        ret = stripped;
    }
    if let Some(stripped) = ret.strip_prefix('!') {
        ret = stripped;
    }
    ret.rsplit_once(':').map(|(_, name)| name).unwrap_or(ret)
}
