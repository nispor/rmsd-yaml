// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use serde::ser::{SerializeMap, SerializeSeq, Serializer};

use crate::{
    Error, ErrorKind, Mapping, YamlParseOption, YamlParser, YamlPosition,
    YamlScalarStyle, YamlTag,
};

#[derive(Debug, Clone, Default)]
pub struct Value {
    pub data: ValueData,
    pub start: YamlPosition,
    pub end: YamlPosition,
    /// Round-trip metadata from the parsed events (scalar style,
    /// anchor, alias). Preserved by [`Value::to_string`] so a
    /// parsed document can be dumped byte-identically, but
    /// deliberately *not* part of [`PartialEq`]/[`Hash`]: two values
    /// with the same data are equal regardless of their style.
    pub meta: ValueMeta,
}

/// Round-trip metadata attached to a parsed [`Value`].
#[derive(Debug, Clone, Default)]
pub struct ValueMeta {
    /// The scalar style of a `String` node (or of the scalar wrapped
    /// in a `Tag`). `None` for values built in code.
    pub scalar_style: Option<YamlScalarStyle>,
    /// The anchor declared on this node (`&name`).
    pub anchor: Option<String>,
    /// The alias this node was produced from (`*name`). The `data` is
    /// the resolved value; the dump renders `*name` instead.
    pub alias: Option<String>,
    /// The document started with an explicit `---` marker.
    pub doc_explicit: bool,
    /// The document ended with an explicit `...` marker.
    pub doc_end_explicit: bool,
}

// `meta.scalar_style` is excluded from equality/hash so the value
// semantics of `Value` (and of map keys) are unchanged by the
// presentation style. The anchor/alias are part of the node identity
// (an `&a x` key and a `*a` key are different nodes and must both be
// preserved by the dump).
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        // Two nodes are equal when their data is equal and their
        // round-trip metadata (anchor/alias) matches. Positions and
        // scalar style do not take part, so e.g. a single-quoted and a
        // plain scalar with the same text compare equal. Anchors and
        // aliases take part so that an anchored node and a distinct
        // alias reference to it stay separate map keys, preserving
        // byte-identical round-trip (yaml-test-suite:
        // aliases-in-flow-objects).
        self.data == other.data
            && self.meta.anchor == other.meta.anchor
            && self.meta.alias == other.meta.alias
    }
}

impl Eq for Value {}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.data.hash(state);
        self.meta.anchor.hash(state);
        self.meta.alias.hash(state);
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

impl FromStr for Value {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Error> {
        Self::from_str_with_opt(input, &YamlParseOption::default())
    }
}

impl Value {
    /// Like [`Value::from_str`](FromStr::from_str), but with
    /// configurable resource limits instead of the defaults. A
    /// separate associated function since `FromStr::from_str` cannot
    /// take extra parameters.
    pub fn from_str_with_opt(
        input: &str,
        option: &YamlParseOption,
    ) -> Result<Self, Error> {
        let events = YamlParser::parse_to_events_with_max_depth(
            input,
            option.max_depth,
        )?;
        Self::compose_with_limits(events, option.max_depth, option.max_nodes)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self {
            data: ValueData::String(s.to_string()),
            ..Default::default()
        }
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self {
            data: ValueData::String(s),
            ..Default::default()
        }
    }
}

