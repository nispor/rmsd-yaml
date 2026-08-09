// SPDX-License-Identifier: Apache-2.0

// Code here is based on example code in
//      https://serde.rs/impl-serializer.html
//      (https://github.com/serde-rs/serde-rs.github.io)
// which is licensed under CC-BY-SA-4.0 license

use std::fmt::Write;

use serde::{Serialize, ser};

use crate::{
    Error, ErrorKind, YamlPosition, base64, escape_double_quoted,
    to_out_yaml_scalar_plain, to_out_yaml_scalar_sq, to_scalar_string,
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
    /// Set when the last written node was a keep-chomped block scalar
    /// ending in blank lines; a `...` line is appended at the end.
    open_ended: bool,
    /// Depth of mapping values being serialized: a block sequence that
    /// is a mapping value is written indentless (`port:\n- name: eth1`),
    /// matching `serde_yaml`.
    map_value_depth: usize,
    /// Per open sequence: whether it is a mapping value (indentless) so
    /// `end()` only unwinds the indent it actually added.
    seq_indentless_stack: Vec<bool>,
    /// Per open serde collection (sequence/map/struct): how many
    /// elements or entries were written, so an empty collection can be
    /// rendered explicitly (`{}` / `[]`) in `end()`.
    collection_entry_counts: Vec<usize>,
}

