// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::{
    RmsdError, RmsdPosition, TokensIter, YamlToken, YamlTokenData, YamlValueMap,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContainerType {
    Map,
    Array,
    Scalar,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct YamlValue {
    pub data: YamlValueData,
    pub start: RmsdPosition,
    pub end: RmsdPosition,
}

impl std::fmt::Display for YamlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

impl FromStr for YamlValue {
    type Err = RmsdError;

    fn from_str(input: &str) -> Result<Self, RmsdError> {
        let tokens = YamlToken::parse(input)?;
        // TODO: Process Tag, Directive and Anchor
        Self::try_from(tokens)
    }
}

impl YamlValue {
    pub fn as_char(&self) -> Result<char, RmsdError> {
        if let YamlValueData::Scalar(v) = &self.data {
            if v.len() == 1 {
                Ok(v.chars().next().unwrap())
            } else {
                Err(RmsdError::unexpected_yaml_node_type(
                    format!("Expecting a char, but got multi-char string {v}"),
                    self.start,
                ))
            }
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!("Expecting a char, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_str(&self) -> Result<&str, RmsdError> {
        if let YamlValueData::Scalar(v) = &self.data {
            Ok(v.as_str())
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!("Expecting a string, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_bool(&self) -> Result<bool, RmsdError> {
        if let YamlValueData::Scalar(s) = &self.data {
            match s.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(RmsdError::invalid_bool(s.as_str(), self.start)),
            }
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!("Expecting a bool, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_u64(&self) -> Result<u64, RmsdError> {
        if let YamlValueData::Scalar(s) = &self.data {
            if s.starts_with("0x") | s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16).map_err(|_| {
                    RmsdError::invalid_number(s.as_str(), self.start)
                })
            } else if s.starts_with("0o") | s.starts_with("0O") {
                u64::from_str_radix(&s[2..], 8).map_err(|_| {
                    RmsdError::invalid_number(s.as_str(), self.start)
                })
            } else if s.starts_with("0b") | s.starts_with("0b") {
                u64::from_str_radix(&s[2..], 2).map_err(|_| {
                    RmsdError::invalid_number(s.as_str(), self.start)
                })
            } else {
                u64::from_str(s.as_str()).map_err(|_| {
                    RmsdError::invalid_number(s.as_str(), self.start)
                })
            }
        } else {
            Err(RmsdError::unexpected_yaml_node_type(
                format!("Expecting a bool, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_u32(&self) -> Result<u32, RmsdError> {
        let num = self.as_u64()?;
        if num > u32::MAX as u64 {
            Err(RmsdError::number_overflow(
                format!(
                    "Specified number {} overflow u32:MAX {}",
                    num,
                    u32::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as u32)
        }
    }

    pub fn as_u16(&self) -> Result<u16, RmsdError> {
        let num = self.as_u64()?;
        if num > u16::MAX as u64 {
            Err(RmsdError::number_overflow(
                format!(
                    "Specified number {} overflow u16:MAX {}",
                    num,
                    u16::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as u16)
        }
    }

    pub fn as_u8(&self) -> Result<u8, RmsdError> {
        let num = self.as_u64()?;
        if num > u8::MAX as u64 {
            Err(RmsdError::number_overflow(
                format!("Specified number {} overflow u8:MAX {}", num, u8::MAX),
                self.start,
            ))
        } else {
            Ok(num as u8)
        }
    }
}

impl TryFrom<Vec<YamlToken>> for YamlValue {
    type Error = RmsdError;

    fn try_from(tokens: Vec<YamlToken>) -> Result<Self, Self::Error> {
        let mut tokens = tokens;
        let mut ret = Self {
            data: YamlValueData::Null,
            start: tokens
                .as_slice()
                .first()
                .map(|t| t.start)
                .unwrap_or(RmsdPosition::EOF),
            end: tokens
                .as_slice()
                .get(tokens.len() - 1)
                .map(|t| t.end)
                .unwrap_or(RmsdPosition::EOF),
        };

        // Determine the container type
        match get_container_type(&tokens) {
            ContainerType::Map => {
                let mut iter = TokensIter::new(tokens);
                ret.data = YamlValueData::Map(Box::new(get_map(&mut iter)?));
            }
            ContainerType::Array => {
                let mut iter = TokensIter::new(tokens);
                ret.data = YamlValueData::Sequence(get_array(&mut iter)?);
            }
            ContainerType::Scalar => {
                if tokens.is_empty() {
                } else {
                    ret.data = get_scalar(tokens.pop().unwrap())?;
                }
            }
            ContainerType::Null => (),
        }

        Ok(ret)
    }
}

fn get_container_type(tokens: &[YamlToken]) -> ContainerType {
    if let Some(first_token) = tokens.first() {
        if first_token.data == YamlTokenData::BlockSequenceIndicator
            || first_token.data == YamlTokenData::FlowSequenceStart
        {
            ContainerType::Array
        } else if first_token.data == YamlTokenData::FlowMapStart
            || first_token.data == YamlTokenData::MapKeyIndicator
            || tokens
                .get(1)
                .map(|t| t.data == YamlTokenData::MapValueIndicator)
                == Some(true)
        {
            ContainerType::Map
        } else {
            ContainerType::Scalar
        }
    } else {
        ContainerType::Null
    }
}

fn get_map(iter: &mut TokensIter) -> Result<YamlValueMap, RmsdError> {
    let mut ret = YamlValueMap::new();

    let indent = if let Some(first_token) = iter.peek() {
        first_token.indent
    } else {
        return Ok(ret);
    };

    let mut key: Option<YamlValue> = None;
    while let Some(token) = iter.peek() {
        if token.indent < indent {
            return Ok(ret);
        }

        match &token.data {
            YamlTokenData::Scalar(_) => {
                if let Some(k) = key.take() {
                    if token.indent == indent {
                        // The unwrap is safe here as the `peek()` already check
                        // it is not None.
                        let token = iter.next().unwrap();
                        if let YamlTokenData::Scalar(s) = token.data {
                            let value = YamlValue {
                                data: YamlValueData::Scalar(s),
                                start: token.start,
                                end: token.end,
                            };
                            ret.insert(k, value);
                        } else {
                            unreachable!()
                        }
                    } else {
                        // The value is nested
                        let nested_tokens =
                            iter.remove_tokens_with_the_same_indent();
                        ret.insert(k, YamlValue::try_from(nested_tokens)?);
                    }
                } else {
                    // The unwrap is safe here as the `peek()` already check
                    // it is not None.
                    let token = iter.next().unwrap();
                    if let YamlTokenData::Scalar(s) = token.data {
                        key = Some(YamlValue {
                            data: YamlValueData::Scalar(s),
                            start: token.start,
                            end: token.end,
                        });
                    } else {
                        unreachable!();
                    }
                }
            }
            YamlTokenData::MapValueIndicator => {
                if key.is_none() {
                    return Err(RmsdError::unexpected_yaml_node_type(
                        "Got map value indicator `:` with \
                        no key defined before"
                            .to_string(),
                        token.start,
                    ));
                }
                iter.next();
            }
            _ => todo!(),
        }
    }
    Ok(ret)
}

fn get_array(_iter: &mut TokensIter) -> Result<Vec<YamlValue>, RmsdError> {
    todo!()
}

fn get_scalar(token: YamlToken) -> Result<YamlValueData, RmsdError> {
    if let YamlTokenData::Scalar(s) = token.data {
        Ok(YamlValueData::Scalar(s))
    } else {
        Err(RmsdError::unexpected_yaml_node_type(
            format!("Expecting scalar but got {}", token.data),
            token.start,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct YamlTag {
    pub name: String,
    pub data: YamlValueData,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum YamlValueData {
    #[default]
    Null,
    Scalar(String),
    Sequence(Vec<YamlValue>),
    Map(Box<YamlValueMap>),
    // Tag(Box<YamlTag>),
}

impl std::fmt::Display for YamlValueData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}
