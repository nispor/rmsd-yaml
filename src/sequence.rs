// SPDX-License-Identifier: Apache-2.0

use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    ErrorKind, YamlCollectionStyle, YamlDeserializer, YamlError, YamlEvent,
    YamlParser, YamlScalarStyle, YamlState, YamlValue,
    parser::{is_document_end_marker, is_document_start_marker},
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
        let saved_block_indent = self.block_indent;
        let saved_seq_entry_indent = self.seq_entry_indent;
        self.block_indent = Some(indent_count);
        self.seq_entry_indent = None;
        self.push_event(YamlEvent::SequenceStart(
            anchor,
            tag,
            YamlCollectionStyle::Block,
            self.scanner.next_pos,
        ));
        self.push_state(YamlState::InBlockSequnce);
        // All entries that start on a new line must share the same
        // indentation as the first new-line entry (YAML 1.2.2 SPEC,
        // 8.2.1); a `- ` at a different indentation ends the sequence
        // (e.g. `- key: value\n - item1` is an error).
        let mut entry_indent: Option<usize> = None;
        while let Some(line) = self.scanner.peek_line() {
            if line.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
                self.scanner.next_line();
                continue;
            }
            let cur_indent = line.chars().take_while(|c| *c == ' ').count();
            // A same-line entry (e.g. the inner dash of `- - a` or the
            // dash after `: `) starts at the scanner position, so the
            // indentation checks only apply from the next line on.
            let at_line_start = self.scanner.done_pos.column == 0
                || self.scanner.done_pos.line != self.scanner.next_pos.line
                || matches!(
                    self.scanner.peek_char(),
                    None | Some('\n') | Some('\r')
                );
            if at_line_start && cur_indent < indent_count {
                break;
            }
            let trimmed = line.trim_start_matches(' ');
            if at_line_start && (trimmed.starts_with("- ") || trimmed == "-") {
                match entry_indent {
                    Some(prev) if cur_indent != prev => break,
                    None => entry_indent = Some(cur_indent),
                    _ => {}
                }
                self.seq_entry_indent = entry_indent;
            }

            if trimmed.starts_with('#')
                && self.scanner.done_pos.line != self.scanner.next_pos.line
            {
                // Comment lines do not belong to the sequence. The
                // line-start check excludes trailing content on a line
                // that is still being processed.
                self.scanner.advance_till_linebreak();
                continue;
            }
            if is_document_end_marker(trimmed) {
                // Document end marker: leave it for the stream handler.
                break;
            }
            if is_document_start_marker(trimmed) {
                // Document start marker: the sequence ends here.
                break;
            }

            if trimmed.starts_with(':')
                && (trimmed == ":"
                    || trimmed.starts_with(": ")
                    || trimmed.starts_with(":\t")
                    || trimmed.starts_with(":#"))
            {
                // An explicit mapping value line (`: value`) ends a
                // block sequence used as a mapping key.
                break;
            }
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
            } else if trimmed.starts_with("- ") || trimmed.starts_with("-\t") {
                let tab_after_dash = trimmed.starts_with("-\t");
                self.scanner.advance(cur_indent + 2);
                self.skip_block_indicator_separation(tab_after_dash)?;
                if self.scanner.peek_char() == Some('#') {
                    // A comment after the entry indicator: the entry
                    // is empty unless content follows at a deeper
                    // indentation (e.g. `- # Empty\n- |` is an empty
                    // entry, `- # c\n  seq2` has value `seq2`).
                    let seq_indent = self.block_indent.unwrap_or(cur_indent);
                    let mut rest = self.scanner.remains();
                    let mut next_indent: Option<usize> = None;
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
                        next_indent = Some(
                            line.chars().take_while(|c| *c == ' ').count(),
                        );
                        break;
                    }
                    match next_indent {
                        Some(indent) if indent > seq_indent => {
                            while !matches!(
                                self.scanner.peek_char(),
                                None | Some('\n') | Some('\r')
                            ) {
                                self.scanner.next_char();
                            }
                            self.scanner.next_char(); // '\n'
                            self.skip_comment_and_empty_lines();
                            self.handle_node(indent, indent, None, None)?;
                        }
                        _ => {
                            // Empty entry: consume the comment line.
                            while !matches!(
                                self.scanner.peek_char(),
                                None | Some('\n') | Some('\r')
                            ) {
                                self.scanner.next_char();
                            }
                            self.scanner.next_char(); // '\n' or '\r'
                            let pos = self.scanner.done_pos;
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
                } else {
                    // Continuation lines of the entry content only need
                    // to be indented more than the sequence's own
                    // indentation, not as much as the content column
                    // (yaml-test-suite: legal-tab-after-indentation,
                    // sequence-entry-that-looks-like-two-with-wrong-indentation).
                    let entry_floor =
                        self.block_indent.unwrap_or(cur_indent) + 1;
                    self.handle_node(0, entry_floor, None, None)?;
                }
            } else if trimmed.is_empty() {
                self.scanner.next_line();
                continue;
            } else {
                // A line that is not a `- ` entry at the sequence's
                // indentation belongs to the parent context (e.g. a
                // mapping key after a block-sequence value at the same
                // indentation, as in `foo:\n- 42\nbar: 1`).
                break;
            }
        }

        self.push_event(YamlEvent::SequenceEnd(self.scanner.done_pos));
        self.pop_state();
        self.block_indent = saved_block_indent;
        self.seq_entry_indent = saved_seq_entry_indent;
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
        let flow_start_line = self.scanner.done_pos.line;
        self.check_flow_line_start()?;
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some(']') {
            self.scanner.next_char();
        } else {
            loop {
                self.check_flow_line_start()?;
                if self.scanner.peek_char() == Some(',') {
                    return Err(YamlError::new(
                        ErrorKind::UnfinishedSequenceIndicator,
                        "A flow sequence entry may not start with ','"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                self.check_flow_entry_indentation(flow_start_line)?;
                self.handle_flow_seq_entry()?;
                self.scanner.skip_flow_separation();
                match self.scanner.peek_char() {
                    Some(',') => {
                        self.scanner.next_char();
                        // A comment directly after the comma (without a
                        // separation space) is an error.
                        if self.scanner.peek_char() == Some('#') {
                            return Err(YamlError::new(
                                ErrorKind::AmbiguityPlainScalar,
                                "Comment must be preceded by a space \
                                     after                                  \
                                     ',' in a flow sequence"
                                    .to_string(),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                        self.scanner.skip_flow_separation();
                        if self.scanner.peek_char() == Some(']') {
                            // A trailing comma before the closing
                            // bracket is allowed, e.g. `[a, b, ]`.
                            self.scanner.next_char();
                            break;
                        }
                        if self.scanner.peek_char() == Some(',') {
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
        if self.scanner.peek_char() == Some('?')
            && matches!(
                self.scanner.remains().chars().nth(1),
                None | Some(' ')
                    | Some('\t')
                    | Some('\n')
                    | Some('\r')
                    | Some('#')
            )
        {
            // An explicit-key entry, e.g. `[ ? foo\n   : bar ]`: the
            // key (and the value after `:`) are flow nodes and may
            // span lines.
            let start_pos = self.scanner.next_pos;
            self.scanner.next_char(); // consume '?'
            self.push_event(YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Flow,
                start_pos,
            ));
            self.scanner.skip_flow_separation();
            self.handle_flow_node()?;
            self.scanner.skip_flow_separation();
            if self.scanner.peek_char() == Some(':') {
                self.scanner.next_char();
                self.scanner.skip_flow_separation();
                self.handle_flow_node()?;
            } else {
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
            self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
            return Ok(());
        }
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
            // Single-pair entry with an empty key, e.g. `[ : value ]`;
            // a `:` followed directly by content (`[:x]`) is a plain
            // scalar.
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
        let key_start_line = self.scanner.next_pos.line;
        self.handle_flow_node()?;
        self.scanner.skip_flow_separation();
        if self.scanner.peek_char() == Some(':') {
            // An implicit key must be contained in a single line
            // (YAML 1.2.2 SPEC, 7.4.5), so a key that spans a line
            // break is an error, e.g. `[ key\n  : value ]`.
            if self.scanner.next_pos.line != key_start_line {
                return Err(YamlError::new(
                    ErrorKind::InvalidImplicitKey,
                    "Implicit mapping key must be contained in a single line"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            }
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
