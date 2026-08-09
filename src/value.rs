// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use crate::{
    ErrorKind, YamlError, YamlParser, YamlPosition, YamlScalarStyle, YamlTag,
    YamlValueMap,
};

#[derive(Debug, Clone, Default)]
pub struct YamlValue {
    pub data: YamlValueData,
    pub start: YamlPosition,
    pub end: YamlPosition,
    /// Round-trip metadata from the parsed events (scalar style,
    /// anchor, alias). Preserved by [`YamlValue::to_string`] so a
    /// parsed document can be dumped byte-identically, but
    /// deliberately *not* part of [`PartialEq`]/[`Hash`]: two values
    /// with the same data are equal regardless of their style.
    pub meta: YamlValueMeta,
}

/// Round-trip metadata attached to a parsed [`YamlValue`].
#[derive(Debug, Clone, Default)]
pub struct YamlValueMeta {
    /// The scalar style of a `String` node (or of the scalar wrapped
    /// in a `Tag`). `None` for values built in code.
    pub scalar_style: Option<YamlScalarStyle>,
    /// The anchor declared on this node (`&name`).
    pub anchor: Option<String>,
    /// The alias this node was produced from (`*name`). The `data` is
    /// the resolved value; the dump renders `*name` instead.
    pub alias: Option<String>,
}

// `meta` is excluded from equality/hash so the value semantics of
// `YamlValue` (and of map keys) are unchanged by the round-trip info.
impl PartialEq for YamlValue {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.start == other.start
            && self.end == other.end
    }
}

impl Eq for YamlValue {}

impl std::hash::Hash for YamlValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.start.hash(state);
        self.end.hash(state);
    }
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
                format!("Expecting a char, but got {}", self.data),
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
                format!("Expecting a string, but got {}", self.data),
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
                format!("Expecting a bool, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn is_bool(&self) -> bool {
        self.as_bool().is_ok()
    }

    /// Whether the value resolves to null per the YAML Core Schema:
    /// `null`, `Null`, `NULL`, `~` or an empty scalar.
    pub fn is_null(&self) -> bool {
        if self.data == YamlValueData::Null {
            return true;
        }
        if let YamlValueData::String(s) = &self.data {
            return matches!(s.as_str(), "null" | "Null" | "NULL" | "~" | "");
        }
        false
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

    pub fn is_float(&self) -> bool {
        if let YamlValueData::String(s) = &self.data {
            str_is_float(s)
        } else {
            false
        }
    }

    pub fn as_f64(&self) -> Result<f64, YamlError> {
        if let YamlValueData::String(s) = &self.data {
            match s.as_str() {
                ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => {
                    Ok(f64::INFINITY)
                }
                "-.inf" | "-.Inf" | "-.INF" => Ok(f64::NEG_INFINITY),
                ".nan" | ".NaN" | ".NAN" => Ok(f64::NAN),
                _ => s.parse::<f64>().map_err(|_| {
                    YamlError::new(
                        ErrorKind::InvalidNumber,
                        format!("Expecting a float, but got {s}"),
                        self.start,
                        self.end,
                    )
                }),
            }
        } else {
            Err(YamlError::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", self.data),
                self.start,
                self.end,
            ))
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
                format!("Expecting a number, but got {}", self.data),
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
                format!("Expecting a number, but got {}", self.data),
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
    if s.is_empty() {
        return false;
    }
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

fn str_is_float(s: &str) -> bool {
    matches!(
        s,
        ".inf"
            | ".Inf"
            | ".INF"
            | "+.inf"
            | "+.Inf"
            | "+.INF"
            | "-.inf"
            | "-.Inf"
            | "-.INF"
            | ".nan"
            | ".NaN"
            | ".NAN"
    ) || s.parse::<f64>().is_ok()
}

/// Deserialize a parsed [`YamlValue`] from the `YamlDeserializer` used
/// by [`from_str`](crate::from_str). The parse result is already a
/// `YamlValue`; this visits it back into a fresh value tree.
///
/// Note: `YamlValueData::Tag` nodes are rebuilt from the variant name
/// derived by [`YamlValueEnumAccess`](crate::variant::YamlValueEnumAccess),
/// which loses the original tag URI (e.g. `<tag:yaml.org,2002:int>`
/// becomes `!int`). For a lossless parse use
/// [`to_value`](crate::to_value) instead.
impl<'de> serde::Deserialize<'de> for YamlValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(YamlValueVisitor)
    }
}

struct YamlValueVisitor;

impl<'de> serde::de::Visitor<'de> for YamlValueVisitor {
    type Value = YamlValue;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(YamlValue {
            data: YamlValueData::Null,
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(YamlValue {
            data: YamlValueData::Null,
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(YamlValue {
            data: YamlValueData::String(v.to_string()),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(YamlValue {
            data: YamlValueData::String(v.to_string()),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(YamlValue {
            data: YamlValueData::String(v),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v.to_string())
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v.to_string())
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v.to_string())
    }

    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&v.to_string())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut array = Vec::new();
        while let Some(item) = seq.next_element::<YamlValue>()? {
            array.push(item);
        }
        Ok(YamlValue {
            data: YamlValueData::Array(array),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut data = YamlValueMap::new();
        while let Some((key, value)) =
            map.next_entry::<YamlValue, YamlValue>()?
        {
            data.insert(key, value);
        }
        Ok(YamlValue {
            data: YamlValueData::Map(Box::new(data)),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::EnumAccess<'de>,
    {
        use serde::de::VariantAccess;
        let (name, variant) = data.variant_seed(YamlVariantName)?;
        let inner = variant.newtype_variant_seed(YamlValueVisitor)?;
        Ok(YamlValue {
            data: YamlValueData::Tag(Box::new(YamlTag {
                name: format!("!{name}"),
                data: inner.data,
            })),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }
}

struct YamlVariantName;

impl<'de> serde::de::DeserializeSeed<'de> for YamlValueVisitor {
    type Value = YamlValue;

    fn deserialize<D>(self, deserializer: D) -> Result<YamlValue, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for YamlVariantName {
    type Value = String;

    fn deserialize<D>(self, deserializer: D) -> Result<String, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NameVisitor;
        impl serde::de::Visitor<'_> for NameVisitor {
            type Value = String;
            fn expecting(
                &self,
                formatter: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                formatter.write_str("an enum variant name")
            }
            fn visit_str<E>(self, v: &str) -> Result<String, E> {
                Ok(v.to_string())
            }
        }
        deserializer.deserialize_str(NameVisitor)
    }
}
