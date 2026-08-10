// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::{io::Read, str::FromStr};

use serde::{
    Deserialize, Serialize,
    de::{Deserializer, Expected, Visitor},
};

use crate::{
    Error, ErrorKind, MappingAccess, SequenceAccess, Value, ValueData,
    ValueEnumAccess, YamlPosition, YamlScalarStyle,
    compose::MAX_COMPOSED_NODES, parser::MAX_NESTING_DEPTH,
};

/// Options controlling the resource limits enforced while parsing
/// YAML, guarding against maliciously crafted input (deeply nested
/// documents, anchor/alias expansion bombs) exhausting the stack or
/// memory. Used by the `_with_opt` variants of [`from_str`] and
/// [`from_reader`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct YamlParseOption {
    /// Maximum node nesting depth (block or flow, sequences and
    /// mappings). Default is 128.
    pub max_depth: usize,
    /// Maximum number of `Value` nodes a single document may realize
    /// during composition, including nodes duplicated by resolving an
    /// alias. Default is 1,000,000.
    pub max_nodes: usize,
    /// Maximum number of bytes [`from_reader_with_opt`] will read from
    /// the stream before giving up with `ErrorKind::InputTooLarge`, or
    /// `0` for no limit. Default is 0 (no limit, matching
    /// `from_reader`'s historical behavior). Has no effect on
    /// [`from_str_with_opt`], whose input is already fully in memory
    /// by the time it receives it; set this instead of wrapping the
    /// reader in `Read::take` yourself when reading from an
    /// untrusted, size-unbounded source (e.g. a network socket).
    pub max_input_bytes: usize,
}

