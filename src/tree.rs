// SPDX-License-Identifier: Apache-2.0

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlPosition, YamlScanner, YamlState,
};

#[derive(Debug)]
pub(crate) struct YamlTreeParser<'a> {
    pub(crate) scanner: YamlScanner<'a>,
    states: Vec<YamlState>,
    events: Vec<YamlEvent>,
}

impl<'a> YamlTreeParser<'a> {
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
        self.states.pop();
    }

    pub(crate) fn parse(input: &'a str) -> Result<Vec<YamlEvent>, YamlError> {
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
                        "YamlTreeParser::parse(): dead-loop: remains {:?}",
                        parser.scanner.remains()
                    ),
                    cur_pos,
                    cur_pos,
                ));
            }
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
                continue;
            }
            if line.starts_with("---") {
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak_or_space();
                self.handle_node(0, 0)?;
            } else if line == "..." {
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
                self.handle_node(0, 0)?;
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
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_node {} {}, {:?}",
            first_indent_count,
            rest_indent_count,
            self.scanner.remains()
        );
        // Ignore less indented empty line and comment line
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches(' ');
            let indent_count = line.chars().take_while(|c| *c == ' ').count();
            if trimmed.is_empty() && indent_count <= first_indent_count {
                self.scanner.advance_till_linebreak();
                continue;
            } else if trimmed.starts_with("# ") {
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
                self.handle_block_seq(expected_indent_count)?;
            } else if trimmed.starts_with('\'') || trimmed.starts_with('"') {
                // Flow style does not care indentation
                self.handle_scalar(0, 0)?;
            } else if trimmed.contains(": ") {
                self.handle_block_map(rest_indent_count)?;
            } else if trimmed.ends_with(":") {
                self.handle_block_map(rest_indent_count)?;
            } else if trimmed.starts_with("[") {
                self.handle_flow_seq()?;
            } else if trimmed.starts_with("{") {
                self.handle_flow_map()?;
            } else if line.trim_start_matches(' ').starts_with('\t') {
                return Err(YamlError::new(
                    ErrorKind::InvalidStartOfToken,
                    "Tab(\\t) cannot be used as start of any YAML node"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else {
                self.handle_scalar(first_indent_count, rest_indent_count)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;

    const TEST_DATA_FOLDER_PATH: &str = "yaml-test-suit-data";
    const DESCRIPTION_FILE_NAME: &str = "===";
    const INPUT_YAML_FILE_NAME: &str = "in.yaml";
    const TEST_EVENT_FILE_NAME: &str = "test.event";

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
            YamlTreeParser::parse("\n\r\n--- # test command\n...").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::DocumentEnd(true, YamlPosition::new(4, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn yaml_test_suit() {
        init_logger();

        let test_data_dir =
            std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
                .join(TEST_DATA_FOLDER_PATH);

        for entry in std::fs::read_dir(test_data_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if path.join(DESCRIPTION_FILE_NAME).exists() {
                    run_test(&path);
                } else {
                    for dir_entry in std::fs::read_dir(&path).unwrap() {
                        let entry = dir_entry.unwrap();
                        let path = entry.path();
                        if path.join(DESCRIPTION_FILE_NAME).exists() {
                            run_test(&path);
                        }
                    }
                }
            }
        }
    }

    static INIT_LOGGER: std::sync::Once = std::sync::Once::new();

    fn init_logger() {
        INIT_LOGGER.call_once(|| {
            env_logger::builder()
                .filter_level(log::LevelFilter::Trace)
                .init()
        });
    }

    fn run_test(path: &Path) {
        let test_name = read_file(&path.join(DESCRIPTION_FILE_NAME));
        let input_yaml = read_file(&path.join(INPUT_YAML_FILE_NAME));
        let expected_events = read_file(&path.join(TEST_EVENT_FILE_NAME));

        log::trace!("{}: {test_name}", path.file_name().unwrap().display());
        log::trace!("\n{}", input_yaml);

        let result = YamlTreeParser::parse(&input_yaml);

        if path.join("error").exists() {
            assert!(result.is_err());
        } else {
            let mut events_str = String::new();
            for event in result.unwrap() {
                events_str.push_str(&event.to_string());
                events_str.push('\n');
            }

            assert_eq!(expected_events, events_str);
        }
    }

    fn read_file(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }
}
