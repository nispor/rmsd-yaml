// SPDX-License-Identifier: Apache-2.0

use serde::{
    de::{DeserializeSeed, Deserializer, EnumAccess, VariantAccess, Visitor},
    forward_to_deserialize_any,
};

use crate::{
    Error, ErrorKind, Value, ValueData, YamlDeserializer, YamlScalarStyle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValueEnumAccess {
    value: Value,
    tag_name: Option<String>,
}

impl ValueEnumAccess {
    pub(crate) fn new(value: Value) -> Self {
        let tag_name = match &value.data {
            ValueData::Tag(tag) => Some(tag.name.clone()),
            _ => None,
        };
        Self { value, tag_name }
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
        if let Some(tag_name) = &self.tag_name {
            let value = Value {
                start: self.value.start,
                end: self.value.end,
                data: self.value.data,
                meta: self.value.meta.clone(),
            };
            let mut deserializer = YamlDeserializer {
                parsed: value,
                ..Default::default()
            };
            match tag_name.as_str() {
                "<tag:yaml.org,2002:str>" | "<!>" => {
                    deserializer.parsed.meta.scalar_style =
                        Some(YamlScalarStyle::SingleQuoted);
                }
                "<tag:yaml.org,2002:int>"
                | "<tag:yaml.org,2002:float>"
                | "<tag:yaml.org,2002:bool>"
                | "<tag:yaml.org,2002:null>" => {
                    deserializer.parsed.meta.scalar_style =
                        Some(YamlScalarStyle::Plain);
                }
                _ => {}
            }
            seed.deserialize(&mut deserializer)
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
            let tag_name = TagNameDeserializer::new(tag.name.as_str());
            Ok((
                seed.deserialize(tag_name)?,
                Self {
                    value: Value {
                        data: tag.data.clone(),
                        start: self.value.start,
                        end: self.value.end,
                        meta: self.value.meta.clone(),
                    },
                    tag_name: self.tag_name.clone(),
                },
            ))
        } else {
            Ok((
                seed.deserialize(&mut YamlDeserializer {
                    parsed: self.value.clone(),
                    ..Default::default()
                })?,
                Self {
                    value: self.value,
                    tag_name: None,
                },
            ))
        }
    }
}

/// Deserializer for a resolved tag name. Rust enum variant seeds
/// receive the short variant name (`str`, `Alpha`); a `Value` seed
/// receives the full resolved tag (`<tag:yaml.org,2002:str>`,
/// `<!Alpha>`) by using `deserialize_any`.
struct TagNameDeserializer {
    full: String,
    short: String,
}

impl TagNameDeserializer {
    fn new(full: &str) -> Self {
        Self {
            full: full.to_string(),
            short: variant_name_from_tag(full).to_string(),
        }
    }
}

impl<'de> Deserializer<'de> for TagNameDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.full)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(&self.short)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.short)
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

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf option unit unit_struct newtype_struct seq
        tuple tuple_struct map struct enum ignored_any
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
