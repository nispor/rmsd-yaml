// SPDX-License-Identifier: Apache-2.0

use crate::{
    ErrorKind, YamlChar, YamlError, YamlEvent, YamlPosition, YamlScanner,
    YamlState,
};

#[derive(Debug)]
pub struct YamlTreeParser<'a> {
    pub(crate) scanner: YamlScanner<'a>,
    pub(crate) states: Vec<YamlState>,
    pub(crate) events: Vec<YamlEvent>,
    pub(crate) cur_idents: usize,
}

const MAX_LOOP_COUNT: usize = 10;

impl<'a> YamlTreeParser<'a> {
    /// Current state
    pub(crate) fn cur_state(&self) -> &YamlState {
        self.states.last().unwrap_or(&YamlState::EndOfFile)
    }

    /// Previous state
    pub(crate) fn pre_state(&self) -> &YamlState {
        if self.states.len() >= 2 {
            self.states
                .get(self.states.len() - 2)
                .unwrap_or(&YamlState::EndOfFile)
        } else {
            &YamlState::EndOfFile
        }
    }

    pub fn parse(input: &'a str) -> Result<Vec<YamlEvent>, YamlError> {
        let mut parser = Self {
            scanner: YamlScanner::new(input),
            states: vec![YamlState::InStream],
            events: vec![YamlEvent::StreamStart],
            cur_idents: 0,
        };
        let mut loop_counter = 0;
        while !parser.cur_state().eof() {
            let cur_pos = parser.scanner.done_pos;
            match parser.cur_state() {
                YamlState::InStream => parser.handle_in_stream()?,
                YamlState::InDocument => parser.handle_in_doc()?,
                YamlState::InBlockSequnce(_) => parser.handle_in_block_seq()?,
                YamlState::InScalar => parser.handle_in_scalar()?,
                YamlState::EndOfFile => break,
                _ => todo!(),
            }
            if parser.scanner.done_pos == cur_pos {
                loop_counter += 1;
                if loop_counter >= MAX_LOOP_COUNT {
                    return Err(YamlError::new(
                        ErrorKind::Bug,
                        format!(
                            "Dead-loop in parser: remains {:?}",
                            parser.scanner.remains()
                        ),
                        cur_pos,
                        cur_pos,
                    ));
                }
            } else {
                loop_counter = 0;
            }
        }

        return Ok(parser.events);
    }

    /// Stream started, but not `---` or string other than `b-break` found yet.
    fn handle_in_stream(&mut self) -> Result<(), YamlError> {
        if let Some(next_char) = self.scanner.peek_char().map(YamlChar::from) {
            if next_char.is_line_break() {
                self.scanner.next_char();
            } else if self.scanner.peek_till_linebreak_or_space().trim_end()
                == "---"
            {
                self.events.push(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak_or_space();
                self.states.push(YamlState::InDocument);
            } else {
                self.events.push(YamlEvent::DocumentStart(
                    false,
                    self.scanner.next_pos,
                ));
                self.states.push(YamlState::InDocument);
            }
        } else {
            if !self
                .events
                .iter()
                .any(|e| matches!(e, YamlEvent::DocumentEnd(_, _)))
            {
                // Empty content
                self.events
                    .push(YamlEvent::DocumentStart(false, YamlPosition::EOF));
                self.events
                    .push(YamlEvent::DocumentEnd(false, YamlPosition::EOF));
                self.states.pop();
            }
            self.events.push(YamlEvent::StreamEnd);
            self.states.pop();
        }
        Ok(())
    }

    fn handle_in_doc(&mut self) -> Result<(), YamlError> {
        let pos = self.scanner.next_pos;
        if let Some(next_char) = self.scanner.peek_char().map(YamlChar::from) {
            if next_char.is_line_break() {
                self.cur_idents += 0;
                self.scanner.next_char();
            } else if next_char.is_indent() {
                self.cur_idents += 1;
                self.scanner.next_char();
            } else if next_char.as_ref() == &'\t' {
                return Err(YamlError::new(
                    ErrorKind::InvalidStartOfToken,
                    format!("Tab(\\t) cannot be used as start of any token"),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else if next_char.is_comment() {
                self.scanner.advance_till_linebreak();
            } else if next_char.as_ref() == &'-'
                && self.scanner.advance_if_starts_with("- ")
            {
                self.events.push(YamlEvent::SequenceStart(pos));
                self.states.push(YamlState::InBlockSequnce(self.cur_idents));
            } else if self.scanner.peek_till_linebreak_or_space().trim_end()
                == "..."
            {
                self.states.pop();
                self.events
                    .push(YamlEvent::DocumentEnd(true, self.scanner.next_pos));
                self.scanner.advance_till_linebreak_or_space();
                self.cur_idents = 0;
            } else {
                self.states.push(YamlState::InScalar);
            }
        } else {
            self.events
                .push(YamlEvent::DocumentEnd(false, self.scanner.done_pos));
            self.states.pop();
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
            YamlTreeParser::parse("\n\r\n---").unwrap(),
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
            YamlTreeParser::parse("\n\r\n---\na\n...").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::Scalar(
                    "a".to_string(),
                    YamlPosition::new(4, 1),
                    YamlPosition::new(4, 2)
                ),
                YamlEvent::DocumentEnd(true, YamlPosition::new(5, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_document_with_comment() {
        assert_eq!(
            YamlTreeParser::parse("\n\r\n--- # test command\n...").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::DocumentEnd(true, YamlPosition::new(4, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }
}
