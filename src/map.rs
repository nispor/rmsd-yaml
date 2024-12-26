// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{RmsdDeserializer, RmsdError, RmsdPosition, YamlValue};

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

        println!("HAHA key {:?}", key);

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

        println!("HAHA val {:?}", value);

        seed.deserialize(&mut RmsdDeserializer { parsed: value })
    }
}
