// SPDX-License-Identifier: Apache-2.0

use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    RmsdDeserializer, RmsdError, RmsdPosition, TokensIter, YamlTokenData,
    YamlValue, YamlValueData,
};

// Should have leading [ and tailing ] removed.
// With `in_flow` set to true, we will read till end of iter without checking
// indents.
// Because we don't the start and ends when iter is empty, this function will
// raise error if so, please handle it before calling this function.
pub(crate) fn get_array(
    iter: &mut TokensIter,
    in_flow: bool,
) -> Result<YamlValue, RmsdError> {
    let mut items = Vec::new();
    let (start, mut end, indent) = if let Some(first_token) = iter.peek() {
        (first_token.start, first_token.end, first_token.indent)
    } else {
        return Err(RmsdError::bug(
            "get_array(): Got empty tokens".to_string(),
            RmsdPosition::EOF,
        ));
    };

    let mut previous_element_tokens = Vec::new();

    while let Some(token) = iter.peek() {
        if (!in_flow) && token.indent < indent {
            break;
        }
        match token.data {
            YamlTokenData::BlockSequenceIndicator => {
                if in_flow {
                    return Err(RmsdError::unexpected_yaml_node_type(
                        "Cannot place - right after [".to_string(),
                        token.start,
                    ));
                } else {
                    let token = iter.next().unwrap();
                    if token.indent > indent {
                        previous_element_tokens.push(token);
                    } else if !previous_element_tokens.is_empty() {
                        items.push(YamlValue::parse(&mut TokensIter::new(
                            std::mem::take(&mut previous_element_tokens),
                        ))?);
                    }
                }
            }
            YamlTokenData::FlowSequenceStart => {
                let mut element_tokens =
                    iter.remove_tokens_of_seq_flow(true)?;
                previous_element_tokens.append(&mut element_tokens);
            }
            YamlTokenData::FlowMapStart => {
                let mut element_tokens =
                    iter.remove_tokens_of_map_flow(true)?;
                previous_element_tokens.append(&mut element_tokens);
            }
            YamlTokenData::CollectEntry => {
                // We will have empty previous_element_tokens for `,` after `{}`
                // or `[]`.
                if in_flow {
                    if !previous_element_tokens.is_empty() {
                        items.push(YamlValue::parse(&mut TokensIter::new(
                            std::mem::take(&mut previous_element_tokens),
                        ))?);
                    }
                } else {
                    return Err(RmsdError::unexpected_yaml_node_type(
                        "Cannot use `,` in sequence/array without [ ]"
                            .to_string(),
                        token.start,
                    ));
                }
                iter.next();
            }
            _ => {
                previous_element_tokens.push(iter.next().unwrap());
            }
        }
    }
    if !previous_element_tokens.is_empty() {
        items.push(YamlValue::parse(&mut TokensIter::new(
            previous_element_tokens,
        ))?);
    }
    if !items.is_empty() {
        end = items[items.len() - 1].end
    }
    Ok(YamlValue {
        start,
        end,
        data: YamlValueData::Sequence(items),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlValueSeqAccess {
    data: Vec<YamlValue>,
}

impl YamlValueSeqAccess {
    pub(crate) fn new(data: Vec<YamlValue>) -> Self {
        // The Vec::pop() is much quicker than Vec::remove(0), so we
        // reverse it.
        let mut data = data;
        data.reverse();
        Self { data }
    }
}

impl<'de> SeqAccess<'de> for YamlValueSeqAccess {
    type Error = RmsdError;

    fn next_element_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        if let Some(value) = self.data.pop() {
            seed.deserialize(&mut RmsdDeserializer { parsed: value })
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.data.len())
    }
}
