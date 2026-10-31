// SPDX-License-Identifier: Apache-2.0

//! Multi-document stream deserialization.
//!
//! [`Deserializer`] is a lazy iterator over the documents of a YAML
//! stream. Each item is a single-document [`DocumentDeserializer`]
//! that implements [`serde::Deserializer`].

use std::io::Read;

use serde::de::{self, Visitor};

use crate::{
    Error, YamlEvent, YamlParseOption, YamlParser, YamlPosition,
    YamlScalarStyle, compose::MAX_COMPOSED_NODES, event_de::EventDeserializer,
    parser::MAX_NESTING_DEPTH,
};

#[derive(Debug)]
enum Source<'a> {
    Borrowed(&'a str),
    Owned(String),
}

impl Source<'_> {
    fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(input) => input,
            Self::Owned(input) => input,
        }
    }
}

/// A lazy iterator over the documents of a YAML stream.
///
/// ```rust
/// use rmsd_yaml::Deserializer;
/// use serde::Deserialize;
///
/// let input = "---\nname: first\n---\nname: second\n";
/// let names: Vec<String> = Deserializer::from_str(input)
///     .map(|document| {
///         #[derive(Deserialize)]
///         struct Doc {
///             name: String,
///         }
///         Doc::deserialize(document).unwrap().name
///     })
///     .collect();
/// assert_eq!(names, ["first", "second"]);
/// ```
#[derive(Debug)]
pub struct Deserializer<'a> {
    source: Source<'a>,
    /// Byte offset of the next unparsed document.
    offset: usize,
    /// Line number of the byte at `offset`.
    start_line: usize,
    max_depth: usize,
    max_nodes: usize,
    /// Whether the previous document ended with an explicit `...`.
    doc_terminated: bool,
    pending_error: Option<Error>,
    done: bool,
}

impl<'a> Deserializer<'a> {
    /// Create a stream deserializer over `input` with default resource
    /// limits.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(input: &'a str) -> Self {
        Self::from_str_with_opt(input, YamlParseOption::default())
    }

    /// Create a stream deserializer over `input` with the given
    /// resource limits.
    pub fn from_str_with_opt(input: &'a str, option: YamlParseOption) -> Self {
        Self::from_source(Source::Borrowed(input), option)
    }

    /// Create a stream deserializer over an I/O reader with default
    /// resource limits.
    ///
    /// The reader is buffered in full before parsing starts; use
    /// [`YamlParseOption::max_input_bytes`] to bound it.
    pub fn from_reader<R>(reader: R) -> Self
    where
        R: Read,
    {
        Self::from_reader_with_opt(reader, YamlParseOption::default())
    }

    /// Create a stream deserializer over an I/O reader with the given
    /// resource limits.
    pub fn from_reader_with_opt<R>(
        mut reader: R,
        option: YamlParseOption,
    ) -> Self
    where
        R: Read,
    {
        match crate::deserializer::read_to_string_with_limit(
            &mut reader,
            option.max_input_bytes,
        ) {
            Ok(input) => Self::from_source(Source::Owned(input), option),
            Err(error) => Self::from_error(error, option),
        }
    }

    fn from_error(error: Error, option: YamlParseOption) -> Self {
        Self {
            source: Source::Owned(String::new()),
            offset: 0,
            start_line: 1,
            max_depth: option.max_depth,
            max_nodes: option.max_nodes,
            doc_terminated: true,
            pending_error: Some(error),
            done: false,
        }
    }

    fn from_source(source: Source<'a>, option: YamlParseOption) -> Self {
        let input = YamlParser::strip_bom(source.as_str());
        // `strip_bom` only removes a prefix; the difference is the BOM
        // length in bytes.
        let offset = source.as_str().len() - input.len();
        match YamlParser::validate_input(input) {
            Ok(()) => Self {
                source,
                offset,
                start_line: 1,
                max_depth: option.max_depth,
                max_nodes: option.max_nodes,
                doc_terminated: true,
                pending_error: None,
                done: false,
            },
            Err(error) => {
                let mut deserializer = Self::from_error(error, option);
                deserializer.source = source;
                deserializer.offset = offset;
                deserializer
            }
        }
    }
}

impl Deserializer<'_> {
    fn single<T>(
        mut self,
        deserialize: impl FnOnce(DocumentDeserializer) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let first = self.next().unwrap_or_else(DocumentDeserializer::empty);
        let value = deserialize(first)?;
        if let Some(second) = self.next() {
            if let Some(error) = second.error_ref() {
                return Err(error.clone());
            }
            return Err(Error::new(
                crate::ErrorKind::NoSupportMultipleDocuments,
                "No support of multiple YAML documents".to_string(),
                YamlPosition::default(),
                YamlPosition::default(),
            ));
        }
        Ok(value)
    }
}

macro_rules! forward_stream_method {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.single(|document| document.$method(visitor))
        }
    };
}

