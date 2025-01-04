// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{
    get_array, get_tag, RmsdDeserializer, RmsdError, RmsdPosition, TokensIter,
    YamlTokenData, YamlValue, YamlValueData,
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

// Should have leading { and ending } removed.
// Since we cannot tell the start and end for empty iter, please
// check it before call this function, otherwise an error will emit.
pub(crate) fn get_map(
    iter: &mut TokensIter,
    in_flow: bool,
) -> Result<YamlValue, RmsdError> {
    let mut map = YamlValueMap::new();

    let (start, mut end, indent) = if let Some(first_token) = iter.peek() {
        (first_token.start, first_token.end, first_token.indent)
    } else {
        return Err(RmsdError::bug(
            "get_map(): Got empty tokens".to_string(),
            RmsdPosition::EOF,
        ));
    };

    let mut key: Option<YamlValue> = None;
    while let Some(token) = iter.peek() {
        // Only break on indent if not inside of flow style
        if (!in_flow) && token.indent < indent {
            break;
        }

        match &token.data {
            YamlTokenData::Scalar(_) => {
                let value = if in_flow || token.indent == indent {
                    let token = iter.next().unwrap();
                    YamlValue::parse(&mut TokensIter::new(vec![token]))?
                } else {
                    // nested map
                    YamlValue::parse(&mut TokensIter::new(
                        iter.remove_tokens_with_the_same_or_more_indent(),
                    ))?
                };
                if let Some(k) = key.take() {
                    end = value.end;
                    map.insert(k, value);
                } else {
                    key = Some(value);
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
            YamlTokenData::FlowMapStart => {
                let start = token.start;
                let sub_tokens = iter.remove_tokens_of_map_flow(false)?;
                let value = if sub_tokens.is_empty() {
                    YamlValue {
                        data: YamlValueData::Map(Box::new(YamlValueMap::new())),
                        start,
                        end: iter.end,
                    }
                } else {
                    let mut sub_iter = TokensIter::new(sub_tokens);
                    get_map(&mut sub_iter, true)?
                };
                if let Some(k) = key.take() {
                    end = value.end;
                    map.insert(k, value);
                } else {
                    key = Some(value);
                }
            }
            YamlTokenData::BlockSequenceIndicator => {
                let value = YamlValue::parse(&mut TokensIter::new(
                    iter.remove_tokens_with_the_same_or_more_indent(),
                ))?;
                if let Some(k) = key.take() {
                    end = value.end;
                    map.insert(k, value);
                } else {
                    key = Some(value);
                }
            }
            YamlTokenData::FlowSequenceStart => {
                let start = token.start;
                let sub_tokens = iter.remove_tokens_of_seq_flow(false)?;
                let value = if sub_tokens.is_empty() {
                    YamlValue {
                        start,
                        end: iter.end,
                        data: YamlValueData::Sequence(Vec::new()),
                    }
                } else {
                    let mut sub_iter = TokensIter::new(sub_tokens);
                    get_array(&mut sub_iter, true)?
                };
                if let Some(k) = key.take() {
                    end = value.end;
                    map.insert(k, value);
                } else {
                    key = Some(value);
                }
            }
            YamlTokenData::LocalTag(_) => {
                let value = get_tag(iter)?;
                if let Some(k) = key.take() {
                    end = value.end;
                    map.insert(k, value);
                } else {
                    key = Some(value);
                }
            }
            YamlTokenData::CollectEntry => {
                if !in_flow {
                    return Err(RmsdError::bug(
                        format!("get_map(): Unexpected token {}", token),
                        token.start,
                    ));
                }
                iter.next();
            }
            _ => {
                return Err(RmsdError::bug(
                    format!("get_map(): Unexpected token {}", token),
                    token.start,
                ));
            }
        }
    }
    Ok(YamlValue {
        start,
        end,
        data: YamlValueData::Map(Box::new(map)),
    })
}
