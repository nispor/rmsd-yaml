// SPDX-License-Identifier: Apache-2.0

use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    get_map, get_tag, RmsdDeserializer, RmsdError, RmsdPosition, TokensIter,
    YamlTokenData, YamlValue, YamlValueData,
};

// Should have leading [ and tailing ] removed.
// With `in_flow` set to true, we will read till end of iter without checking
// indents.
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
                    if !previous_element_tokens.is_empty() {
                        items.push(YamlValue::parse(&mut TokensIter::new(
                            std::mem::take(&mut previous_element_tokens),
                        ))?);
                    }
                    iter.next();
                }
            }
            YamlTokenData::FlowSequenceStart => {
                let mut element_iter =
                    TokensIter::new(iter.remove_tokens_of_seq_flow()?);
                items.push(get_array(&mut element_iter, true)?);
            }
            YamlTokenData::FlowMapStart => {
                let mut element_iter =
                    TokensIter::new(iter.remove_tokens_of_map_flow()?);
                items.push(get_map(&mut element_iter, true)?);
            }
            YamlTokenData::CollectEntry => {
                // We will have empty previous_element_tokens for `,` after `{}`
                // or `[]`.
                if !previous_element_tokens.is_empty() {
                    items.push(YamlValue::parse(&mut TokensIter::new(
                        std::mem::take(&mut previous_element_tokens),
                    ))?);
                }
                iter.next();
            }
            YamlTokenData::Scalar(_) => {
                previous_element_tokens.push(iter.next().unwrap());
            }
            YamlTokenData::MapValueIndicator => {
                previous_element_tokens.push(iter.next().unwrap());
            }
            YamlTokenData::LocalTag(_) => {
                items.push(get_tag(iter)?);
            }
            _ => {
                return Err(RmsdError::bug(
                    format!("get_array(): Unexpected token {token:?}"),
                    token.start,
                ));
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
