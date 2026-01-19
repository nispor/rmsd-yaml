// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlEvent, YamlTag, YamlTreeParser};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
enum ChompingMethod {
    Strip,
    #[default]
    Clip,
    Keep,
}

impl<'a> YamlTreeParser<'a> {
    /// Advance the scanner till scalar ends.
    pub(crate) fn handle_scalar(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_scalar {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        if let Some(line) = self.scanner.peek_line()
            && let Some(next_char) = line.trim_start_matches(' ').chars().next()
        {
            if line == "..." {
                return Ok(());
            }
            match next_char {
                '|' => {
                    self.scanner.advance_till_non_space();
                    self.scanner.next_char();
                    self.handle_literal_block_scalar()?;
                }
                '>' => {
                    self.scanner.advance_till_non_space();
                    self.scanner.next_char();
                    self.handle_folded_block_scalar()?;
                }
                '\'' => {
                    self.scanner.advance_till_non_space();
                    self.scanner.next_char();
                    self.handle_single_quoted_flow_scalar()?;
                }
                '"' => {
                    self.scanner.advance_till_non_space();
                    self.scanner.next_char();
                    self.handle_double_quoted_flow_scalar()?;
                }
                _ => {
                    self.handle_plain_scalar(
                        first_indent_count,
                        rest_indent_count,
                    )?;
                }
            }
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
        log::trace!("handle_literal_block_scalar {:?}", self.scanner.remains());
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
                    if line.trim_start_matches(' ').is_empty() {
                        self.scanner.next_line();
                        ret.push('\n');
                    } else {
                        break;
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
                ret = ret.trim_end_matches(['\n', '\r']).to_string();
            }
            ChompingMethod::Clip => {
                // the final line break character is preserved in the scalar’s
                // content. However, any trailing empty lines are excluded from
                // the scalar’s content.
                ret = ret.trim_end_matches(['\n', '\r']).to_string();
                ret.push('\n');
            }
            ChompingMethod::Keep => (),
        }

        let end_pos = self.scanner.done_pos;

        self.push_event(YamlEvent::Scalar(None, ret, start_pos, end_pos));
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

    pub(crate) fn handle_plain_scalar(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_plain_scalar {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        let mut start_pos = self.scanner.next_pos;
        let mut string_to_fold: Vec<&str> = Vec::new();
        let mut is_first_line = true;
        let mut tag: Option<YamlTag> = None;
        while let Some(line) = self.scanner.peek_line() {
            let pre_pos = self.scanner.done_pos;
            let cur_indent_count =
                line.chars().take_while(|c| *c == ' ').count();

            let expected_indent_count = if is_first_line {
                first_indent_count
            } else {
                rest_indent_count
            };

            if cur_indent_count < expected_indent_count {
                break;
            }
            if is_first_line {
                start_pos.column += cur_indent_count;
                is_first_line = false;
            }

            // document end indicator
            if line == "..." {
                break;
            }

            let trimmed = line.trim_start_matches(' ');
            if self.cur_state().is_block_seq() && trimmed.starts_with("- ") {
                break;
            }

            if trimmed.starts_with("!") {
                tag = self.handle_tag();
            }
            let Some(line) = self.scanner.peek_line() else {
                continue;
            };

            self.validate_plain_scalar(line)?;

            if self.cur_state().is_block_map_key() {
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                if let Some(offset) = line.find(": ") {
                    self.scanner.advance_offset(offset);
                    self.push_event(YamlEvent::Scalar(
                        tag,
                        line[expected_indent_count..offset].to_string(),
                        start_pos,
                        self.scanner.done_pos,
                    ));
                    return Ok(());
                } else if line.ends_with(":")
                    && line != ":"
                    && let Some(offset) = line.find(":")
                {
                    self.scanner.advance_offset(offset);
                    self.push_event(YamlEvent::Scalar(
                        tag,
                        line[expected_indent_count..line.len() - 1].to_string(),
                        start_pos,
                        self.scanner.done_pos,
                    ));
                    return Ok(());
                } else {
                    self.scanner.advance_till_linebreak();
                    return Err(YamlError::new(
                        ErrorKind::InvalidImplicitKey,
                        "Implicit key should contains ': ' within single line \
                         or ending with :"
                            .to_string(),
                        pre_pos,
                        self.scanner.done_pos,
                    ));
                }
            } else {
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      All leading and trailing white space characters are
                //      excluded from the content. Each continuation line must
                //      therefore contain at least one non-space character.
                //      Empty lines, if any, are consumed as part of the
                //      line folding.
                self.scanner.next_line();
                string_to_fold
                    .push(line.trim_matches(|c: char| matches!(c, '\t' | ' ')));

                if self.scanner.done_pos == pre_pos {
                    return Err(YamlError::new(
                        ErrorKind::Bug,
                        format!(
                            "handle_plain_scalar (): dead loop, remains {:?}",
                            self.scanner.remains(),
                        ),
                        pre_pos,
                        pre_pos,
                    ));
                }
            }
        }
        let str_val = fold_string(string_to_fold);
        let mut end_pos = self.scanner.done_pos;
        if !str_val.contains('\n') && end_pos.line == start_pos.line {
            end_pos.column = start_pos.column + str_val.chars().count() - 1;
        }

        self.push_event(YamlEvent::Scalar(tag, str_val, start_pos, end_pos));
        Ok(())
    }

    fn validate_plain_scalar(&mut self, line: &str) -> Result<(), YamlError> {
        // YAML SPEC 1.2, 7.3.3. Plain Style:
        //      Plain scalars must not begin with most indicators, as this
        //      would cause ambiguity with other YAML constructs.  However,
        //      the “:”, “?” and “-” indicators may be used as the first
        //      character if followed by a non-space “safe” character, as
        //      this causes no ambiguity.
        if let Some(first_char) = line.trim_start_matches(' ').chars().next() {
            match first_char {
                ',' | '[' | ']' | '{' | '}' | '#' | '&' | '*' | '!' | '|'
                | '>' | '\'' | '"' | '%' | '@' | '`' => {
                    return Err(YamlError::new(
                        ErrorKind::InvalidPlainScalarStart,
                        format!(
                            "Plain scalar should not start with '{first_char} \
                             '"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                ':' | '?' | '-' => {
                    if Some(' ') == self.scanner.remains().chars().nth(1) {
                        return Err(YamlError::new(
                            ErrorKind::InvalidPlainScalarStart,
                            format!(
                                "Plain scalar should not start with \
                                 '{first_char} '"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
                _ => (),
            }
        }

        // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
        //      Plain scalars must never contain the “: ” and “ #”
        //      character combinations.
        if !self.cur_state().is_block_map_key()
            && let Some(offset) = line.find(": ")
        {
            let pre_pos = self.scanner.done_pos;
            self.scanner.advance_offset(offset);
            return Err(YamlError::new(
                ErrorKind::AmbiguityPlainScalar,
                format!(
                    "Plain style scalar should not contains ': ' as it will \
                     cause ambiguity for mapping key: {line}"
                ),
                pre_pos,
                self.scanner.done_pos,
            ));
        }
        if let Some(offset) = line.find(" #") {
            let pre_pos = self.scanner.done_pos;
            self.scanner.advance_offset(offset);
            return Err(YamlError::new(
                ErrorKind::AmbiguityPlainScalar,
                format!(
                    "Plain style scalar should not contains ' #' as it will \
                     cause ambiguity for comment: {line}"
                ),
                pre_pos,
                self.scanner.done_pos,
            ));
        }

        // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
        //      In addition, inside flow collections, or when used as
        //      implicit keys, plain scalars must not contain the “[”, “]”,
        //      “{”, “}” and “,” characters.
        if self.cur_state().is_flow() || self.cur_state().is_block_map_key() {
            let pre_pos = self.scanner.done_pos;
            if let Some(offset) = line.find(['[', ']', '{', '}']) {
                self.scanner.advance_offset(offset);
                return Err(YamlError::new(
                    ErrorKind::AmbiguityPlainScalar,
                    "Inside flow collections, or when used as implicit keys, \
                     plain scalars must not contain the '[', ']', '{', and \
                     '}' characters"
                        .to_string(),
                    pre_pos,
                    self.scanner.done_pos,
                ));
            }
        }

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
    let mut ret = String::new();
    let mut iter = string_to_fold.into_iter().peekable();
    // the first line break is discarded and the rest are retained as content.
    if let Some(first_line) = iter.next() {
        ret.push_str(first_line);
    }
    let mut has_new_line_break = false;
    for line in iter {
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
                    None,
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
        assert_eq!(
            YamlTreeParser::parse("--- |3\n    abc \n    def\n   \n  \n")
                .unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    None,
                    " abc \n def\n".to_string(),
                    YamlPosition::new(2, 4),
                    YamlPosition::new(5, 3),
                ),
                YamlEvent::DocumentEnd(false, YamlPosition::new(5, 3)),
                YamlEvent::StreamEnd,
            ]
        );
    }

    #[test]
    fn test_block_scalar_literal_block_strip_fixed_ident() {
        let expected = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
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
                None,
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
    fn test_block_scalar_literal_all_indented() {
        assert_eq!(
            YamlTreeParser::parse("---\n   |\n   abc\n   def\n\n").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(1, 1)),
                YamlEvent::Scalar(
                    None,
                    "abc\ndef\n".to_string(),
                    YamlPosition::new(3, 4),
                    YamlPosition::new(5, 1)
                ),
                YamlEvent::DocumentEnd(false, YamlPosition::new(5, 1)),
                YamlEvent::StreamEnd,
            ]
        )
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
                    None,
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