impl<'de> de::Deserializer<'de> for Deserializer<'de> {
    type Error = Error;

    forward_stream_method!(deserialize_any);
    forward_stream_method!(deserialize_bool);
    forward_stream_method!(deserialize_i8);
    forward_stream_method!(deserialize_i16);
    forward_stream_method!(deserialize_i32);
    forward_stream_method!(deserialize_i64);
    forward_stream_method!(deserialize_u8);
    forward_stream_method!(deserialize_u16);
    forward_stream_method!(deserialize_u32);
    forward_stream_method!(deserialize_u64);
    forward_stream_method!(deserialize_f32);
    forward_stream_method!(deserialize_f64);
    forward_stream_method!(deserialize_char);
    forward_stream_method!(deserialize_str);
    forward_stream_method!(deserialize_string);
    forward_stream_method!(deserialize_bytes);
    forward_stream_method!(deserialize_byte_buf);
    forward_stream_method!(deserialize_option);
    forward_stream_method!(deserialize_unit);
    forward_stream_method!(deserialize_seq);
    forward_stream_method!(deserialize_map);
    forward_stream_method!(deserialize_identifier);
    forward_stream_method!(deserialize_ignored_any);

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| document.deserialize_unit_struct(name, visitor))
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| {
            document.deserialize_newtype_struct(name, visitor)
        })
    }

    fn deserialize_tuple<V>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| document.deserialize_tuple(len, visitor))
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| {
            document.deserialize_tuple_struct(name, len, visitor)
        })
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| {
            document.deserialize_struct(name, fields, visitor)
        })
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.single(|document| {
            document.deserialize_enum(name, variants, visitor)
        })
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

impl Iterator for Deserializer<'_> {
    type Item = DocumentDeserializer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if let Some(error) = self.pending_error.take() {
            self.done = true;
            return Some(DocumentDeserializer::from_error(error));
        }
        let (result, consumed, line, doc_terminated) = {
            let remaining = &self.source.as_str()[self.offset..];
            let mut parser = YamlParser::new_validated(
                remaining,
                self.max_depth,
                self.doc_terminated,
                self.start_line,
            );
            let result = parser.next_document();
            (
                result,
                parser.offset(),
                parser.current_line(),
                parser.is_doc_terminated(),
            )
        };
        self.offset += consumed;
        self.start_line = line;
        self.doc_terminated = doc_terminated;
        match result {
            Ok(Some(events)) => Some(DocumentDeserializer::from_events(
                events,
                self.max_depth,
                self.max_nodes,
            )),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(error) => {
                self.done = true;
                Some(DocumentDeserializer::from_error(error))
            }
        }
    }
}

/// A single document from a [`Deserializer`] stream.
///
/// A document that failed to parse carries its error and returns it
/// from every `serde::Deserializer` method, so the failure surfaces
/// when the document is actually deserialized.
#[derive(Debug)]
pub struct DocumentDeserializer {
    inner: Option<EventDeserializer>,
    error: Option<Error>,
}

impl DocumentDeserializer {
    pub(crate) fn from_events(
        events: Vec<YamlEvent>,
        max_depth: usize,
        max_nodes: usize,
    ) -> Self {
        match EventDeserializer::new_document(events, max_depth, max_nodes) {
            Ok(inner) => Self {
                inner: Some(inner),
                error: None,
            },
            Err(error) => Self::from_error(error),
        }
    }

    pub(crate) fn from_error(error: Error) -> Self {
        Self {
            inner: None,
            error: Some(error),
        }
    }

    /// An empty stream still deserializes as a null document, matching
    /// the single-document APIs' historical behavior.
    pub(crate) fn empty() -> Self {
        let events = vec![
            YamlEvent::DocumentStart(false, YamlPosition::EOF),
            YamlEvent::Scalar(
                None,
                None,
                String::new(),
                YamlScalarStyle::Plain,
                YamlPosition::EOF,
                YamlPosition::EOF,
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::EOF),
        ];
        Self::from_events(events, MAX_NESTING_DEPTH, MAX_COMPOSED_NODES)
    }

    pub(crate) fn error_ref(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    fn into_inner(self) -> Result<EventDeserializer, Error> {
        match (self.inner, self.error) {
            (_, Some(error)) => Err(error),
            (Some(inner), None) => Ok(inner),
            (None, None) => Err(Error::new(
                crate::ErrorKind::Bug,
                "empty DocumentDeserializer".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            )),
        }
    }
}

macro_rules! forward_document_method {
    ($method:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            let mut inner = self.into_inner()?;
            (&mut inner).$method(visitor)
        }
    };
}

impl<'de> de::Deserializer<'de> for DocumentDeserializer {
    type Error = Error;

    forward_document_method!(deserialize_any);
    forward_document_method!(deserialize_bool);
    forward_document_method!(deserialize_i8);
    forward_document_method!(deserialize_i16);
    forward_document_method!(deserialize_i32);
    forward_document_method!(deserialize_i64);
    forward_document_method!(deserialize_u8);
    forward_document_method!(deserialize_u16);
    forward_document_method!(deserialize_u32);
    forward_document_method!(deserialize_u64);
    forward_document_method!(deserialize_f32);
    forward_document_method!(deserialize_f64);
    forward_document_method!(deserialize_char);
    forward_document_method!(deserialize_str);
    forward_document_method!(deserialize_string);
    forward_document_method!(deserialize_bytes);
    forward_document_method!(deserialize_byte_buf);
    forward_document_method!(deserialize_option);
    forward_document_method!(deserialize_unit);
    forward_document_method!(deserialize_seq);
    forward_document_method!(deserialize_map);
    forward_document_method!(deserialize_identifier);
    forward_document_method!(deserialize_ignored_any);

    fn deserialize_unit_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_newtype_struct(name, visitor)
    }

    fn deserialize_tuple<V>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_tuple(len, visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_tuple_struct(name, len, visitor)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_struct(name, fields, visitor)
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.into_inner()?;
        inner.deserialize_enum(name, variants, visitor)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}
