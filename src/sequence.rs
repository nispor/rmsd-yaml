// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlEvent, YamlState, YamlTreeParser};

impl<'a> YamlTreeParser<'a> {
    /// Invoked when there is `: ` in line or ends with `:`.
    /// Advance till map finished.
    pub(crate) fn handle_block_seq(
        &mut self,
        indent_count: usize,
    ) -> Result<(), YamlError> {
        eprintln!(
            "handle_block_seq {} {:?}",
            indent_count,
            self.scanner.remains()
        );
        self.events
            .push(YamlEvent::SequenceStart(self.scanner.next_pos));
        self.states.push(YamlState::InBlockSequnce);
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
                self.handle_node(indent_count + 1, indent_count + 1)?;
            } else if trimmed.starts_with("- ") {
                self.scanner.advance(indent_count + 2);
                self.handle_node(0, indent_count)?;
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

        self.events
            .push(YamlEvent::SequenceEnd(self.scanner.done_pos));
        self.states.pop();
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
                    "abc".to_string(),
                    YamlPosition::new(1, 5),
                    YamlPosition::new(1, 7)
                ),
                YamlEvent::Scalar(
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
