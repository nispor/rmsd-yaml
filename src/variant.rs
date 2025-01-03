// SPDX-License-Identifier: Apache-2.0

use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, VariantAccess, Visitor,
};

use crate::{
    RmsdDeserializer, RmsdError, RmsdPosition, TokensIter, YamlTag,
    YamlTokenData, YamlValue, YamlValueData,
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
    type Error = RmsdError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        if matches!(self.value.data, YamlValueData::Scalar(_)) {
            Ok(())
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!(
                    "Expecting enum/variant string, but got {}",
                    self.value.data
                ),
                RmsdPosition::EOF,
            ))
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        if let YamlValueData::Tag(tag) = &self.value.data {
            let value = YamlValue {
                start: self.value.start,
                end: self.value.end,
                data: tag.data.clone(),
            };
            seed.deserialize(&mut RmsdDeserializer { parsed: value })
        } else {
            seed.deserialize(&mut RmsdDeserializer {
                parsed: self.value.clone(),
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
        RmsdDeserializer {
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
        RmsdDeserializer {
            parsed: self.value.clone(),
        }
        .deserialize_map(visitor)
    }
}

impl<'de> EnumAccess<'de> for YamlValueEnumAccess {
    type Error = RmsdError;
    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let val = seed.deserialize(&mut RmsdDeserializer {
            parsed: self.value.clone(),
        })?;
        Ok((val, self))
    }
}

pub(crate) fn get_tag(iter: &mut TokensIter) -> Result<YamlValue, RmsdError> {
    if let Some(token) = iter.next() {
        if let YamlTokenData::LocalTag(name) = token.data {
            let data_tokens = iter.remove_tokens_with_the_same_or_more_indent();
            let end = if !data_tokens.is_empty() {
                if let Some(end_token) = data_tokens.last() {
                    end_token.end
                } else {
                    token.end
                }
            } else {
                token.end
            };
            let mut data_iter = TokensIter::new(data_tokens);
            Ok(YamlValue {
                start: token.start,
                end,
                data: YamlValueData::Tag(Box::new(YamlTag {
                    name,
                    data: YamlValue::parse(&mut data_iter)?.data,
                })),
            })
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!(
                    "get_tag() been invoked against TokensIter not leading \
                    with LocalTag but {}",
                    token.data
                ),
                token.start,
            ))
        }
    } else {
        Err(RmsdError::bug(
            "get_tag() been invoked against empty TokenIter".to_string(),
            RmsdPosition::EOF,
        ))
    }
}
