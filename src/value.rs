// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::{
    ErrorKind, YamlError, YamlParser, YamlPosition, YamlTag,
    YamlValueMap,
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
        let events = YamlParser::parse_to_events(input)?;
        Self::compose(events)
    }
}

impl YamlValue {
    pub fn as_char(&self) -> Result<char, YamlError> {
        if let YamlValueData::String(v) = &self.data {
            if v.len() == 1 {
                Ok(v.chars().next().unwrap())
            } else {
                Err(YamlError::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a char, but got multi-char string {v}"),
                    self.start,
                    self.end,
                ))
            }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a char, but got {}", &self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_str(&self) -> Result<&str, YamlError> {
        if let YamlValueData::String(v) = &self.data {
            Ok(v.as_str())
        } else if let YamlValueData::Tag(tag) = &self.data {
            // The `as_str()` is called to get tag name of enum instead of
            // content.
            Ok(tag.name.as_str())
        } else if self.data == YamlValueData::Null {
            Ok("")
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a string, but got {}", &self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_bool(&self) -> Result<bool, YamlError> {
        if let YamlValueData::String(s) = &self.data {
            match s.as_str() {
                "true" => Ok(true),
                "false" => Ok(false),
                _ => Err(YamlError::new(
                    ErrorKind::InvalidBool,
                    format!("Expecting bool (true or false), but got {s}"),
                    self.start,
                    self.end,
                )),
            }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a bool, but got {}", &self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn is_bool(&self) -> bool {
        self.as_bool().is_ok()
    }

    pub fn is_integer(&self) -> bool {
        if let YamlValueData::String(s) = &self.data {
            str_is_integer(s)
        } else {
            false
        }
    }

    pub fn is_signed_integer(&self) -> bool {
        if let YamlValueData::String(s) = &self.data {
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
        if let YamlValueData::String(s) = &self.data {
            if s.starts_with("0x") | s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting unsigned hexadecimal integer like \
                             0xfa, but got {s}"
                        ),
                        self.start,
                        self.end,
                    )
                })
            } else if s.starts_with("0o") | s.starts_with("0O") {
                u64::from_str_radix(&s[2..], 8).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting unsigned octal integer like 0o20, but \
                             got {s}"
                        ),
                        self.start,
                        self.end,
                    )
                })
            } else if s.starts_with("0b") | s.starts_with("0B") {
                u64::from_str_radix(&s[2..], 2).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting unsigned binary integer like 0b10, but \
                             got {s}"
                        ),
                        self.start,
                        self.end,
                    )
                })
            } else {
                u64::from_str(s.as_str()).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting unsigned integer like 87, but got {s}"
                        ),
                        self.start,
                        self.end,
                    )
                })
            }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", &self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_u32(&self) -> Result<u32, YamlError> {
        let num = self.as_u64()?;
        if num > u32::MAX as u64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow u32::MAX {}",
                    num,
                    u32::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as u32)
        }
    }

    pub fn as_u16(&self) -> Result<u16, YamlError> {
        let num = self.as_u64()?;
        if num > u16::MAX as u64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow u16::MAX {}",
                    num,
                    u16::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as u16)
        }
    }

    pub fn as_u8(&self) -> Result<u8, YamlError> {
        let num = self.as_u64()?;
        if num > u8::MAX as u64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow u8::MAX {}",
                    num,
                    u8::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as u8)
        }
    }

    pub fn as_i64(&self) -> Result<i64, YamlError> {
        if let YamlValueData::String(s) = &self.data {
            let original = s;
            let positive: bool = !s.starts_with("-");

            let s = s.as_str().strip_prefix("-").unwrap_or(s.as_str());

            let s = s.strip_prefix("+").unwrap_or(s);

            let number = if s.starts_with("0x") | s.starts_with("0X") {
                i64::from_str_radix(&s[2..], 16).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting signed hexadecimal integer like -0xfa, \
                             but got {original}"
                        ),
                        self.start,
                        self.end,
                    )
                })?
            } else if s.starts_with("0o") | s.starts_with("0O") {
                i64::from_str_radix(&s[2..], 8).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting signed octal integer like -0o20, but \
                             got {original}"
                        ),
                        self.start,
                        self.end,
                    )
                })?
            } else if s.starts_with("0b") | s.starts_with("0B") {
                i64::from_str_radix(&s[2..], 2).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting signed binary integer like -0b10, but \
                             got {original}"
                        ),
                        self.start,
                        self.end,
                    )
                })?
            } else {
                i64::from_str(s).map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting signed integer like -1298, but got \
                             {original}"
                        ),
                        self.start,
                        self.end,
                    )
                })?
            };
            if positive { Ok(number) } else { Ok(0 - number) }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", &self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_i32(&self) -> Result<i32, YamlError> {
        let num = self.as_i64()?;
        if num > i32::MAX as i64 || num < i32::MIN as i64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow i32 range [{}, {}]",
                    num,
                    i32::MIN,
                    i32::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as i32)
        }
    }

    pub fn as_i16(&self) -> Result<i16, YamlError> {
        let num = self.as_i64()?;
        if num > i16::MAX as i64 || num < i16::MIN as i64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow i16 range [{}, {}]",
                    num,
                    i16::MIN,
                    i16::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as i16)
        }
    }

    pub fn as_i8(&self) -> Result<i8, YamlError> {
        let num = self.as_i64()?;
        if num > i8::MAX as i64 || num < i8::MIN as i64 {
            Err(YamlError::new(
                ErrorKind::NumberOverflow,
                format!(
                    "Specified number {} overflow u8 range [{}, {}]",
                    num,
                    i8::MIN,
                    i8::MAX
                ),
                self.start,
                self.end,
            ))
        } else {
            Ok(num as i8)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum YamlValueData {
    #[default]
    Null,
    String(String),
    Array(Vec<YamlValue>),
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
