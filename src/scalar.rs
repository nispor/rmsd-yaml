// SPDX-License-Identifier: Apache-2.0

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlPosition, YamlState, YamlTreeParser,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
enum ChompingMethod {
    Strip,
    #[default]
    Clip,
    Keep,
}

impl<'a> YamlTreeParser<'a> {
    pub(crate) fn handle_in_scalar(&mut self) -> Result<(), YamlError> {
        if let Some(next_char) = self.scanner.peek_char() {
            match next_char {
                '|' => {
                    self.scanner.next_char();
                    self.handle_literal_block_scalar()?;
                }
                '>' => {
                    self.scanner.next_char();
                    self.handle_folded_block_scalar()?;
                }
                '\'' => {
                    self.scanner.next_char();
                    self.handle_single_quoted_flow_scalar()?;
                }
                '"' => {
                    self.scanner.next_char();
                    self.handle_double_quoted_flow_scalar()?;
                }
                _ => {
                    self.handle_plain_flow_scalar()?;
                }
            }
        } else {
            self.states.pop();
        }
        Ok(())
    }

    /// Consume till literal block scalar ends by:
    /// 1. End of file
    /// 2. `...`
    /// 3. Less indention
    pub(crate) fn handle_literal_block_scalar(
        &mut self,
    ) -> Result<(), YamlError> {
        let mut ret = String::new();
        let mut indentation_indicator: Option<usize> = None;
        let mut chomping_method = ChompingMethod::default();
        let mut start_pos = self.scanner.next_pos;

        if let Some(next_char) = self.scanner.peek_char() {
            match next_char {
                '1'..'9' => {
                    self.scanner.next_char();
                    indentation_indicator = Some(
                        next_char
                            .to_digit(10)
                            .map(|d| d as usize)
                            .unwrap_or_default(),
                    );
                    if self.scanner.advance_if_starts_with("+") {
                        chomping_method = ChompingMethod::Strip;
                    } else if self.scanner.advance_if_starts_with("-") {
                        chomping_method = ChompingMethod::Keep;
                    }
                }
                '+' => {
                    self.scanner.next_char();
                    chomping_method = ChompingMethod::Strip;
                    if let Some(d) = self
                        .scanner
                        .peek_char()
                        .and_then(|c| c.to_digit(10))
                        .map(|d| d as usize)
                    {
                        self.scanner.next_char();
                        indentation_indicator = Some(d);
                    }
                }
                '-' => {
                    self.scanner.next_char();
                    chomping_method = ChompingMethod::Keep;
                    if let Some(d) = self
                        .scanner
                        .peek_char()
                        .and_then(|c| c.to_digit(10))
                        .map(|d| d as usize)
                    {
                        self.scanner.next_char();
                        indentation_indicator = Some(d);
                    }
                }
                _ => (),
            }
            // After `|` and its optional indicators, we should get a line
            // break or comments or both.
            self.scanner.expect_comment_or_line_break()?;

            let leading_space_count = self.scanner.count_block_identation();
            let desired_indent = if let Some(d) = indentation_indicator {
                d
            } else {
                leading_space_count
            };
            start_pos = self.scanner.next_pos;
            start_pos.column += desired_indent;
            while let Some(line) = self.scanner.peek_line() {
                let pre_pos = self.scanner.done_pos;
                let leading_space =
                    line.chars().take_while(|c| c == &' ').count();
                if leading_space < desired_indent {
                    if self.cur_state().is_container() {
                        self.states.pop();
                        self.events.push(YamlEvent::Scalar(
                            ret,
                            start_pos,
                            self.scanner.done_pos,
                        ));
                        return Ok(());
                    } else {
                        // If we don't have parent container(map/sequence),
                        // less indented line are treated as empty line.
                        self.scanner.next_line();
                        ret.push('\n');
                    }
                } else if let Some(line) = self.scanner.next_line() {
                    // Remove indent then append
                    ret.push_str(&line[desired_indent..]);
                    ret.push('\n');
                } else {
                    // No line left
                    break;
                }

                if self.scanner.done_pos == pre_pos {
                    return Err(YamlError::new(
                        ErrorKind::Bug,
                        format!(
                            "handle_literal_block_scalar(): dead loop, \
                             remains {:?}",
                            self.scanner.remains(),
                        ),
                        pre_pos,
                        pre_pos,
                    ));
                }
            }
        }

        match chomping_method {
            ChompingMethod::Strip => {
                // the final line break and any trailing empty lines are
                // excluded from the scalar’s content.
                ret = ret
                    .trim_end_matches(|c| matches!(c, '\n' | '\r'))
                    .to_string();
            }
            ChompingMethod::Clip => {
                // the final line break character is preserved in the scalar’s
                // content. However, any trailing empty lines are excluded from
                // the scalar’s content.
                ret = ret
                    .trim_end_matches(|c| matches!(c, '\n' | '\r'))
                    .to_string();
                ret.push('\n');
            }
            ChompingMethod::Keep => (),
        }

        self.events.push(YamlEvent::Scalar(
            ret,
            start_pos,
            self.scanner.done_pos,
        ));
        self.states.pop();
        Ok(())
    }

