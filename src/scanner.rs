// SPDX-License-Identifier: Apache-2.0

use std::str::CharIndices;

use crate::{ErrorKind, YamlError, YamlPosition};

#[derive(Debug)]
pub(crate) struct YamlScanner<'a> {
    // We are Peekable does not have `as_str()`, so we use  CharIndices
    iter: CharIndices<'a>,
    pub(crate) next_pos: YamlPosition,
    pub(crate) done_pos: YamlPosition,
}

impl<'a> YamlScanner<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self {
            iter: input.char_indices(),
            next_pos: if input.is_empty() {
                YamlPosition::EOF
            } else {
                YamlPosition::new(1, 1)
            },
            done_pos: if input.is_empty() {
                YamlPosition::EOF
            } else {
                YamlPosition::new(1, 0)
            },
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.iter.as_str().is_empty()
    }

    pub(crate) fn remains(&self) -> &'a str {
        self.iter.as_str()
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        self.iter.as_str().chars().next()
    }

    pub(crate) fn peek_till_linebreak_or_space(&self) -> &str {
        self.remains()
            .split(['\r', '\n', ' '])
            .next()
            .unwrap_or_default()
    }

    pub(crate) fn peek_till_linebreak(&self) -> &str {
        self.remains()
            .split(['\r', '\n'])
            .next()
            .unwrap_or_default()
    }

    /// Count leading spaces of the first non-empty line
    /// YAML SPEC 1.2, 8.1.1.1. Block Indentation Indicator
    ///     If no indentation indicator is given, then the content indentation
    ///     level is equal to the number of leading spaces on the first
    ///     non-empty line of the contents. If there is no non-empty line then
    ///     the content indentation level is equal to the number of spaces on
    ///     the longest line.
    pub(crate) fn count_block_identation(&self) -> usize {
        let mut max_indent = 0usize;
        // The rust str::lines() will not take standalone `\r` as new line, but
        // YAML spec says `\r` is new line. So we use split here.
        for line in self.remains().split(['\n', '\r']) {
            if line.chars().all(|c| c == ' ') {
                let cur_indent = line.chars().count();
                if max_indent < cur_indent {
                    max_indent = cur_indent;
                }
            } else {
                return line.chars().take_while(|c| c == &' ').count();
            }
        }
        max_indent
    }

    pub(crate) fn peek_line(&self) -> Option<&'a str> {
        if self.remains().is_empty() {
            None
        } else {
            Some(
                self.remains()
                    .split_once(['\n', '\r'])
                    .map(|(s, _)| s)
                    .unwrap_or(self.remains()),
            )
        }
    }

    pub(crate) fn next_line(&mut self) -> Option<&'a str> {
        let ret = self.peek_line();
        log::trace!("next line {:?}", ret);
        self.advance_till_linebreak();
        ret
    }

    /// Advance character counts
    pub(crate) fn advance(&mut self, count: usize) {
        for _ in 0..count {
            self.next_char();
        }
    }

    /// Advance byte counts
    pub(crate) fn advance_offset(&mut self, offset: usize) {
        let end_offset = self.iter.offset() + offset;
        if self.remains().len() > offset {
            while self.iter.offset() < end_offset {
                self.next_char();
            }
        }
    }

    pub(crate) fn advance_till_linebreak(&mut self) {
        self.advance(self.peek_till_linebreak().chars().count());
        self.next_char();
    }

    pub(crate) fn advance_till_linebreak_or_space(&mut self) {
        self.advance(self.peek_till_linebreak_or_space().chars().count());
        self.next_char();
    }

    pub(crate) fn advance_till_non_space(&mut self) {
        while let Some(next_char) = self.peek_char()
            && next_char == ' '
        {
            self.next_char();
        }
    }

    pub(crate) fn advance_if_starts_with(&mut self, prefix: &str) -> bool {
        if self.remains().starts_with(prefix) {
            for _ in 0..prefix.chars().count() {
                self.next_char();
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn next_char(&mut self) -> Option<char> {
        let c = self.iter.next()?.1;
        log::trace!("next char {:?}", c);
        // Windows use `\r\n` for single line break, so we should not increase
        // line number if found `\r` and next one is `\n`.
        if c == '\n' || (c == '\r' && self.peek_char() != Some('\n')) {
            self.done_pos = self.next_pos;
            self.next_pos.next_line();
        } else if !self.remains().is_empty() {
            self.done_pos = self.next_pos;
            self.next_pos.next_column();
        } else {
            self.done_pos = self.next_pos;
        }

        Some(c)
    }

    /// Consume comment or line break or both.
    /// Raise Error if not followed by comment or line break.
    pub(crate) fn expect_comment_or_line_break(
        &mut self,
    ) -> Result<(), YamlError> {
        while let Some(c) = self.next_char() {
            match c {
                '\r' | '\n' => {
                    break;
                }
                '#' => {
                    self.advance_till_linebreak();
                }
                ' ' => {
                    continue;
                }
                c => {
                    return Err(YamlError::new(
                        ErrorKind::ExpectingCommentOrLineBreak,
                        format!("Got {c}, but expecting comment or line break"),
                        self.done_pos,
                        self.done_pos,
                    ));
                }
            }
        }
        Ok(())
    }
}
