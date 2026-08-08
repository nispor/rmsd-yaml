// SPDX-License-Identifier: Apache-2.0

use crate::{ErrorKind, YamlError, YamlParser};

impl<'a> YamlParser<'a> {
    /// Parse a YAML anchor (the name after `&`). The scanner must stay at
    /// the `&` character.
    ///
    /// YAML 1.2.2 SPEC, 6.9.2. Node Anchors:
    ///     An anchored node need not be referenced by an alias. But an
    ///     alias cannot be anchored.
    pub(crate) fn handle_anchor(&mut self) -> Result<String, YamlError> {
        if self.scanner.next_char() != Some('&') {
            return Err(YamlError::new(
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
        let name = self.scanner.peek_till_linebreak_or_space();
        if name.is_empty() {
            return Err(YamlError::new(
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
            return Err(YamlError::new(
                ErrorKind::InvalidAnchor,
                format!("Invalid character '{c}' in anchor name"),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        let name = name.to_string();
        self.scanner.advance_till_linebreak_or_space();
        if self.scanner.peek_char() == Some('*') {
            return Err(YamlError::new(
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
    pub(crate) fn handle_alias(&mut self) -> Result<String, YamlError> {
        if self.scanner.next_char() != Some('*') {
            return Err(YamlError::new(
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
        let name = self.scanner.peek_till_linebreak_or_space();
        if name.is_empty() {
            return Err(YamlError::new(
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
            return Err(YamlError::new(
                ErrorKind::InvalidAlias,
                format!("Invalid character '{c}' in alias name"),
                start_pos,
                self.scanner.next_pos,
            ));
        }
        let name = name.to_string();
        self.scanner.advance_till_linebreak_or_space();
        Ok(name)
    }
}
