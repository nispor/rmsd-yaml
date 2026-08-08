// SPDX-License-Identifier: Apache-2.0

use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    ErrorKind, YamlCollectionStyle, YamlDeserializer, YamlError, YamlEvent,
    YamlParser, YamlScalarStyle, YamlState, YamlValue,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlValueSeqAccess {
    data: Vec<YamlValue>,
}

impl YamlValueSeqAccess {
    pub(crate) fn new(data: Vec<YamlValue>) -> Self {
        // The Vec::pop() is much quicker than Vec::remove(0), so we
        // reverse it.
        let mut data = data;
        data.reverse();
        Self { data }
    }
}

impl<'de> SeqAccess<'de> for YamlValueSeqAccess {
    type Error = YamlError;

    fn next_element_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if let Some(value) = self.data.pop() {
            seed.deserialize(&mut YamlDeserializer { parsed: value })
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len())
    }
}

impl<'a> YamlParser<'a> {
    /// Invoked when there is `: ` in line or ends with `:`.
    /// Advance till map finished.
    pub(crate) fn handle_block_seq(
        &mut self,
        indent_count: usize,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_seq {} {:?}",
            indent_count,
            self.scanner.remains()
        );
        self.push_event(YamlEvent::SequenceStart(
            anchor,
            tag,
            YamlCollectionStyle::Block,
            self.scanner.next_pos,
        ));
        self.push_state(YamlState::InBlockSequnce);
        while let Some(line) = self.scanner.peek_line() {
            if line.is_empty() {
                self.scanner.next_line();
                continue;
            }
            let cur_indent = line.chars().take_while(|c| *c == ' ').count();
            // A same-line entry (e.g. the inner dash of `- - a`) starts
            // at the scanner position, so the indent check only applies
            // from the next line on.
            let mid_line =
                self.scanner.done_pos.line == self.scanner.next_pos.line;
            if !mid_line && cur_indent < indent_count {
                break;
            }
            let trimmed = line.trim_start_matches(' ');

            if trimmed == "-" {
                self.scanner.next_line();
                if let Some(next_line) = self.scanner.peek_line() {
                    let next_indent =
                        next_line.chars().take_while(|c| *c == ' ').count();
                    self.handle_node(next_indent, next_indent, None, None)?;
                } else {
                    if self.scanner.remains().is_empty() {
                        // Empty array
                        self.push_event(YamlEvent::Scalar(
                            None,
                            None,
                            String::new(),
                            YamlScalarStyle::Plain,
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                    }
                }
            } else if trimmed.starts_with("- ") {
                self.scanner.advance(cur_indent + 2);
                self.handle_node(0, cur_indent + 2, None, None)?;
            } else if trimmed.is_empty() {
                self.scanner.next_line();
                continue;
            } else {
                return Err(YamlError::new(
                    ErrorKind::InvalidSequnceStartIndicator,
                    format!(
                        "Expecting '-\\n' or '- ' as sequence start \
                         indicator, but got: {line:?}"
                    ),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            }
        }

        self.push_event(YamlEvent::SequenceEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }

    /// Consume the scanner till a flow sequence is finished and insert
    /// the parsed events. The scanner must stay at `[`.
    ///
    /// YAML 1.2.2 SPEC, 7.4.1. Flow Sequences.
    pub(crate) fn handle_flow_seq(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        let start_pos = self.scanner.next_pos;
        self.scanner.next_char();
        self.push_event(YamlEvent::SequenceStart(
            anchor,
            tag,
            YamlCollectionStyle::Flow,
            start_pos,
        ));
        self.push_state(YamlState::InFlowSequnce);
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some(']') {
            self.scanner.next_char();
        } else {
            loop {
                self.handle_flow_seq_entry()?;
                self.scanner.skip_flow_separation();
                match self.scanner.peek_char() {
                    Some(',') => {
                        self.scanner.next_char();
                        self.scanner.skip_flow_separation();
                        if matches!(
                            self.scanner.peek_char(),
                            Some(',') | Some(']')
                        ) {
                            return Err(YamlError::new(
                                ErrorKind::UnfinishedSequenceIndicator,
                                format!(
                                    "Expecting an entry after ',' in flow \
                                     sequence, but got: {:?}",
                                    self.scanner.remains()
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    }
                    Some(']') => {
                        self.scanner.next_char();
                        break;
                    }
                    Some(c) => {
                        return Err(YamlError::new(
                            ErrorKind::UnfinishedSequenceIndicator,
                            format!(
                                "Expecting ',' or ']' in flow sequence, but \
                                 got '{c}'"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                    None => {
                        return Err(YamlError::new(
                            ErrorKind::UnfinishedSequenceIndicator,
                            "Unfinished flow sequence".to_string(),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
            }
        }
        self.push_event(YamlEvent::SequenceEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }

    /// Parse a flow sequence entry. An entry containing `key: value` is
    /// a single-pair flow mapping.
    fn handle_flow_seq_entry(&mut self) -> Result<(), YamlError> {
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some(':') {
            // Single-pair entry with an empty key, e.g. `[ : value ]`
            let start_pos = self.scanner.next_pos;
            self.push_event(YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Flow,
                start_pos,
            ));
            self.push_event(YamlEvent::Scalar(
                None,
                None,
                String::new(),
                YamlScalarStyle::Plain,
                start_pos,
                start_pos,
            ));
            self.scanner.next_char();
            self.handle_flow_node()?;
            self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
            return Ok(());
        }
        let events_start = self.events_len();
        self.handle_flow_node()?;
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some(':') {
            // Single-pair entry, e.g. `[ key: value ]`. Wrap the
            // already-emitted key node events into a flow mapping.
            let key_events = self.take_events_since(events_start);
            self.push_event(YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Flow,
                self.scanner.next_pos,
            ));
            for event in key_events {
                self.push_event(event);
            }
            self.scanner.next_char();
            self.handle_flow_node()?;
            self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
        }
        Ok(())
    }
}