impl Default for YamlParseOption {
    fn default() -> Self {
        Self {
            max_depth: MAX_NESTING_DEPTH,
            max_nodes: MAX_COMPOSED_NODES,
            max_input_bytes: 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct YamlDeserializer {
    pub(crate) parsed: Value,
    // Path of the value currently being deserialized, e.g.
    // `config[0].cwnd`, used to prefix type-mismatch error messages
    // like serde_yaml (`cwnd: invalid type: integer `-20`, expected u32`).
    pub(crate) path: String,
}

impl YamlDeserializer {
    /// Build a `serde_yaml`-style `invalid type` error message
    /// (`invalid type: {actual}, expected {expected}`) for the value
    /// currently being deserialized, keeping the original error kind
    /// (e.g. `NumberOverflow`).
    fn invalid_type_error<'de, V>(&self, kind: ErrorKind, visitor: &V) -> Error
    where
        V: Visitor<'de>,
    {
        let actual = self.parsed.unexpected();
        let expected: &dyn Expected = visitor;
        let msg = format!("invalid type: {actual}, expected {expected}");
        Error::new(kind, msg, self.parsed.start, self.parsed.end)
    }
}

pub fn from_str<'a, T>(s: &'a str) -> Result<T, Error>
where
    T: Deserialize<'a>,
{
    from_str_with_opt(s, YamlParseOption::default())
}

/// Like [`from_str`], but with configurable resource limits instead of
/// the defaults:
///
/// ```
/// use rmsd_yaml::{YamlParseOption, from_str_with_opt};
///
/// let mut option = YamlParseOption::default();
/// option.max_depth = 4;
/// let err = from_str_with_opt::<i32>("[[[[[1]]]]]", option).unwrap_err();
/// assert_eq!(err.kind(), rmsd_yaml::ErrorKind::RecursionLimitExceeded);
/// ```
pub fn from_str_with_opt<'a, T>(
    s: &'a str,
    option: YamlParseOption,
) -> Result<T, Error>
where
    T: Deserialize<'a>,
{
    let parsed = Value::from_str_with_opt(s, &option)?;
    let mut deserializer = YamlDeserializer {
        parsed,
        path: String::new(),
    };

    T::deserialize(&mut deserializer)
}

/// Serialize a value into a [`Value`] tree, mirroring `serde_yaml::to_value`.
///
/// The value is first serialized to YAML text and then composed back into a
/// [`Value`] tree, so the result carries round-trip metadata just like a
/// parsed document.
pub fn to_value<T>(value: T) -> Result<Value, Error>
where
    T: Serialize,
{
    let yaml = crate::to_string(&value)?;
    Value::from_str(&yaml)
}

/// Deserialize an instance of type `T` from an I/O stream of YAML.
///
/// Mirrors `serde_yaml::from_reader`. `rdr` is buffered into memory in
/// full before parsing starts, with no size limit by default: when
/// reading from an untrusted, size-unbounded source (e.g. a network
/// socket), either wrap `rdr` in [`Read::take`](std::io::Read::take)
/// yourself, or use [`from_reader_with_opt`] with
/// `YamlParseOption::max_input_bytes` set.
pub fn from_reader<R, T>(rdr: R) -> Result<T, Error>
where
    R: std::io::Read,
    T: serde::de::DeserializeOwned,
{
    from_reader_with_opt(rdr, YamlParseOption::default())
}

/// Like [`from_reader`], but with configurable resource limits instead
/// of the defaults:
///
/// ```
/// use rmsd_yaml::{YamlParseOption, from_reader_with_opt};
///
/// let mut option = YamlParseOption::default();
/// option.max_input_bytes = 8;
/// let err = from_reader_with_opt::<_, String>(
///     "this input is over 8 bytes long".as_bytes(),
///     option,
/// )
/// .unwrap_err();
/// assert_eq!(err.kind(), rmsd_yaml::ErrorKind::InputTooLarge);
/// ```
pub fn from_reader_with_opt<R, T>(
    mut rdr: R,
    option: YamlParseOption,
) -> Result<T, Error>
where
    R: std::io::Read,
    T: serde::de::DeserializeOwned,
{
    let content = read_to_string_with_limit(&mut rdr, option.max_input_bytes)?;
    from_str_with_opt(&content, option)
}

/// Read `rdr` to a `String`, capped at `max_input_bytes` (or
/// unbounded when it is `0`). Reads one byte past the limit so a
/// stream containing exactly `max_input_bytes` can be told apart from
/// one that exceeds it, instead of silently truncating.
fn read_to_string_with_limit(
    rdr: &mut impl std::io::Read,
    max_input_bytes: usize,
) -> Result<String, Error> {
    if max_input_bytes == 0 {
        let mut content = String::new();
        rdr.read_to_string(&mut content)?;
        return Ok(content);
    }
    let take_limit = (max_input_bytes as u64).saturating_add(1);
    let mut content = String::new();
    rdr.take(take_limit).read_to_string(&mut content)?;
    if content.len() > max_input_bytes {
        return Err(Error::new(
            ErrorKind::InputTooLarge,
            format!(
                "YAML input exceeds the maximum supported size of \
                 {max_input_bytes} bytes"
            ),
            YamlPosition::EOF,
            YamlPosition::EOF,
        ));
    }
    Ok(content)
}

impl<'de> Deserializer<'de> for &mut YamlDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match &self.parsed.data {
            ValueData::String(_) => {
                if self.parsed.is_null() {
                    self.deserialize_unit(visitor)
                } else if self.parsed.is_bool() {
                    self.deserialize_bool(visitor)
                } else if self.parsed.is_integer() {
                    self.deserialize_u64(visitor)
                } else if self.parsed.is_signed_integer() {
                    self.deserialize_i64(visitor)
                } else if self.parsed.is_float() {
                    self.deserialize_f64(visitor)
                } else {
                    self.deserialize_str(visitor)
                }
            }
            ValueData::Array(_) => self.deserialize_seq(visitor),
            ValueData::Map(_) => self.deserialize_map(visitor),
            ValueData::Tag(tag) => {
                // A YAML tag is metadata consumed by `deserialize_enum`
                // (Rust enum variants are matched against the tag name
                // via `ValueEnumAccess`); when the caller instead wants
                // "any" value (e.g. deserializing into a generic
                // `Value`), the node resolves from its underlying data,
                // matching `serde_yaml`. The YAML core-schema scalar
                // tags (`!!str`, `!!int`, `!!float`, `!!bool`,
                // `!!null`) and the bare non-specific tag `!` (which
                // resolves to `!!str`, YAML 1.2.2 SPEC, 10.3 Core
                // Schema) force that specific type instead of
                // auto-detecting it from content; every other tag
                // (custom tags, and core-schema collection/binary tags
                // like `!!seq`, `!!map`, `!!set`, `!!binary`) is
                // transparent.
                let mut inner = YamlDeserializer {
                    parsed: Value {
                        data: tag.data.clone(),
                        start: self.parsed.start,
                        end: self.parsed.end,
                        meta: self.parsed.meta.clone(),
                    },
                    path: self.path.clone(),
                };
                match tag.name.as_str() {
                    "<tag:yaml.org,2002:str>" | "<!>" => {
                        inner.deserialize_str(visitor)
                    }
                    "<tag:yaml.org,2002:int>" => {
                        // The tag forces integer resolution regardless
                        // of how the scalar was styled in the source
                        // (e.g. `!!int "23"` is `23`, not the string
                        // `"23"`), unlike implicit (untagged)
                        // resolution, which only auto-detects plain
                        // scalars.
                        inner.parsed.meta.scalar_style =
                            Some(YamlScalarStyle::Plain);
                        if inner.parsed.is_integer() {
                            inner.deserialize_u64(visitor)
                        } else {
                            inner.deserialize_i64(visitor)
                        }
                    }
                    "<tag:yaml.org,2002:float>" => {
                        inner.parsed.meta.scalar_style =
                            Some(YamlScalarStyle::Plain);
                        inner.deserialize_f64(visitor)
                    }
                    "<tag:yaml.org,2002:bool>" => {
                        inner.parsed.meta.scalar_style =
                            Some(YamlScalarStyle::Plain);
                        inner.deserialize_bool(visitor)
                    }
                    "<tag:yaml.org,2002:null>" => {
                        inner.parsed.meta.scalar_style =
                            Some(YamlScalarStyle::Plain);
                        inner.deserialize_unit(visitor)
                    }
                    _ => inner.deserialize_any(visitor),
                }
            }
            v => Err(Error::new(
                ErrorKind::Bug,
                format!("deserialize_any() got unexpected data {v:?}"),
                self.parsed.start,
                self.parsed.end,
            )),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_bool() {
            Ok(v) => visitor.visit_bool(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_i8() {
            Ok(v) => visitor.visit_i8(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_i16() {
            Ok(v) => visitor.visit_i16(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_i32() {
            Ok(v) => visitor.visit_i32(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_i64() {
            Ok(v) => visitor.visit_i64(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_u8() {
            Ok(v) => visitor.visit_u8(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_u16() {
            Ok(v) => visitor.visit_u16(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_u32() {
            Ok(v) => visitor.visit_u32(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_u64() {
            Ok(v) => visitor.visit_u64(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.parsed.as_f64() {
            Ok(v) => visitor.visit_f64(v),
            Err(e) => Err(self.invalid_type_error(e.kind(), &visitor)),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_char(self.parsed.as_char()?)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.parsed.as_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.parsed.as_str()?.to_string())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // Only `!!binary` tagged scalars carry binary data; plain
        // scalars and other tags are rejected like serde_yaml.
        if let ValueData::Tag(tag) = &self.parsed.data
            && tag.name == "<tag:yaml.org,2002:binary>"
            && let ValueData::String(s) = &tag.data
        {
            let bytes = crate::base64::decode(s).map_err(|e| {
                Error::new(
                    ErrorKind::InvalidNumber,
                    format!("Invalid base64 in !!binary tag: {e}"),
                    self.parsed.start,
                    self.parsed.end,
                )
            })?;
            return visitor.visit_byte_buf(bytes);
        }
        Err(Error::new(
            ErrorKind::BytesUnsupported,
            "Deserializing bytes is not supported (expected !!binary tag)"
                .to_string(),
            self.parsed.start,
            self.parsed.end,
        ))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.parsed.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if self.parsed.is_null() {
            visitor.visit_unit()
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting null, but got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let ValueData::Array(v) = &self.parsed.data {
            // TODO: We cannot move data output of `&mut self`, so we use
            // to_vec() to clone here. Maybe should use `Option<Value>` for
            // Self::parsed, where we can use `Option::take()` to move data out.
            let access = SequenceAccess::new(v.to_vec(), self.path.clone());
            visitor.visit_seq(access)
        } else if let ValueData::Tag(tag) = &self.parsed.data {
            if let ValueData::Array(v) = &tag.data {
                let access = SequenceAccess::new(v.to_vec(), self.path.clone());
                visitor.visit_seq(access)
            } else {
                Err(Error::new(
                    ErrorKind::UnexpectedYamlNodeType,
                    format!(
                        "Expecting a sequence in tag, got {}",
                        self.parsed.data
                    ),
                    self.parsed.start,
                    self.parsed.end,
                ))
            }
        } else if self.parsed.is_null() {
            // `a:` and `a: null` deserialize into an empty sequence,
            // matching serde_yaml.
            let access = SequenceAccess::new(vec![], self.path.clone());
            visitor.visit_seq(access)
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a sequence, got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_tuple<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if let ValueData::Map(v) = &self.parsed.data {
            // TODO: We cannot move data output of `&mut self`, so we use clone
            // here. Maybe should use `Option<Value>` for Self::parsed,
            // where we can use `Option::take()` to move data out.
            let access = MappingAccess::new(*v.clone(), self.path.clone());
            visitor.visit_map(access)
        } else if self.parsed.is_null() {
            // `a:` and `a: null` deserialize into an empty map/struct,
            // matching serde_yaml.
            let access =
                MappingAccess::new(Default::default(), self.path.clone());
            visitor.visit_map(access)
        } else {
            Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting a map, got {}", self.parsed.data),
                self.parsed.start,
                self.parsed.end,
            ))
        }
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // TODO: We cannot move data output of `&mut self`, so we use clone
        // here. Maybe should use `Option<Value>` for Self::parsed,
        // where we can use `Option::take()` to move data out.
        let access = ValueEnumAccess::new(self.parsed.clone());

        visitor.visit_enum(access)
    }

    fn deserialize_identifier<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(
        self,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}
