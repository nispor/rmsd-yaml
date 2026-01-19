// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlEvent, YamlState, YamlTreeParser};

impl<'a> YamlTreeParser<'a> {
    /// Invoked when there is `: ` in line or ends with `:`.
    /// Advance till map finished.
    pub(crate) fn handle_block_seq(
        &mut self,
        indent_count: usize,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_seq {} {:?}",
            indent_count,
            self.scanner.remains()
        );
        self.push_event(YamlEvent::SequenceStart(self.scanner.next_pos));
        self.push_state(YamlState::InBlockSequnce);
        while let Some(line) = self.scanner.peek_line() {
            if line.is_empty() {
                continue;
            }
            let cur_indent = line.chars().take_while(|c| *c == ' ').count();
            if cur_indent < indent_count {
                break;
            }
            let trimmed = line.trim_start_matches(' ');

            if trimmed == "-" {
                self.scanner.next_line();
                if let Some(next_line) = self.scanner.peek_line() {
                    let next_indent =
                        next_line.chars().take_while(|c| *c == ' ').count();
                    self.handle_node(next_indent, next_indent)?;
                } else {
                    if self.scanner.remains().is_empty() {
                        // Empty array
                        self.push_event(YamlEvent::Scalar(
                            None,
                            String::new(),
                            self.scanner.done_pos,
                            self.scanner.done_pos,
                        ));
                    }
                }
            } else if trimmed.starts_with("- ") {
                self.scanner.advance(cur_indent + 2);
                self.handle_node(0, cur_indent + 2)?;
            } else {
                return Err(YamlError::new(
                    ErrorKind::InvalidSequnceStartIndicator,
                    "Expecting '-\\n' or '- ' as sequence start Indicator"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            }
        }

        self.push_event(YamlEvent::SequenceEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }

    pub(crate) fn handle_flow_seq(&mut self) -> Result<(), YamlError> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::YamlPosition;

    #[test]
    fn test_sequence_of_plain_scalar() {
        assert_eq!(
            YamlTreeParser::parse("  - abc\n  - def\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::SequenceStart(YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    None,
                    "abc".to_string(),
                    YamlPosition::new(1, 5),
                    YamlPosition::new(1, 7)
                ),
                YamlEvent::Scalar(
                    None,
                    "def".to_string(),
                    YamlPosition::new(2, 5),
                    YamlPosition::new(2, 7)
                ),
                YamlEvent::SequenceEnd(YamlPosition::new(2, 8)),
                YamlEvent::DocumentEnd(false, YamlPosition::new(2, 8)),
                YamlEvent::StreamEnd,
            ]
        )
    }
}
