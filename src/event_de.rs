// SPDX-License-Identifier: Apache-2.0

//! Deserialization directly from the event stream.
//!
//! Unlike [`crate::YamlDeserializer`], which walks a composed
//! [`Value`] tree, [`EventDeserializer`] walks the events produced by
//! the parser. An anchored node is remembered as an event range, so an
//! alias can replay that range without cloning a subtree into a
//! `Value`.

use std::{cell::Cell, collections::HashMap, ops::Range, rc::Rc};

use serde::de::{
    self, DeserializeSeed, Deserializer as _, EnumAccess, Expected, MapAccess,
    SeqAccess, VariantAccess, Visitor,
};

use crate::{
    Error, ErrorKind, Value, ValueData, ValueMeta, YamlEvent, YamlPosition,
    YamlScalarStyle, variant::TagNameDeserializer,
};

/// Per-document event data shared by the root cursor, nested
/// collection accessors and alias cursors.
#[derive(Debug)]
struct DocumentEvents {
    events: Rc<[YamlEvent]>,
    /// For every `SequenceStart`/`MapStart`, the index of its matching
    /// end event.
    matches: Vec<usize>,
    /// Anchor name to the event ranges of nodes carrying that anchor,
    /// in completion order. Duplicate names are allowed; the alias
    /// lookup picks the most recent range completed before the alias.
    anchors: HashMap<String, Vec<Range<usize>>>,
    max_depth: usize,
    max_nodes: usize,
}

