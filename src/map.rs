// SPDX-License-Identifier: Apache-2.0

use crate::{YamlError, YamlEvent, YamlState, YamlTreeParser};

impl<'a> YamlTreeParser<'a> {
    /// Consume the scanner till a block map is finished.
    pub(crate) fn handle_block_map(
        &mut self,
        indent_count: usize,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_map {} {:?}",
            indent_count,
            self.scanner.remains()
        );
        self.push_event(YamlEvent::MapStart(self.scanner.next_pos));
        let _next_indent_count = indent_count;
        let mut value_first_indent_count = 0;
        let mut value_rest_indent_count = 0;
        while let Some(line) = self.scanner.peek_line() {
            if line.is_empty() {
                continue;
            }
            let cur_indent = line.chars().take_while(|c| *c == ' ').count();
            if cur_indent < indent_count {
                break;
            }

            if self.cur_state().is_block_map_value() {
                self.handle_node(
                    value_first_indent_count,
                    value_rest_indent_count,
                );
                self.pop_state();
            } else {
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                self.push_state(YamlState::InBlockMapKey);
                self.handle_plain_scalar(indent_count, indent_count)?;
                self.pop_state();
                self.push_state(YamlState::InBlockMapValue);
                if line.ends_with(":") {
                    self.scanner.next_line();
                    value_first_indent_count = indent_count + 1;
                    value_rest_indent_count = indent_count + 1;
                } else if let Some(offset) = line.find(": ") {
                    self.scanner.advance_offset(2);
                    value_first_indent_count = 0;
                    value_rest_indent_count =
                        line[..offset].chars().count() + 2;
                }
                self.pop_state();
                self.push_state(YamlState::InBlockMapValue);
            }
        }

        self.push_event(YamlEvent::MapEnd(self.scanner.done_pos));
        self.pop_state();
        Ok(())
    }

    /// Consume the scanner till a flow map is finished and insert the parsed
    /// event.
    pub(crate) fn handle_flow_map(&mut self) -> Result<(), YamlError> {
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
        assert_eq!(
            YamlTreeParser::parse("a: 1\nb: 2\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::MapStart(YamlPosition::new(1, 1)),
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
            YamlTreeParser::parse("a:\n  b\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::MapStart(YamlPosition::new(1, 1)),
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