pub fn to_string_with_opt<T>(
    value: &T,
    option: YamlSerializeOption,
) -> Result<String, Error>
where
    T: Serialize,
{
    if option.indent_count < 2 {
        return Err(Error::new(
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

pub fn to_string<T>(value: &T) -> Result<String, Error>
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

    /// Whether the output currently ends with a `- ` or `? ` node
    /// prefix (written by this serializer) that the first collection
    /// item should follow inline. An explicit `--- ` document marker is
    /// *not* such a prefix: collections move to the next line after it.
    fn at_node_prefix(&self) -> bool {
        (self.output.ends_with("- ") && !self.output.ends_with("--- "))
            || self.output.ends_with("? ")
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
    ///
    /// When `indentless` (a block sequence that is a mapping value) the
    /// collection body is written at the key's own indentation,
    /// matching `serde_yaml` (`port:\n- name: eth1`).
    fn start_collection(&mut self, indentless: bool) {
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
        if !indentless {
            self.current_indent_level += 1;
        }
    }

    /// Finish a serde collection that wrote `entries` items: when it is
    /// empty, render it explicitly (`{}` / `[]`) instead of leaving the
    /// bare `key:` line produced by [`Self::start_collection`], matching
    /// serde_yaml (`routes: {}`, `address: []`).
    fn finish_collection(&mut self, entries: usize, flow: &str) {
        if entries > 0 {
            return;
        }
        if self.output.ends_with(":\n") {
            // `key:\n` -> `key: {}` / `key: []`.
            self.output.pop();
            self.output.push(' ');
            self.output.push_str(flow);
        } else if self.output.ends_with("- ")
            || self.output.ends_with("? ")
            || self.output.is_empty()
        {
            self.output.push_str(flow);
        }
    }

    fn inc_collection_entry_count(&mut self) {
        if let Some(count) = self.collection_entry_counts.last_mut() {
            *count += 1;
        }
    }

    /// Close an open serde sequence, rendering an empty one as `[]`.
    fn finish_seq(&mut self) {
        let entries = self.collection_entry_counts.pop().unwrap_or(0);
        self.finish_collection(entries, "[]");
        if self.seq_indentless_stack.pop() == Some(false)
            && self.current_indent_level > 0
        {
            self.current_indent_level -= 1;
        }
    }

    /// Close an open serde map/struct, rendering an empty one as `{}`.
    fn finish_map(&mut self) {
        let entries = self.collection_entry_counts.pop().unwrap_or(0);
        self.finish_collection(entries, "{}");
        if self.current_indent_level > 0 {
            self.current_indent_level -= 1;
        }
    }

    pub(crate) fn serialize_yaml_value(
        &mut self,
        value: &crate::Value,
    ) -> Result<(), Error> {
        self.serialize_yaml_value_ctx(value, ValueCtx::Root)
    }

    /// Write a block scalar (`|` or `>` style) at the current position,
    /// ported from libyaml's `yaml_emitter_write_literal_scalar` /
    /// `yaml_emitter_write_folded_scalar` (which is what the
    /// yaml-test-suite `out.yaml` files follow).
    fn write_block_scalar(
        &mut self,
        value: &str,
        style: crate::YamlScalarStyle,
    ) {
        use crate::YamlScalarStyle::*;
        // Content is indented two spaces past the indentation of the
        // enclosing collection (the block scalar's "block indent").
        let indent = self.current_indent_level.saturating_sub(1)
            * self.option.indent_count
            + 2;
        // Chomping: strip when the value does not end in a line break,
        // keep when it ends in two or more (the trailing blank lines
        // are preserved as content and the document is closed with
        // `...`), otherwise clip (default, no indicator).
        let chomp = if value.is_empty() || !value.ends_with('\n') {
            "-"
        } else if value == "\n" || value.ends_with("\n\n") {
            "+"
        } else {
            ""
        };
        if chomp == "+" {
            self.open_ended = true;
        }
        // An explicit indentation indicator is only needed when the
        // value starts with a space or a line break.
        let hint = if value.starts_with(' ') || value.starts_with('\n') {
            "2"
        } else {
            ""
        };
        let indicator = if style == Literal { "|" } else { ">" };
        writeln!(self.output, "{indicator}{hint}{chomp}").ok();
        let chars: Vec<char> = value.chars().collect();
        match style {
            Literal => self.write_literal_content(&chars, indent),
            Folded => self.write_folded_content(&chars, indent),
            _ => unreachable!(),
        }
    }

    /// Write the content lines of a literal (`|`) block scalar. Every
    /// line break starts a new line (blank lines stay blank); content
    /// lines are indented by `indent`.
    fn write_literal_content(&mut self, chars: &[char], indent: usize) {
        let mut breaks = true;
        for &c in chars {
            if c == '\n' {
                self.output.push('\n');
                breaks = true;
            } else {
                if breaks {
                    for _ in 0..indent {
                        self.output.push(' ');
                    }
                }
                self.output.push(c);
                breaks = false;
            }
        }
    }

    /// Write the content lines of a folded (`>`) block scalar, ported
    /// from libyaml's `yaml_emitter_write_folded_scalar`: a single
    /// line break followed by non-blank content becomes a line break;
    /// breaks at the end of the value (blank lines) are kept as
    /// empty lines.
    fn write_folded_content(&mut self, chars: &[char], indent: usize) {
        let mut breaks = true;
        let mut leading_spaces = true;
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\n' {
                if !breaks && !leading_spaces {
                    // A single break followed by non-blank content is
                    // written as a line break; a break followed by a
                    // blank line or the end stays a folded space.
                    let mut k = i;
                    while k < chars.len() && chars[k] == '\n' {
                        k += 1;
                    }
                    if k < chars.len() && !matches!(chars[k], ' ' | '\t' | '\n')
                    {
                        self.output.push('\n');
                    }
                }
                self.output.push('\n');
                i += 1;
                breaks = true;
            } else {
                if breaks {
                    for _ in 0..indent {
                        self.output.push(' ');
                    }
                    leading_spaces = c == ' ';
                }
                self.output.push(c);
                i += 1;
                breaks = false;
            }
        }
    }

    fn serialize_yaml_value_ctx(
        &mut self,
        value: &crate::Value,
        ctx: ValueCtx,
    ) -> Result<(), Error> {
        use crate::ValueData;
        // Any node written after a keep-chomped block scalar invalidates
        // the pending `...` closing line.
        self.open_ended = false;
        // An alias is rendered as `*name` and stops here (its `data` is
        // the resolved value).
        if let Some(alias) = &value.meta.alias {
            write!(self.output, "*{alias}").ok();
            self.pending_tag = false;
            return Ok(());
        }
        // An anchor is rendered as `&name ` before the node content.
        if let Some(anchor) = &value.meta.anchor {
            write!(self.output, "&{anchor} ").ok();
        }
        match &value.data {
            ValueData::Null => {
                // An empty document (no content) renders as nothing.
                self.pending_tag = false;
                Ok(())
            }
            ValueData::String(s) => {
                match value.meta.scalar_style {
                    Some(crate::YamlScalarStyle::Literal)
                    | Some(crate::YamlScalarStyle::Folded) => {
                        if block_allowed(s, value.meta.scalar_style.unwrap()) {
                            let style = value.meta.scalar_style.unwrap();
                            self.write_block_scalar(s, style);
                        } else {
                            // A block scalar whose content cannot be
                            // re-rendered as a block (e.g. a line
                            // starting with a space) is double-quoted.
                            write!(
                                self.output,
                                "{}\"{}\"",
                                self.get_indent(),
                                escape_double_quoted(s)
                            )
                            .ok();
                        }
                        self.pending_tag = false;
                    }
                    Some(crate::YamlScalarStyle::SingleQuoted) => {
                        write!(
                            self.output,
                            "{}{}",
                            self.get_indent(),
                            to_out_yaml_scalar_sq(s, true)
                        )
                        .ok();
                        self.pending_tag = false;
                    }
                    Some(crate::YamlScalarStyle::DoubleQuoted) => {
                        write!(
                            self.output,
                            "{}\"{}\"",
                            self.get_indent(),
                            escape_double_quoted(s)
                        )
                        .ok();
                        self.pending_tag = false;
                    }
                    _ => {
                        write!(
                            self.output,
                            "{}{}",
                            self.get_indent(),
                            to_out_yaml_scalar_plain(s)
                        )
                        .ok();
                        self.pending_tag = false;
                        if s.is_empty() && value.meta.anchor.is_some() {
                            // `&name ` alone is the anchored empty scalar.
                            self.output.pop();
                        }
                    }
                }
                Ok(())
            }
            ValueData::Array(items) => {
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
                    && !self.at_node_prefix()
                {
                    // Drop the space after a `&anchor ` written before
                    // the collection (`seq: &anchor\n- a`).
                    if self.output.ends_with(' ') {
                        self.output.pop();
                    }
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
            ValueData::Map(map) => {
                if map.is_empty() {
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
                    && !self.at_node_prefix()
                {
                    // Drop the space after a `&anchor ` written before
                    // the collection (`top1: &node1\n  key: ...`).
                    if self.output.ends_with(' ') {
                        self.output.pop();
                    }
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
                        // An anchored collection key is written
                        // indentless (`? &a\n- a`); an unanchored one
                        // keeps the first item inline (`? - d\n  - e`).
                        let key_ctx = if key.meta.anchor.is_some() {
                            ValueCtx::MapValue
                        } else {
                            ValueCtx::SeqItem
                        };
                        self.serialize_yaml_value_ctx(key, key_ctx)?;
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
            ValueData::Tag(tag) => {
                self.write_tag(&tag_shorthand(&tag.name));
                let empty = matches!(
                    &tag.data,
                    crate::ValueData::String(s) if s.is_empty()
                ) || matches!(&tag.data, crate::ValueData::Null);
                // The style/anchor meta lives on the outer node (the
                // event carried both the tag and the style); propagate
                // it so the wrapped data is dumped with its style. The
                // anchor is written once, before the tag.
                let mut meta = value.meta.clone();
                meta.anchor = None;
                let inner = crate::Value {
                    data: tag.data.clone(),
                    start: value.start,
                    end: value.end,
                    meta,
                };
                self.serialize_yaml_value_ctx(&inner, ctx)?;
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
fn is_simple_key(key: &crate::Value) -> bool {
    // An alias is always a simple key (`*name :`), regardless of what
    // it resolves to.
    if key.meta.alias.is_some() {
        return true;
    }
    match &key.data {
        crate::ValueData::String(s) => !s.contains('\n'),
        crate::ValueData::Tag(tag) => {
            matches!(&tag.data, crate::ValueData::String(s) if !s.contains('\n'))
        }
        crate::ValueData::Array(items) => items.is_empty(),
        crate::ValueData::Map(map) => map.is_empty(),
        _ => false,
    }
}

/// The text of a simple key, rendered before the `:` — including a
/// leading `&anchor ` (so `&a a:` and the anchored empty key `&a :`)
/// and the `*alias ` spacing for alias keys (`*b : *a`).
fn simple_key_text(key: &crate::Value) -> String {
    let mut text = String::new();
    if let Some(alias) = &key.meta.alias {
        text.push_str(&format!("*{alias} "));
        return text;
    }
    if let Some(anchor) = &key.meta.anchor {
        text.push_str(&format!("&{anchor} "));
    }
    text.push_str(&match &key.data {
        crate::ValueData::String(s) => match key.meta.scalar_style {
            Some(crate::YamlScalarStyle::SingleQuoted) => {
                to_out_yaml_scalar_sq(s, false)
            }
            Some(crate::YamlScalarStyle::DoubleQuoted) => {
                format!("\"{}\"", escape_double_quoted(s))
            }
            _ => to_out_yaml_scalar_plain(s),
        },
        crate::ValueData::Tag(tag) => {
            let mut tag_text = format!("!{}", tag_shorthand(&tag.name));
            if let crate::ValueData::String(s) = &tag.data {
                tag_text.push(' ');
                // An anchored tagged key is rendered with a quoted
                // scalar (`&a1 !!str "foo"`), matching
                // `spec-example-6-23-node-properties`.
                let scalar = if key.meta.anchor.is_some() {
                    format!("\"{}\"", escape_double_quoted(s))
                } else {
                    to_out_yaml_scalar_plain(s)
                };
                tag_text.push_str(&scalar);
            }
            tag_text
        }
        crate::ValueData::Array(_) => "[]".to_string(),
        crate::ValueData::Map(_) => "{}".to_string(),
        _ => String::new(),
    });
    text
}

/// Convert a stored tag URI (e.g. `<tag:yaml.org,2002:int>`, `<!foo>`)
/// back to its YAML shorthand form (`!!int`, `!foo`), keeping unmatched
/// prefixes verbatim (`!<tag:...>`). The leading `!` is omitted: the
/// caller (e.g. `write_tag`) prepends it.
/// Whether a scalar value can be rendered as a block scalar (`|`/`>`).
///
/// Fitted to the yaml-test-suite `out.yaml` data (libyaml's
/// `yaml_emitter_analyze_scalar`, with the tab handling relaxed to
/// match the hand-authored files):
/// * a space followed by a line break (`space_break`) or by a tab is not
///   allowed (e.g. `block-scalar-keep`, `spec-example-6-4`);
/// * a tab followed by a line break is not allowed in folded scalars
///   (`spec-example-8-2`) but is fine in literal ones
///   (`tabs-in-various-contexts/001`);
/// * a tab at the start of a content line is fine (`spec-example-5-12`,
///   `spec-example-8-7`);
/// * the value must not end in a space or tab.
fn block_allowed(value: &str, style: crate::YamlScalarStyle) -> bool {
    if value.is_empty() {
        return false;
    }
    if value.ends_with(' ') || value.ends_with('\t') {
        return false;
    }
    let chars: Vec<char> = value.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        match c {
            ' ' => {
                if chars.get(i + 1).is_some_and(|&n| n == '\n' || n == '\t') {
                    return false;
                }
            }
            '\t' => {
                if style == crate::YamlScalarStyle::Folded
                    && chars.get(i + 1) == Some(&'\n')
                {
                    return false;
                }
            }
            c if (c as u32) < 0x20 && c != '\n' => return false,
            c if (0x7f..=0x9f).contains(&(c as u32)) => return false,
            _ => {}
        }
    }
    true
}

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

/// Dump a parsed [`Value`] back to YAML (the `to_yaml` workflow),
/// using the default [`YamlSerializeOption`].
impl crate::Value {
    pub fn to_string(&self) -> Result<String, Error> {
        self.to_string_with_opt(YamlSerializeOption::default())
    }

    /// Dump a parsed [`Value`] back to YAML, honoring `option`.
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
    ) -> Result<String, Error> {
        if option.indent_count < 2 {
            return Err(Error::new(
                ErrorKind::IndentTooSmall,
                "Minimum supported indent count is 2".to_string(),
                YamlPosition::EOF,
                YamlPosition::EOF,
            ));
        }
        let mut serializer = YamlSerializer {
            output: if option.leading_start_indicator || self.meta.doc_explicit
            {
                // An explicit document marker puts the root node's
                // properties (anchor/tag) or scalar inline after `---`.
                "--- ".to_string()
            } else {
                String::new()
            },
            option,
            ..Default::default()
        };
        serializer.serialize_yaml_value(self)?;
        let mut output = std::mem::take(&mut serializer.output);
        if output == "--- " {
            // An explicit `---` with an empty document.
            output = "---".to_string();
        }
        if output.is_empty() {
            return Ok(String::new());
        }
        if serializer.open_ended || self.meta.doc_end_explicit {
            // A keep-chomped block scalar ending in blank lines, or an
            // explicit `...` document-end marker, closes the document.
            if !output.ends_with('\n') {
                output.push('\n');
            }
            output.push_str("...\n");
        }
        if output.ends_with("\n\n") {
            output.pop();
        }
        if !output.ends_with('\n') {
            output.push('\n');
        }
        Ok(output)
    }
}

impl ser::Serializer for &mut YamlSerializer {
    type Ok = ();

    type Error = Error;

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
    fn serialize_bool(self, v: bool) -> Result<(), Error> {
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

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        write!(self.output, "{}{v}", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    // YAML does not have special handling for char, just treat it as str
    fn serialize_char(self, v: char) -> Result<(), Error> {
        self.serialize_str(&v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
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
    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        write!(
            self.output,
            "{}!!binary {}",
            self.get_indent(),
            base64::encode(v)
        )
        .ok();
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        write!(self.output, "{}null", self.get_indent()).ok();
        self.pending_tag = false;
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.serialize_none()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.serialize_none()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error>
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
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.write_tag(variant);
        value.serialize(self)
    }

    fn serialize_seq(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeSeq, Error> {
        // A block sequence that is a mapping value is written indentless
        // (serde_yaml: `port:\n- 1\n- 2`), i.e. the items sit at the
        // key's own indentation. A sequence that is a sequence item
        // (output ends with `- `) is indented normally.
        let indentless =
            self.map_value_depth > 0 && !self.output.ends_with("- ");
        self.seq_indentless_stack.push(indentless);
        self.collection_entry_counts.push(0);
        self.start_collection(indentless);
        Ok(self)
    }

    fn serialize_tuple(
        self,
        len: usize,
    ) -> Result<Self::SerializeTuple, Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
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
    ) -> Result<Self::SerializeTupleVariant, Error> {
        writeln!(self.output, "{}!{variant}", self.get_indent()).ok();
        self.serialize_seq(Some(_len))
    }

    // Maps are represented in JSON as `{ K: V, K: V, ... }`.
    fn serialize_map(
        self,
        _len: Option<usize>,
    ) -> Result<Self::SerializeMap, Error> {
        self.collection_entry_counts.push(0);
        self.start_collection(false);
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
    ) -> Result<Self::SerializeStruct, Error> {
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
    ) -> Result<Self::SerializeStructVariant, Error> {
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
    type Error = Error;

    // Serialize a single element of the sequence.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    // Close the sequence.
    fn end(self) -> Result<(), Error> {
        self.finish_seq();
        Ok(())
    }
}

impl ser::SerializeTuple for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_seq();
        Ok(())
    }
}

impl ser::SerializeTupleStruct for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_seq();
        Ok(())
    }
}

impl ser::SerializeTupleVariant for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        write!(self.output, "{}- ", self.get_indent()).ok();
        value.serialize(&mut **self)?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_seq();
        Ok(())
    }
}

impl ser::SerializeMap for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        key.serialize(&mut **self)?;
        self.output += ": ";
        Ok(())
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.map_value_depth += 1;
        let result = value.serialize(&mut **self);
        self.map_value_depth -= 1;
        result?;
        if !self.output.ends_with("\n") {
            self.output.push('\n');
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_map();
        Ok(())
    }
}

// Structs are like maps in which the keys are constrained to be compile-time
// constant strings.
impl ser::SerializeStruct for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        key.serialize(&mut **self)?;
        self.output += ": ";
        self.map_value_depth += 1;
        let result = value.serialize(&mut **self);
        self.map_value_depth -= 1;
        result?;
        if !self.output.ends_with("\n") {
            self.output += "\n";
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_map();
        Ok(())
    }
}

impl ser::SerializeStructVariant for &mut YamlSerializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.inc_collection_entry_count();
        key.serialize(&mut **self)?;
        self.output += ": ";
        self.map_value_depth += 1;
        let result = value.serialize(&mut **self);
        self.map_value_depth -= 1;
        result?;
        if !self.output.ends_with("\n") {
            self.output += "\n";
        }
        Ok(())
    }

    fn end(self) -> Result<(), Error> {
        self.finish_map();
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
