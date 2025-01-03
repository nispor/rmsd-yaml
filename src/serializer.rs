// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::fmt::Write;

use serde::{ser, Serialize};

use crate::{to_scalar_string, RmsdError};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RmsdSerializeOption {
    /// Whether include `---\n` at the beginning. Default is false.
    pub leading_start_indicator: bool,
    /// How many space should be used for each indent level. Default is 2.
    pub indent_count: usize,
    /// The max width of each line. 0 means no limit. Default is 80.
    pub max_width: usize,
}

impl Default for RmsdSerializeOption {
    fn default() -> Self {
        Self {
            leading_start_indicator: false,
            indent_count: 2,
            max_width: 80,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RmsdSerializer {
    option: RmsdSerializeOption,
    output: String,
    current_indent_level: usize,
}

pub fn to_string_with_opt<T>(
    value: &T,
    option: RmsdSerializeOption,
) -> Result<String, RmsdError>
where
    T: Serialize,
{
    if option.indent_count < 2 {
        return Err(RmsdError::indent_to_small());
    }
    let mut serializer = RmsdSerializer {
        output: if option.leading_start_indicator {
            "---\n".to_string()
        } else {
            String::new()
        },
        option,
        ..Default::default()
    };
    value.serialize(&mut serializer)?;
    if serializer.output.ends_with("\n") {
        serializer.output.pop();
    }
    Ok(serializer.output)
}

pub fn to_string<T>(value: &T) -> Result<String, RmsdError>
where
    T: Serialize,
{
    to_string_with_opt(value, RmsdSerializeOption::default())
}

impl RmsdSerializer {
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
}

impl ser::Serializer for &mut RmsdSerializer {
    type Ok = ();

    type Error = RmsdError;

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
    fn serialize_bool(self, v: bool) -> Result<(), RmsdError> {
        write!(
            self.output,
            "{}{}",
            self.get_indent(),
            if v { "true" } else { "false" }
        )
        .ok();
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<(), RmsdError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<(), RmsdError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<(), RmsdError> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<(), RmsdError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), RmsdError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<(), RmsdError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<(), RmsdError> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<(), RmsdError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();

        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), RmsdError> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<(), RmsdError> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        Ok(())
    }

    // YAML does not have special handling for char, just treat it as str
    fn serialize_char(self, v: char) -> Result<(), RmsdError> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<(), RmsdError> {
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
        Ok(())
    }

    // TODO: use base64 show them and also deserialize
    fn serialize_bytes(self, v: &[u8]) -> Result<(), RmsdError> {
        use serde::ser::SerializeSeq;
        let mut seq = self.serialize_seq(Some(v.len()))?;
        for byte in v {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }

    fn serialize_none(self) -> Result<(), RmsdError> {
        write!(self.output, "{}null", self.get_indent()).ok();
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), RmsdError> {
        self.serialize_none()
    }

    fn serialize_unit_struct(
        self,
        name: &'static str,
    ) -> Result<(), RmsdError> {
        write!(self.output, "{}!{name} null", self.get_indent()).ok();
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), RmsdError> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        writeln!(self.output, "{}!{name}", self.get_indent()).ok();
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        writeln!(self.output, "{}!{name}", self.get_indent()).ok();
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, RmsdError> {
        if self.output.ends_with(": ") {
            self.output.pop();
        }

        if !self.output.ends_with("\n")
            && !self.output.is_empty()
            && !self.output.ends_with("- ")
        {
            self.output.push('\n');
        }
        self.current_indent_level += 1;
        Ok(self)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, RmsdError> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, RmsdError> {
        self.serialize_seq(Some(len))
    }

    // Tuple variants are represented in JSON as `{ NAME: [DATA...] }`. Again
    // this method is only responsible for the externally tagged representation.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, RmsdError> {
        todo!()
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeMap, RmsdError> {
        if self.output.ends_with(": ") {
            self.output.pop();
            self.output += "\n";
        }
        self.current_indent_level += 1;
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
    ) -> Result<Self::SerializeStruct, RmsdError> {
        self.serialize_map(Some(len))
    }

    // Struct variants are represented in JSON as `{ NAME: { K: V, ... } }`.
    // This is the externally tagged representation.
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, RmsdError> {
        write!(self.output, "{}!{}", self.get_indent(), name).ok();
        variant.serialize(&mut *self)?;
        Ok(self)
    }
}

// The following 7 impls deal with the serialization of compound types like
// sequences and maps. Serialization of such types is begun by a Serializer
// method and followed by zero or more calls to serialize individual elements of
// the compound type and one call to end the compound type.
//
// This impl is SerializeSeq so these methods are called after `serialize_seq`
// is called on the Serializer.
impl ser::SerializeSeq for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), RmsdError>
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
    fn end(self) -> Result<(), RmsdError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTuple for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), RmsdError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTupleStruct for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<(), RmsdError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeTupleVariant for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<(), RmsdError> {
        todo!()
    }
}

impl ser::SerializeMap for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)?;
        self.output += ": ";
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), RmsdError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

// Structs are like maps in which the keys are constrained to be compile-time
// constant strings.
impl ser::SerializeStruct for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), RmsdError>
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

    fn end(self) -> Result<(), RmsdError> {
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
        Ok(())
    }
}

impl ser::SerializeStructVariant for &mut RmsdSerializer {
    type Ok = ();
    type Error = RmsdError;

    fn serialize_field<T>(
        &mut self,
        _key: &'static str,
        _value: &T,
    ) -> Result<(), RmsdError>
    where
        T: ?Sized + Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<(), RmsdError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ErrorKind;

    #[test]
    fn test_indent_too_small() {
        let opt = RmsdSerializeOption {
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
