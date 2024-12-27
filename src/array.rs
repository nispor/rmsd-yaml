// SPDX-License-Identifier: Apache-2.0

use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    RmsdDeserializer, RmsdError, TokensIter, YamlToken, YamlTokenData,
    YamlValue,
};

pub(crate) fn get_array(
    iter: &mut TokensIter,
) -> Result<Vec<YamlValue>, RmsdError> {
    let mut ret: Vec<YamlValue> = Vec::new();

    let indent = if let Some(first_token) = iter.peek() {
        first_token.indent
    } else {
        return Ok(ret);
    };

    while let Some(token) = iter.peek() {
        if token.indent < indent {
            return Ok(ret);
        }
        if token.data == YamlTokenData::BlockSequenceIndicator {
            iter.next();
            let entry_tokens = get_seq_element_tokens(iter, indent);
            ret.push(YamlValue::try_from(entry_tokens)?);
        }
    }

    Ok(ret)
}

/// Take token until:
/// * indent miss-match
/// * reach another `-` with the same indent
fn get_seq_element_tokens(
    iter: &mut TokensIter,
    indent: usize,
) -> Vec<YamlToken> {
    let mut ret = Vec::new();
    while let Some(token) = iter.peek() {
        if token.indent < indent
            || token.data == YamlTokenData::BlockSequenceIndicator
        {
            return ret;
        } else {
            let token = iter.next().unwrap();
            ret.push(token);
        }
    }
    ret
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
