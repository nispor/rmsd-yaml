// SPDX-License-Identifier: Apache-2.0

use crate::{RmsdError, RmsdPosition, YamlToken, YamlTokenData};

#[derive(Debug, Clone)]
pub(crate) struct TokensIter {
    pub(crate) data: Vec<Option<YamlToken>>,
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

    /// Remove the follow up tokens with the same or more indent level
    /// as token got from `self.next()`.
    /// Return if we see a `- ` with the same indent level.
    pub(crate) fn remove_tokens_with_the_same_or_more_indent(
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
            if token.indent >= indent {
                ret.push(self.next().unwrap());
            } else {
                return ret;
            }
        }
        ret
    }

    // The first token should be `{`
    // Return tokens do not contains leading { and tailing }
    pub(crate) fn remove_tokens_of_map_flow(
        &mut self,
    ) -> Result<Vec<YamlToken>, RmsdError> {
        let mut ret = Vec::new();
        let mut need_ends = 0;
        let mut last_map_start_pos = RmsdPosition::EOF;

        while let Some(token) = self.next() {
            match token.data {
                YamlTokenData::FlowMapStart => {
                    last_map_start_pos = token.start;
                    if need_ends != 0 {
                        ret.push(token);
                    }
                    need_ends += 1;
                }
                YamlTokenData::FlowMapEnd => {
                    if need_ends == 0 {
                        return Err(RmsdError::unexpected_yaml_node_type(
                            "Got } without leading {".to_string(),
                            token.start,
                        ));
                    } else {
                        need_ends -= 1;
                        if need_ends == 0 {
                            return Ok(ret);
                        } else {
                            ret.push(token);
                        }
                    }
                }
                _ => {
                    ret.push(token);
                }
            }
        }

        if need_ends == 0 {
            Err(RmsdError::bug(
                format!(
                    "remove_tokens_of_map_block() invoked against token \
                    without map start indicator {{ {:?}",
                    self
                ),
                RmsdPosition::EOF,
            ))
        } else {
            Err(RmsdError::unfinished_map_indicator(last_map_start_pos))
        }
    }

    // The first token should be `[`,
    // Return tokens do not contains leading [ and tailing ]
    pub(crate) fn remove_tokens_of_seq_flow(
        &mut self,
    ) -> Result<Vec<YamlToken>, RmsdError> {
        let mut ret = Vec::new();
        let mut need_ends = 0;
        let mut last_seq_start_pos = RmsdPosition::EOF;
        while let Some(token) = self.next() {
            match token.data {
                YamlTokenData::FlowSequenceStart => {
                    last_seq_start_pos = token.start;
                    if need_ends != 0 {
                        ret.push(token);
                    }
                    need_ends += 1;
                }
                YamlTokenData::FlowSequenceEnd => {
                    if need_ends == 0 {
                        return Err(RmsdError::unexpected_yaml_node_type(
                            "Got ] without leading [".to_string(),
                            token.start,
                        ));
                    } else {
                        need_ends -= 1;
                        if need_ends == 0 {
                            return Ok(ret);
                        } else {
                            ret.push(token);
                        }
                    }
                }
                _ => {
                    ret.push(token);
                }
            }
        }

        if need_ends == 0 {
            Err(RmsdError::bug(
                format!(
                    "remove_tokens_of_seq_block() invoked against token \
                    without seq start indicator [ {:?}",
                    self
                ),
                RmsdPosition::EOF,
            ))
        } else {
            Err(RmsdError::unfinished_seq_indicator(last_seq_start_pos))
        }
    }
}
