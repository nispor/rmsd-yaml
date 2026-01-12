// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::{
    get_array, get_map, get_tag, YamlError, YamlPosition, TokensIter,
    YamlToken, YamlTokenData, YamlValueMap,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct YamlValue {
    pub data: YamlValueData,
    pub start: YamlPosition,
    pub end: YamlPosition,
}

impl std::fmt::Display for YamlValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

impl FromStr for YamlValue {
    type Err = YamlError;

    fn from_str(input: &str) -> Result<Self, YamlError> {
        let trees = YamlTree::parse(input)?;
        let graph = YamlGrap::compose(trees)?;
        Self::parse(&mut iter)
    }
}

impl YamlValue {
    pub fn as_char(&self) -> Result<char, YamlError> {
        if let YamlValueData::Scalar(v) = &self.data {
            if v.len() == 1 {
                Ok(v.chars().next().unwrap())
            } else {
                Err(YamlError::unexpected_yaml_node_type(
                    format!("Expecting a char, but got multi-char string {v}"),
                    self.start,
                ))
            }
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a char, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_str(&self) -> Result<&str, YamlError> {
        if let YamlValueData::Scalar(v) = &self.data {
            Ok(v.as_str())
        } else if let YamlValueData::Tag(tag) = &self.data {
            // The `as_str()` is called to get tag name of enum instead of
            // content.
            Ok(tag.name.as_str())
        } else if self.data == YamlValueData::Null {
            Ok("")
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a string, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_bool(&self) -> Result<bool, YamlError> {
        if let YamlValueData::Scalar(s) = &self.data {
            match s.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(YamlError::invalid_bool(s.as_str(), self.start)),
            }
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a bool, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn is_bool(&self) -> bool {
        self.as_bool().is_ok()
    }

    pub fn is_integer(&self) -> bool {
        if let YamlValueData::Scalar(s) = &self.data {
            str_is_integer(s)
        } else {
            false
        }
    }

    pub fn is_signed_integer(&self) -> bool {
        if let YamlValueData::Scalar(s) = &self.data {
            if s.starts_with("-") || s.starts_with("+") {
                str_is_integer(&s[1..])
            } else {
                false
            }
        } else {
            false
        }
    }

    pub fn as_u64(&self) -> Result<u64, YamlError> {
        if let YamlValueData::Scalar(s) = &self.data {
            if s.starts_with("0x") | s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16).map_err(|_| {
                    YamlError::invalid_number(s.as_str(), self.start)
                })
            } else if s.starts_with("0o") | s.starts_with("0O") {
                u64::from_str_radix(&s[2..], 8).map_err(|_| {
                    YamlError::invalid_number(s.as_str(), self.start)
                })
            } else if s.starts_with("0b") | s.starts_with("0B") {
                u64::from_str_radix(&s[2..], 2).map_err(|_| {
                    YamlError::invalid_number(s.as_str(), self.start)
                })
            } else {
                u64::from_str(s.as_str()).map_err(|_| {
                    YamlError::invalid_number(s.as_str(), self.start)
                })
            }
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a number, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_u32(&self) -> Result<u32, YamlError> {
        let num = self.as_u64()?;
        if num > u32::MAX as u64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow u32::MAX {}",
                    num,
                    u32::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as u32)
        }
    }

    pub fn as_u16(&self) -> Result<u16, YamlError> {
        let num = self.as_u64()?;
        if num > u16::MAX as u64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow u16::MAX {}",
                    num,
                    u16::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as u16)
        }
    }

    pub fn as_u8(&self) -> Result<u8, YamlError> {
        let num = self.as_u64()?;
        if num > u8::MAX as u64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow u8::MAX {}",
                    num,
                    u8::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as u8)
        }
    }

    pub fn as_i64(&self) -> Result<i64, YamlError> {
        if let YamlValueData::Scalar(s) = &self.data {
            let positive: bool = !s.starts_with("-");

            let s = s.as_str().strip_prefix("-").unwrap_or(s.as_str());

            let s = s.strip_prefix("+").unwrap_or(s);

            let number = if s.starts_with("0x") | s.starts_with("0X") {
                i64::from_str_radix(&s[2..], 16)
                    .map_err(|_| YamlError::invalid_number(s, self.start))?
            } else if s.starts_with("0o") | s.starts_with("0O") {
                i64::from_str_radix(&s[2..], 8)
                    .map_err(|_| YamlError::invalid_number(s, self.start))?
            } else if s.starts_with("0b") | s.starts_with("0B") {
                i64::from_str_radix(&s[2..], 2)
                    .map_err(|_| YamlError::invalid_number(s, self.start))?
            } else {
                i64::from_str(s)
                    .map_err(|_| YamlError::invalid_number(s, self.start))?
            };
            if positive {
                Ok(number)
            } else {
                Ok(0 - number)
            }
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting a number, but got {}", &self.data),
                self.start,
            ))
        }
    }

    pub fn as_i32(&self) -> Result<i32, YamlError> {
        let num = self.as_i64()?;
        if num > i32::MAX as i64 || num < i32::MIN as i64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow i32 range [{}, {}]",
                    num,
                    i32::MIN,
                    i32::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as i32)
        }
    }

    pub fn as_i16(&self) -> Result<i16, YamlError> {
        let num = self.as_i64()?;
        if num > i16::MAX as i64 || num < i16::MIN as i64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow i16 range [{}, {}]",
                    num,
                    i16::MIN,
                    i16::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as i16)
        }
    }

    pub fn as_i8(&self) -> Result<i8, YamlError> {
        let num = self.as_i64()?;
        if num > i8::MAX as i64 || num < i8::MIN as i64 {
            Err(YamlError::number_overflow(
                format!(
                    "Specified number {} overflow u8 range [{}, {}]",
                    num,
                    i8::MIN,
                    i8::MAX
                ),
                self.start,
            ))
        } else {
            Ok(num as i8)
        }
    }
}

