// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct YamlChar(pub(crate) char);

impl YamlChar {
    pub(crate) fn is_line_break(&self) -> bool {
        matches!(self.0, '\r' | '\n')
    }

    pub(crate) fn is_comment(&self) -> bool {
        self.0 == '#'
    }

    pub(crate) fn is_indent(&self) -> bool {
        self.0 == ' '
    }
}

impl AsRef<char> for YamlChar {
    fn as_ref(&self) -> &char {
        &self.0
    }
}

impl From<char> for YamlChar {
    fn from(c: char) -> Self {
        Self(c)
    }
}
