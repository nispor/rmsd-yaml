// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::RmsdError;

/// Position of character
/// Both line and column are starting from 1, e.g. First character is
/// line 1 column 1. If first line is empty line, line 1 column 0 is for
/// null of this line.
/// Default to first character of first line: line 1 column 1.
/// The line 0 and column 0 means End of file [RmsdPosition::EOF]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct RmsdPosition {
    /// Line number, start from 1.
    pub line: usize,
    /// Column number, start from 1.
    pub column: usize,
}

impl Default for RmsdPosition {
    fn default() -> Self {
        Self { line: 1, column: 1 }
    }
}

impl std::fmt::Display for RmsdPosition {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        if self == &Self::EOF {
            write!(f, "EOF")
        } else {
            write!(f, "line {} column {}", self.line, self.column)
        }
    }
}

impl TryFrom<&str> for RmsdPosition {
    type Error = RmsdError;

    fn try_from(value: &str) -> Result<Self, RmsdError> {
        let err_msg = format!(
            "Expecting format `line [0-9]+ column [0-9]+`, \
            but got: {value}"
        );
        let splited: Vec<&str> = value.split(" ").take(4).collect();

        if splited.len() != 4 || splited[0] != "line" || splited[2] != "column"
        {
            return Err(RmsdError::invalid_pos(err_msg.as_str()));
        }

        let line = usize::from_str(splited[1])
            .map_err(|_| RmsdError::invalid_pos(err_msg.as_str()))?;

        let column = usize::from_str(splited[3])
            .map_err(|_| RmsdError::invalid_pos(err_msg.as_str()))?;

        Ok(Self { line, column })
    }
}

impl RmsdPosition {
    pub const EOF: Self = Self { line: 0, column: 0 };

    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    pub fn next_column(&mut self) {
        self.column += 1;
    }

    pub fn next_line(&mut self) {
        self.line += 1;
        self.column = 1;
    }

    /// Increase self by line and column count of specified string
    pub fn add_by_str(&mut self, value: &str) {
        let lines: Vec<&str> = value.lines().collect();

        if lines.len() > 1 {
            self.line += lines.len() - 1;
            self.column = lines[lines.len() - 1].len();
        } else {
            self.column += value.len();
        }
    }
}
