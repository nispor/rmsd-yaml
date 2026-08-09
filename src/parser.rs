// SPDX-License-Identifier: Apache-2.0

use std::cmp::max;

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlPosition, YamlScalarStyle,
    YamlScanner, YamlState,
};

#[derive(Debug)]
pub(crate) struct YamlParser<'a> {
    pub(crate) scanner: YamlScanner<'a>,
    states: Vec<YamlState>,
    events: Vec<YamlEvent>,
    /// Indentation level of the innermost block collection, or `None`
    /// when the current node is not inside a block collection (e.g. a
    /// top-level document node). Block scalar content indentation is
    /// relative to it (YAML 1.2.2 SPEC, 8.1.1.1).
    pub(crate) block_indent: Option<usize>,
}

impl<'a> YamlParser<'a> {
    /// Current state
    pub(crate) fn cur_state(&self) -> &YamlState {
        self.states.last().unwrap_or(&YamlState::EndOfFile)
    }

    pub(crate) fn push_event(&mut self, event: YamlEvent) {
        log::trace!("Got event {:?}", event);
        self.events.push(event);
    }

    /// Current count of the pushed events.
    pub(crate) fn events_len(&self) -> usize {
        self.events.len()
    }

    /// Take the events pushed since `start` out. Used to wrap an
    /// already-emitted flow node into a single-pair flow mapping.
    pub(crate) fn take_events_since(&mut self, start: usize) -> Vec<YamlEvent> {
        self.events.drain(start..).collect()
    }

    pub(crate) fn push_state(&mut self, state: YamlState) {
        log::trace!("Push state {:?}", state);
        self.states.push(state);
    }

    pub(crate) fn pop_state(&mut self) {
        let state = self.states.pop();
        log::trace!("Pop state: {:?}", state);
    }

    pub(crate) fn parse_to_events(
        input: &'a str,
    ) -> Result<Vec<YamlEvent>, YamlError> {
        let mut parser = Self {
            scanner: YamlScanner::new(input),
            states: Vec::new(),
            events: Vec::new(),
            block_indent: None,
        };
        while !parser.scanner.is_empty() {
            let cur_pos = parser.scanner.done_pos;
            parser.handle_stream()?;
            if parser.scanner.done_pos == cur_pos {
                return Err(YamlError::new(
                    ErrorKind::Bug,
                    format!(
                        "YamlParser::parse_to_events(): dead-loop: remains \
                         {:?}",
                        parser.scanner.remains()
                    ),
                    cur_pos,
                    cur_pos,
                ));
            }
        }
        for event in &parser.events {
            log::trace!("{:?}", event);
        }

        Ok(parser.events)
    }

