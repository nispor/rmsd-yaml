// SPDX-License-Identifier: Apache-2.0

use std::cmp::max;

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlPosition, YamlScanner, YamlState,
};

#[derive(Debug)]
pub(crate) struct YamlParser<'a> {
    pub(crate) scanner: YamlScanner<'a>,
    states: Vec<YamlState>,
    events: Vec<YamlEvent>,
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

    pub(crate) fn push_state(&mut self, state: YamlState) {
        log::trace!("Push state {:?}", state);
        self.states.push(state);
    }

    pub(crate) fn pop_state(&mut self) {
        log::trace!("Pop state: {:?}", self.states.pop());
    }

    pub(crate) fn parse_to_events(
        input: &'a str,
    ) -> Result<Vec<YamlEvent>, YamlError> {
        let mut parser = Self {
            scanner: YamlScanner::new(input),
            states: Vec::new(),
            events: Vec::new(),
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
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches(' ');
            if trimmed.is_empty() {
                self.scanner.advance_till_linebreak();
            } else if trimmed == "---" {
                let indent_count =
                    line.chars().take_while(|c| *c == ' ').count();
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak();
                self.handle_node(indent_count, indent_count, None)?;
            } else if let Some(offset) = line.find("--- ") {
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_offset(offset + 4);
                self.handle_node(0, 0, None)?;
            } else if trimmed == "..." {
                self.push_event(YamlEvent::DocumentEnd(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak_or_space();
            } else {
                self.push_event(YamlEvent::DocumentStart(
                    false,
                    self.scanner.next_pos,
                ));
                self.handle_node(0, 0, None)?;
            }
        }

        if !self
            .events
            .iter()
            .any(|e| matches!(e, YamlEvent::DocumentStart(_, _)))
        {
            // Empty content
            self.push_event(YamlEvent::DocumentStart(false, YamlPosition::EOF));
        }
        // No explicit document end `...`
        if !self
            .events
            .iter()
            .any(|e| matches!(e, YamlEvent::DocumentEnd(_, _)))
        {
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
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_node {} {} {:?}, {:?}",
            first_indent_count,
            rest_indent_count,
            tag,
            self.scanner.remains()
        );
        // Ignore less indented empty line and comment line
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches(' ');
            let indent_count = line.chars().take_while(|c| *c == ' ').count();
            if (trimmed.is_empty() && indent_count <= first_indent_count)
                || trimmed.starts_with("# ")
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
                let expected_indent_count =
                    rest_indent_count + indent_count - first_indent_count;
                self.handle_block_seq(expected_indent_count, tag)?;
            } else if trimmed.starts_with('\'') || trimmed.starts_with('"') {
                // Flow style does not care indentation
                self.handle_scalar(0, 0, tag)?;
            } else if trimmed.contains(": ") {
                // Guess out the indent

                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, indent_count),
                    tag,
                )?;
            } else if trimmed.ends_with(":") {
                self.handle_block_map(
                    first_indent_count,
                    rest_indent_count,
                    tag,
                )?;
            } else if trimmed.starts_with("[") {
                self.handle_flow_seq(tag)?;
            } else if trimmed.starts_with("{") {
                self.handle_flow_map(tag)?;
            } else if trimmed.starts_with("!") {
                self.scanner.advance(indent_count);
                // Tag decorating its container
                let tag = self.handle_tag();
                self.handle_node(first_indent_count, rest_indent_count, tag)?;
            } else if line.trim_start_matches(' ').starts_with('\t') {
                return Err(YamlError::new(
                    ErrorKind::InvalidStartOfToken,
                    "Tab(\\t) cannot be used as start of any YAML node"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else {
                self.handle_scalar(first_indent_count, rest_indent_count, tag)?;
            }
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
                    "a".to_string(),
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
