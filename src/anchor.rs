// SPDX-License-Identifier: Apache-2.0

use crate::{Error, ErrorKind, YamlParser};

impl<'a> YamlParser<'a> {
    /// Parse a YAML anchor (the name after `&`). The scanner must stay at
    /// the `&` character.
    ///
    /// YAML 1.2.2 SPEC, 6.9.2. Node Anchors:
    ///     An anchored node need not be referenced by an alias. But an
    ///     alias cannot be anchored.
    pub(crate) fn handle_anchor(&mut self) -> Result<String, Error> {
        if self.scanner.next_char() != Some('&') {
            return Err(Error::new(
                ErrorKind::Bug,
                format!(
                    "handle_anchor() got a scanner not started with &: {:?}",
                    self.scanner.remains()
                ),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        let start_pos = self.scanner.done_pos;
        // The anchor/alias name ends at whitespace, a line break or a
        // flow indicator (`*b,` in `[a, *b, c]`).
        let mut name = String::new();
        while let Some(c) = self.scanner.peek_char() {
            if c.is_whitespace() || matches!(c, ',' | '[' | ']' | '{' | '}') {
                break;
            }
            name.push(c);
            self.scanner.next_char();
        }
        if name.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidAnchor,
                "Empty anchor name".to_string(),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        // YAML 1.2.2 SPEC, 6.9.2. Node Anchors:
        //      ns-anchor-char ::= ns-char - c-flow-indicator
        if let Some(c) = name
            .chars()
            .find(|c| matches!(c, ',' | '[' | ']' | '{' | '}'))
        {
            return Err(Error::new(
                ErrorKind::InvalidAnchor,
                format!("Invalid character '{c}' in anchor name"),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        let name = name.to_string();
        // Consume exactly one separating white-space character after
        // the name. A flow indicator (`[`, `]`, `{`, `}`, `,`) is not
        // separation: it terminates the node and must be left for the
        // caller (e.g. `[&a, b]` is an anchored empty node followed by
        // an entry `b`).
        let separated = match self.scanner.peek_char() {
            Some('\r') => {
                self.scanner.next_char();
                if self.scanner.peek_char() == Some('\n') {
                    self.scanner.next_char();
                }
                true
            }
            Some(' ') | Some('\t') | Some('\n') => {
                self.scanner.next_char();
                true
            }
            _ => false,
        };
        // Node properties must be separated from their content (YAML
        // 1.2.2 SPEC, production [161]). A flow sequence/mapping that
        // starts immediately after the anchor name is content, not an
        // empty node, so it is invalid without separation.
        if !separated
            && matches!(self.scanner.peek_char(), Some('[') | Some('{'))
        {
            return Err(Error::new(
                ErrorKind::InvalidAnchor,
                "Node properties must be separated from the node content"
                    .to_string(),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        if self.scanner.peek_char() == Some('*') {
            return Err(Error::new(
                ErrorKind::InvalidAnchor,
                "Anchor cannot be attached to an alias".to_string(),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        Ok(name)
    }

    /// Parse a YAML alias (the name after `*`). The scanner must stay at
    /// the `*` character.
    ///
    /// YAML 1.2.2 SPEC, 6.9.2. Node Anchors:
    ///     An alias node is denoted by an `*` indicator followed by the
    ///     anchor name.
    pub(crate) fn handle_alias(&mut self) -> Result<String, Error> {
        if self.scanner.next_char() != Some('*') {
            return Err(Error::new(
                ErrorKind::Bug,
                format!(
                    "handle_alias() got a scanner not started with *: {:?}",
                    self.scanner.remains()
                ),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        let start_pos = self.scanner.done_pos;
        // The anchor/alias name ends at whitespace, a line break or a
        // flow indicator (`*b,` in `[a, *b, c]`).
        let mut name = String::new();
        while let Some(c) = self.scanner.peek_char() {
            if c.is_whitespace() || matches!(c, ',' | '[' | ']' | '{' | '}') {
                break;
            }
            name.push(c);
            self.scanner.next_char();
        }
        if name.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidAlias,
                "Empty alias name".to_string(),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        // YAML 1.2.2 SPEC, 6.9.2. Node Anchors:
        //      ns-anchor-char ::= ns-char - c-flow-indicator
        if let Some(c) = name
            .chars()
            .find(|c| matches!(c, ',' | '[' | ']' | '{' | '}'))
        {
            return Err(Error::new(
                ErrorKind::InvalidAlias,
                format!("Invalid character '{c}' in alias name"),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        Ok(name)
    }
}
