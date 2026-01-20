// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{
    ErrorKind, YamlDeserializer, YamlError, YamlEvent, YamlParser,
    YamlPosition, YamlState, YamlValue,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    type Error = YamlError;

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

        seed.deserialize(&mut YamlDeserializer { parsed: key })
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
            return Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                "Expecting a map value, but none".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        };

        seed.deserialize(&mut YamlDeserializer { parsed: value })
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len())
    }
}

impl<'a> YamlParser<'a> {
    /// Consume the scanner till a block map is finished.
    pub(crate) fn handle_block_map(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_map {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        self.push_event(YamlEvent::MapStart(tag, self.scanner.next_pos));
        self.push_state(YamlState::InBlockMapKey);
        let mut value_first_indent_count = first_indent_count;
        let mut value_rest_indent_count = first_indent_count;
        let mut is_first_line = true;
        while let Some(line) = self.scanner.peek_line() {
            let pre_pos = self.scanner.done_pos;
            if line.is_empty() {
                self.scanner.next_line();
                continue;
            }
            let cur_indent = line.chars().take_while(|c| *c == ' ').count();
            let desired_indent_count = if is_first_line {
                is_first_line = false;
                first_indent_count
            } else {
                rest_indent_count
            };

            if cur_indent < desired_indent_count {
                break;
            }

            if self.cur_state().is_block_map_value() {
                self.handle_node(
                    value_first_indent_count,
                    value_rest_indent_count,
                    None,
                )?;
                self.pop_state();
            } else {
                if !self.cur_state().is_block_map_key() {
                    self.push_state(YamlState::InBlockMapKey);
                }
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                let _spliter_offset = line.find(": ");
                self.handle_plain_scalar(
                    desired_indent_count,
                    desired_indent_count,
                    None,
                )?;
                let Some(line) = self.scanner.peek_line() else {
                    continue;
                };
                self.pop_state();
                self.push_state(YamlState::InBlockMapValue);
                let trimmed_line = line.trim_end_matches(' ');
                // TODO: Handle comment after `:`
                if trimmed_line.ends_with(":") {
                    self.scanner.next_line();
                    if let Some(next_line) = self.scanner.peek_line() {
                        let next_line_indent_count =
                            next_line.chars().take_while(|c| *c == ' ').count();
                        if next_line_indent_count < desired_indent_count {
                            return Err(YamlError::new(
                                ErrorKind::Bug,
                                format!(
                                    "Got less indented than parent: {}",
                                    self.scanner.remains()
                                ),
                                self.scanner.done_pos,
                                self.scanner.done_pos,
                            ));
                        } else {
                            value_first_indent_count = next_line_indent_count;
                            value_rest_indent_count = next_line_indent_count;
                        }
                    } else {
                        // No next line after ':\n', so empty value
                        self.push_event(YamlEvent::Scalar(
                            None,
                            String::new(),
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                        break;
                    }
                } else if line.contains(": ") {
                    self.scanner.advance_offset(2);
                    value_first_indent_count = 0;
                    value_rest_indent_count = self.scanner.done_pos.column;
                } else if trimmed_line.is_empty() {
                    self.scanner.next_line();
                } else {
                    return Err(YamlError::new(
                        ErrorKind::Bug,
                        format!(
                            "Expecting ending with : or contains ': ', but \
                             got {}",
                            line
                        ),
                        self.scanner.done_pos,
                        self.scanner.done_pos,
                    ));
                }
                self.handle_node(
                    value_first_indent_count,
                    value_rest_indent_count,
                    None,
                )?;
                self.pop_state();
            }
            if pre_pos == self.scanner.done_pos {
                return Err(YamlError::new(
                    ErrorKind::Bug,
                    format!(
                        "handle_block_map(): Dead loop on: {:?}",
                        self.scanner.remains()
                    ),
                    self.scanner.done_pos,
                    self.scanner.done_pos,
                ));
            }
        }

        self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }

    /// Consume the scanner till a flow map is finished and insert the parsed
    /// event.
    pub(crate) fn handle_flow_map(
        &mut self,
        _tag: Option<String>,
    ) -> Result<(), YamlError> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::YamlPosition;

    #[test]
    fn test_map_of_plain_scalar() {
        crate::testlib::init_logger();

        assert_eq!(
            YamlParser::parse_to_events("a: 1\nb: 2\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::MapStart(None, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    None,
                    "a".to_string(),
                    YamlPosition::new(1, 1),
                    YamlPosition::new(1, 1)
                ),
                YamlEvent::Scalar(
                    None,
                    "1".to_string(),
                    YamlPosition::new(1, 4),
                    YamlPosition::new(1, 4)
                ),
                YamlEvent::Scalar(
                    None,
                    "b".to_string(),
                    YamlPosition::new(2, 1),
                    YamlPosition::new(2, 1)
                ),
                YamlEvent::Scalar(
                    None,
                    "2".to_string(),
                    YamlPosition::new(2, 4),
                    YamlPosition::new(2, 4)
                ),
                YamlEvent::MapEnd(YamlPosition::new(2, 5)),
                YamlEvent::DocumentEnd(false, YamlPosition::new(2, 5)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_map_of_plain_scalar_in_two_lines() {
        assert_eq!(
            YamlParser::parse_to_events("a:\n  b\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::MapStart(None, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    None,
                    "a".to_string(),
                    YamlPosition::new(1, 1),
                    YamlPosition::new(1, 1)
                ),
                YamlEvent::Scalar(
                    None,
                    "b".to_string(),
                    YamlPosition::new(2, 3),
                    YamlPosition::new(2, 3)
                ),
                YamlEvent::MapEnd(YamlPosition::new(2, 4)),
                YamlEvent::DocumentEnd(false, YamlPosition::new(2, 4)),
                YamlEvent::StreamEnd,
            ]
        )
    }
}
