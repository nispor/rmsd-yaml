// SPDX-License-Identifier: Apache-2.0

use crate::YamlToken;

#[derive(Debug, Clone)]
pub(crate) struct TokensIter {
    data: Vec<Option<YamlToken>>,
    /// The index of data pending removal
    index: usize,
}

impl TokensIter {
    pub(crate) fn new(tokens: Vec<YamlToken>) -> Self {
        let data: Vec<Option<YamlToken>> =
            tokens.into_iter().map(Some).collect();
        Self { data, index: 0 }
    }

    /// Remove the next token
    pub(crate) fn next(&mut self) -> Option<YamlToken> {
        if self.index >= self.data.len() {
            return None;
        }
        if let Some(token) = self.data.get_mut(self.index) {
            self.index += 1;

            token.take()
        } else {
            None
        }
    }

    /// Get reference to next YamlToken without removing it.
    /// Be careful on dead loop when using this function.
    pub(crate) fn peek(&mut self) -> Option<&YamlToken> {
        if let Some(Some(token)) = self.data.get(self.index) {
            Some(token)
        } else {
            None
        }
    }

    /// Remove the follow up tokens with the same indent level as next one.
    pub(crate) fn remove_tokens_with_the_same_indent(
        &mut self,
    ) -> Vec<YamlToken> {
        let mut ret = Vec::new();
        let indent;
        if let Some(first_token) = self.next() {
            indent = first_token.indent;
            ret.push(first_token);
        } else {
            return ret;
        }

        while let Some(token) = self.peek() {
            if token.indent == indent {
                ret.push(self.next().unwrap());
            } else {
                return ret;
            }
        }
        ret
    }
}