impl DocumentEvents {
    fn build(
        events: Vec<YamlEvent>,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<Self, Error> {
        let mut matches = vec![0usize; events.len()];
        let mut anchors: HashMap<String, Vec<Range<usize>>> = HashMap::new();
        let mut stack: Vec<(usize, Option<String>)> = Vec::new();
        for (index, event) in events.iter().enumerate() {
            match event {
                YamlEvent::SequenceStart(anchor, _, _, _)
                | YamlEvent::MapStart(anchor, _, _, _) => {
                    stack.push((index, anchor.clone()));
                }
                YamlEvent::SequenceEnd(_) | YamlEvent::MapEnd(_) => {
                    let Some((start, anchor)) = stack.pop() else {
                        return Err(Error::new(
                            ErrorKind::Bug,
                            "event stream has an unmatched collection end"
                                .to_string(),
                            YamlPosition::EOF,
                            YamlPosition::EOF,
                        ));
                    };
                    matches[start] = index;
                    if let Some(anchor) = anchor {
                        anchors
                            .entry(anchor)
                            .or_default()
                            .push(start..index + 1);
                    }
                }
                YamlEvent::Scalar(anchor, _, _, _, _, _) => {
                    if let Some(anchor) = anchor {
                        anchors
                            .entry(anchor.clone())
                            .or_default()
                            .push(index..index + 1);
                    }
                }
                YamlEvent::Alias(_, _)
                | YamlEvent::StreamStart
                | YamlEvent::StreamEnd
                | YamlEvent::DocumentStart(_, _)
                | YamlEvent::DocumentEnd(_, _) => {}
            }
        }
        if !stack.is_empty() {
            return Err(Error::new(
                ErrorKind::Bug,
                "event stream has an unterminated collection".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        }
        Ok(Self {
            events: events.into(),
            matches,
            anchors,
            max_depth,
            max_nodes,
        })
    }
}

/// A cursor over one document's events.
#[derive(Clone, Debug)]
pub(crate) struct EventDeserializer {
    doc: Rc<DocumentEvents>,
    pos: usize,
    /// Exclusive end of this cursor's event range.
    end: usize,
    depth: usize,
    path: String,
    remaining_nodes: Rc<Cell<usize>>,
    /// Hide the tag on the event at this index. Used when replaying
    /// the content wrapped by a tagged node.
    suppress_tag_at: Option<usize>,
}

impl EventDeserializer {
    pub(crate) fn new_document(
        events: Vec<YamlEvent>,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<Self, Error> {
        let doc = Rc::new(DocumentEvents::build(events, max_depth, max_nodes)?);
        let remaining_nodes = Rc::new(Cell::new(doc.max_nodes));
        // `events[0]` is `DocumentStart`, `events[len - 1]` is
        // `DocumentEnd`; the node occupies `1..len - 1`.
        let end = doc.events.len().saturating_sub(1);
        Ok(Self {
            doc,
            pos: 1,
            end,
            depth: 0,
            path: String::new(),
            remaining_nodes,
            suppress_tag_at: None,
        })
    }

    fn child(&self, pos: usize, end: usize) -> Self {
        Self {
            doc: Rc::clone(&self.doc),
            pos,
            end,
            depth: self.depth,
            path: self.path.clone(),
            remaining_nodes: Rc::clone(&self.remaining_nodes),
            suppress_tag_at: None,
        }
    }

    fn event(&self, index: usize) -> &YamlEvent {
        &self.doc.events[index]
    }

    fn event_start(&self, index: usize) -> YamlPosition {
        match self.event(index) {
            YamlEvent::Scalar(_, _, _, _, start, _)
            | YamlEvent::SequenceStart(_, _, _, start)
            | YamlEvent::MapStart(_, _, _, start)
            | YamlEvent::Alias(_, start)
            | YamlEvent::DocumentStart(_, start)
            | YamlEvent::DocumentEnd(_, start) => *start,
            YamlEvent::StreamStart | YamlEvent::StreamEnd => YamlPosition::EOF,
            YamlEvent::SequenceEnd(pos) | YamlEvent::MapEnd(pos) => *pos,
        }
    }

    fn invalid_error(&self, message: String) -> Error {
        let pos = if self.pos < self.doc.events.len() {
            self.event_start(self.pos)
        } else {
            YamlPosition::EOF
        };
        Error::new(ErrorKind::Bug, message, pos, pos)
    }

    fn check_event(&self) -> Result<(), Error> {
        if self.pos < self.end {
            Ok(())
        } else {
            Err(self.invalid_error(format!(
                "unexpected end of event stream (cursor {} end {})",
                self.pos, self.end
            )))
        }
    }

    fn effective_tag(&self, index: usize) -> Option<&str> {
        if self.suppress_tag_at == Some(index) {
            return None;
        }
        match self.event(index) {
            YamlEvent::Scalar(_, tag, _, _, _, _)
            | YamlEvent::SequenceStart(_, tag, _, _)
            | YamlEvent::MapStart(_, tag, _, _) => tag.as_deref(),
            _ => None,
        }
    }

    fn scalar_value_at(&self, index: usize) -> Option<Value> {
        let YamlEvent::Scalar(anchor, tag, value, style, start, end) =
            self.event(index)
        else {
            return None;
        };
        let mut scalar_style = Some(*style);
        if let Some(tag) = tag {
            // Force the type requested by the core-schema tags, like
            // `tagged_inner()` does for the `Value` deserializer.
            match tag.as_str() {
                "<tag:yaml.org,2002:str>" | "<!>" => {
                    scalar_style = Some(YamlScalarStyle::SingleQuoted);
                }
                "<tag:yaml.org,2002:int>"
                | "<tag:yaml.org,2002:float>"
                | "<tag:yaml.org,2002:bool>"
                | "<tag:yaml.org,2002:null>" => {
                    scalar_style = Some(YamlScalarStyle::Plain);
                }
                _ => {}
            }
        }
        Some(Value {
            data: ValueData::String(value.clone()),
            start: *start,
            end: *end,
            meta: ValueMeta {
                scalar_style,
                anchor: anchor.clone(),
                ..Default::default()
            },
        })
    }

    /// The exclusive index just past the node starting at `index`.
    fn node_end(&self, index: usize) -> Result<usize, Error> {
        match self.event(index) {
            YamlEvent::Scalar(_, _, _, _, _, _) | YamlEvent::Alias(_, _) => {
                Ok(index + 1)
            }
            YamlEvent::SequenceStart(_, _, _, _)
            | YamlEvent::MapStart(_, _, _, _) => {
                let end = self.doc.matches[index];
                if end == 0 {
                    Err(self.invalid_error(format!(
                        "collection at event {index} has no matching end"
                    )))
                } else {
                    Ok(end + 1)
                }
            }
            _ => Err(self.invalid_error(format!(
                "unexpected event at index {index}: {:?}",
                self.event(index)
            ))),
        }
    }

    fn charge_node(&self, index: usize) -> Result<(), Error> {
        let remaining = self.remaining_nodes.get();
        if remaining == 0 {
            let pos = self.event_start(index);
            return Err(Error::new(
                ErrorKind::AliasExpansionLimitExceeded,
                format!(
                    "YAML document deserializes more than {} value nodes; \
                     this is likely an anchor/alias expansion bomb",
                    self.doc.max_nodes
                ),
                pos,
                pos,
            ));
        }
        self.remaining_nodes.set(remaining - 1);
        Ok(())
    }

    fn enter_collection(&self, index: usize) -> Result<(), Error> {
        if self.depth >= self.doc.max_depth {
            let pos = self.event_start(index);
            return Err(Error::new(
                ErrorKind::RecursionLimitExceeded,
                format!(
                    "YAML node nesting exceeds the maximum supported depth of \
                     {}",
                    self.doc.max_depth
                ),
                pos,
                pos,
            ));
        }
        Ok(())
    }

    fn alias_range(&self, index: usize) -> Result<Range<usize>, Error> {
        let YamlEvent::Alias(name, pos) = self.event(index) else {
            return Err(
                self.invalid_error(format!("event {index} is not an alias"))
            );
        };
        let ranges = self.doc.anchors.get(name).ok_or_else(|| {
            Error::new(
                ErrorKind::UnknownAlias,
                format!("Alias *{name} does not reference any anchored node"),
                *pos,
                *pos,
            )
        })?;
        ranges
            .iter()
            .rev()
            .find(|range| range.end <= index)
            .cloned()
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::UnknownAlias,
                    format!(
                        "Alias *{name} does not reference a node completed \
                         before it"
                    ),
                    *pos,
                    *pos,
                )
            })
    }

    fn alias_cursor(&self, index: usize) -> Result<Self, Error> {
        let range = self.alias_range(index)?;
        Ok(self.child(range.start, range.end))
    }

    fn node_description(&self, index: usize) -> String {
        match self.event(index) {
            YamlEvent::Scalar(_, _, value, _, _, _) => {
                format!("scalar {value:?}")
            }
            YamlEvent::SequenceStart(_, _, _, _) => "sequence".to_string(),
            YamlEvent::MapStart(_, _, _, _) => "map".to_string(),
            YamlEvent::Alias(name, _) => format!("alias *{name}"),
            other => format!("{other:?}"),
        }
    }

    fn unexpected(&self, expected: &str) -> Error {
        let pos = if self.pos < self.doc.events.len() {
            self.event_start(self.pos)
        } else {
            YamlPosition::EOF
        };
        Error::new(
            ErrorKind::UnexpectedYamlNodeType,
            format!(
                "Expecting {expected}, but got {}",
                self.node_description(self.pos)
            ),
            pos,
            pos,
        )
    }

    fn take_scalar(&mut self, expected: &str) -> Result<Value, Error> {
        self.check_event()?;
        let index = self.pos;
        match self.event(index) {
            YamlEvent::Alias(_, _) => {
                let mut child = self.alias_cursor(index)?;
                let value = child.take_scalar(expected)?;
                self.pos = index + 1;
                Ok(value)
            }
            YamlEvent::Scalar(_, _, _, _, _, _) => {
                self.charge_node(index)?;
                let value =
                    self.scalar_value_at(index).expect("event is a scalar");
                self.pos = index + 1;
                Ok(value)
            }
            _ => Err(self.unexpected(expected)),
        }
    }

    fn invalid_type_error<'de, V>(
        &self,
        kind: ErrorKind,
        visitor: &V,
        value: &Value,
    ) -> Error
    where
        V: Visitor<'de>,
    {
        let actual = value.unexpected();
        let expected: &dyn Expected = visitor;
        Error::new(
            kind,
            format!("invalid type: {actual}, expected {expected}"),
            value.start,
            value.end,
        )
    }