    /// Consume till folded block scalar ends by:
    /// 1. End of file
    /// 2. `...`
    /// 3. Less indention
    pub(crate) fn handle_folded_block_scalar(
        &mut self,
    ) -> Result<(), YamlError> {
        todo!()
    }

    pub(crate) fn handle_single_quoted_flow_scalar(
        &mut self,
    ) -> Result<(), YamlError> {
        todo!()
    }

    pub(crate) fn handle_double_quoted_flow_scalar(
        &mut self,
    ) -> Result<(), YamlError> {
        todo!()
    }

    pub(crate) fn handle_plain_flow_scalar(&mut self) -> Result<(), YamlError> {
        let start_pos = self.scanner.next_pos;
        // YAML SPEC 1.2, 7.3.3. Plain Style:
        //      Plain scalars must not begin with most indicators, as this
        //      would cause ambiguity with other YAML constructs.  However,
        //      the “:”, “?” and “-” indicators may be used as the first
        //      character if followed by a non-space “safe” character, as
        //      this causes no ambiguity.
        if let Some(first_char) = self.scanner.peek_char() {
            match first_char {
                ',' | '[' | ']' | '{' | '}' | '#' | '&' | '*' | '!' | '|'
                | '>' | '\'' | '"' | '%' | '@' | '`' => {
                    return Err(YamlError::new(
                        ErrorKind::InvalidPlainScalarStart,
                        format!(
                            "Plain scalar should not start with '{first_char} \
                             '"
                        ),
                        start_pos,
                        self.scanner.next_pos,
                    ));
                }
                ':' | '?' | '-' => {
                    if Some(' ')
                        == self.scanner.remains().chars().skip(1).next()
                    {
                        return Err(YamlError::new(
                            ErrorKind::InvalidPlainScalarStart,
                            format!(
                                "Plain scalar should not start with \
                                 '{first_char} '"
                            ),
                            start_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
                _ => (),
            }
        }

        let mut string_to_fold: Vec<&str> = Vec::new();
        while let Some(line) = self.scanner.peek_line() {
            let pre_pos = self.scanner.done_pos;

            // document end indicator
            if line == "..." {
                break;
            }

            // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
            //      Plain scalars must never contain the “: ” and “ #”
            //      character combinations.
            if let Some(offset) = line.find(": ").or_else(|| line.find(" #")) {
                self.scanner.advance_offset(offset);
                return Err(YamlError::new(
                    ErrorKind::AmbiguityPlainScalar,
                    format!(
                        "Plain style scalar should not contains ': ' or ' #' \
                         as it will cause ambiguity for mapping key or \
                         comment: {line}"
                    ),
                    pre_pos,
                    self.scanner.done_pos,
                ));
            }

            // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
            //      In addition, inside flow collections, or when used as
            //      implicit keys, plain scalars must not contain the “[”, “]”,
            //      “{”, “}” and “,” characters.
            if self.pre_state().is_flow() || self.pre_state().is_block_map_key()
            {
                if let Some(offset) =
                    line.find(|c: char| matches!(c, '[' | ']' | '{' | '}'))
                {
                    self.scanner.advance_offset(offset);
                    return Err(YamlError::new(
                        ErrorKind::AmbiguityPlainScalar,
                        "Inside flow collections, or when used as implicit \
                         keys, plain scalars must not contain the '[', ']', \
                         '{', and '}' characters"
                            .to_string(),
                        pre_pos,
                        self.scanner.done_pos,
                    ));
                }
            }

            if self.pre_state().is_block_map_key() {
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                if let Some(offset) = line.find(": ") {
                    self.scanner.advance_offset(offset);
                    self.events.push(YamlEvent::Scalar(
                        line[..offset].to_string(),
                        start_pos,
                        self.scanner.done_pos,
                    ));
                    self.states.pop();
                    return Ok(());
                } else {
                    self.scanner.advance_till_linebreak();
                    return Err(YamlError::new(
                        ErrorKind::InvalidImplicitKey,
                        "Implicit key should contains ': ' within single line"
                            .to_string(),
                        pre_pos,
                        self.scanner.done_pos,
                    ));
                }
            }

            // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
            //      All leading and trailing white space characters are
            //      excluded from the content. Each continuation line must
            //      therefore contain at least one non-space character. Empty
            //      lines, if any, are consumed as part of the line folding.
            self.scanner.next_line();
            string_to_fold
                .push(line.trim_matches(|c: char| matches!(c, '\t' | ' ')));

            if self.scanner.done_pos == pre_pos {
                return Err(YamlError::new(
                    ErrorKind::Bug,
                    format!(
                        "handle_plain_flow_scalar (): dead loop, remains {:?}",
                        self.scanner.remains(),
                    ),
                    pre_pos,
                    pre_pos,
                ));
            }
        }

        self.events.push(YamlEvent::Scalar(
            fold_string(string_to_fold),
            start_pos,
            self.scanner.done_pos,
        ));
        self.states.pop();
        Ok(())
    }
}

// YAML 1.2.2 SPEC, 6.5. Line Folding:
//      Line folding allows long lines to be broken for readability, while
//      retaining the semantics of the original long line. If a line break is
//      followed by an empty line, it is trimmed;
//      Otherwise (the following line is not empty), the line break is
//      converted to a single space (x20).
//      A folded non-empty line may end with either of the above line breaks.
fn fold_string(string_to_fold: Vec<&str>) -> String {
    println!("HAHA41 {:?}", string_to_fold);
    let mut ret = String::new();
    let mut iter = string_to_fold.into_iter().peekable();
    // the first line break is discarded and the rest are retained as content.
    if let Some(first_line) = iter.next() {
        ret.push_str(first_line);
    }
    let mut has_new_line_break = false;
    while let Some(line) = iter.next() {
        if line.is_empty() {
            has_new_line_break = true;
            ret.push('\n');
        } else {
            if !has_new_line_break {
                ret.push(' ');
            }
            ret.push_str(line);
            has_new_line_break = false;
        }
    }

    ret
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::YamlPosition;

    #[test]
    fn test_block_scalar_literal_block_clip_auto() {
        assert_eq!(
            YamlTreeParser::parse("--- |\n abc \n def\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    "abc \ndef\n".to_string(),
                    YamlPosition::new(2, 2),
                    YamlPosition::new(3, 5)
                ),
                YamlEvent::DocumentEnd(false, YamlPosition::new(3, 5)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_block_scalar_literal_block_clip_fixed_ident() {
        let expected = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                " abc \n def\n".to_string(),
                YamlPosition::new(2, 4),
                YamlPosition::new(5, 3),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(5, 3)),
            YamlEvent::StreamEnd,
        ];
        assert_eq!(
            YamlTreeParser::parse("--- |3\n    abc \n    def\n   \n  \n")
                .unwrap(),
            expected,
        );
    }

    #[test]
    fn test_block_scalar_literal_block_strip_fixed_ident() {
        let expected = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                " abc \n def".to_string(),
                YamlPosition::new(2, 4),
                YamlPosition::new(3, 8),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(3, 8)),
            YamlEvent::StreamEnd,
        ];
        assert_eq!(
            YamlTreeParser::parse("--- |3+\n    abc \n    def\n").unwrap(),
            expected
        );
        assert_eq!(
            YamlTreeParser::parse("--- |+3\n    abc \n    def\n").unwrap(),
            expected
        );
    }

    #[test]
    fn test_block_scalar_literal_block_keep_fixed_ident() {
        let expected = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                " abc \n def  \n\n\n".to_string(),
                YamlPosition::new(2, 4),
                YamlPosition::new(5, 1),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(5, 1)),
            YamlEvent::StreamEnd,
        ];
        assert_eq!(
            YamlTreeParser::parse("--- |3-\n    abc \n    def  \n   \n\n")
                .unwrap(),
            expected
        );
        assert_eq!(
            YamlTreeParser::parse("--- |-3\n    abc \n    def  \n   \n\n")
                .unwrap(),
            expected
        );
    }

    #[test]
    fn test_plain_scalar_folding() {
        assert_eq!(
            YamlTreeParser::parse(
                "1st non-empty\n\n 2nd non-empty \n\t3rd non-empty"
            )
            .unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    "1st non-empty\n2nd non-empty 3rd non-empty".to_string(),
                    YamlPosition::new(1, 1),
                    YamlPosition::new(4, 14)
                ),
                YamlEvent::DocumentEnd(false, YamlPosition::new(4, 14)),
                YamlEvent::StreamEnd,
            ]
        )
    }
}
