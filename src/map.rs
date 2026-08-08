// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{
    ErrorKind, YamlCollectionStyle, YamlDeserializer, YamlError, YamlEvent,
    YamlParser, YamlPosition, YamlScalarStyle, YamlState, YamlValue,
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
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_map {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        self.push_event(YamlEvent::MapStart(
            anchor,
            tag,
            YamlCollectionStyle::Block,
            self.scanner.next_pos,
        ));
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
                let trimmed_key = line.trim_start_matches(' ');
                let mut value_anchor = None;
                let mut value_tag = None;
                if trimmed_key.starts_with('&') {
                    // e.g. `&a a: b` or `&key [a]: v`: the anchor belongs
                    // to the key node, not the map. After stripping the
                    // anchor, the remains act as a line starting at the
                    // scanner position.
                    self.scanner.advance(cur_indent);
                    let key_anchor = Some(self.handle_anchor()?);
                    while self.scanner.peek_char() == Some(' ') {
                        self.scanner.next_char();
                    }
                    match self.scanner.peek_char() {
                        Some('[') => {
                            self.handle_flow_seq(key_anchor, None)?;
                            self.expect_colon_after_key()?;
                        }
                        Some('{') => {
                            self.handle_flow_map(key_anchor, None)?;
                            self.expect_colon_after_key()?;
                        }
                        _ => {
                            self.handle_plain_scalar(0, 0, key_anchor, None)?;
                        }
                    }
                } else if trimmed_key.starts_with('*') {
                    // e.g. `*b : *a`: an alias as key
                    self.scanner.advance(cur_indent);
                    let name = self.handle_alias()?;
                    self.push_event(YamlEvent::Alias(
                        name,
                        self.scanner.next_pos,
                    ));
                    // Place the scanner at the `:` after the alias key so
                    // that the common post-key handling applies.
                    self.expect_colon_after_key()?;
                } else if trimmed_key.starts_with('[')
                    || trimmed_key.starts_with('{')
                {
                    // A flow collection as key, e.g. `[a, b]: value`
                    self.scanner.advance(cur_indent);
                    if trimmed_key.starts_with('[') {
                        self.handle_flow_seq(None, None)?;
                    } else {
                        self.handle_flow_map(None, None)?;
                    }
                    self.expect_colon_after_key()?;
                } else {
                    self.handle_plain_scalar(
                        desired_indent_count,
                        desired_indent_count,
                        None,
                        None,
                    )?;
                }
                let Some(line) = self.scanner.peek_line() else {
                    continue;
                };
                self.pop_state();
                self.push_state(YamlState::InBlockMapValue);
                let trimmed_line = line.trim_end_matches(' ');
                // TODO: Handle comment after `:`
                if trimmed_line.ends_with(":") && !line.contains(": ") {
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
                            None,
                            String::new(),
                            YamlScalarStyle::Plain,
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                        break;
                    }
                } else if line.contains(": ") {
                    self.scanner.advance_offset(2);
                    // Node properties of a same-line value, e.g.
                    // `a: &anchor`
                    while let Some(property_line) = self.scanner.peek_line() {
                        let property_trimmed =
                            property_line.trim_start_matches(' ');
                        let property_indent = property_line
                            .chars()
                            .take_while(|c| *c == ' ')
                            .count();
                        if property_trimmed.starts_with('&')
                            && value_anchor.is_none()
                        {
                            self.scanner.advance(property_indent);
                            value_anchor = Some(self.handle_anchor()?);
                        } else if property_trimmed.starts_with('!')
                            && value_tag.is_none()
                        {
                            self.scanner.advance(property_indent);
                            value_tag = self.handle_tag();
                        } else {
                            break;
                        }
                    }
                    if (value_anchor.is_some() || value_tag.is_some())
                        && self.value_has_no_content(desired_indent_count)
                    {
                        // e.g. `a: &anchor\nb: *anchor`: the node
                        // properties decorate an empty value node.
                        self.push_event(YamlEvent::Scalar(
                            value_anchor,
                            value_tag,
                            String::new(),
                            YamlScalarStyle::Plain,
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                        self.pop_state();
                        continue;
                    }
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
                    value_anchor,
                    value_tag,
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

    /// Check whether the value node carrying node properties (anchor or
    /// tag) is empty, e.g.
    ///     a: &anchor
    ///     b: *anchor
    /// The value is empty when nothing follows the node properties on the
    /// same line and the next content line is not indented deeper than
    /// the key.
    fn value_has_no_content(&self, key_indent_count: usize) -> bool {
        let mut remains = self.scanner.remains();
        if self.scanner.done_pos.line == self.scanner.next_pos.line {
            // Node properties ended mid-line: check the remains of this
            // line first.
            let rest_of_line =
                remains.split(['\n', '\r']).next().unwrap_or_default();
            let trimmed = rest_of_line.trim_start_matches(' ');
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                return false;
            }
            let Some(offset) = remains.find(['\n', '\r']) else {
                return true;
            };
            remains = &remains[offset + 1..];
        }
        for line in remains.split(['\n', '\r']) {
            let trimmed = line.trim_start_matches(' ');
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            return line.chars().take_while(|c| *c == ' ').count()
                <= key_indent_count;
        }
        true
    }

    /// Place the scanner at the `:` following a non-scalar mapping key
    /// (alias or flow collection).
    fn expect_colon_after_key(&mut self) -> Result<(), YamlError> {
        while self.scanner.peek_char() == Some(' ') {
            self.scanner.next_char();
        }
        if self.scanner.peek_char() != Some(':') {
            return Err(YamlError::new(
                ErrorKind::InvalidImplicitKey,
                format!(
                    "Expecting ':' after the mapping key, but got {:?}",
                    self.scanner.remains()
                ),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        Ok(())
    }

    /// Consume the scanner till a flow map is finished and insert the
    /// parsed events. The scanner must stay at `{`.
    ///
    /// YAML 1.2.2 SPEC, 7.4.2. Flow Mappings.
    pub(crate) fn handle_flow_map(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        let start_pos = self.scanner.next_pos;
        self.scanner.next_char();
        self.push_event(YamlEvent::MapStart(
            anchor,
            tag,
            YamlCollectionStyle::Flow,
            start_pos,
        ));
        self.push_state(YamlState::InFlowMapKey);
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some('}') {
            self.scanner.next_char();
        } else {
            loop {
                // Key
                match self.scanner.peek_char() {
                    Some(':') => {
                        // Empty key, e.g. `{: value}`
                        let pos = self.scanner.next_pos;
                        self.push_event(YamlEvent::Scalar(
                            None,
                            None,
                            String::new(),
                            YamlScalarStyle::Plain,
                            pos,
                            pos,
                        ));
                    }
                    Some('?') => {
                        // Explicit key, e.g. `{? key : value}`
                        self.scanner.next_char();
                        self.scanner.skip_flow_separation();
                        if self.scanner.peek_char() == Some(':') {
                            let pos = self.scanner.next_pos;
                            self.push_event(YamlEvent::Scalar(
                                None,
                                None,
                                String::new(),
                                YamlScalarStyle::Plain,
                                pos,
                                pos,
                            ));
                        } else {
                            self.handle_flow_node()?;
                        }
                    }
                    _ => {
                        self.handle_flow_node()?;
                    }
                }
                self.scanner.skip_flow_separation();
                // Value
                if self.scanner.peek_char() == Some(':') {
                    self.scanner.next_char();
                    self.scanner.skip_flow_separation();
                    self.handle_flow_node()?;
                } else {
                    // Omitted value, e.g. `{a}` or `{http://foo.com}`
                    let pos = self.scanner.next_pos;
                    self.push_event(YamlEvent::Scalar(
                        None,
                        None,
                        String::new(),
                        YamlScalarStyle::Plain,
                        pos,
                        pos,
                    ));
                }
                self.scanner.skip_flow_separation();
                match self.scanner.peek_char() {
                    Some(',') => {
                        self.scanner.next_char();
                        self.scanner.skip_flow_separation();
                        // A trailing comma before '}' is tolerated in
                        // flow mappings.
                        if self.scanner.peek_char() == Some('}') {
                            break;
                        }
                        if self.scanner.peek_char() == Some(',') {
                            return Err(YamlError::new(
                                ErrorKind::UnfinishedMapIndicator,
                                format!(
                                    "Expecting a key after ',' in flow \
                                     mapping, but got: {:?}",
                                    self.scanner.remains()
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    }
                    Some('}') => {
                        self.scanner.next_char();
                        break;
                    }
                    Some(c) => {
                        return Err(YamlError::new(
                            ErrorKind::UnfinishedMapIndicator,
                            format!(
                                "Expecting ',' or '}}' in flow mapping, but \
                                 got '{c}'"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                    None => {
                        return Err(YamlError::new(
                            ErrorKind::UnfinishedMapIndicator,
                            "Unfinished flow mapping".to_string(),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
            }
        }
        self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }
}
