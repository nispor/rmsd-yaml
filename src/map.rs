// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{
    RmsdDeserializer, RmsdError, RmsdPosition, TokensIter, YamlTokenData,
    YamlValue, YamlValueData,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlValueMap(IndexMap<YamlValue, YamlValue>);

impl std::hash::Hash for YamlValueMap {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let mut h: u64 = 0;
        for (k, v) in &self.0 {
            let mut hasher = DefaultHasher::new();
            k.hash(&mut hasher);
            v.hash(&mut hasher);
            h ^= hasher.finish();
        }
        state.write_u64(h);
    }
}

impl YamlValueMap {
    pub(crate) fn new() -> Self {
        Self(IndexMap::new())
    }

    pub(crate) fn insert(&mut self, key: YamlValue, val: YamlValue) {
        self.0.insert(key, val);
    }

    pub(crate) fn pop(&mut self) -> Option<(YamlValue, YamlValue)> {
        self.0.pop()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlValueMapAccess {
    data: YamlValueMap,
    // Used to cache key drained from data
    cached_key: Option<YamlValue>,
    // Used to cache value drained from data
    cached_value: Option<YamlValue>,
}

impl YamlValueMapAccess {
    pub(crate) fn new(data: YamlValueMap) -> Self {
        Self {
            data,
            cached_key: None,
            cached_value: None,
        }
    }
}

impl<'de> MapAccess<'de> for YamlValueMapAccess {
    type Error = RmsdError;

    fn next_key_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        let key = if let Some(k) = self.cached_key.take() {
            k
        } else if let Some((k, v)) = self.data.pop() {
            self.cached_value = Some(v);
            k
        } else {
            return Ok(None);
        };

        seed.deserialize(&mut RmsdDeserializer { parsed: key })
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let value = if let Some(v) = self.cached_value.take() {
            v
        } else if let Some((k, v)) = self.data.pop() {
            self.cached_key = Some(k);
            v
        } else {
            return Err(RmsdError::unexpected_yaml_node_type(
                "Expecting a map value, but none".to_string(),
                RmsdPosition::EOF,
            ));
        };

        seed.deserialize(&mut RmsdDeserializer { parsed: value })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len())
    }
}

pub(crate) fn get_map(
    iter: &mut TokensIter,
) -> Result<YamlValueMap, RmsdError> {
    let mut ret = YamlValueMap::new();

    let indent = if let Some(first_token) = iter.peek() {
        first_token.indent
    } else {
        return Ok(ret);
    };

    let mut key: Option<YamlValue> = None;
    while let Some(token) = iter.peek() {
        if token.indent < indent {
            return Ok(ret);
        }

        match &token.data {
            YamlTokenData::Scalar(_) => {
                if let Some(k) = key.take() {
                    if token.indent == indent {
                        // The unwrap is safe here as the `peek()` already check
                        // it is not None.
                        let token = iter.next().unwrap();
                        if let YamlTokenData::Scalar(s) = token.data {
                            let value = YamlValue {
                                data: YamlValueData::Scalar(s),
                                start: token.start,
                                end: token.end,
                            };
                            ret.insert(k, value);
                        } else {
                            unreachable!()
                        }
                    } else {
                        // The value is nested
                        let nested_tokens =
                            iter.remove_tokens_with_the_same_indent();
                        ret.insert(k, YamlValue::try_from(nested_tokens)?);
                    }
                } else {
                    // The unwrap is safe here as the `peek()` already check
                    // it is not None.
                    let token = iter.next().unwrap();
                    if let YamlTokenData::Scalar(s) = token.data {
                        key = Some(YamlValue {
                            data: YamlValueData::Scalar(s),
                            start: token.start,
                            end: token.end,
                        });
                    } else {
                        unreachable!();
                    }
                }
            }
            YamlTokenData::MapValueIndicator => {
                if key.is_none() {
                    return Err(RmsdError::unexpected_yaml_node_type(
                        "Got map value indicator `:` with \
                        no key defined before"
                            .to_string(),
                        token.start,
                    ));
                }
                iter.next();
            }
            // Support JSON format
            _ => todo!(),
        }
    }
    Ok(ret)
}