    /// Stream started, but not `---` or string other than `b-break` found yet.
    fn handle_stream(&mut self) -> Result<(), YamlError> {
        self.push_event(YamlEvent::StreamStart);
        log::trace!("handle_stream {:?}", self.scanner.remains());
        // Whether the previous document was terminated by `...` (or no
        // document has been parsed yet).
        let mut doc_terminated = true;
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches(' ');
            if trimmed.is_empty() {
                self.scanner.advance_till_linebreak();
            } else if trimmed.starts_with('#') {
                // Comment lines are ignored at stream level.
                self.scanner.advance_till_linebreak();
            } else if trimmed == "---" {
                let indent_count =
                    line.chars().take_while(|c| *c == ' ').count();
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak();
                self.handle_node(indent_count, indent_count, None, None)?;
                doc_terminated = false;
            } else if let Some(offset) = line.find("--- ") {
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_offset(offset + 4);
                self.handle_node(0, 0, None, None)?;
                doc_terminated = false;
            } else if trimmed == "..." {
                if self
                    .events
                    .iter()
                    .any(|e| matches!(e, YamlEvent::DocumentStart(_, _)))
                {
                    self.push_event(YamlEvent::DocumentEnd(
                        true,
                        self.scanner.next_pos,
                    ));
                }
                self.scanner.advance_till_linebreak_or_space();
                doc_terminated = true;
            } else {
                if !doc_terminated {
                    return Err(YamlError::new(
                        ErrorKind::MissingDocumentEndMarkerBeforeDirective,
                        format!(
                            "Expecting document end marker `...` before new \
                             document content: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                self.push_event(YamlEvent::DocumentStart(
                    false,
                    self.scanner.next_pos,
                ));
                self.handle_node(0, 0, None, None)?;
                doc_terminated = false;
            }
        }

        let has_doc_start = self
            .events
            .iter()
            .any(|e| matches!(e, YamlEvent::DocumentStart(_, _)));
        let has_doc_end = self
            .events
            .iter()
            .any(|e| matches!(e, YamlEvent::DocumentEnd(_, _)));
        if !has_doc_start && !has_doc_end {
            // Empty content
            self.push_event(YamlEvent::DocumentStart(false, YamlPosition::EOF));
        }
        // No explicit document end `...`
        if !has_doc_end {
            self.push_event(YamlEvent::DocumentEnd(
                false,
                self.scanner.done_pos,
            ));
        }
        self.push_event(YamlEvent::StreamEnd);
        Ok(())
    }

    /// Handle a container or scalar
    pub(crate) fn handle_node(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        mut anchor: Option<String>,
        mut tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_node {} {} {:?} {:?}, {:?}",
            first_indent_count,
            rest_indent_count,
            anchor,
            tag,
            self.scanner.remains()
        );
        // Ignore less indented empty line and comment line
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches(' ');
            let indent_count = line.chars().take_while(|c| *c == ' ').count();
            if (trimmed.is_empty() && indent_count <= first_indent_count)
                || trimmed.starts_with('#')
            {
                self.scanner.advance_till_linebreak();
                continue;
            } else {
                break;
            }
        }