impl Value {
    pub fn as_char(&self) -> Result<char, Error> {
        if let ValueData::String(v) = &self.data {
            // A null / bool / number scalar must not be coerced to its
            // single-character text (`~` -> '~', `0` -> '0'), keeping
            // type-safety consistent with the other `as_*` helpers.
            if self.is_null()
                || self.is_bool()
                || self.is_signed_integer()
                || self.is_integer()
                || self.is_float()
            {
                return Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a char, but got {}", self.data),
                    self.start,
                    self.end,
                ));
            }
            if v.len() == 1 {
                Ok(v.chars().next().unwrap())
            } else {
                Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a char, but got multi-char string {v}"),
                    self.start,
                    self.end,
                ))
            }
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a char, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_str(&self) -> Result<&str, Error> {
        if let ValueData::String(v) = &self.data {
            Ok(v.as_str())
        } else if let ValueData::Tag(tag) = &self.data {
            // A tag is metadata (consumed separately by
            // `ValueEnumAccess` for enum deserialization); accessing
            // the value as a string resolves through it to the
            // underlying scalar content, matching `serde_yaml`.
            if let ValueData::String(v) = &tag.data {
                Ok(v.as_str())
            } else {
                Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a string, but got {}", tag.data),
                    self.start,
                    self.end,
                ))
            }
        } else if self.data == ValueData::Null {
            Ok("")
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a string, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    /// Access a value inside a mapping by string key, mirroring
    /// `serde_yaml::Value::get()`. Returns `None` when the value is
    /// not a mapping or the key is absent.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match &self.data {
            ValueData::Map(map) => map.get(&Value::from(key)),
            _ => None,
        }
    }

    /// The value as a sequence, mirroring
    /// `serde_yaml::Value::as_sequence()`.
    pub fn as_sequence(&self) -> Option<&Vec<Value>> {
        match &self.data {
            ValueData::Array(values) => Some(values),
            _ => None,
        }
    }

    /// The value as a mapping, mirroring
    /// `serde_yaml::Value::as_mapping()`.
    pub fn as_mapping(&self) -> Option<&Mapping> {
        match &self.data {
            ValueData::Map(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Result<bool, Error> {
        if let ValueData::String(s) = &self.data {
            if !self.is_plain_scalar() {
                return Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a bool, but got string {s:?}"),
                    self.start,
                    self.end,
                ));
            }
            match s.as_str() {
                "true" | "True" | "TRUE" => Ok(true),
                "false" | "False" | "FALSE" => Ok(false),
                _ => Err(Error::new(
                    ErrorKind::InvalidBool,
                    format!("Expecting bool (true or false), but got {s}"),
                    self.start,
                    self.end,
                )),
            }
        } else {
            Err(Error::new(
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

    /// Whether this value is a *plain* (unquoted) scalar. Only plain
    /// scalars take part in YAML Core Schema implicit type resolution
    /// (null, booleans, numbers, YAML 1.2.2 SPEC, 10.3): an explicitly
    /// quoted scalar such as `"true"`, `"42"` or `"null"` is always an
    /// ordinary string, matching `serde_yaml`.
    fn is_plain_scalar(&self) -> bool {
        matches!(self.data, ValueData::String(_))
            && matches!(
                self.meta.scalar_style,
                None | Some(YamlScalarStyle::Plain)
            )
    }

    /// Whether the value resolves to null per the YAML Core Schema:
    /// `null`, `Null`, `NULL`, `~` or an empty scalar.
    pub fn is_null(&self) -> bool {
        if self.data == ValueData::Null {
            return true;
        }
        if let ValueData::String(s) = &self.data {
            // Only a *plain* scalar can be null (YAML 1.2.2 SPEC,
            // 7.3.1, tag resolution): an explicitly quoted scalar such
            // as `"null"`, `"~"` or `""` is an ordinary string
            // (serde_yaml deserializes it into `Some(...)`).
            if !self.is_plain_scalar() {
                return false;
            }
            return matches!(s.as_str(), "null" | "Null" | "NULL" | "~" | "");
        }
        false
    }

    /// The serde `Unexpected` describing what this value actually is,
    /// mirroring `serde_yaml`'s `invalid_type` error reporting, e.g.
    /// `Unexpected::Signed(-20)` for the scalar `-20`.
    pub(crate) fn unexpected<'a>(&'a self) -> serde::de::Unexpected<'a> {
        if self.is_null() {
            return serde::de::Unexpected::Unit;
        }
        if self.is_bool() {
            return serde::de::Unexpected::Bool(
                self.as_bool().unwrap_or(false),
            );
        }
        if self.is_signed_integer() {
            return serde::de::Unexpected::Signed(self.as_i64().unwrap_or(0));
        }
        if self.is_integer() {
            return serde::de::Unexpected::Unsigned(self.as_u64().unwrap_or(0));
        }
        if self.is_float() {
            return serde::de::Unexpected::Float(self.as_f64().unwrap_or(0.0));
        }
        let unexpected: serde::de::Unexpected<'a> = match &self.data {
            ValueData::String(s) => serde::de::Unexpected::Str(s),
            ValueData::Array(_) => serde::de::Unexpected::Seq,
            ValueData::Map(_) => serde::de::Unexpected::Map,
            _ => serde::de::Unexpected::Other("tagged value"),
        };
        unexpected
    }

    pub fn is_integer(&self) -> bool {
        if let ValueData::String(s) = &self.data {
            self.is_plain_scalar() && str_is_integer(s)
        } else {
            false
        }
    }

    pub fn is_signed_integer(&self) -> bool {
        if let ValueData::String(s) = &self.data {
            if !self.is_plain_scalar() {
                return false;
            }
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
        if let ValueData::String(s) = &self.data {
            self.is_plain_scalar() && str_is_float(s)
        } else {
            false
        }
    }

    pub fn as_f64(&self) -> Result<f64, Error> {
        if let ValueData::String(s) = &self.data {
            if !self.is_plain_scalar() {
                return Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a number, but got string {s:?}"),
                    self.start,
                    self.end,
                ));
            }
            match s.as_str() {
                ".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF" => {
                    Ok(f64::INFINITY)
                }
                "-.inf" | "-.Inf" | "-.INF" => Ok(f64::NEG_INFINITY),
                ".nan" | ".NaN" | ".NAN" => Ok(f64::NAN),
                _ => {
                    if is_rust_float_word(s) {
                        // `inf`, `nan`, `infinity` (any casing/sign) are
                        // not valid YAML 1.2 floats, only the dot forms
                        // are (already handled above). Keep them as
                        // strings.
                        return Err(Error::new(
                            ErrorKind::UnexpectedYamlNodeType,
                            format!("Expecting a float, but got {s:?}"),
                            self.start,
                            self.end,
                        ));
                    }
                    s.parse::<f64>().map_err(|_| {
                        Error::new(
                            ErrorKind::InvalidNumber,
                            format!("Expecting a float, but got {s}"),
                            self.start,
                            self.end,
                        )
                    })
                }
            }
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_u64(&self) -> Result<u64, Error> {
        if let ValueData::String(s) = &self.data {
            if !self.is_plain_scalar() {
                return Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a number, but got string {s:?}"),
                    self.start,
                    self.end,
                ));
            }
            if s.starts_with("0x") | s.starts_with("0X") {
                u64::from_str_radix(&s[2..], 16).map_err(|_| {
                    Error::new(
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
                    Error::new(
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
                    Error::new(
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
                    Error::new(
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
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_u32(&self) -> Result<u32, Error> {
        let num = self.as_u64()?;
        if num > u32::MAX as u64 {
            Err(Error::new(
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

    pub fn as_u16(&self) -> Result<u16, Error> {
        let num = self.as_u64()?;
        if num > u16::MAX as u64 {
            Err(Error::new(
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

    pub fn as_u8(&self) -> Result<u8, Error> {
        let num = self.as_u64()?;
        if num > u8::MAX as u64 {
            Err(Error::new(
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

    pub fn as_i64(&self) -> Result<i64, Error> {
        if let ValueData::String(s) = &self.data {
            if !self.is_plain_scalar() {
                return Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!("Expecting a number, but got string {s:?}"),
                    self.start,
                    self.end,
                ));
            }
            let original = s.as_str();
            let positive = !original.starts_with('-');
            let unsigned =
                original.strip_prefix(['+', '-']).unwrap_or(original);

            let (radix, digits) = if unsigned.starts_with("0x")
                || unsigned.starts_with("0X")
            {
                (16, &unsigned[2..])
            } else if unsigned.starts_with("0o") || unsigned.starts_with("0O") {
                (8, &unsigned[2..])
            } else if unsigned.starts_with("0b") || unsigned.starts_with("0B") {
                (2, &unsigned[2..])
            } else {
                (10, unsigned)
            };

            // Parse the magnitude into an `i128` first instead of an
            // `i64`: `i64::MIN` has a magnitude (`2^63`) that does not
            // fit in an `i64`, so magnitude-then-negate would reject it.
            // The sign is applied and the result is range-checked in
            // `i128` space.
            let mag: i128 =
                i128::from_str_radix(digits, radix).map_err(|_| {
                    Error::new(
                        ErrorKind::InvalidNumber,
                        format!(
                            "Expecting signed integer like -1298, but got \
                             {original}"
                        ),
                        self.start,
                        self.end,
                    )
                })?;
            let value = if positive { mag } else { -mag };
            if value < i64::MIN as i128 || value > i64::MAX as i128 {
                return Err(Error::new(
                    ErrorKind::InvalidNumber,
                    format!(
                        "Expecting signed integer like -1298, but got \
                         {original}"
                    ),
                    self.start,
                    self.end,
                ));
            }
            Ok(value as i64)
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a number, but got {}", self.data),
                self.start,
                self.end,
            ))
        }
    }

    pub fn as_i32(&self) -> Result<i32, Error> {
        let num = self.as_i64()?;
        if num > i32::MAX as i64 || num < i32::MIN as i64 {
            Err(Error::new(
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

    pub fn as_i16(&self) -> Result<i16, Error> {
        let num = self.as_i64()?;
        if num > i16::MAX as i64 || num < i16::MIN as i64 {
            Err(Error::new(
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

    pub fn as_i8(&self) -> Result<i8, Error> {
        let num = self.as_i64()?;
        if num > i8::MAX as i64 || num < i8::MIN as i64 {
            Err(Error::new(
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
pub enum ValueData {
    #[default]
    Null,
    String(String),
    Array(Vec<Value>),
    Map(Box<Mapping>),
    Tag(Box<YamlTag>),
}

impl std::fmt::Display for ValueData {
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
    ) || !is_rust_float_word(s) && s.parse::<f64>().is_ok()
}

/// Whether `s`, after an optional sign, is one of the bare words that
/// Rust's `f64::FromStr` accepts (`inf`, `infinity` and `nan` in any
/// casing) but that the YAML 1.2 Core Schema does **not** resolve as
/// floats (only the dot forms `.inf`, `+.inf`, `-.inf` and `.nan` are;
/// YAML 1.2.2 SPEC, 10.3.2). Such scalars must stay ordinary strings
/// rather than leaking Rust's `f64::parse` leniency.
fn is_rust_float_word(s: &str) -> bool {
    let t = s.trim_start_matches(['+', '-']);
    matches!(t.to_ascii_lowercase().as_str(), "inf" | "infinity" | "nan")
}

/// Deserialize a parsed [`Value`] from the `YamlDeserializer` used
/// by [`from_str`](crate::from_str). The parse result is already a
/// `Value`; this visits it back into a fresh value tree.
///
/// Note: `ValueData::Tag` nodes are rebuilt from the variant name
/// derived by [`ValueEnumAccess`](crate::variant::ValueEnumAccess),
/// which loses the original tag URI (e.g. `<tag:yaml.org,2002:int>`
/// becomes `!int`). For a lossless parse use
/// [`Value::from_str`](std::str::FromStr) instead.
impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ValueVisitor)
    }
}

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.data {
            ValueData::Null => serializer.serialize_unit(),
            ValueData::String(s) => serializer.serialize_str(s),
            ValueData::Array(values) => {
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(value)?;
                }
                seq.end()
            }
            ValueData::Map(map) => {
                let mut map_ser = serializer.serialize_map(Some(map.len()))?;
                for (key, value) in map.iter() {
                    map_ser.serialize_entry(key, value)?;
                }
                map_ser.end()
            }
            ValueData::Tag(tag) => serializer.collect_str(&tag.name),
        }
    }
}

struct ValueVisitor;

impl<'de> serde::de::Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value {
            data: ValueData::Null,
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value {
            data: ValueData::Null,
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Value {
            data: ValueData::String(v.to_string()),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value {
            data: ValueData::String(v.to_string()),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Value {
            data: ValueData::String(v),
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
        while let Some(item) = seq.next_element::<Value>()? {
            array.push(item);
        }
        Ok(Value {
            data: ValueData::Array(array),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut data = Mapping::new();
        while let Some((key, value)) = map.next_entry::<Value, Value>()? {
            data.insert(key, value);
        }
        Ok(Value {
            data: ValueData::Map(Box::new(data)),
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
        let (name, variant) = data.variant_seed(VariantName)?;
        let inner = variant.newtype_variant_seed(ValueVisitor)?;
        Ok(Value {
            data: ValueData::Tag(Box::new(YamlTag {
                name: format!("!{name}"),
                data: inner.data,
            })),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        })
    }
}

struct VariantName;

impl<'de> serde::de::DeserializeSeed<'de> for ValueVisitor {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for VariantName {
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
