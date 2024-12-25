// SPDX-License-Identifier: Apache-2.0

use crate::RmsdPosition;

#[derive(Debug, Clone)]
pub(crate) struct CharsIter<'s> {
    /// The position of last drained char.
    pos: RmsdPosition,
    /// The position of pending char.
    next_pos: RmsdPosition,
    iter: std::str::Chars<'s>,
}

impl<'s> CharsIter<'s> {
    pub(crate) fn new(value: &'s str) -> Self {
        Self {
            pos: RmsdPosition::EOF,
            next_pos: RmsdPosition::default(),
            iter: value.chars(),
        }
    }

    /// Return next char
    pub(crate) fn next(&mut self) -> Option<char> {
        let c = self.iter.next()?;

        self.pos = self.next_pos;
        match c {
            '\r' => self.next(),
            '\n' => {
                self.next_pos.next_line();
                Some(c)
            }
            _ => {
                self.next_pos.next_column();
                Some(c)
            }
        }
    }

    pub(crate) fn peek(&mut self) -> Option<char> {
        while let Some(c) = self.iter.as_str().chars().next() {
            if c != '\r' {
                return Some(c);
            }
        }
        None
    }

    /// The position of last drained char.
    pub(crate) fn pos(&self) -> RmsdPosition {
        self.pos
    }

    /// The position of pending char.
    pub(crate) fn next_pos(&self) -> RmsdPosition {
        self.next_pos
    }

    /// Discard white space ' ' or '\t'.
    pub(crate) fn dicard_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.next();
            } else {
                break;
            }
        }
    }
}