impl YamlValue {
    pub(crate) fn construct(graph: YamlGrap) -> Result<Self, YamlError> {
    }

    pub(crate) fn parse(iter: &mut TokensIter) -> Result<Self, YamlError> {
        let ret = if let Some(token) = iter.peek() {
            match token.data {
                YamlTokenData::FlowSequenceStart => {
                    let start = token.start;
                    let tokens = iter.remove_tokens_of_seq_flow(false)?;
                    if tokens.is_empty() {
                        return Ok(Self {
                            start,
                            end: iter.end,
                            data: YamlValueData::Sequence(Vec::new()),
                        });
                    } else {
                        let mut sub_iter = TokensIter::new(tokens);
                        get_array(&mut sub_iter, true)
                    }
                }
                YamlTokenData::BlockSequenceIndicator => get_array(iter, false),
                YamlTokenData::FlowMapStart => {
                    let start = token.start;
                    let tokens = iter.remove_tokens_of_map_flow(false)?;
                    if tokens.is_empty() {
                        return Ok(Self {
                            start,
                            end: iter.end,
                            data: YamlValueData::Map(Box::new(
                                YamlValueMap::new(),
                            )),
                        });
                    } else {
                        let mut iter = TokensIter::new(tokens);
                        get_map(&mut iter, true)
                    }
                }
                YamlTokenData::MapKeyIndicator => get_map(iter, false),
                YamlTokenData::LocalTag(_) => get_tag(iter),
                YamlTokenData::Null => Ok(YamlValue {
                    start: token.start,
                    end: token.end,
                    data: YamlValueData::Null,
                }),
                _ => {
                    if iter.data.get(1).and_then(|t| {
                        t.as_ref()
                            .map(|t| t.data == YamlTokenData::MapValueIndicator)
                    }) == Some(true)
                    {
                        get_map(iter, false)
                    } else {
                        get_scalar(iter)
                    }
                }
            }
        } else {
            Ok(Self {
                start: YamlPosition::EOF,
                end: YamlPosition::EOF,
                data: YamlValueData::Null,
            })
        };
        ret
    }
}

fn get_scalar(iter: &mut TokensIter) -> Result<YamlValue, YamlError> {
    if let Some(token) = iter.next() {
        if let YamlTokenData::Scalar(s) = token.data {
            Ok(YamlValue {
                start: token.start,
                end: token.end,
                data: YamlValueData::Scalar(s),
            })
        } else {
            Err(YamlError::unexpected_yaml_node_type(
                format!("Expecting scalar, but got {}", token.data),
                token.start,
            ))
        }
    } else {
        Ok(YamlValue {
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            data: YamlValueData::Null,
        })
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
    Tag(Box<YamlTag>),
}

impl std::fmt::Display for YamlValueData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

fn str_is_integer(s: &str) -> bool {
    if s.starts_with("0x") | s.starts_with("0X") {
        s[2..].chars().all(|c| c.is_ascii_hexdigit())
    } else if s.starts_with("0o") | s.starts_with("0O") {
        s[2..].chars().all(|c| c.is_digit(8))
    } else if s.starts_with("0b") | s.starts_with("0B") {
        s[2..].chars().all(|c| c.is_digit(2))
    } else {
        s.chars().all(|c| c.is_ascii_digit())
    }
}
