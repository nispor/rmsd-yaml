// SPDX-License-Identifier: Apache-2.0

use std::hash::{DefaultHasher, Hasher};

use indexmap::IndexMap;
use serde::de::{DeserializeSeed, MapAccess};

use crate::{
    Error, ErrorKind, Value, YamlCollectionStyle, YamlDeserializer, YamlEvent,
    YamlParser, YamlPosition, YamlScalarStyle, YamlState,
    parser::{
        find_key_value_separator, flow_collection_is_key,
        is_document_end_marker, is_document_start_marker,
        tab_content_is_block_node,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Mapping(IndexMap<Value, Value>);

impl std::hash::Hash for Mapping {
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

impl Mapping {
    pub fn new() -> Self {
        Self(IndexMap::new())
    }

    pub fn insert(&mut self, key: Value, val: Value) {
        self.0.insert(key, val);
    }

    pub fn get(&self, key: &Value) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn contains_key(&self, key: &Value) -> bool {
        self.0.contains_key(key)
    }

    pub fn pop(&mut self) -> Option<(Value, Value)> {
        self.0.pop()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Value, &Value)> {
        self.0.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MappingAccess {
    data: Mapping,
    // Used to cache key drained from data
    cached_key: Option<Value>,
    // Used to cache value drained from data
    cached_value: Option<Value>,
    // Path of the mapping itself, e.g. `config[0]`, used to build the
    // value path (`config[0].cwnd`) for `invalid type` errors.
    path: String,
    // Key of the entry whose value is being deserialized.
    current_key: Option<Value>,
}

impl MappingAccess {
    pub(crate) fn new(data: Mapping, path: String) -> Self {
        Self {
            data,
            cached_key: None,
            cached_value: None,
            path,
            current_key: None,
        }
    }
}

impl<'de> MapAccess<'de> for MappingAccess {
    type Error = Error;

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
        self.current_key = Some(key.clone());

        seed.deserialize(&mut YamlDeserializer {
            parsed: key,
            path: String::new(),
        })
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
            return Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                "Expecting a map value, but none".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        };

        let mut path = self.path.clone();
        if let Some(key) = self.current_key.take()
            && let Ok(key) = key.as_str()
        {
            if path.is_empty() {
                path = key.to_string();
            } else {
                path.push('.');
                path.push_str(key);
            }
        }

        seed.deserialize(&mut YamlDeserializer {
            parsed: value,
            path: path.clone(),
        })
        .map_err(|e| e.with_path(&path))
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
    ) -> Result<(), Error> {
        log::trace!(
            "handle_block_map {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        let saved_block_indent = self.block_indent;
        let saved_map_key_indent = self.map_key_indent;
        self.block_indent = Some(rest_indent_count);
        self.map_key_indent = None;
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
            if line.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
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

            let trimmed_line = line.trim_start_matches(' ');
            if trimmed_line.starts_with('#')
                && self.scanner.done_pos.line != self.scanner.next_pos.line
            {
                // Comment lines do not belong to the mapping. The
                // line-start check excludes trailing content on a line
                // that is still being processed, e.g. `key: "v"# bad`.
                self.scanner.advance_till_linebreak();
                continue;
            }
            if trimmed_line.starts_with('#')
                && self.scanner.done_pos.line == self.scanner.next_pos.line
            {
                // A trailing comment on the line where the previous
                // node ended (e.g. `a: "v" # comment`): the comment is
                // a presentation detail, consume it and move on.
                self.scanner.advance_till_linebreak();
                continue;
            }
            if is_document_end_marker(trimmed_line) {
                // Document end marker: leave it for the stream handler.
                break;
            }
            if is_document_start_marker(trimmed_line) {
                // Document start marker: the mapping ends here.
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
                // Back to key mode for the next iteration.
                self.push_state(YamlState::InBlockMapKey);
            } else {
                if !self.cur_state().is_block_map_key() {
                    self.push_state(YamlState::InBlockMapKey);
                }
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                // All keys of a block mapping must sit at the same
                // column as the first key (YAML 1.2.2 SPEC, 8.2.2);
                // a key at a different indentation is an error (e.g.
                // `map:\n  key1: v\n key2: v`).
                if let Some(key_indent) = self.map_key_indent {
                    if cur_indent != key_indent {
                        return Err(Error::new(
                            ErrorKind::LessIndentedWithoutParent,
                            format!(
                                "A block mapping key must sit at column \
                                 {key_indent} like the first key, but got: \
                                 {line}"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                } else {
                    // The key's own column: for a key that follows
                    // content on the same line (e.g. a block sequence
                    // entry) that is the scanner's column, otherwise
                    // the line's indentation.
                    self.map_key_indent = if self.scanner.done_pos.line
                        == self.scanner.next_pos.line
                    {
                        Some(self.scanner.next_pos.column.saturating_sub(1))
                    } else {
                        Some(cur_indent)
                    };
                }
                let trimmed_key = line.trim_start_matches(' ');
                if trimmed_key.starts_with('\t')
                    && tab_content_is_block_node(
                        trimmed_key.trim_start_matches('\t'),
                    )
                {
                    // A tab in the key indentation followed by
                    // block-node-looking content is invalid.
                    return Err(Error::new(
                        ErrorKind::InvalidStartOfToken,
                        "Tab(\\t) cannot be used as indentation in a mapping \
                         key"
                        .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                let mut value_anchor = None;
                let mut value_tag = None;
                if trimmed_key == "?"
                    || trimmed_key.starts_with("? ")
                    || trimmed_key.starts_with("?\t")
                {
                    log::trace!(
                        "handle_block_map explicit key: {trimmed_key:?}"
                    );
                    // Explicit key (YAML 1.2.2 SPEC, 8.2.4), e.g.
                    //     ? explicit key
                    //     : value
                    self.scanner.advance(cur_indent);
                    self.scanner.next_char(); // consume '?'
                    self.skip_block_indicator_separation(
                        trimmed_key.starts_with("?\t"),
                    )?;
                    self.scanner.skip_flow_separation();
                    self.handle_explicit_key()?;
                    self.pop_state();
                    self.push_state(YamlState::InBlockMapValue);
                    self.scanner.skip_flow_separation();
                    let mut value_consumed = false;
                    if self.scanner.peek_char() == Some(':')
                        && matches!(
                            self.scanner.remains().chars().nth(1),
                            None | Some(' ')
                                | Some('\t')
                                | Some('\n')
                                | Some('\r')
                                | Some('#')
                        )
                    {
                        // Same-line `: value` after the key.
                        self.scanner.next_char();
                        self.parse_explicit_value_after_colon()?;
                        value_consumed = true;
                    } else if let Some(next_line) = self.scanner.peek_line() {
                        let next_trimmed = next_line.trim_start_matches(' ');
                        if next_trimmed == ":"
                            || next_trimmed.starts_with(": ")
                            || next_trimmed.starts_with(":\t")
                            || next_trimmed.starts_with(":#")
                        {
                            // Value on the following line: `: value`.
                            self.scanner.advance_till_non_space();
                            self.scanner.next_char(); // consume ':'
                            self.parse_explicit_value_after_colon()?;
                            value_consumed = true;
                        }
                    }
                    if !value_consumed {
                        // Explicit key without a value.
                        self.push_event(YamlEvent::Scalar(
                            None,
                            None,
                            String::new(),
                            YamlScalarStyle::Plain,
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                    }
                    self.pop_state();
                    // Back to key mode for the next iteration.
                    self.push_state(YamlState::InBlockMapKey);
                    continue;
                } else if trimmed_key == ":"
                    || trimmed_key.starts_with(": ")
                    || trimmed_key.starts_with(":\t")
                    || trimmed_key.starts_with(":#")
                {
                    // An explicit entry with an empty key, e.g.
                    // `: value` or `: # comment` (YAML 1.2.2 SPEC,
                    // 8.2.4).
                    log::trace!(
                        "handle_block_map explicit empty key: {trimmed_key:?}"
                    );
                    self.scanner.advance(cur_indent);
                    self.scanner.next_char(); // consume ':'
                    let pos = self.scanner.done_pos;
                    self.push_event(YamlEvent::Scalar(
                        None,
                        None,
                        String::new(),
                        YamlScalarStyle::Plain,
                        pos,
                        pos,
                    ));
                    self.pop_state();
                    self.push_state(YamlState::InBlockMapValue);
                    self.parse_explicit_value_after_colon()?;
                    self.pop_state();
                    self.push_state(YamlState::InBlockMapKey);
                    continue;
                } else if trimmed_key.starts_with('&') {
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
                        Some('\'') | Some('"') => {
                            // An anchored quoted key, e.g.
                            // `&anchor 'key': value`.
                            self.handle_scalar(0, 0, key_anchor, None)?;
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
                } else if trimmed_key.starts_with('\'')
                    || trimmed_key.starts_with('"')
                {
                    // A quoted scalar as key, e.g. `"foo": 23`
                    self.scanner.advance(cur_indent);
                    self.handle_scalar(0, 0, None, None)?;
                    self.expect_colon_after_key()?;
                } else if trimmed_key.starts_with('!') {
                    // A tagged key, e.g. `!!str : value` (empty tagged
                    // scalar) or `!foo key : value`.
                    // The key content (or the `:` separator) must sit
                    // on the tag's own line; a tag alone (`!!map` with
                    // the content on the next line) is an error.
                    let tag_token =
                        trimmed_key.split([' ', '\t']).next().unwrap_or("");
                    let after_tag = trimmed_key[tag_token.len()..]
                        .trim_start_matches([' ', '\t'])
                        .split('#')
                        .next()
                        .unwrap_or("")
                        .trim_end();
                    if after_tag.is_empty() {
                        return Err(Error::new(
                            ErrorKind::InvalidImplicitKey,
                            format!(
                                "A tagged mapping key must have content or a \
                                 ':' on the same line, but got: \
                                 {trimmed_key:?}"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                    self.scanner.advance(cur_indent);
                    let key_tag = self.handle_tag()?;
                    // The key content (or the `:` separator) must sit
                    // on the tag's own line; a line break right after
                    // the tag is an error (e.g. `!!map\n  a: b`).
                    while matches!(
                        self.scanner.peek_char(),
                        Some(' ') | Some('\t')
                    ) {
                        self.scanner.next_char();
                    }
                    if self.scanner.peek_char() == Some('&') {
                        // A tagged and anchored key, e.g.
                        // `!!str &a "foo": value`.
                        let key_anchor = Some(self.handle_anchor()?);
                        self.scanner.skip_flow_separation();
                        if self.scanner.peek_char() == Some(':') {
                            let pos = self.scanner.next_pos;
                            self.push_event(YamlEvent::Scalar(
                                key_anchor,
                                key_tag,
                                String::new(),
                                YamlScalarStyle::Plain,
                                pos,
                                pos,
                            ));
                            self.expect_colon_after_key()?;
                        } else if matches!(
                            self.scanner.peek_char(),
                            Some('\'') | Some('"')
                        ) {
                            self.handle_scalar(0, 0, key_anchor, key_tag)?;
                            self.expect_colon_after_key()?;
                        } else {
                            self.handle_plain_scalar(
                                0, 0, key_anchor, key_tag,
                            )?;
                        }
                    } else if self.scanner.peek_char() == Some(':') {
                        let pos = self.scanner.next_pos;
                        self.push_event(YamlEvent::Scalar(
                            None,
                            key_tag,
                            String::new(),
                            YamlScalarStyle::Plain,
                            pos,
                            pos,
                        ));
                        self.expect_colon_after_key()?;
                    } else {
                        self.handle_plain_scalar(0, 0, None, key_tag)?;
                    }
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
                    self.skip_comment_and_empty_lines();
                    if let Some(next_line) = self.scanner.peek_line() {
                        let next_line_indent_count =
                            next_line.chars().take_while(|c| *c == ' ').count();
                        let next_trimmed =
                            next_line.trim_start_matches([' ', '\t']);
                        let is_seq = next_trimmed == "-"
                            || next_trimmed.starts_with("- ");
                        // The value must be indented deeper than the
                        // key, except for a zero-indented block
                        // sequence (YAML 1.2.2 SPEC, 8.2.2), e.g.
                        // `seq:\n&anchor` is an error but `seq:\n- a`
                        // is a valid zero-indented sequence.
                        if next_line_indent_count < desired_indent_count
                            || (next_line_indent_count == desired_indent_count
                                && !is_seq)
                        {
                            return Err(Error::new(
                                ErrorKind::Bug,
                                format!(
                                    "The value of a mapping entry must be \
                                     indented more than the key: {next_line}"
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
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
                } else if line.contains(": ") || line.contains(":\t") {
                    self.scanner.advance_offset(2);
                    // A tab right after the `:` is part of the value
                    // separation; validate it like any other block
                    // indicator separation.
                    self.skip_block_indicator_separation(false)?;
                    // Node properties of a same-line value, e.g.
                    // `a: &anchor`
                    while let Some(property_line) = self.scanner.peek_line() {
                        let property_trimmed =
                            property_line.trim_start_matches(' ');
                        let property_indent = property_line
                            .chars()
                            .take_while(|c| *c == ' ')
                            .count();
                        // A property on a new line must be indented
                        // deeper than the mapping; a line at the same
                        // indentation is a sibling key (e.g.
                        // `key: &x\n!!map`).
                        if self.scanner.done_pos.line
                            != self.scanner.next_pos.line
                            && property_indent <= desired_indent_count
                        {
                            break;
                        }
                        if property_trimmed.starts_with('&')
                            && value_anchor.is_none()
                        {
                            self.scanner.advance(property_indent);
                            value_anchor = Some(self.handle_anchor()?);
                        } else if property_trimmed.starts_with('!')
                            && value_tag.is_none()
                        {
                            self.scanner.advance(property_indent);
                            value_tag = self.handle_tag()?;
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
                        // Back to key mode for the next iteration.
                        self.push_state(YamlState::InBlockMapKey);
                        continue;
                    }
                    if self.scanner.done_pos.line != self.scanner.next_pos.line
                    {
                        // The node properties decorate a node whose
                        // content sits on the following lines (e.g.
                        // `a: !B\n  - 1`); re-derive the content
                        // indentation from the first content line.
                        self.skip_comment_and_empty_lines();
                        if let Some(content_line) = self.scanner.peek_line() {
                            value_first_indent_count = content_line
                                .chars()
                                .take_while(|c| *c == ' ')
                                .count();
                            value_rest_indent_count = value_first_indent_count;
                        } else {
                            value_first_indent_count = 0;
                            value_rest_indent_count = rest_indent_count + 1;
                        }
                    } else {
                        // A value on the same line as the key (e.g.
                        // `a: value`): continuation lines only need to
                        // be indented more than the mapping's own key
                        // indentation (`rest_indent_count`), not as
                        // much as the first content character.
                        value_first_indent_count = 0;
                        value_rest_indent_count = rest_indent_count + 1;
                    }
                } else if trimmed_line.is_empty() {
                    self.scanner.next_line();
                } else {
                    return Err(Error::new(
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
                // Back to key mode for the next iteration.
                self.push_state(YamlState::InBlockMapKey);
            }
            if pre_pos == self.scanner.done_pos {
                return Err(Error::new(
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
        self.block_indent = saved_block_indent;
        self.map_key_indent = saved_map_key_indent;
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
            let trimmed = line.trim_start_matches([' ', '\t']);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let indent = line.chars().take_while(|c| *c == ' ').count();
            // A zero-indented block sequence is still valid content
            // (e.g. `sequence: !!seq\n- entry`).
            let is_zero_indented_seq = indent == key_indent_count
                && (trimmed == "-" || trimmed.starts_with("- "));
            return indent <= key_indent_count && !is_zero_indented_seq;
        }
        true
    }

    /// Parse the key node of an explicit mapping entry (`? key`).
    fn handle_explicit_key(&mut self) -> Result<(), Error> {
        log::trace!(
            "handle_explicit_key starts at {:?}",
            self.scanner.remains()
        );
        let mut anchor = None;
        let mut tag = None;
        // The key content must sit on the same line as the `?`
        // indicator; the anchor helper may consume the trailing line
        // break, so compare against the `?`'s line captured up front.
        let key_line = self.scanner.done_pos.line;
        loop {
            match self.scanner.peek_char() {
                Some(' ') | Some('\t') => {
                    self.scanner.next_char();
                }
                Some('&') if anchor.is_none() => {
                    anchor = Some(self.handle_anchor()?);
                }
                Some('!') if tag.is_none() => {
                    tag = self.handle_tag()?;
                }
                _ => break,
            }
        }
        // The key content must be on the same line as the `?`
        // indicator; when the line ended, the key is empty.
        let same_line = self.scanner.next_pos.line == key_line;
        if !same_line {
            // The key may still be a block node on the following lines
            // (e.g. a zero-indented block sequence after a lone `?`,
            // `?\n- a\n- b`). The sequence may sit at the mapping's own
            // indentation; any other node must be deeper.
            let map_indent = self.block_indent.unwrap_or(0);
            let mut rest = self.scanner.remains();
            let mut next_indent: Option<usize> = None;
            let mut is_seq = false;
            loop {
                let line = rest
                    .split_once(['\n', '\r'])
                    .map(|(s, _)| s)
                    .unwrap_or(rest);
                let trimmed = line.trim_start_matches([' ', '\t']);
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    match rest.find(['\n', '\r']) {
                        Some(i) => {
                            rest = &rest[i + 1..];
                            if rest.starts_with('\n') {
                                rest = &rest[1..];
                            }
                        }
                        None => break,
                    }
                    continue;
                }
                next_indent =
                    Some(line.chars().take_while(|c| *c == ' ').count());
                is_seq = trimmed == "-" || trimmed.starts_with("- ");
                break;
            }
            if let Some(indent) = next_indent {
                if is_seq && indent >= map_indent {
                    // The scanner already sits at the first content
                    // line (the caller consumed the line break after
                    // the `?`), so parse the sequence directly.
                    self.handle_block_seq(indent, anchor, tag)?;
                    return Ok(());
                }
                if indent > map_indent {
                    self.handle_node(indent, indent, anchor, tag)?;
                    return Ok(());
                }
            }
            log::trace!(
                "handle_explicit_key empty, anchor={anchor:?} tag={tag:?}"
            );
            // Empty explicit key.
            self.push_event(YamlEvent::Scalar(
                anchor,
                tag,
                String::new(),
                YamlScalarStyle::Plain,
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
            return Ok(());
        }
        match (same_line, self.scanner.peek_char()) {
            (_, None) | (_, Some('#')) => {
                log::trace!(
                    "handle_explicit_key empty, anchor={anchor:?} tag={tag:?}"
                );
                // Empty explicit key.
                self.push_event(YamlEvent::Scalar(
                    anchor,
                    tag,
                    String::new(),
                    YamlScalarStyle::Plain,
                    self.scanner.done_pos,
                    self.scanner.done_pos,
                ));
            }
            (false, Some(_)) => {
                // Unreachable: `!same_line` returns above.
                unreachable!();
            }
            (true, Some('\'')) | (true, Some('"')) => {
                self.handle_scalar(0, 0, anchor, tag)?;
            }
            (true, Some('[')) | (true, Some('{')) => {
                // A flow collection as the explicit key; when followed
                // by `:`, it is a compact block mapping key, e.g.
                // `? []: x`.
                let remainder = self.scanner.remains();
                let first_line =
                    remainder.split(['\n', '\r']).next().unwrap_or_default();
                if flow_collection_is_key(first_line) {
                    let key_column =
                        self.scanner.next_pos.column.saturating_sub(1);
                    self.handle_block_map(0, key_column, anchor, tag)?;
                } else if self.scanner.peek_char() == Some('[') {
                    self.handle_flow_seq(anchor, tag)?;
                } else {
                    self.handle_flow_map(anchor, tag)?;
                }
            }
            (true, Some('|')) => {
                self.scanner.advance_till_non_space();
                self.scanner.next_char();
                self.handle_block_scalar(anchor, tag, true)?;
            }
            (true, Some('>')) => {
                self.scanner.advance_till_non_space();
                self.scanner.next_char();
                self.handle_block_scalar(anchor, tag, false)?;
            }
            (true, Some('-')) => {
                // A block sequence as key, e.g. `? - a`: the entries
                // keep the indentation of the dash itself.
                self.handle_block_seq(
                    self.scanner.done_pos.column,
                    anchor,
                    tag,
                )?;
            }
            (true, Some('&')) => {
                // An anchored empty key, e.g. `? &d` (YAML 1.2.2
                // SPEC, 8.2.4).
                let key_anchor = Some(self.handle_anchor()?);
                let pos = self.scanner.next_pos;
                self.push_event(YamlEvent::Scalar(
                    key_anchor,
                    tag,
                    String::new(),
                    YamlScalarStyle::Plain,
                    pos,
                    pos,
                ));
            }
            (true, Some(_)) => {
                // A plain-scalar key on the `?` line; when the content
                // looks like a mapping (`? earth: blue`, `? : x`), the
                // key is a compact block mapping.
                let key_content =
                    self.scanner.remains().trim_start_matches([' ', '\t']);
                let key_first_line =
                    key_content.split(['\n', '\r']).next().unwrap_or_default();
                if find_key_value_separator(key_first_line).is_some()
                    || key_first_line.starts_with(':')
                    || key_first_line.starts_with(": ")
                {
                    let key_column =
                        self.scanner.next_pos.column.saturating_sub(1);
                    self.handle_block_map(0, key_column, anchor, tag)?;
                    return Ok(());
                }
                // A plain-scalar key on the `?` line. Unlike an
                // implicit key it does not need a `:`, it simply ends
                // at the line break (or a comment); continuation lines
                // fold in (e.g. `? a\n  true`).
                let start_pos = self.scanner.next_pos;
                let mut key = String::new();
                while let Some(c) = self.scanner.peek_char() {
                    let next_is_separation = matches!(
                        self.scanner.remains().chars().nth(1),
                        None | Some(' ') | Some('\t') | Some('\n') | Some('\r')
                    );
                    match c {
                        '\n' | '\r' => break,
                        '#' if key.ends_with(' ') || key.is_empty() => break,
                        ':' if next_is_separation => break,
                        _ => {
                            self.scanner.next_char();
                            key.push(c);
                        }
                    }
                }
                let floor = self.block_indent.unwrap_or(0) + 1;
                // The remainder of the first key line after the scalar
                // is either an inline comment, nothing, or a `: ` value
                // (e.g. `? : x`); in the latter case it must be left
                // for the explicit-value handling.
                let remainder = self.scanner.remains();
                let remainder_trimmed =
                    remainder.trim_start_matches([' ', '\t']);
                let ends_with_comment = remainder_trimmed.starts_with('#');
                let is_value_remainder = remainder_trimmed.starts_with(':');
                if !is_value_remainder {
                    // Skip an inline comment and the line break, then
                    // fold in any continuation lines.
                    if ends_with_comment || remainder_trimmed.is_empty() {
                        while !matches!(
                            self.scanner.peek_char(),
                            None | Some('\n') | Some('\r')
                        ) {
                            self.scanner.next_char();
                        }
                    }
                    self.scanner.next_char(); // '\n' or '\r'
                    if self.scanner.peek_char() == Some('\n') {
                        self.scanner.next_char(); // '\r\n'
                    }
                }
                while let Some(next_line) = self.scanner.peek_line() {
                    let next_trimmed = next_line.trim_start_matches(' ');
                    let next_indent =
                        next_line.chars().take_while(|c| *c == ' ').count();
                    let is_value_line = next_trimmed.starts_with(':')
                        && (next_trimmed == ":"
                            || next_trimmed.starts_with(": ")
                            || next_trimmed.starts_with(":\t")
                            || next_trimmed.starts_with(":#"));
                    if is_value_line
                        || next_indent < floor
                        || next_trimmed.is_empty()
                        || next_trimmed.starts_with('#')
                        || is_document_start_marker(next_trimmed)
                        || is_document_end_marker(next_trimmed)
                    {
                        break;
                    }
                    self.scanner.next_line();
                    let content = next_line.trim_matches([' ', '\t']);
                    if !content.is_empty() {
                        key.push(' ');
                        key.push_str(content);
                    }
                }
                let key = key.trim_end_matches([' ', '\t', ':']).to_string();
                self.push_event(YamlEvent::Scalar(
                    anchor,
                    tag,
                    key,
                    YamlScalarStyle::Plain,
                    start_pos,
                    self.scanner.done_pos,
                ));
            }
        }
        Ok(())
    }

    /// Parse the value of an explicit mapping entry, with the scanner
    /// positioned right after the `:`.
    fn parse_explicit_value_after_colon(&mut self) -> Result<(), Error> {
        // Skip same-line separation (validating tabs) and an inline
        // comment, e.g. `: # comment`.
        self.skip_block_indicator_separation(false)?;
        if self.scanner.peek_char() == Some('#') {
            // Consume the inline comment but leave the line break for
            // the lookahead below.
            while !matches!(
                self.scanner.peek_char(),
                None | Some('\n') | Some('\r')
            ) {
                self.scanner.next_char();
            }
        }
        // `:` followed directly by a line break: the value is empty
        // unless content follows on the next lines (at a deeper
        // indentation, or a zero-indented block sequence).
        if matches!(self.scanner.peek_char(), None | Some('\n') | Some('\r')) {
            let map_indent = self.block_indent.unwrap_or(0);
            let mut rest = self.scanner.remains();
            let mut next_indent: Option<usize> = None;
            let mut is_seq = false;
            loop {
                let line = rest
                    .split_once(['\n', '\r'])
                    .map(|(s, _)| s)
                    .unwrap_or(rest);
                let trimmed = line.trim_start_matches([' ', '\t']);
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    match rest.find(['\n', '\r']) {
                        Some(i) => {
                            rest = &rest[i + 1..];
                            if rest.starts_with('\n') {
                                rest = &rest[1..];
                            }
                        }
                        None => break,
                    }
                    continue;
                }
                next_indent =
                    Some(line.chars().take_while(|c| *c == ' ').count());
                is_seq = trimmed == "-" || trimmed.starts_with("- ");
                break;
            }
            let pos = self.scanner.next_pos;
            match next_indent {
                Some(indent) if is_seq && indent >= map_indent => {
                    self.scanner.next_line();
                    self.skip_comment_and_empty_lines();
                    self.handle_block_seq(indent, None, None)?;
                }
                Some(indent) if indent > map_indent => {
                    self.scanner.next_line();
                    self.skip_comment_and_empty_lines();
                    self.handle_node(indent, indent, None, None)?;
                }
                _ => {
                    self.push_event(YamlEvent::Scalar(
                        None,
                        None,
                        String::new(),
                        YamlScalarStyle::Plain,
                        pos,
                        pos,
                    ));
                }
            }
            return Ok(());
        }
        self.parse_explicit_value()
    }

    /// Parse the value node of an explicit mapping entry after its `:`.
    fn parse_explicit_value(&mut self) -> Result<(), Error> {
        let saved = self.in_explicit_value;
        self.in_explicit_value = true;
        let result = (|| {
            match self.scanner.peek_char() {
                Some('-') => {
                    // A block sequence value on the `:` line, e.g.
                    // `: - one` (YAML 1.2.2 SPEC, 8.2.1): the entries
                    // keep the indentation of the dash itself.
                    self.handle_block_seq(
                        self.scanner.done_pos.column,
                        None,
                        None,
                    )?;
                }
                _ => {
                    self.handle_node(
                        0,
                        self.scanner.done_pos.column,
                        None,
                        None,
                    )?;
                }
            }
            Ok(())
        })();
        self.in_explicit_value = saved;
        result
    }

    /// Place the scanner at the `:` following a non-scalar mapping key
    /// (alias or flow collection).
    fn expect_colon_after_key(&mut self) -> Result<(), Error> {
        while self.scanner.peek_char() == Some(' ') {
            self.scanner.next_char();
        }
        if self.scanner.peek_char() != Some(':') {
            return Err(Error::new(
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
    ) -> Result<(), Error> {
        let start_pos = self.scanner.next_pos;
        self.scanner.next_char();
        self.push_event(YamlEvent::MapStart(
            anchor,
            tag,
            YamlCollectionStyle::Flow,
            start_pos,
        ));
        self.push_state(YamlState::InFlowMapKey);
        let flow_start_line = self.scanner.done_pos.line;
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some('}') {
            self.scanner.next_char();
        } else {
            loop {
                // Key: the next flow line must not start with a tab
                // followed by content or a document marker.
                if let Some(line) =
                    self.scanner.remains().split(['\n', '\r']).nth(1)
                {
                    let trimmed = line.trim_start_matches([' ', '\t']);
                    if (line.starts_with('\t') && !trimmed.is_empty())
                        || trimmed.starts_with("---")
                        || trimmed.starts_with("...")
                    {
                        return Err(Error::new(
                            ErrorKind::InvalidStartOfToken,
                            format!(
                                "A flow collection line may not start with a \
                                 tab or a document marker: {line}"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
                self.scanner.skip_flow_separation();
                self.check_flow_entry_indentation(flow_start_line)?;
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
                    Some('?')
                        if matches!(
                            self.scanner.remains().chars().nth(1),
                            None | Some(' ')
                                | Some('\t')
                                | Some('\n')
                                | Some('\r')
                                | Some('#')
                        ) =>
                    {
                        // Explicit key, e.g. `{? key : value}`; a `?`
                        // followed directly by content (`{?foo: bar}`)
                        // is a plain scalar key.
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
                        // A comment directly after the comma (without a
                        // separation space) is an error.
                        if self.scanner.peek_char() == Some('#') {
                            return Err(
                                Error::new(
                                    ErrorKind::AmbiguityPlainScalar,
                                    "Comment must be preceded by a space \
                                     after                                  \
                                     ',' in a flow mapping"
                                        .to_string(),
                                    self.scanner.next_pos,
                                    self.scanner.next_pos,
                                ),
                            );
                        }
                        self.scanner.skip_flow_separation();
                        // A trailing comma before '}' is tolerated in
                        // flow mappings.
                        if self.scanner.peek_char() == Some('}') {
                            self.scanner.next_char();
                            break;
                        }
                        if self.scanner.peek_char() == Some(',') {
                            return Err(Error::new(
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
                        return Err(Error::new(
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
                        return Err(Error::new(
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