        if let Some(line) = self.scanner.peek_line() {
            let indent_count = line.chars().take_while(|c| *c == ' ').count();

            if indent_count < first_indent_count {
                if self.cur_state().is_container() {
                    return Ok(());
                } else {
                    return Err(YamlError::new(
                        ErrorKind::LessIndentedWithoutParent,
                        format!("Less indented but without parent: {:?}", line),
                        self.scanner.next_pos,
                        {
                            self.scanner.next_line();
                            self.scanner.done_pos
                        },
                    ));
                }
            }

            let trimmed = line.trim_start_matches(' ');

            if trimmed.starts_with("- ") || trimmed == "-" {
                if self.cur_state().is_block_map_value()
                    && self.scanner.done_pos.line == self.scanner.next_pos.line
                {
                    // YAML 1.2.2 SPEC, 8.2.1. Block Sequences:
                    //     A block sequence entry is not allowed on the
                    //     same line as a mapping key.
                    return Err(YamlError::new(
                        ErrorKind::InvalidSequnceStartIndicator,
                        format!(
                            "Block sequence entry is not allowed on the \
                                 same                              line as a \
                                 mapping key: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                let expected_indent_count =
                    rest_indent_count + indent_count - first_indent_count;
                self.handle_block_seq(expected_indent_count, anchor, tag)?;
            } else if trimmed.starts_with('\'') || trimmed.starts_with('"') {
                // Flow style does not care indentation
                self.handle_scalar(0, 0, anchor, tag)?;
            } else if trimmed.starts_with('*') {
                self.scanner.advance(indent_count);
                let name = self.handle_alias()?;
                self.push_event(YamlEvent::Alias(name, self.scanner.next_pos));
            } else if trimmed.starts_with("[") {
                self.handle_flow_seq(anchor, tag)?;
                self.expect_flow_end_separation()?;
            } else if trimmed.starts_with("{") {
                self.handle_flow_map(anchor, tag)?;
                self.expect_flow_end_separation()?;
            } else if trimmed.contains(": ") {
                // Guess out the indent

                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, indent_count),
                    anchor,
                    tag,
                )?;
            } else if trimmed.ends_with(":") {
                self.handle_block_map(
                    first_indent_count,
                    rest_indent_count,
                    anchor,
                    tag,
                )?;
            } else if trimmed.starts_with('!') || trimmed.starts_with('&') {
                self.scanner.advance(indent_count);
                // YAML 1.2.2 SPEC, 6.9. Node Properties:
                //      Node properties may appear in any order, but each
                //      at most once.
                let mut property_found = false;
                while let Some(property_line) = self.scanner.peek_line() {
                    let property_trimmed =
                        property_line.trim_start_matches(' ');
                    let property_indent =
                        property_line.chars().take_while(|c| *c == ' ').count();
                    if property_trimmed.starts_with('&') {
                        if anchor.is_none() {
                            self.scanner.advance(property_indent);
                            anchor = Some(self.handle_anchor()?);
                            property_found = true;
                        } else if let Some(after_anchor) =
                            property_trimmed.split_once([' ', '\t'])
                        {
                            // A second anchor is allowed when it belongs
                            // to an implicit mapping key of the anchored
                            // node, e.g.
                            //      &node
                            //      &key key: value
                            let after_anchor =
                                after_anchor.1.trim_start_matches(' ');
                            if after_anchor.contains(": ")
                                || after_anchor.ends_with(':')
                            {
                                property_found = true;
                                break;
                            } else {
                                return Err(YamlError::new(
                                    ErrorKind::InvalidAnchor,
                                    format!(
                                        "Node can have at most one anchor, \
                                         but got: {property_line}"
                                    ),
                                    self.scanner.next_pos,
                                    self.scanner.next_pos,
                                ));
                            }
                        } else {
                            return Err(YamlError::new(
                                ErrorKind::InvalidAnchor,
                                format!(
                                    "Node can have at most one anchor, but \
                                     got: {property_line}"
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    } else if property_trimmed.starts_with('!') {
                        if tag.is_none() {
                            self.scanner.advance(property_indent);
                            // Tag decorating its container
                            tag = self.handle_tag();
                            property_found = true;
                        } else {
                            return Err(YamlError::new(
                                ErrorKind::InvalidAnchor,
                                format!(
                                    "Node can have at most one tag, but got: \
                                     {property_line}"
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    } else {
                        break;
                    }
                }
                if !property_found {
                    return Err(YamlError::new(
                        ErrorKind::InvalidAnchor,
                        format!(
                            "Node can have at most one anchor and one tag, \
                             but got: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                if self.scanner.done_pos.line == self.scanner.next_pos.line
                    && let Some(content_line) = self.scanner.peek_line()
                {
                    let content_trimmed = content_line.trim_start_matches(' ');
                    if content_trimmed.starts_with("- ")
                        || content_trimmed == "-"
                    {
                        return Err(YamlError::new(
                            ErrorKind::InvalidAnchor,
                            format!(
                                "Node property cannot be placed before a \
                                 block sequence entry on the same line: \
                                 {content_line}"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
                // The node properties may be followed by the content on
                // the same line or on a following line. When the
                // properties ended at a line break, the content may sit
                // at a different indentation than the properties (e.g.
                // `!foo\n>1`), so re-derive the indentation from the
                // content line. The content line must be more indented
                // than the enclosing block collection, otherwise the
                // properties decorate an empty node and the next line
                // belongs to the parent (e.g. `- &a\n- b`).
                let content_more_indented = self
                    .block_indent
                    .map(|indent| {
                        self.scanner
                            .peek_line()
                            .map(|l| {
                                l.chars().take_while(|c| *c == ' ').count()
                                    > indent
                            })
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
                if content_more_indented
                    && self.scanner.done_pos.line != self.scanner.next_pos.line
                    && let Some(content_line) = self.scanner.peek_line()
                {
                    let content_indent =
                        content_line.chars().take_while(|c| *c == ' ').count();
                    self.handle_node(
                        content_indent,
                        content_indent,
                        anchor,
                        tag,
                    )?;
                } else {
                    self.handle_node(
                        first_indent_count,
                        rest_indent_count,
                        anchor,
                        tag,
                    )?;
                }
            } else if trimmed == "..." {
                // Document end marker: this node ends here, and the
                // stream handler will emit `DocumentEnd`.
                return Ok(());
            } else if trimmed.starts_with('%') {
                return Err(YamlError::new(
                    ErrorKind::MissingDocumentEndMarkerBeforeDirective,
                    format!(
                        "Directive is only allowed before document start \
                         marker `---` or after document end marker `...`, but \
                         got: {line}"
                    ),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else if line.trim_start_matches(' ').starts_with('\t') {
                return Err(YamlError::new(
                    ErrorKind::InvalidStartOfToken,
                    "Tab(\\t) cannot be used as start of any YAML node"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else {
                self.handle_scalar(
                    first_indent_count,
                    rest_indent_count,
                    anchor,
                    tag,
                )?;
            }
        } else if anchor.is_some() || tag.is_some() {
            // Node properties without content: empty scalar node
            // (e.g. standalone `!` or `&anchor` at EOF).
            self.push_event(YamlEvent::Scalar(
                anchor,
                tag,
                String::new(),
                YamlScalarStyle::Plain,
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        Ok(())
    }

    /// After a flow collection in block context, only spaces, a line
    /// break, a comment or EOF may follow.
    fn expect_flow_end_separation(&mut self) -> Result<(), YamlError> {
        while self.scanner.peek_char() == Some(' ') {
            self.scanner.next_char();
        }
        match self.scanner.peek_char() {
            None | Some('\n') | Some('\r') | Some('#') => Ok(()),
            Some(c) => Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!(
                    "Expecting a line break or comment after a flow \
                     collection, but got '{c}'"
                ),
                self.scanner.next_pos,
                self.scanner.next_pos,
            )),
        }
    }

    /// Handle a node inside a flow collection. The entry terminators
    /// (`,` plus `]` or `}`) are handled by the caller.
    pub(crate) fn handle_flow_node(&mut self) -> Result<(), YamlError> {
        self.scanner.skip_flow_separation();
        // YAML 1.2.2 SPEC, 6.9. Node Properties:
        //      Node properties may appear in any order, but each at
        //      most once.
        let mut anchor = None;
        let mut tag = None;
        loop {
            match self.scanner.peek_char() {
                Some('&') if anchor.is_none() => {
                    anchor = Some(self.handle_anchor()?);
                    self.scanner.skip_flow_separation();
                }
                Some('!') if tag.is_none() => {
                    tag = self.handle_tag();
                    self.scanner.skip_flow_separation();
                }
                _ => break,
            }
        }
        match self.scanner.peek_char() {
            Some('[') => self.handle_flow_seq(anchor, tag)?,
            Some('{') => self.handle_flow_map(anchor, tag)?,
            Some('*') => {
                if anchor.is_some() || tag.is_some() {
                    return Err(YamlError::new(
                        ErrorKind::InvalidAnchor,
                        "Alias cannot carry node properties".to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                let name = self.handle_alias()?;
                self.push_event(YamlEvent::Alias(name, self.scanner.next_pos));
            }
            Some('"') => self.handle_double_quoted_flow_scalar(anchor, tag)?,
            Some('\'') => self.handle_single_quoted_flow_scalar(anchor, tag)?,
            Some(',') | Some(']') | Some('}') | None => {
                // Empty node, e.g. an omitted value in a flow mapping.
                let pos = self.scanner.next_pos;
                self.push_event(YamlEvent::Scalar(
                    anchor,
                    tag,
                    String::new(),
                    YamlScalarStyle::Plain,
                    pos,
                    pos,
                ));
            }
            Some(_) => self.handle_flow_plain_scalar(anchor, tag)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_document_explcitly_start() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n---").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::DocumentEnd(false, YamlPosition::new(3, 3)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_document_explcitly_start_and_end() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n---\na\n...").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::Scalar(
                    None,
                    None,
                    "a".to_string(),
                    YamlScalarStyle::Plain,
                    YamlPosition::new(4, 1),
                    YamlPosition::new(4, 1)
                ),
                YamlEvent::DocumentEnd(true, YamlPosition::new(5, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_document_with_comment() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n--- # test command\n...")
                .unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::DocumentEnd(true, YamlPosition::new(4, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }
}