    fn deserialize_any_scalar<'de, V>(
        &mut self,
        index: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let value = self.scalar_value_at(index).expect("event is a scalar");
        self.charge_node(index)?;
        self.pos = index + 1;
        if value.is_null() {
            visitor.visit_unit()
        } else if value.is_bool() {
            visitor.visit_bool(value.as_bool()?)
        } else if value.is_integer() {
            visitor.visit_u64(value.as_u64()?)
        } else if value.is_signed_integer() {
            visitor.visit_i64(value.as_i64()?)
        } else if value.is_float() {
            visitor.visit_f64(value.as_f64()?)
        } else {
            visitor.visit_str(value.as_str()?)
        }
    }
}

macro_rules! deserialize_scalar {
    ($method:ident, $visit:ident, $convert:ident) => {
        fn $method<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            let value = self.take_scalar(stringify!($method))?;
            match value.$convert() {
                Ok(value) => visitor.$visit(value),
                Err(error) => {
                    Err(self.invalid_type_error(error.kind(), &visitor, &value))
                }
            }
        }
    };
}

impl<'de> de::Deserializer<'de> for &mut EventDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        let tag = self.effective_tag(index).map(str::to_string);
        match self.event(index) {
            YamlEvent::Alias(_, _) => {
                let mut child = self.alias_cursor(index)?;
                let value = child.deserialize_any(visitor)?;
                self.pos = index + 1;
                Ok(value)
            }
            YamlEvent::Scalar(_, _, _, _, _, _) => {
                if let Some(tag) = tag {
                    let end = self.node_end(index)?;
                    let access =
                        EventEnumAccess::new(self.child(index, end), Some(tag));
                    let value = visitor.visit_enum(access)?;
                    self.pos = end;
                    Ok(value)
                } else {
                    self.deserialize_any_scalar(index, visitor)
                }
            }
            YamlEvent::SequenceStart(_, _, _, _) => {
                if let Some(tag) = tag {
                    let end = self.node_end(index)?;
                    let access =
                        EventEnumAccess::new(self.child(index, end), Some(tag));
                    let value = visitor.visit_enum(access)?;
                    self.pos = end;
                    Ok(value)
                } else {
                    self.deserialize_seq(visitor)
                }
            }
            YamlEvent::MapStart(_, _, _, _) => {
                if let Some(tag) = tag {
                    let end = self.node_end(index)?;
                    let access =
                        EventEnumAccess::new(self.child(index, end), Some(tag));
                    let value = visitor.visit_enum(access)?;
                    self.pos = end;
                    Ok(value)
                } else {
                    self.deserialize_map(visitor)
                }
            }
            _ => Err(self.unexpected("a value")),
        }
    }

    deserialize_scalar!(deserialize_bool, visit_bool, as_bool);
    deserialize_scalar!(deserialize_i8, visit_i8, as_i8);
    deserialize_scalar!(deserialize_i16, visit_i16, as_i16);
    deserialize_scalar!(deserialize_i32, visit_i32, as_i32);
    deserialize_scalar!(deserialize_i64, visit_i64, as_i64);
    deserialize_scalar!(deserialize_u8, visit_u8, as_u8);
    deserialize_scalar!(deserialize_u16, visit_u16, as_u16);
    deserialize_scalar!(deserialize_u32, visit_u32, as_u32);
    deserialize_scalar!(deserialize_u64, visit_u64, as_u64);
    deserialize_scalar!(deserialize_f64, visit_f64, as_f64);
    deserialize_scalar!(deserialize_char, visit_char, as_char);

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_f64(visitor)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let value = self.take_scalar("a string")?;
        visitor.visit_str(value.as_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let value = self.take_scalar("a string")?;
        visitor.visit_string(value.as_str()?.to_string())
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_byte_buf(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_byte_buf(visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        if let YamlEvent::Scalar(_, Some(tag), value, _, start, end) =
            self.event(index)
            && tag == "<tag:yaml.org,2002:binary>"
        {
            let bytes = crate::base64::decode(value).map_err(|error| {
                Error::new(
                    ErrorKind::InvalidNumber,
                    format!("Invalid base64 in !!binary tag: {error}"),
                    *start,
                    *end,
                )
            })?;
            self.charge_node(index)?;
            self.pos = index + 1;
            return visitor.visit_byte_buf(bytes);
        }
        let pos = self.event_start(index);
        Err(Error::new(
            ErrorKind::BytesUnsupported,
            "Deserializing bytes is not supported (expected !!binary tag)"
                .to_string(),
            pos,
            pos,
        ))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_option(visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        if let YamlEvent::Scalar(_, _, _, _, _, _) = self.event(index) {
            let value = self.scalar_value_at(index).expect("event is a scalar");
            if self.effective_tag(index).is_none() && value.is_null() {
                self.charge_node(index)?;
                self.pos = index + 1;
                return visitor.visit_none();
            }
        }
        visitor.visit_some(self)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_unit(visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        if let YamlEvent::Scalar(_, _, _, _, start, end) = self.event(index) {
            let value = self.scalar_value_at(index).expect("event is a scalar");
            if self.effective_tag(index).is_none() && value.is_null() {
                self.charge_node(index)?;
                self.pos = index + 1;
                return visitor.visit_unit();
            }
            return Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!("Expecting null, but got {}", value.data),
                *start,
                *end,
            ));
        }
        Err(self.unexpected("null"))
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_seq(visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        if let YamlEvent::SequenceStart(_, _, _, _) = self.event(index) {
            self.enter_collection(index)?;
            self.charge_node(index)?;
            let end = self.node_end(index)?;
            let mut child = self.child(index + 1, end - 1);
            child.depth += 1;
            let access = EventSeqAccess::new(child, self.path.clone());
            let value = visitor.visit_seq(access)?;
            self.pos = end;
            return Ok(value);
        }
        if let YamlEvent::Scalar(_, _, _, _, _, _) = self.event(index) {
            let value = self.scalar_value_at(index).expect("event is a scalar");
            if self.effective_tag(index).is_none() && value.is_null() {
                // `a:` and `a: null` deserialize into an empty
                // sequence, matching serde_yaml.
                self.charge_node(index)?;
                self.pos = index + 1;
                let access = EventSeqAccess::new(
                    self.child(index + 1, index + 1),
                    self.path.clone(),
                );
                return visitor.visit_seq(access);
            }
        }
        Err(self.unexpected("a sequence"))
    }

    fn deserialize_tuple<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
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
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_map(visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        if let YamlEvent::MapStart(_, _, _, _) = self.event(index) {
            self.enter_collection(index)?;
            self.charge_node(index)?;
            let end = self.node_end(index)?;
            let mut child = self.child(index + 1, end - 1);
            child.depth += 1;
            let access = EventMapAccess::new(child, self.path.clone());
            let value = visitor.visit_map(access)?;
            self.pos = end;
            return Ok(value);
        }
        if let YamlEvent::Scalar(_, _, _, _, _, _) = self.event(index) {
            let value = self.scalar_value_at(index).expect("event is a scalar");
            if self.effective_tag(index).is_none() && value.is_null() {
                // `a:` and `a: null` deserialize into an empty
                // map/struct, matching serde_yaml.
                self.charge_node(index)?;
                self.pos = index + 1;
                let access = EventMapAccess::new(
                    self.child(index + 1, index + 1),
                    self.path.clone(),
                );
                return visitor.visit_map(access);
            }
        }
        Err(self.unexpected("a map"))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
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
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.check_event()?;
        let index = self.pos;
        if let YamlEvent::Alias(_, _) = self.event(index) {
            let mut child = self.alias_cursor(index)?;
            let value = child.deserialize_enum(_name, _variants, visitor)?;
            self.pos = index + 1;
            return Ok(value);
        }
        let tag = self.effective_tag(index).map(str::to_string);
        let end = self.node_end(index)?;
        let access = EventEnumAccess::new(self.child(index, end), tag);
        let value = visitor.visit_enum(access)?;
        self.pos = end;
        Ok(value)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

struct EventSeqAccess {
    cursor: EventDeserializer,
    path: String,
    index: usize,
}

impl EventSeqAccess {
    fn new(cursor: EventDeserializer, path: String) -> Self {
        Self {
            cursor,
            path,
            index: 0,
        }
    }
}

impl<'de> SeqAccess<'de> for EventSeqAccess {
    type Error = Error;

    fn next_element_seed<K>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.cursor.pos >= self.cursor.end {
            return Ok(None);
        }
        let before = self.cursor.pos;
        let mut path = self.path.clone();
        path.push('[');
        path.push_str(&self.index.to_string());
        path.push(']');
        let old_path = std::mem::replace(&mut self.cursor.path, path.clone());
        let result = seed.deserialize(&mut self.cursor);
        self.cursor.path = old_path;
        self.index += 1;
        if self.cursor.pos == before && result.is_ok() {
            return Err(self.cursor.invalid_error(
                "sequence element deserialized without consuming an event"
                    .to_string(),
            ));
        }
        result.map_err(|error| error.with_path(&path)).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }
}

struct EventMapAccess {
    cursor: EventDeserializer,
    path: String,
    current_key_path: Option<String>,
}

impl EventMapAccess {
    fn new(cursor: EventDeserializer, path: String) -> Self {
        Self {
            cursor,
            path,
            current_key_path: None,
        }
    }

    fn key_text(&self, index: usize) -> Option<String> {
        match &self.cursor.doc.events[index] {
            YamlEvent::Scalar(_, _, value, _, _, _) => Some(value.clone()),
            YamlEvent::Alias(_, _) => {
                let range = self.cursor.alias_range(index).ok()?;
                self.key_text(range.start)
            }
            _ => None,
        }
    }
}

impl<'de> MapAccess<'de> for EventMapAccess {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.cursor.pos >= self.cursor.end {
            return Ok(None);
        }
        let before = self.cursor.pos;
        let key_path = self.key_text(before);
        let old_path = std::mem::take(&mut self.cursor.path);
        let result = seed.deserialize(&mut self.cursor);
        self.cursor.path = old_path;
        self.current_key_path = key_path;
        if self.cursor.pos == before && result.is_ok() {
            return Err(self.cursor.invalid_error(
                "mapping key deserialized without consuming an event"
                    .to_string(),
            ));
        }
        result.map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: DeserializeSeed<'de>,
    {
        if self.cursor.pos >= self.cursor.end {
            return Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                "Expecting a map value, but none".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        }
        let mut path = self.path.clone();
        if let Some(key) = self.current_key_path.take() {
            if path.is_empty() {
                path = key;
            } else {
                path.push('.');
                path.push_str(&key);
            }
        }
        let old_path = std::mem::replace(&mut self.cursor.path, path.clone());
        let result = seed.deserialize(&mut self.cursor);
        self.cursor.path = old_path;
        result.map_err(|error| error.with_path(&path))
    }

    fn size_hint(&self) -> Option<usize> {
        None
    }
}

struct EventEnumAccess {
    cursor: EventDeserializer,
    tag_name: Option<String>,
}

impl EventEnumAccess {
    fn new(cursor: EventDeserializer, tag_name: Option<String>) -> Self {
        Self { cursor, tag_name }
    }

    fn inner(self) -> EventDeserializer {
        let mut inner = self.cursor;
        if self.tag_name.is_some() {
            inner.suppress_tag_at = Some(inner.pos);
        }
        inner
    }
}

impl<'de> EnumAccess<'de> for EventEnumAccess {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error>
    where
        V: DeserializeSeed<'de>,
    {
        if let Some(tag) = &self.tag_name {
            let value = seed.deserialize(TagNameDeserializer::new(tag))?;
            Ok((value, self))
        } else {
            let mut temporary = self.cursor.clone();
            let value = seed.deserialize(&mut temporary)?;
            Ok((value, self))
        }
    }
}

impl<'de> VariantAccess<'de> for EventEnumAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        match self.cursor.event(self.cursor.pos) {
            YamlEvent::Scalar(_, _, _, _, _, _) => Ok(()),
            _ => Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!(
                    "Expecting enum/variant string, but got {}",
                    self.cursor.node_description(self.cursor.pos)
                ),
                self.cursor.event_start(self.cursor.pos),
                self.cursor.event_start(self.cursor.pos),
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        let mut inner = self.inner();
        seed.deserialize(&mut inner)
    }

    fn tuple_variant<V>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.inner();
        inner.deserialize_seq(visitor)
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        let mut inner = self.inner();
        inner.deserialize_map(visitor)
    }
}
