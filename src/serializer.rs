// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::fmt::Write;

use serde::{Serialize, ser};

use crate::{
    ErrorKind, YamlError, YamlPosition, base64, to_out_yaml_scalar,
    to_scalar_string,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct YamlSerializeOption {
    /// Whether include `---\n` at the beginning. Default is false.
    pub leading_start_indicator: bool,
    /// How many space should be used for each indent level. Default is 2.
    pub indent_count: usize,
    /// The max width of each line. 0 means no limit. Default is 80.
    pub max_width: usize,
}

impl Default for YamlSerializeOption {
    fn default() -> Self {
        Self {
            leading_start_indicator: false,
            indent_count: 2,
            max_width: 80,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct YamlSerializer {
    option: YamlSerializeOption,
    output: String,
    current_indent_level: usize,
    /// Set when a `!Tag ` was written for a newtype variant; the tag is
    /// followed by a space only when the value is a scalar.
    pending_tag: bool,
}

pub fn to_string_with_opt<T>(
    value: &T,
    option: YamlSerializeOption,
) -> Result<String, YamlError>
where
    T: Serialize,
{
    if option.indent_count < 2 {
        return Err(YamlError::new(
            ErrorKind::IndentTooSmall,
            "Minimum supported indent count is 2".to_string(),
            YamlPosition::EOF,
            YamlPosition::EOF,
        ));
    }
    let mut serializer = YamlSerializer {
        output: if option.leading_start_indicator {
            "---\n".to_string()
        } else {
            String::new()
        },
        option,
        ..Default::default()
    };
    value.serialize(&mut serializer)?;
    if serializer.output.ends_with("\n\n") {
        serializer.output.pop();
    }
    if !serializer.output.ends_with("\n") {
        serializer.output.push('\n');
    }
    Ok(serializer.output)
}

pub fn to_string<T>(value: &T) -> Result<String, YamlError>
where
    T: Serialize,
{
    to_string_with_opt(value, YamlSerializeOption::default())
}

impl YamlSerializer {
    fn get_indent_count(&self) -> usize {
        if !self.output.ends_with("\n")
            || self.output.ends_with("- ")
            || self.current_indent_level == 0
        {
            0
        } else {
            (self.current_indent_level - 1) * self.option.indent_count
        }
    }

    pub(crate) fn get_indent(&self) -> String {
        " ".repeat(self.get_indent_count())
    }

    /// Write a `!Tag` before a value. When the value is a scalar, a
    /// space follows (`!Tag value`); when it is a collection, the
    /// collection moves to the next line (`!Tag\n- ...`).
    fn write_tag(&mut self, tag: &str) {
        write!(self.output, "{}!{tag} ", self.get_indent()).ok();
        self.pending_tag = true;
    }

    /// Start a collection: a pending `!Tag ` (or `key: `) is completed
    /// with a line break before the collection body.
    fn start_collection(&mut self) {
        if self.pending_tag && self.output.ends_with(' ') {
            self.output.pop();
            self.output.push('\n');
            self.pending_tag = false;
        } else if self.output.ends_with(": ") {
            self.output.pop();
            self.output.push('\n');
        } else if !self.output.ends_with("\n")
            && !self.output.is_empty()
            && !self.output.ends_with("- ")
        {
            self.output.push('\n');
        }
        self.current_indent_level += 1;
    }

    pub(crate) fn serialize_yaml_value(
        &mut self,
        value: &crate::YamlValue,
    ) -> Result<(), YamlError> {
        self.serialize_yaml_value_ctx(value, ValueCtx::Root)
    }

    /// Serialize the data of a [`YamlValueData::Tag`] (or nested tag),
    /// keeping the current layout context.
    fn serialize_yaml_value_data(
        &mut self,
        data: &crate::YamlValueData,
        ctx: ValueCtx,
    ) -> Result<(), YamlError> {
        let value = crate::YamlValue {
            data: data.clone(),
            start: YamlPosition::EOF,
            end: YamlPosition::EOF,
            ..Default::default()
        };
        self.serialize_yaml_value_ctx(&value, ctx)
    }

    fn serialize_yaml_value_ctx(
        &mut self,
        value: &crate::YamlValue,
        ctx: ValueCtx,
    ) -> Result<(), YamlError> {
        use crate::YamlValueData;
        match &value.data {
            YamlValueData::Null => {
                // An empty document (no content) renders as nothing.
                self.pending_tag = false;
                Ok(())
            }
            YamlValueData::String(s) => {
                write!(
                    self.output,
                    "{}{}",
                    self.get_indent(),
                    to_out_yaml_scalar(s)
                )
                .ok();
                self.pending_tag = false;
                Ok(())
            }
            YamlValueData::Array(items) => {
                if items.is_empty() {
                    write!(self.output, "{}[]", self.get_indent()).ok();
                    self.pending_tag = false;
                    return Ok(());
                }
                let indentless = matches!(ctx, ValueCtx::MapValue);
                if self.pending_tag {
                    self.output.pop();
                    self.output.push('\n');
                    self.pending_tag = false;
                } else if self.output.ends_with(": ") {
                    if indentless {
                        // `key:` stays (the space is dropped), items on
                        // the following lines at the key's own indent.
                        self.output.pop();
                        self.output.push('\n');
                    }
                    // In a sequence-item context (e.g. after an explicit
                    // `: `) the first item stays on the same line.
                } else if !self.output.ends_with("\n")
                    && !self.output.is_empty()
                    && !self.output.ends_with("- ")
                    && !self.output.ends_with("? ")
                {
                    self.output.push('\n');
                }
                if !indentless {
                    self.current_indent_level += 1;
                }
                for item in items {
                    let before = self.output.len();
                    write!(self.output, "{}- ", self.get_indent()).ok();
                    self.serialize_yaml_value_ctx(item, ValueCtx::SeqItem)?;
                    if self.output.len() == before + 2 {
                        // Empty item: `- ` alone renders as `-`.
                        self.output.pop();
                    }
                    if !self.output.ends_with('\n') {
                        self.output.push('\n');
                    }
                }
                if !indentless {
                    self.current_indent_level -= 1;
                }
                Ok(())
            }
            YamlValueData::Map(map) => {
                if map.len() == 0 {
                    write!(self.output, "{}{{}}", self.get_indent()).ok();
                    self.pending_tag = false;
                    return Ok(());
                }
                // A mapping value is always indented (only a sequence
                // value is written indentless).
                if self.pending_tag {
                    self.output.pop();
                    self.output.push('\n');
                    self.pending_tag = false;
                } else if self.output.ends_with(": ") {
                    if matches!(ctx, ValueCtx::SeqItem) {
                        // After an explicit `: ` the first key stays on
                        // the same line (`: hr: 65`).
                    } else {
                        // `key:` stays (the space is dropped), the
                        // sub-keys are written indented.
                        self.output.pop();
                        self.output.push('\n');
                    }
                } else if !self.output.ends_with("\n")
                    && !self.output.is_empty()
                    && !self.output.ends_with("- ")
                    && !self.output.ends_with("? ")
                {
                    self.output.push('\n');
                }
                self.current_indent_level += 1;
                for (key, item) in map.iter() {
                    if is_simple_key(key) {
                        write!(
                            self.output,
                            "{}{}: ",
                            self.get_indent(),
                            simple_key_text(key)
                        )
                        .ok();
                        let before = self.output.len();
                        self.serialize_yaml_value_ctx(
                            item,
                            ValueCtx::MapValue,
                        )?;
                        if self.output.len() == before {
                            // Empty value: `key: ` renders as `key:`.
                            self.output.pop();
                        }
                        if !self.output.ends_with('\n') {
                            self.output.push('\n');
                        }
                    } else {
                        // Explicit `? key` form, value on its own `: `;
                        // a collection value starts inline on that line.
                        write!(self.output, "{}? ", self.get_indent()).ok();
                        self.serialize_yaml_value_ctx(key, ValueCtx::SeqItem)?;
                        if !self.output.ends_with('\n') {
                            self.output.push('\n');
                        }
                        write!(self.output, "{}: ", self.get_indent()).ok();
                        let before = self.output.len();
                        self.serialize_yaml_value_ctx(item, ValueCtx::SeqItem)?;
                        if self.output.len() == before {
                            self.output.pop();
                        }
                        if !self.output.ends_with('\n') {
                            self.output.push('\n');
                        }
                    }
                }
                self.current_indent_level -= 1;
                Ok(())
            }
            YamlValueData::Tag(tag) => {
                self.write_tag(&tag_shorthand(&tag.name));
                let empty =
                    matches!(
                        &tag.data,
                        crate::YamlValueData::String(s) if s.is_empty()
                    ) || matches!(&tag.data, crate::YamlValueData::Null);
                self.serialize_yaml_value_data(&tag.data, ctx)?;
                if empty && self.output.ends_with(' ') {
                    self.output.pop();
                }
                Ok(())
            }
        }
    }
}

/// Where the current value is being written, which decides the layout
/// of nested collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueCtx {
    Root,
    SeqItem,
    MapValue,
}

/// Whether a mapping key can be written in the simple `key: value`
/// form (a single-line scalar, a tag on a single-line scalar, or an
/// empty collection).
fn is_simple_key(key: &crate::YamlValue) -> bool {
    match &key.data {
        crate::YamlValueData::String(s) => !s.contains('\n'),
        crate::YamlValueData::Tag(tag) => {
            matches!(&tag.data, crate::YamlValueData::String(s) if !s.contains('\n'))
        }
        crate::YamlValueData::Array(items) => items.is_empty(),
        crate::YamlValueData::Map(map) => map.len() == 0,
        _ => false,
    }
}

/// The text of a simple key: the scalar itself, a tag on a scalar
/// (e.g. `!!str a`, and `!!str ` for a tagged empty scalar, so the
/// following `:` is separated by a space), or an empty collection.
fn simple_key_text(key: &crate::YamlValue) -> String {
    match &key.data {
        crate::YamlValueData::String(s) => to_out_yaml_scalar(s),
        crate::YamlValueData::Tag(tag) => {
            let mut text = format!("!{}", tag_shorthand(&tag.name));
            if let crate::YamlValueData::String(s) = &tag.data {
                if s.is_empty() {
                    text.push(' ');
                } else {
                    text.push(' ');
                    text.push_str(s);
                }
            }
            text
        }
        crate::YamlValueData::Array(_) => "[]".to_string(),
        crate::YamlValueData::Map(_) => "{}".to_string(),
        _ => String::new(),
    }
}

/// Convert a stored tag URI (e.g. `<tag:yaml.org,2002:int>`, `<!foo>`)
/// back to its YAML shorthand form (`!!int`, `!foo`), keeping unmatched
/// prefixes verbatim (`!<tag:...>`). The leading `!` is omitted: the
/// caller (e.g. `write_tag`) prepends it.
fn tag_shorthand(name: &str) -> String {
    let inner = name
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(name);
    if let Some(suffix) = inner.strip_prefix("tag:yaml.org,2002:") {
        return format!("!{suffix}");
    }
    if let Some(rest) = inner.strip_prefix('!') {
        return rest.to_string();
    }
    format!("<{inner}>")
}

/// Dump a parsed [`YamlValue`] back to YAML (the `to_yaml` workflow),
/// using the default [`YamlSerializeOption`].
impl crate::YamlValue {
    pub fn to_string(&self) -> Result<String, YamlError> {
        self.to_string_with_opt(YamlSerializeOption::default())
    }

    /// Dump a parsed [`YamlValue`] back to YAML, honoring `option`.
    ///
    /// `indent_count` controls the block indentation (minimum 2) and
    /// `leading_start_indicator` prepends a `---` document header.
    /// `max_width` is accepted for compatibility with
    /// [`to_string_with_opt`] but has no effect here: the yaml-test-suite
    /// `out.yaml` files never fold long lines, so the value dump never
    /// wraps (only the serde serializer path folds).
    pub fn to_string_with_opt(
        &self,
        option: YamlSerializeOption,
    ) -> Result<String, YamlError> {
        if option.indent_count < 2 {
            return Err(YamlError::new(
                ErrorKind::IndentTooSmall,
                "Minimum supported indent count is 2".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        }
        let mut serializer = YamlSerializer {
            output: if option.leading_start_indicator {
                "---\n".to_string()
            } else {
                String::new()
            },
            option,
            ..Default::default()
        };
        serializer.serialize_yaml_value(self)?;
        if serializer.output.is_empty() {
            return Ok(String::new());
        }
        if serializer.output.ends_with("\n\n") {
            serializer.output.pop();
        }
        if !serializer.output.ends_with('\n') {
            serializer.output.push('\n');
        }
        Ok(serializer.output)
    }
}

impl ser::Serializer for &mut YamlSerializer {
    type Ok = ();

    type Error = YamlError;

    // Associated types for keeping track of additional state while serializing
    // compound data structures like sequences and maps. In this case no
    // additional state is required beyond what is already stored in the
    // Serializer struct.
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    // Here we go with the simple methods. The following 12 methods receive one
    // of the primitive types of the data model and map it to JSON by appending
    // into the output string.
    fn serialize_bool(self, v: bool) -> Result<(), YamlError> {
        write!(
            self.output,
            "{}{}",
            self.get_indent(),
            if v { "true" } else { "false" }
        )
        .ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<(), YamlError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<(), YamlError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<(), YamlError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<(), YamlError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), YamlError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<(), YamlError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<(), YamlError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<(), YamlError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), YamlError> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<(), YamlError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    // YAML does not have special handling for char, just treat it as str
    fn serialize_char(self, v: char) -> Result<(), YamlError> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<(), YamlError> {
        write!(
            self.output,
            "{}{}",
            self.get_indent(),
            to_scalar_string(
                self.current_indent_level * self.option.indent_count,
                v,
                self.option.max_width
            )
        )
        .ok();
        self.pending_tag = false;
        Ok(())
    }

    // TODO: use base64 show them and also deserialize
    fn serialize_bytes(self, v: &[u8]) -> Result<(), YamlError> {
        write!(
            self.output,
            "{}!!binary {}",
            self.get_indent(),
            base64::encode(v)
        )
        .ok();
        Ok(())
    }

    fn serialize_none(self) -> Result<(), YamlError> {
        write!(self.output, "{}null", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), YamlError> {
        self.serialize_none()
    }

    fn serialize_unit_struct(
        self,
        _name: &'static str,
    ) -> Result<(), YamlError> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), YamlError> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        self.write_tag(variant);
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, YamlError> {
        self.start_collection();
        Ok(self)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, YamlError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, YamlError> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in YAML as `!Variant` followed by
    // the sequence of fields.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, YamlError> {
        writeln!(self.output, "{}!{variant}", self.get_indent()).ok();
        self.serialize_seq(Some(_len))
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeMap, YamlError> {
        self.start_collection();
        Ok(self)
    }

    // Structs look just like maps in JSON. In particular, JSON requires that we
    // serialize the field names of the struct. Other formats may be able to
    // omit the field names when serializing structs because the corresponding
    // Deserialize implementation is required to know what the keys are without
    // looking at the serialized data.
    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, YamlError> {
        self.serialize_map(Some(len))
    }

    // Struct variants are represented in YAML as `!Variant` followed by
    // the mapping of fields.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, YamlError> {
        writeln!(self.output, "{}!{variant}", self.get_indent()).ok();
        self.serialize_map(Some(_len))
    }
}

// The following 7 impls deal with the serialization of compound types like
// sequences and maps. Serialization of such types is begun by a Serializer
// method and followed by zero or more calls to serialize individual elements of
// the compound type and one call to end the compound type.
//
// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl ser::SerializeSeq for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    // Close the sequence.
    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTuple for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTupleStruct for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTupleVariant for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeMap for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        self.output += ": ";
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

// Structs are like maps in which the keys are constrained to be compile-time
// constant strings.
impl ser::SerializeStruct for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        self.output += ": ";
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output += "\n";
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeStructVariant for &mut YamlSerializer {
    type Ok = ();
    type Error = YamlError;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), YamlError>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        self.output += ": ";
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output += "\n";
        }
        Ok(())
    }

    fn end(self) -> Result<(), YamlError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn test_indent_too_small() {
        let opt = YamlSerializeOption {
            indent_count: 1,
            ..Default::default()
        };
        let result = to_string_with_opt(&"abc", opt);

        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::IndentTooSmall);
        }
    }
}
