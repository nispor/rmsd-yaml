// SPDX-License-Identifier: Apache-2.0

use crate::parser::is_document_end_marker;
use crate::{ErrorKind, YamlError, YamlEvent, YamlParser, YamlScalarStyle};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
enum ChompingMethod {
    Strip,
    #[default]
    Clip,
    Keep,
}

impl<'a> YamlParser<'a> {
    fn get_indent_indicator_and_chomping_method(
        &mut self,
    ) -> Result<(Option<usize>, ChompingMethod), YamlError> {
        let mut indentation_indicator: Option<usize> = None;
        let mut chomping_method = ChompingMethod::default();
        if let Some(next_char) = self.scanner.peek_char() {
            match next_char {
                '1'..'9' => {
                    self.scanner.next_char();
                    indentation_indicator =
                        Some(next_char.to_digit(10).unwrap() as usize);
                    if self.scanner.advance_if_starts_with("+") {
                        chomping_method = ChompingMethod::Keep;
                    } else if self.scanner.advance_if_starts_with("-") {
                        chomping_method = ChompingMethod::Strip;
                    }
                }
                '0' => {
                    return Err(YamlError::new(
                        ErrorKind::InvalidStartOfToken,
                        "Block scalar indentation indicator must not be 0"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                '+' => {
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
                '-' => {
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
                _ => (),
            }
        }

        Ok((indentation_indicator, chomping_method))
    }

    /// Advance the scanner till scalar ends.
    pub(crate) fn handle_scalar(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        anchor: Option<String>,
        tag: Option<String>,
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
                    self.handle_block_scalar(anchor, tag, true)?;
                }
                '>' => {
                    self.scanner.advance_till_non_space();
                    self.scanner.next_char();
                    self.handle_block_scalar(anchor, tag, false)?;
                }
                '\'' => {
                    self.scanner.advance_till_non_space();
                    self.handle_single_quoted_flow_scalar(anchor, tag)?;
                }
                '"' => {
                    self.scanner.advance_till_non_space();
                    self.handle_double_quoted_flow_scalar(anchor, tag)?;
                }
                _ => {
                    self.handle_plain_scalar(
                        first_indent_count,
                        rest_indent_count,
                        anchor,
                        tag,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Consume a block scalar (`|` literal or `>` folded) whose style
    /// indicator has already been consumed by the caller.
    ///
    /// YAML 1.2.2 SPEC, 8.1. Block Scalar Styles.
    pub(crate) fn handle_block_scalar(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
        literal: bool,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_block_scalar {} {:?}",
            if literal { "literal" } else { "folded" },
            self.scanner.remains()
        );
        let (indentation_indicator, chomping_method) =
            self.get_indent_indicator_and_chomping_method()?;

        // After the style indicator and its optional indicators, we should
        // get a line break or comments or both.
        self.scanner.expect_comment_or_line_break()?;

        let mut start_pos = self.scanner.next_pos;
        // The content indentation must be more indented than the parent
        // block collection (when there is one).
        let parent_floor = self.block_indent.map_or(0, |b| b + 1);

        let (content_indent, mut trailing_breaks) =
            if let Some(d) = indentation_indicator {
                (d + self.block_indent.unwrap_or(0), 0)
            } else {
                self.detect_block_scalar_indent(parent_floor)?
            };
        // The scalar content starts at the content indentation level.
        // (The literal style reports the content column; the folded
        // style keeps the header column for backward compatibility.)
        if literal {
            start_pos.column += content_indent;
        }

        let mut content = String::new();
        // A line break pending after the previous content line.
        let mut leading_break = false;
        // Whether the previous content line started with a space or tab
        // (a more-indented line), which prevents line folding.
        let mut leading_blank = false;

        loop {
            // Consume empty lines. A line is empty when it only contains
            // spaces and has at most `content_indent` of them; a line with
            // more spaces is itself a content line (YAML 1.2.2, 8.1.1.1).
            while let Some(line) = self.scanner.peek_line() {
                let spaces = line.chars().take_while(|c| *c == ' ').count();
                if line.chars().all(|c| c == ' ') && spaces <= content_indent {
                    self.scanner.advance_till_linebreak();
                    trailing_breaks += 1;
                } else {
                    break;
                }
            }

            let Some(line) = self.scanner.peek_line() else {
                break;
            };
            let spaces = line.chars().take_while(|c| *c == ' ').count();

            // The document end marker `...` terminates the block scalar
            // even when it is as indented as the content.
            if is_document_end_marker(line.trim_start_matches(' ')) {
                break;
            }

            // A tab inside the indentation region is an error (a tab
            // character where an indentation space is expected).
            if spaces < content_indent && line.chars().nth(spaces) == Some('\t')
            {
                return Err(YamlError::new(
                    ErrorKind::InvalidStartOfToken,
                    "Found a tab character where an indentation space is \
                     expected in a block scalar"
                        .to_string(),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            }
            if spaces < content_indent {
                // A non-empty line less indented than the content: the
                // block scalar ends here (comment or next token).
                break;
            }

            let rest: String = line.chars().skip(content_indent).collect();
            self.scanner.advance_till_linebreak();

            let current_blank = rest
                .chars()
                .next()
                .map(|c| c == ' ' || c == '\t')
                .unwrap_or(true);

            // YAML 1.2.2 SPEC, 8.1.3. Folded Style:
            //     Line breaks and empty lines separating folded and
            //     more-indented lines are not folded. A line break is
            //     folded into a single space only when both neighbouring
            //     lines are regular (non-blank) lines with no empty lines
            //     in between.
            if !literal && leading_break && !leading_blank && !current_blank {
                if trailing_breaks == 0 {
                    content.push(' ');
                }
            } else if leading_break {
                content.push('\n');
            }
            for _ in 0..trailing_breaks {
                content.push('\n');
            }
            trailing_breaks = 0;
            content.push_str(&rest);
            leading_break = true;
            leading_blank = current_blank;
        }

        // YAML 1.2.2 SPEC, 8.1.1.2. Block Chomping Indicator.
        match chomping_method {
            ChompingMethod::Strip => (),
            ChompingMethod::Clip | ChompingMethod::Keep => {
                if leading_break {
                    content.push('\n');
                }
            }
        }
        if chomping_method == ChompingMethod::Keep {
            for _ in 0..trailing_breaks {
                content.push('\n');
            }
        }

        let end_pos = self.scanner.done_pos;
        self.push_event(YamlEvent::Scalar(
            anchor,
            tag,
            content,
            if literal {
                YamlScalarStyle::Literal
            } else {
                YamlScalarStyle::Folded
            },
            start_pos,
            end_pos,
        ));
        Ok(())
    }

    /// Determine the content indentation level of a block scalar that has
    /// no explicit indentation indicator. The leading empty lines are
    /// consumed here and reported back so they can later be joined into
    /// the content (or chomped).
    ///
    /// YAML 1.2.2 SPEC, 8.1.1.1. Block Indentation Indicator:
    ///     If no indentation indicator is given, then the content
    ///     indentation level is equal to the number of leading spaces on
    ///     the first non-empty line of the contents. If there is no
    ///     non-empty line then the content indentation level is equal to
    ///     the number of spaces on the longest line.
    ///     It is an error for any of the leading empty lines to contain
    ///     more spaces than the first non-empty line.
    fn detect_block_scalar_indent(
        &mut self,
        parent_floor: usize,
    ) -> Result<(usize, usize), YamlError> {
        let mut max_empty_indent = 0usize;
        let mut leading_empties = 0usize;
        while let Some(line) = self.scanner.peek_line() {
            let indent = line.chars().take_while(|c| *c == ' ').count();
            if line.chars().all(|c| c == ' ') {
                max_empty_indent = max_empty_indent.max(indent);
                leading_empties += 1;
                self.scanner.advance_till_linebreak();
            } else {
                if max_empty_indent > indent {
                    return Err(YamlError::new(
                        ErrorKind::InvalidStartOfToken,
                        "Leading empty lines must not contain more spaces \
                         than the first non-empty line of a block scalar"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                return Ok((indent.max(parent_floor), leading_empties));
            }
        }
        Ok((max_empty_indent.max(parent_floor), leading_empties))
    }

    /// Parse a single-quoted flow scalar.    /// Parse a single-quoted flow scalar. The scanner must stay at the
    /// opening `'`.
    ///
    /// YAML 1.2.2 SPEC, 7.4.2. Single-Quoted Styles:
    ///     A single-quoted scalar may not contain a single quote unless
    ///     it is escaped by two adjacent single quotes (`''`).
    pub(crate) fn handle_single_quoted_flow_scalar(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        if self.scanner.next_char() != Some('\'') {
            return Err(YamlError::new(
                ErrorKind::Bug,
                format!(
                    "handle_single_quoted_flow_scalar() got a scanner not \
                     started with ': {:?}",
                    self.scanner.remains()
                ),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        let start_pos = self.scanner.next_pos;
        let mut ret = String::new();
        let mut closed = false;
        while let Some(c) = self.scanner.next_char() {
            if c == '\'' {
                if self.scanner.peek_char() == Some('\'') {
                    self.scanner.next_char();
                    ret.push('\'');
                } else {
                    closed = true;
                    break;
                }
            } else {
                ret.push(c);
            }
        }
        if !closed {
            return Err(YamlError::new(
                ErrorKind::UnfinishedQuote,
                format!(
                    "Expecting closing single quote, but got: {:?}",
                    self.scanner.remains()
                ),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        self.push_event(YamlEvent::Scalar(
            anchor,
            tag,
            flow_folding(ret),
            YamlScalarStyle::SingleQuoted,
            start_pos,
            self.scanner.done_pos,
        ));
        Ok(())
    }

    /// Parse a plain scalar within a flow collection. The scalar ends at
    /// `,`, `]`, `}`, or `:` followed by a separation character.
    ///
    /// YAML 1.2.2 SPEC, 7.3.3. Plain Style:
    ///     In flow collections, plain scalars must not contain the `,`,
    ///     `[`, `]`, `{` and `}` characters, and `:` is restricted.
    pub(crate) fn handle_flow_plain_scalar(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        let start_pos = self.scanner.next_pos;
        let mut ret = String::new();
        'scalar: while let Some(c) = self.scanner.peek_char() {
            match c {
                ',' | ']' | '}' => break,
                ':' => {
                    // Inside flow context, `:` only acts as a mapping
                    // indicator when followed by a separation character.
                    let next = self.scanner.remains().chars().nth(1);
                    if matches!(
                        next,
                        None | Some(' ')
                            | Some('\t')
                            | Some('\n')
                            | Some('\r')
                            | Some(',')
                            | Some(']')
                            | Some('}')
                    ) {
                        break;
                    }
                    self.scanner.next_char();
                    ret.push(c);
                }
                '#' if ret.ends_with(' ') || ret.is_empty() => {
                    // A comment starts after white space.
                    break;
                }
                '\n' | '\r' => {
                    // YAML 1.2.2 SPEC, 6.8. Flow Folding: line breaks
                    // inside flow scalars are folded.
                    ret = ret.trim_end_matches(' ').to_string();
                    let mut line_breaks = 0usize;
                    while let Some(w) = self.scanner.peek_char() {
                        match w {
                            '\n' | '\r' => {
                                line_breaks += 1;
                                self.scanner.next_char();
                            }
                            ' ' | '\t' => {
                                self.scanner.next_char();
                            }
                            '#' => {
                                // A comment on its own line ends the
                                // flow scalar; the surrounding flow
                                // collection consumes the comment.
                                break 'scalar;
                            }
                            _ => break,
                        }
                    }
                    match self.scanner.peek_char() {
                        None | Some(',') | Some(']') | Some('}') => break,
                        Some(':') => {
                            let next = self.scanner.remains().chars().nth(1);
                            if matches!(
                                next,
                                None | Some(' ')
                                    | Some('\t')
                                    | Some('\n')
                                    | Some('\r')
                                    | Some(',')
                                    | Some(']')
                                    | Some('}')
                            ) {
                                break;
                            }
                        }
                        Some(_) => (),
                    }
                    if line_breaks <= 1 {
                        ret.push(' ');
                    } else {
                        ret.push('\n');
                    }
                }
                _ => {
                    self.scanner.next_char();
                    ret.push(c);
                }
            }
        }
        let value = ret.trim_end_matches(' ').to_string();
        if value == "-" || value.starts_with("- ") {
            return Err(YamlError::new(
                ErrorKind::InvalidPlainScalarStart,
                format!(
                    "Plain scalar in flow context should not be '-', but got: \
                     {value}"
                ),
                start_pos,
                self.scanner.done_pos,
            ));
        }
        self.push_event(YamlEvent::Scalar(
            anchor,
            tag,
            value,
            YamlScalarStyle::Plain,
            start_pos,
            self.scanner.done_pos,
        ));
        Ok(())
    }

    /// Should start with `"` and end with `"`
    pub(crate) fn handle_double_quoted_flow_scalar(
        &mut self,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), YamlError> {
        let mut ret = String::new();
        let mut first_quote_found = false;
        let mut start_pos = self.scanner.next_pos;
        // Flow folding (YAML 1.2.2, 6.5) applies to *real* line breaks
        // only: a break folds to a single space, unless followed by an
        // empty line, in which case the break is trimmed and each empty
        // line becomes a line feed. Escaped line feeds (e.g. `\n`) are
        // content and must never be folded, so they are pushed verbatim.
        let mut pending_breaks = 0usize;

        while let Some(c) = self.scanner.next_char() {
            match c {
                '"' if first_quote_found => {
                    // The closing quote plays the role of the final
                    // line's content when resolving pending breaks.
                    if pending_breaks == 1 {
                        ret.push(' ');
                    } else {
                        for _ in 1..pending_breaks {
                            ret.push('\n');
                        }
                    }
                    break;
                }
                '"' => {
                    start_pos = self.scanner.done_pos;
                    first_quote_found = true;
                }
                '\\' => {
                    let esc = self.read_escaped_char()?;
                    if pending_breaks == 1 {
                        ret.push(' ');
                    } else {
                        for _ in 1..pending_breaks {
                            ret.push('\n');
                        }
                    }
                    pending_breaks = 0;
                    ret.push(esc);
                }
                '\r' | '\n' => {
                    if c == '\r' && self.scanner.peek_char() == Some('\n') {
                        self.scanner.next_char();
                    }
                    // Separation spaces at the end of the current line
                    // are a presentation detail.
                    let trimmed_len = ret.trim_end_matches([' ', '\t']).len();
                    ret.truncate(trimmed_len);
                    pending_breaks += 1;
                }
                ' ' | '\t' if pending_breaks > 0 => {
                    // Leading separation spaces of a continuation line
                    // are a presentation detail.
                }
                c => {
                    if pending_breaks == 1 {
                        ret.push(' ');
                    } else {
                        for _ in 1..pending_breaks {
                            ret.push('\n');
                        }
                    }
                    pending_breaks = 0;
                    ret.push(c);
                }
            }
        }

        self.push_event(YamlEvent::Scalar(
            anchor,
            tag,
            ret,
            YamlScalarStyle::DoubleQuoted,
            start_pos,
            self.scanner.done_pos,
        ));
        Ok(())
    }

    pub(crate) fn handle_plain_scalar(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        anchor: Option<String>,
        mut tag: Option<String>,
    ) -> Result<(), YamlError> {
        log::trace!(
            "handle_plain_scalar {first_indent_count} {rest_indent_count} {:?}",
            self.scanner.remains()
        );
        let mut start_pos = self.scanner.next_pos;
        let mut string_to_fold: Vec<&str> = Vec::new();
        let mut is_first_line = true;
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
            let first_line = is_first_line;
            if is_first_line {
                start_pos.column += cur_indent_count;
                is_first_line = false;
            }

            // document end indicator (optionally followed by a comment)
            if is_document_end_marker(line.trim_start_matches(' ')) {
                break;
            }

            // A `---` document start marker terminates the scalar too.
            if line.trim_start_matches(' ').starts_with("---")
                && (line.trim_start_matches(' ').len() == 3
                    || line.trim_start_matches(' ').chars().nth(3) == Some(' '))
            {
                break;
            }

            let trimmed = line.trim_start_matches(' ');
            if self.cur_state().is_block_seq() && trimmed.starts_with("- ") {
                break;
            }

            if !self.cur_state().is_block_map_key() && line.contains(": ") {
                break;
            }

            if trimmed.starts_with("!") {
                tag = self.handle_tag()?;
            }
            let Some(mut line) = self.scanner.peek_line() else {
                continue;
            };
            // YAML 1.2.2 SPEC, 7.1. Comment Lines:
            //      Comments are a presentation detail and must not be
            //      used to convey content information.
            let mut scalar_ends_after_comment = false;
            if !self.cur_state().is_block_map_key()
                && let Some(comment_offset) = line.find(" #")
            {
                line = &line[..comment_offset];
                // A plain scalar followed by an inline comment ends at
                // the comment: the following line cannot continue it.
                scalar_ends_after_comment = true;
            }
            let trimmed = line.trim_start_matches(' ');

            // When parsing an implicit mapping key, only the key
            // portion matters for validation: the value after `: `
            // may legitimately contain flow indicators (e.g.
            // `a: {x: 1}`).
            let validation_line = if self.cur_state().is_block_map_key() {
                if let Some(offset) = line.find(": ") {
                    &line[..offset]
                } else if let Some(offset) =
                    line.strip_suffix(':').map(|_| line.len() - 1)
                {
                    &line[..offset]
                } else {
                    line
                }
            } else {
                line
            };
            self.validate_plain_scalar(validation_line, first_line)?;

            if self.cur_state().is_block_map_key() {
                // YAML 1.2.2 SPEC, 7.3.3. Plain Style:
                //      Plain scalars are further restricted to a single line
                //      when contained inside an implicit key.
                if let Some(offset) = line.find(": ") {
                    self.scanner.advance_offset(offset);
                    self.push_event(YamlEvent::Scalar(
                        anchor,
                        tag,
                        line[expected_indent_count..offset].to_string(),
                        YamlScalarStyle::Plain,
                        start_pos,
                        self.scanner.done_pos,
                    ));
                    return Ok(());
                } else if line.ends_with(":")
                    && let Some(offset) = line.find(":")
                {
                    self.scanner.advance_offset(offset);
                    if line == ":" {
                        // Empty key
                        self.push_event(YamlEvent::Scalar(
                            anchor,
                            tag,
                            String::new(),
                            YamlScalarStyle::Plain,
                            start_pos,
                            self.scanner.done_pos,
                        ));
                    } else {
                        self.push_event(YamlEvent::Scalar(
                            anchor,
                            tag,
                            line[expected_indent_count..line.len() - 1]
                                .trim_end()
                                .to_string(),
                            YamlScalarStyle::Plain,
                            start_pos,
                            self.scanner.done_pos,
                        ));
                    }
                    return Ok(());
                } else if trimmed.is_empty() {
                    self.scanner.next_line();
                } else {
                    self.scanner.advance_till_linebreak();
                    return Err(YamlError::new(
                        ErrorKind::InvalidImplicitKey,
                        format!(
                            "Implicit key should contains ': ' within single \
                             line or ending with :, but got: {line:?}"
                        ),
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

                if scalar_ends_after_comment {
                    // The inline comment terminated the scalar; the
                    // next line belongs to the surrounding context.
                    break;
                }

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
        let str_val = line_folding(string_to_fold);
        let mut end_pos = self.scanner.done_pos;
        if !str_val.contains('\n') && end_pos.line == start_pos.line {
            end_pos.column = start_pos.column + str_val.chars().count() - 1;
        }

        self.push_event(YamlEvent::Scalar(
            anchor,
            tag,
            str_val,
            YamlScalarStyle::Plain,
            start_pos,
            end_pos,
        ));
        Ok(())
    }

    fn validate_plain_scalar(
        &mut self,
        line: &str,
        first_line: bool,
    ) -> Result<(), YamlError> {
        // YAML SPEC 1.2, 7.3.3. Plain Style:
        //      Plain scalars must not begin with most indicators, as this
        //      would cause ambiguity with other YAML constructs.  However,
        //      the “:”, “?” and “-” indicators may be used as the first
        //      character if followed by a non-space “safe” character, as
        //      this causes no ambiguity.
        if let Some(first_char) = line.trim_start_matches(' ').chars().next() {
            match first_char {
                ',' | '[' | ']' | '{' | '}' | '#' | '&' | '*' | '!' | '|'
                | '>' | '\'' | '"' | '@' | '`' => {
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
                // The `%` reserved indicator is only rejected on the
                // first line; continuation lines may start with it
                // (e.g. a line that looks like a directive).
                '%' if first_line => {
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
                ':' | '?' | '-'
                    if Some(' ') == self.scanner.remains().chars().nth(1) =>
                {
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
                _ => (),
            }
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
//      followed by an empty line, it is trimmed; the first line break is
//      discarded and the rest are retained as content.
//      Otherwise (the following line is not empty), the line break is
//      converted to a single space (x20).
fn line_folding(string_to_fold: Vec<&str>) -> String {
    let mut ret = String::new();
    let mut iter = string_to_fold.into_iter().peekable();

    let mut first_line_break_trimmed = false;
    while let Some(line) = iter.next() {
        let trimmed = line.trim_matches(|c| matches!(c, ' ' | '\t'));
        let next_line_is_empty = if let Some(next_line) = iter.peek() {
            let trimmed_next_line =
                next_line.trim_matches(|c| matches!(c, ' ' | '\t'));
            Some(trimmed_next_line.is_empty())
        } else {
            None
        };
        if trimmed.is_empty() {
            if first_line_break_trimmed {
                ret.push('\n');
            }
        } else {
            ret.push_str(trimmed);
            first_line_break_trimmed = false;
            match next_line_is_empty {
                Some(true) => {
                    first_line_break_trimmed = true;
                }
                Some(false) => {
                    ret.push(' ');
                }
                None => (),
            }
        }
    }
    ret
}

// YAML 1.2.2: 6.5. Flow Folding
//      Folding in flow styles provides more relaxed semantics. Flow styles
//      typically depend on explicit indicators rather than indentation to
//      convey structure. Hence spaces preceding or following the text in a
//      line are a presentation detail and must not be used to convey content
//      information. Once all such spaces have been discarded, all line breaks
//      are folded without exception.
//      The combined effect of the flow line folding rules is that each
//      “paragraph” is interpreted as a line, empty lines are interpreted as
//      line feeds and text can be freely more-indented without affecting the
//      content information.
fn flow_folding(mut string_to_fold: String) -> String {
    // If first line is empty, since we have `"` at first line, we should not
    // consider first line as empty.
    // If last line is empty, since we have `"` at last line, we should not
    // consider last line as empty neither.
    // To simplify the processing, we manually add `"` to be start and end of
    // string. And then at the end, we purge those.
    string_to_fold.insert(0, '"');
    string_to_fold.push('"');
    let ret = line_folding(string_to_fold.split('\n').collect());
    // Remove leading and trialing `"` we manually added.
    ret[1..ret.len() - 1].to_string()
}

// Escaped ASCII null (x00) character.
const NS_ESC_NULL: char = '0';
// Escaped ASCII bell (x07) character.
const NS_ESC_BELL: char = '7';
// Escaped ASCII backspace (x08) character.
const NS_ESC_BACKSPACE: char = '8';
// Escaped ASCII horizontal tab (x09) character. This is useful at the start or
// the end of a line to force a leading or trailing tab to become part of the
// content.
const NS_ESC_HORIZONTAL_TAB: char = '9';
const NS_ESC_HORIZONTAL_TAB_2: char = 't';
// Escaped ASCII line feed (x0A) character.
const NS_ESC_LINE_FEED: char = 'n';
// Escaped ASCII vertical tab (x0B) character.
const NS_ESC_VERTICAL_TAB: char = 'v';
// Escaped ASCII form feed (x0C) character.
const NS_ESC_FORM_FEED: char = 'f';
// Escaped ASCII carriage return (x0D) character.
const NS_ESC_CARRIAGE_RETURN: char = 'r';
// Escaped ASCII escape (x1B) character.
const NS_ESC_ESCAPE: char = 'e';
// Escaped ASCII slash (x2F), for JSON compatibility.
const NS_ESC_SLASH: char = '/';
// Escaped ASCII back slash (x5C).
const NS_ESC_BACKSLASH: char = '\\';
// Escaped Unicode next line (x85) character.
const NS_ESC_NEXT_LINE: char = 'N';
// Escaped Unicode non_breaking space (xA0) character.
const NS_ESC_NON_BREAKING_SPACE: char = '_';
// Escaped Unicode line separator (x2028) character.
const NS_ESC_LINE_SEPARATOR: char = 'L';
// Escaped Unicode paragraph separator (x2029) character.
const NS_ESC_PARAGRAPH_SEPARATOR: char = 'P';
// Escaped 8_bit Unicode character. 2 chars.
const NS_ESC_8_BIT: char = 'x';
// Escaped 16_bit Unicode character. 4 chars.
const NS_ESC_16_BIT: char = 'u';
// Escaped 32_bit Unicode character. 8 chars.
const NS_ESC_32_BIT: char = 'U';

impl<'a> YamlParser<'a> {
    pub(crate) fn read_escaped_char(&mut self) -> Result<char, YamlError> {
        let c = if let Some(c) = self.scanner.next_char() {
            c
        } else {
            return Err(YamlError::new(
                ErrorKind::InvalidEscapeScalar,
                "No character after escape \\".to_string(),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        };

        let start_pos = self.scanner.done_pos;
        Ok(match c {
            NS_ESC_NULL => '\0',
            NS_ESC_BELL => '\u{07}',
            NS_ESC_BACKSPACE => '\u{08}',
            NS_ESC_HORIZONTAL_TAB | NS_ESC_HORIZONTAL_TAB_2 => '\t',
            NS_ESC_LINE_FEED => '\n',
            NS_ESC_VERTICAL_TAB => '\u{0b}',
            NS_ESC_FORM_FEED => '\u{0c}',
            NS_ESC_CARRIAGE_RETURN => '\u{0d}',
            NS_ESC_ESCAPE => '\u{1b}',
            NS_ESC_SLASH => '/',
            NS_ESC_BACKSLASH => '\\',
            NS_ESC_NEXT_LINE => '\u{85}',
            NS_ESC_NON_BREAKING_SPACE => '\u{a0}',
            NS_ESC_LINE_SEPARATOR => '\u{2028}',
            NS_ESC_PARAGRAPH_SEPARATOR => '\u{2029}',
            NS_ESC_8_BIT | NS_ESC_16_BIT | NS_ESC_32_BIT => {
                let expected_count: usize = match c {
                    NS_ESC_8_BIT => 2,
                    NS_ESC_16_BIT => 4,
                    NS_ESC_32_BIT => 8,
                    _ => unreachable!(),
                };
                let mut val = String::new();
                for _ in 0..expected_count {
                    if let Some(i) = self.scanner.next_char() {
                        val.push(i)
                    } else {
                        break;
                    }
                }
                if val.chars().count() == expected_count {
                    let val_u32 = u32::from_str_radix(val.as_str(), 16)
                        .map_err(|_| {
                            YamlError::new(
                                ErrorKind::InvalidEscapeScalar,
                                format!(
                                    "Escaped unicode \\x{} is not a valid \
                                     unsigned integer in hex",
                                    val
                                ),
                                start_pos,
                                self.scanner.done_pos,
                            )
                        })?;
                    char::from_u32(val_u32).ok_or(YamlError::new(
                        ErrorKind::InvalidEscapeScalar,
                        format!(
                            "Escaped unicode: \\x{} is not a valid unicode",
                            val
                        ),
                        start_pos,
                        self.scanner.done_pos,
                    ))?
                } else {
                    return Err(YamlError::new(
                        ErrorKind::InvalidEscapeScalar,
                        format!(
                            "Expecting {expected_count} characters after \
                             escape \\x, but got:{val}",
                        ),
                        start_pos,
                        self.scanner.done_pos,
                    ));
                }
            }
            _ => {
                return Err(YamlError::new(
                    ErrorKind::InvalidEscapeScalar,
                    format!("Not supported escape \\{c}"),
                    start_pos,
                    self.scanner.done_pos,
                ));
            }
        })
    }
}
