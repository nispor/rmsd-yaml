// SPDX-License-Identifier: Apache-2.0

use std::{cmp::max, collections::HashMap};

use crate::{
    Error, ErrorKind, YamlEvent, YamlScalarStyle, YamlScanner, YamlState,
};

#[derive(Debug)]
pub(crate) struct YamlParser<'a> {
    pub(crate) scanner: YamlScanner<'a>,
    states: Vec<YamlState>,
    events: Vec<YamlEvent>,
    /// Indentation level of the innermost block collection, or `None`
    /// when the current node is not inside a block collection (e.g. a
    /// top-level document node). Block scalar content indentation is
    /// relative to it (YAML 1.2.2 SPEC, 8.1.1.1).
    pub(crate) block_indent: Option<usize>,
    /// Whether the node currently being parsed is the explicit value of
    /// a mapping entry (`: value`); a compact block mapping is allowed
    /// as an explicit value on the same line, unlike an implicit one
    /// (`a: b: c` is an error).
    pub(crate) in_explicit_value: bool,
    /// The 0-based column of the current block mapping's first key;
    /// subsequent keys must sit at the same column.
    pub(crate) map_key_indent: Option<usize>,
    /// The indentation of the block sequence entries currently being
    /// parsed, once established by the first line-start entry; `None`
    /// while the first entry is still mid-line (a compact entry). Used
    /// to decide whether a deeper-indented `- ` line starts a new
    /// entry or continues a plain scalar.
    pub(crate) seq_entry_indent: Option<usize>,
    /// Tag handle (`!`, `!!`, `!name!`) to prefix mapping declared by
    /// `%TAG` directives, scoped to the current document.
    pub(crate) tag_handles: HashMap<String, String>,
    /// Whether a `%YAML`/`%TAG`/reserved directive was seen since the
    /// last document boundary; used to reject directives that are not
    /// followed by a document.
    saw_directive: bool,
    /// Whether a `%YAML` directive was seen since the last document
    /// boundary; a second `%YAML` for the same document is an error.
    yaml_directive_seen: bool,
    /// Current node nesting depth, incremented by `handle_node()` and
    /// `handle_flow_node()` (the two recursive "parse one more node"
    /// entry points) and checked against `max_depth` to guard against a
    /// stack overflow on pathologically deep input.
    depth: usize,
    /// Maximum value `depth` may reach, configurable via
    /// `YamlParseOption::max_depth`. Defaults to `MAX_NESTING_DEPTH`.
    max_depth: usize,
}

/// Maximum supported YAML node nesting depth (block or flow). Chosen
/// well below the depth that overflows the stack (observed around
/// 5,000 nested `[` or `- ` indicators) while comfortably fitting any
/// realistic document.
pub(crate) const MAX_NESTING_DEPTH: usize = 128;

impl<'a> YamlParser<'a> {
    /// Current state
    pub(crate) fn cur_state(&self) -> &YamlState {
        self.states.last().unwrap_or(&YamlState::EndOfFile)
    }

    pub(crate) fn push_event(&mut self, event: YamlEvent) {
        log::trace!("Got event {:?}", event);
        self.events.push(event);
    }

    /// Current count of the pushed events.
    pub(crate) fn events_len(&self) -> usize {
        self.events.len()
    }

    /// Take the events pushed since `start` out. Used to wrap an
    /// already-emitted flow node into a single-pair flow mapping.
    pub(crate) fn take_events_since(&mut self, start: usize) -> Vec<YamlEvent> {
        self.events.drain(start..).collect()
    }

    pub(crate) fn push_state(&mut self, state: YamlState) {
        log::trace!("Push state {:?}", state);
        self.states.push(state);
    }

    pub(crate) fn pop_state(&mut self) {
        let state = self.states.pop();
        log::trace!("Pop state: {:?}", state);
    }

    /// Test-only convenience wrapper for `parse_to_events_with_max_depth`
    /// using the default `MAX_NESTING_DEPTH`; production code goes
    /// through `YamlParseOption` (see `from_str_with_opt`,
    /// `documents_with_opt`) instead.
    #[cfg(test)]
    pub(crate) fn parse_to_events(
        input: &'a str,
    ) -> Result<Vec<YamlEvent>, Error> {
        Self::parse_to_events_with_max_depth(input, MAX_NESTING_DEPTH)
    }

    /// Like [`Self::parse_to_events`], but nodes nested deeper than
    /// `max_depth` are rejected instead of the default
    /// `MAX_NESTING_DEPTH`. Backs `YamlParseOption::max_depth`.
    pub(crate) fn parse_to_events_with_max_depth(
        input: &'a str,
        max_depth: usize,
    ) -> Result<Vec<YamlEvent>, Error> {
        let mut parser = Self {
            scanner: YamlScanner::new(input),
            states: Vec::new(),
            events: Vec::new(),
            block_indent: None,
            in_explicit_value: false,
            map_key_indent: None,
            seq_entry_indent: None,
            tag_handles: HashMap::new(),
            saw_directive: false,
            yaml_directive_seen: false,
            depth: 0,
            max_depth,
        };
        loop {
            let cur_pos = parser.scanner.done_pos;
            parser.handle_stream()?;
            if parser.scanner.is_empty() {
                break;
            }
            if parser.scanner.done_pos == cur_pos {
                return Err(Error::new(
                    ErrorKind::Bug,
                    format!(
                        "YamlParser::parse_to_events(): dead-loop: remains \
                         {:?}",
                        parser.scanner.remains()
                    ),
                    cur_pos,
                    cur_pos,
                ));
            }
        }
        for event in &parser.events {
            log::trace!("{:?}", event);
        }

        Ok(parser.events)
    }

    /// Stream started, but not `---` or string other than `b-break` found yet.
    fn handle_stream(&mut self) -> Result<(), Error> {
        self.push_event(YamlEvent::StreamStart);
        log::trace!("handle_stream {:?}", self.scanner.remains());
        // Whether the previous document was terminated by `...` (or no
        // document has been parsed yet).
        let mut doc_terminated = true;
        // Whether at least one document has been seen in this stream
        // (never reset; used for the empty-stream check).
        let mut had_any_document = false;
        // Whether the document currently being parsed has started.
        let mut doc_started = false;
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches([' ', '\t']);
            if trimmed.is_empty() {
                self.scanner.advance_till_linebreak();
            } else if trimmed.starts_with('#') {
                // Comment lines are ignored at stream level.
                self.scanner.advance_till_linebreak();
            } else if doc_terminated
                && trimmed.starts_with('%')
                && self.handle_directive(trimmed)?
            {
                // `%YAML`, `%TAG` or a reserved directive.
            } else if trimmed == "---" {
                let indent_count =
                    line.chars().take_while(|c| *c == ' ').count();
                if doc_started {
                    // A new document invalidates the previous one's
                    // `%TAG` declarations and implicitly ends the
                    // previous document.
                    self.tag_handles.clear();
                    self.push_event(YamlEvent::DocumentEnd(
                        false,
                        self.scanner.done_pos,
                    ));
                }
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_till_linebreak();
                self.handle_node(indent_count, indent_count, None, None)?;
                doc_terminated = false;
                doc_started = true;
                had_any_document = true;
                self.saw_directive = false;
                self.yaml_directive_seen = false;
            } else if let Some(offset) =
                line.find("--- ").or_else(|| line.find("---\t"))
            {
                if doc_started {
                    self.tag_handles.clear();
                    self.push_event(YamlEvent::DocumentEnd(
                        false,
                        self.scanner.done_pos,
                    ));
                }
                // A block mapping may not start on the same line as the
                // `---` marker (YAML 1.2.2 SPEC, 9.1.2.3), e.g.
                // `--- a: b` or `--- &anchor a: b`.
                let mut rest = line[offset + 4..].trim_start_matches(' ');
                if rest.starts_with('&') || rest.starts_with('!') {
                    rest = rest
                        .split_once([' ', '\t'])
                        .map(|(_, after)| after.trim_start())
                        .unwrap_or("");
                }
                if !rest.is_empty()
                    && !rest
                        .starts_with(['\'', '"', '[', '{', '*', '|', '>', '-'])
                    && (rest.contains(": ") || rest.ends_with(':'))
                {
                    return Err(Error::new(
                        ErrorKind::InvalidImplicitKey,
                        "A block mapping may not follow the `---` marker on \
                         the same line"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                self.push_event(YamlEvent::DocumentStart(
                    true,
                    self.scanner.next_pos,
                ));
                self.scanner.advance_offset(offset + 4);
                self.handle_node(0, 0, None, None)?;
                doc_terminated = false;
                doc_started = true;
                had_any_document = true;
                self.saw_directive = false;
                self.yaml_directive_seen = false;
            } else if is_document_end_marker(trimmed) {
                if self.saw_directive && !doc_started {
                    return Err(Error::new(
                        ErrorKind::InvalidDirective,
                        "Directives must be followed by a document, but the \
                         stream ends with a document end marker"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                if doc_started {
                    self.push_event(YamlEvent::DocumentEnd(
                        true,
                        self.scanner.next_pos,
                    ));
                }
                self.scanner.advance_till_linebreak_or_space();
                doc_terminated = true;
                doc_started = false;
                self.tag_handles.clear();
                self.saw_directive = false;
                self.yaml_directive_seen = false;
            } else {
                if !doc_terminated {
                    return Err(Error::new(
                        ErrorKind::MissingDocumentEndMarkerBeforeDirective,
                        format!(
                            "Expecting document end marker `...` before new \
                             document content: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                self.push_event(YamlEvent::DocumentStart(
                    false,
                    self.scanner.next_pos,
                ));
                self.handle_node(0, 0, None, None)?;
                doc_terminated = false;
                doc_started = true;
                had_any_document = true;
                self.saw_directive = false;
                self.yaml_directive_seen = false;
            }
        }

        if self.saw_directive && !doc_started {
            return Err(Error::new(
                ErrorKind::InvalidDirective,
                "Directives must be followed by a document, but the stream \
                 ended without one"
                    .to_string(),
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }

        if !had_any_document {
            // An empty stream has no documents (e.g. empty input or a
            // bare `...`).
        } else if !doc_terminated {
            // The last document did not end with an explicit `...`.
            self.push_event(YamlEvent::DocumentEnd(
                false,
                self.scanner.done_pos,
            ));
        }
        self.push_event(YamlEvent::StreamEnd);
        Ok(())
    }

    /// Parse a `%YAML`, `%TAG` or reserved directive line. Returns
    /// `Ok(true)` when the line was a directive and has been consumed.
    fn handle_directive(&mut self, trimmed: &str) -> Result<bool, Error> {
        if let Some(rest) = trimmed.strip_prefix("%YAML") {
            if !rest.starts_with([' ', '\t']) {
                // Not `%YAML `; treat as a reserved directive.
                self.saw_directive = true;
                self.scanner.advance_till_linebreak();
                return Ok(true);
            }
            if self.yaml_directive_seen {
                return Err(self.invalid_directive(trimmed));
            }
            let rest = rest.trim_start_matches([' ', '\t']);
            let version =
                rest.split([' ', '\t']).next().unwrap_or_default().trim();
            let after = rest[version.len().min(rest.len())..].trim_start();
            if version.is_empty() || !is_valid_yaml_version(version) {
                return Err(self.invalid_directive(trimmed));
            }
            if !after.is_empty() && !after.starts_with('#') {
                return Err(self.invalid_directive(trimmed));
            }
            self.saw_directive = true;
            self.yaml_directive_seen = true;
            self.scanner.advance_till_linebreak();
            return Ok(true);
        }
        if let Some(rest) = trimmed.strip_prefix("%TAG") {
            if !rest.starts_with([' ', '\t']) {
                self.saw_directive = true;
                self.scanner.advance_till_linebreak();
                return Ok(true);
            }
            let rest = rest.trim_start_matches([' ', '\t']);
            let mut parts = rest.split_whitespace();
            let handle = parts.next().unwrap_or_default();
            let prefix = parts.next().unwrap_or_default();
            if parts.next().is_some()
                || !is_valid_tag_handle(handle)
                || prefix.is_empty()
            {
                return Err(self.invalid_directive(trimmed));
            }
            if self.tag_handles.contains_key(handle) {
                return Err(self.invalid_directive(trimmed));
            }
            self.tag_handles
                .insert(handle.to_string(), prefix.to_string());
            self.saw_directive = true;
            self.scanner.advance_till_linebreak();
            return Ok(true);
        }
        if trimmed.starts_with('%') {
            // Reserved directive: ignored with a warning (YAML 1.2.2
            // SPEC, 6.11).
            self.saw_directive = true;
            self.scanner.advance_till_linebreak();
            return Ok(true);
        }
        Ok(false)
    }

    /// Reject flow-collection lines that start with a tab followed by
    /// content or with a document marker (YAML 1.2.2 SPEC, 7.4;
    /// tabs-in-various-contexts, invalid-document-markers-in-flow-style).
    pub(crate) fn check_flow_line_start(&self) -> Result<(), Error> {
        if let Some(line) = self.scanner.remains().split(['\n', '\r']).nth(1) {
            let trimmed = line.trim_start_matches([' ', '\t']);
            if (line.starts_with('\t') && !trimmed.is_empty())
                && trimmed != "]"
                && trimmed != "}"
                || trimmed.starts_with("---")
                || trimmed.starts_with("...")
            {
                return Err(Error::new(
                    ErrorKind::InvalidStartOfToken,
                    format!(
                        "A flow collection line may not start with a tab or a \
                         document marker: {line}"
                    ),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            }
        }
        Ok(())
    }

    /// In a block collection, a flow collection entry that starts on a
    /// new line must be more indented than the enclosing block
    /// collection (YAML 1.2.2 SPEC, 7.4.1 Flow Sequences / 7.4.2 Flow
    /// Mappings).
    pub(crate) fn check_flow_entry_indentation(
        &self,
        flow_start_line: usize,
    ) -> Result<(), Error> {
        if let Some(floor) = self.block_indent {
            let pos = self.scanner.next_pos;
            log::trace!(
                "check_flow_entry_indentation: floor={floor} line={} start={} \
                 col={}",
                pos.line,
                flow_start_line,
                pos.column
            );
            if pos.line > flow_start_line && pos.column <= floor + 1 {
                return Err(Error::new(
                    ErrorKind::LessIndentedWithoutParent,
                    format!(
                        "Flow collection entry is not indented enough                          (column {} must be > {floor})",
                        pos.column
                    ),
                    pos,
                    pos,
                ));
            }
        }
        Ok(())
    }

    /// Consume comment and empty lines so that `peek_line()` returns
    /// the next content line. Comments never affect indentation.
    pub(crate) fn skip_comment_and_empty_lines(&mut self) {
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches([' ', '\t']);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                self.scanner.advance_till_linebreak();
            } else {
                break;
            }
        }
    }

    /// Skip separation (spaces and tabs) after a block indicator
    /// (`-`, `?`, `:`). When the separation contains a tab, the
    /// following content must be a flow node: a block collection
    /// indicator (`- `, `? `, `: `) or a key-looking token (ending
    /// with `:`) is rejected (yaml-test-suite:
    /// tabs-in-various-contexts).
    pub(crate) fn skip_block_indicator_separation(
        &mut self,
        mut saw_tab: bool,
    ) -> Result<(), Error> {
        while let Some(c) = self.scanner.peek_char() {
            match c {
                ' ' => {
                    self.scanner.next_char();
                }
                '\t' => {
                    saw_tab = true;
                    self.scanner.next_char();
                }
                _ => break,
            }
        }
        if !saw_tab {
            return Ok(());
        }
        let rest = self.scanner.remains();
        if tab_content_is_block_node(rest) {
            return Err(Error::new(
                ErrorKind::InvalidStartOfToken,
                "Tab(\\t) cannot be used as indentation before block node \
                 content"
                    .to_string(),
                self.scanner.next_pos,
                self.scanner.next_pos,
            ));
        }
        Ok(())
    }

    fn invalid_directive(&self, line: &str) -> Error {
        Error::new(
            ErrorKind::InvalidDirective,
            format!("Invalid directive: {line}"),
            self.scanner.next_pos,
            self.scanner.next_pos,
        )
    }

    /// Increment the node-nesting counter and fail once `max_depth` is
    /// exceeded. Every recursive "parse one more node" entry point
    /// (`handle_node`, `handle_flow_node`) must call this before doing
    /// any work; the caller is responsible for decrementing
    /// `self.depth` again once that work is done, however it returns.
    fn enter_node(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(Error::new(
                ErrorKind::RecursionLimitExceeded,
                format!(
                    "YAML node nesting exceeds the maximum supported depth of \
                     {}",
                    self.max_depth
                ),
                self.scanner.next_pos,
                self.scanner.next_pos,
            ));
        }
        Ok(())
    }

    /// Handle a container or scalar
    pub(crate) fn handle_node(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        anchor: Option<String>,
        tag: Option<String>,
    ) -> Result<(), Error> {
        let result = match self.enter_node() {
            Ok(()) => self.handle_node_impl(
                first_indent_count,
                rest_indent_count,
                anchor,
                tag,
            ),
            Err(e) => Err(e),
        };
        self.depth -= 1;
        result
    }

    fn handle_node_impl(
        &mut self,
        first_indent_count: usize,
        rest_indent_count: usize,
        mut anchor: Option<String>,
        mut tag: Option<String>,
    ) -> Result<(), Error> {
        log::trace!(
            "handle_node {} {} {:?} {:?}, {:?}",
            first_indent_count,
            rest_indent_count,
            anchor,
            tag,
            self.scanner.remains()
        );
        // Ignore less indented empty line and comment line
        while let Some(line) = self.scanner.peek_line() {
            let trimmed = line.trim_start_matches([' ', '\t']);
            let indent_count = line.chars().take_while(|c| *c == ' ').count();
            if (trimmed.is_empty() && indent_count <= first_indent_count)
                || trimmed.starts_with('#')
            {
                self.scanner.advance_till_linebreak();
                continue;
            } else {
                break;
            }
        }

        if let Some(line) = self.scanner.peek_line() {
            let indent_count = line.chars().take_while(|c| *c == ' ').count();

            if indent_count < first_indent_count {
                if self.cur_state().is_container() {
                    return Ok(());
                } else {
                    return Err(Error::new(
                        ErrorKind::LessIndentedWithoutParent,
                        format!("Less indented but without parent: {:?}", line),
                        self.scanner.next_pos,
                        {
                            self.scanner.next_line();
                            self.scanner.done_pos
                        },
                    ));
                }
            }

            let trimmed = line.trim_start_matches(' ');

            if trimmed.starts_with("- ")
                || trimmed.starts_with("-\t")
                || trimmed == "-"
            {
                if self.cur_state().is_block_map_value()
                    && self.scanner.done_pos.line == self.scanner.next_pos.line
                {
                    // YAML 1.2.2 SPEC, 8.2.1. Block Sequences:
                    //     A block sequence entry is not allowed on the
                    //     same line as a mapping key.
                    return Err(Error::new(
                        ErrorKind::InvalidSequnceStartIndicator,
                        format!(
                            "Block sequence entry is not allowed on the \
                                 same                              line as a \
                                 mapping key: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                // When the entry starts at the beginning of a line, the
                // sequence keeps that exact indentation (e.g. a mapping
                // value after an inline comment `hr: # c\n  - a`). When
                // the dash follows a parent entry on the same line (e.g.
                // `- - a`), the indentation is derived from the parent.
                let at_line_start = self.scanner.done_pos.column == 0
                    || self.scanner.done_pos.line != self.scanner.next_pos.line
                    || matches!(
                        self.scanner.peek_char(),
                        None | Some('\n') | Some('\r')
                    );
                let expected_indent_count = if at_line_start {
                    indent_count
                } else {
                    rest_indent_count + indent_count - first_indent_count
                };
                self.handle_block_seq(expected_indent_count, anchor, tag)?;
            } else if !self.cur_state().is_block_map_value()
                && (trimmed.starts_with('\'') || trimmed.starts_with('"'))
                && quoted_scalar_is_key(trimmed)
            {
                // A quoted scalar followed by `:` is a mapping key,
                // e.g. `"foo\nbar": 23`.
                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, indent_count),
                    anchor,
                    tag,
                )?;
            } else if trimmed.starts_with('\'') || trimmed.starts_with('"') {
                if quoted_scalar_is_key(trimmed)
                    && self.scanner.done_pos.line != self.scanner.next_pos.line
                {
                    // A quoted scalar used as a block mapping key on a
                    // new line, e.g. `"key1" : scalar1` as the value of
                    // `"top1" :`; a quoted scalar followed by `:` on
                    // the same line as its own key (`a: 'b': c`) is an
                    // error.
                    self.handle_block_map(
                        max(first_indent_count, indent_count),
                        max(rest_indent_count, indent_count),
                        anchor,
                        tag,
                    )?;
                } else {
                    // Flow style does not care indentation
                    self.handle_scalar(
                        first_indent_count,
                        rest_indent_count,
                        anchor,
                        tag,
                    )?;
                    // In block context a quoted scalar may only be
                    // followed by a line break, comment or EOF
                    // (rejects e.g. `a: 'b': c`).
                    self.expect_flow_end_separation()?;
                }
            } else if let Some(after_star) = trimmed.strip_prefix('*') {
                // An alias followed by `:` is a mapping key, e.g.
                // `*alias1 : scalar3`.
                let name_len = after_star
                    .find(|c: char| {
                        c.is_whitespace()
                            || matches!(c, ',' | '[' | ']' | '{' | '}')
                    })
                    .unwrap_or(after_star.len());
                let after_name = after_star[name_len..].trim_start_matches(' ');
                if after_name.starts_with(':')
                    && (after_name == ":"
                        || after_name.starts_with(": ")
                        || after_name.starts_with(":\t")
                        || after_name.starts_with(":#"))
                {
                    self.handle_block_map(
                        max(first_indent_count, indent_count),
                        max(rest_indent_count, indent_count),
                        anchor,
                        tag,
                    )?;
                } else {
                    self.scanner.advance(indent_count);
                    let name = self.handle_alias()?;
                    self.push_event(YamlEvent::Alias(
                        name,
                        self.scanner.next_pos,
                    ));
                    // Consume a trailing comment on the alias's own line
                    // (e.g. `- *a # comment`) so the enclosing block
                    // collection sees the next line.
                    self.skip_comment_and_empty_lines();
                }
            } else if trimmed.starts_with("[") || trimmed.starts_with("{") {
                if flow_collection_is_key(trimmed) {
                    // A flow collection used as a block mapping key,
                    // e.g. `[flow]: block` or `{a: b}: value`.
                    self.handle_block_map(
                        max(first_indent_count, indent_count),
                        max(rest_indent_count, indent_count),
                        anchor,
                        tag,
                    )?;
                } else {
                    // Flow style does not care about indentation, but
                    // the scanner may still sit at the line's leading
                    // spaces (e.g. `  [1, 2]`).
                    self.scanner.advance(indent_count);
                    if trimmed.starts_with("[") {
                        self.handle_flow_seq(anchor, tag)?;
                    } else {
                        self.handle_flow_map(anchor, tag)?;
                    }
                    self.expect_flow_end_separation()?;
                }
            } else if find_key_value_separator(trimmed).is_some() {
                if self.cur_state().is_block_map_value()
                    && self.scanner.done_pos.line == self.scanner.next_pos.line
                    && !self.in_explicit_value
                {
                    // A block mapping may not be a value on the same
                    // line as its own key (YAML 1.2.2 SPEC, 8.2.2),
                    // e.g. `a: b: c: d` is an error.
                    return Err(Error::new(
                        ErrorKind::InvalidImplicitKey,
                        format!(
                            "A block mapping cannot be the value of a mapping \
                             entry on the same line: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                // Guess out the indent: the mapping's entries sit at
                // the key's own column. For a key that follows content
                // on the same line (e.g. a block sequence entry
                // `- key: value`, where the key sits at column 3) that
                // is the scanner's column; for a key at the start of a
                // new line it is the line's indentation.
                let key_column = if self.scanner.done_pos.line
                    == self.scanner.next_pos.line
                {
                    self.scanner.next_pos.column.saturating_sub(1)
                } else {
                    indent_count
                };
                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, key_column),
                    anchor,
                    tag,
                )?;
            } else if trimmed == "?"
                || trimmed.starts_with("? ")
                || trimmed.starts_with("?\t")
            {
                // Explicit mapping keys, e.g. `? key`.
                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, indent_count),
                    anchor,
                    tag,
                )?;
            } else if trimmed.ends_with(":") {
                // A block mapping key ending with `:`. Guess out the
                // indent: the mapping's entries sit at the key's own
                // column. For a key that follows content on the same
                // line (e.g. a block sequence entry `- key:` where the
                // key sits at column 3) that is the scanner's column,
                // for a key at the start of a new line it is the
                // line's indentation.
                let key_column = if self.scanner.done_pos.line
                    == self.scanner.next_pos.line
                {
                    self.scanner.next_pos.column.saturating_sub(1)
                } else {
                    indent_count
                };
                self.handle_block_map(
                    max(first_indent_count, indent_count),
                    max(rest_indent_count, key_column),
                    anchor,
                    tag,
                )?;
            } else if trimmed.starts_with('!') || trimmed.starts_with('&') {
                self.scanner.advance(indent_count);
                // YAML 1.2.2 SPEC, 6.9. Node Properties:
                //      Node properties may appear in any order, but each
                //      at most once.

                //      Node properties may appear in any order, but each
                //      at most once.
                let mut property_found = false;
                while let Some(property_line) = self.scanner.peek_line() {
                    let property_trimmed =
                        property_line.trim_start_matches(' ');
                    let property_indent =
                        property_line.chars().take_while(|c| *c == ' ').count();
                    // Node properties of a mapping value on a new line
                    // must be indented deeper than the mapping; a line
                    // at the same indentation belongs to the parent
                    // (e.g. `key: &x\n!!map` — the `!!map` is a new
                    // key).
                    if self.cur_state().is_block_map_value()
                        && self.scanner.done_pos.line
                            != self.scanner.next_pos.line
                        && let Some(floor) = self.block_indent
                        && property_indent <= floor
                    {
                        break;
                    }
                    // A line whose first two tokens are node properties
                    // and which contains `: ` is an implicit mapping key
                    // of this node, not another property (e.g.
                    // `&a !!str key: v`, `!!str &a key: v`).
                    let first_token = property_trimmed
                        .split([' ', '\t'])
                        .next()
                        .unwrap_or("");
                    let is_prop_first = first_token.starts_with('!')
                        || first_token.starts_with('&');
                    let rest_after_first = property_trimmed
                        [first_token.len()..]
                        .trim_start_matches([' ', '\t']);
                    let second_token = rest_after_first
                        .split([' ', '\t'])
                        .next()
                        .unwrap_or("");
                    let is_prop_second = second_token.starts_with('!')
                        || second_token.starts_with('&');
                    if is_prop_first
                        && is_prop_second
                        && (property_trimmed.contains(": ")
                            || property_trimmed.ends_with(':'))
                    {
                        property_found = true;
                        break;
                    }
                    if property_trimmed.starts_with('&') {
                        if anchor.is_none() {
                            self.scanner.advance(property_indent);
                            anchor = Some(self.handle_anchor()?);
                            property_found = true;
                        } else if let Some(after_anchor) =
                            property_trimmed.split_once([' ', '\t'])
                        {
                            // A second anchor is allowed when it belongs
                            // to an implicit mapping key of the anchored
                            // node, e.g.
                            //      &node
                            //      &key key: value
                            let after_anchor =
                                after_anchor.1.trim_start_matches(' ');
                            if after_anchor.contains(": ")
                                || after_anchor.ends_with(':')
                            {
                                property_found = true;
                                break;
                            } else {
                                return Err(Error::new(
                                    ErrorKind::InvalidAnchor,
                                    format!(
                                        "Node can have at most one anchor, \
                                         but got: {property_line}"
                                    ),
                                    self.scanner.next_pos,
                                    self.scanner.next_pos,
                                ));
                            }
                        } else {
                            return Err(Error::new(
                                ErrorKind::InvalidAnchor,
                                format!(
                                    "Node can have at most one anchor, but \
                                     got: {property_line}"
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    } else if property_trimmed.starts_with('!') {
                        if tag.is_none() {
                            self.scanner.advance(property_indent);
                            // Tag decorating its container
                            tag = self.handle_tag()?;
                            property_found = true;
                        } else {
                            return Err(Error::new(
                                ErrorKind::InvalidAnchor,
                                format!(
                                    "Node can have at most one tag, but got: \
                                     {property_line}"
                                ),
                                self.scanner.next_pos,
                                self.scanner.next_pos,
                            ));
                        }
                    } else {
                        break;
                    }
                }
                if !property_found {
                    return Err(Error::new(
                        ErrorKind::InvalidAnchor,
                        format!(
                            "Node can have at most one anchor and one tag, \
                             but got: {line}"
                        ),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                if self.scanner.done_pos.line == self.scanner.next_pos.line
                    && let Some(content_line) = self.scanner.peek_line()
                {
                    let content_trimmed = content_line.trim_start_matches(' ');
                    if content_trimmed.starts_with("- ")
                        || content_trimmed == "-"
                    {
                        return Err(Error::new(
                            ErrorKind::InvalidAnchor,
                            format!(
                                "Node property cannot be placed before a \
                                 block sequence entry on the same line: \
                                 {content_line}"
                            ),
                            self.scanner.next_pos,
                            self.scanner.next_pos,
                        ));
                    }
                }
                // The node properties may be followed by the content on
                // the same line or on a following line. When the
                // properties ended at a line break, the content may sit
                // at a different indentation than the properties (e.g.
                // `!foo\n>1`), so re-derive the indentation from the
                // content line. The content line must be more indented
                // than the enclosing block collection, otherwise the
                // properties decorate an empty node and the next line
                // belongs to the parent (e.g. `- &a\n- b`).
                self.skip_comment_and_empty_lines();
                let content_more_indented = self
                    .block_indent
                    .map(|indent| {
                        self.scanner
                            .peek_line()
                            .map(|l| {
                                let l_indent =
                                    l.chars().take_while(|c| *c == ' ').count();
                                l_indent > indent
                                    // A zero-indented block sequence is a
                                    // valid mapping value (e.g.
                                    // `seq:\n &a\n- 1`), so the properties
                                    // decorate the sequence.
                                    || (self.cur_state().is_block_map_value()
                                        && l_indent == indent
                                        && l.trim_start_matches(' ')
                                            .starts_with("- "))
                            })
                            .unwrap_or(true)
                    })
                    .unwrap_or(true);
                if content_more_indented
                    && self.scanner.done_pos.line != self.scanner.next_pos.line
                    && let Some(content_line) = self.scanner.peek_line()
                {
                    let content_indent =
                        content_line.chars().take_while(|c| *c == ' ').count();
                    self.handle_node(
                        content_indent,
                        content_indent,
                        anchor,
                        tag,
                    )?;
                } else if self.scanner.done_pos.line
                    == self.scanner.next_pos.line
                    && !matches!(
                        self.scanner.peek_char(),
                        None | Some('\n') | Some('\r')
                    )
                {
                    // The content follows the node properties on the
                    // same line (e.g. `!<!bar> baz`); it is a same-line
                    // value starting at the current column.
                    self.handle_node(
                        0,
                        self.scanner.done_pos.column,
                        anchor,
                        tag,
                    )?;
                } else {
                    // The node properties decorate an empty node: the
                    // next line belongs to the parent context (e.g.
                    // `- &a\n- a`), so emit an empty scalar.
                    self.push_event(YamlEvent::Scalar(
                        anchor,
                        tag,
                        String::new(),
                        YamlScalarStyle::Plain,
                        self.scanner.done_pos,
                        self.scanner.done_pos,
                    ));
                }
            } else if is_document_end_marker(trimmed) {
                // Document end marker with an empty document: emit an
                // empty scalar node (the stream handler emits the
                // `DocumentEnd`).
                self.push_event(YamlEvent::Scalar(
                    None,
                    None,
                    String::new(),
                    YamlScalarStyle::Plain,
                    self.scanner.done_pos,
                    self.scanner.done_pos,
                ));
                return Ok(());
            } else if trimmed.starts_with('%') {
                return Err(Error::new(
                    ErrorKind::MissingDocumentEndMarkerBeforeDirective,
                    format!(
                        "Directive is only allowed before document start \
                         marker `---` or after document end marker `...`, but \
                         got: {line}"
                    ),
                    self.scanner.next_pos,
                    self.scanner.next_pos,
                ));
            } else if line.trim_start_matches(' ').starts_with('\t') {
                // A tab used as indentation is not allowed, except when
                // the node is a flow collection (self-delimiting) or a
                // plain scalar: only block-node-looking content (block
                // indicators or key-looking tokens) is rejected.
                let after_tabs = line.trim_start_matches([' ', '\t']);
                if tab_content_is_block_node(after_tabs) {
                    return Err(Error::new(
                        ErrorKind::InvalidStartOfToken,
                        "Tab(\\t) cannot be used as start of any YAML node"
                            .to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                let leading = line
                    .chars()
                    .take_while(|c| matches!(c, ' ' | '\t'))
                    .count();
                self.scanner.advance(leading);
                // Parse the content as a fresh node at the current
                // scanner position (the original indentation arguments
                // describe the line start, not the content after the
                // tabs).
                self.handle_node(0, 0, anchor, tag)?;
            } else {
                self.handle_scalar(
                    first_indent_count,
                    rest_indent_count,
                    anchor,
                    tag,
                )?;
            }
        } else {
            // No content: an empty scalar node (e.g. an empty document
            // or standalone node properties at EOF).
            self.push_event(YamlEvent::Scalar(
                anchor,
                tag,
                String::new(),
                YamlScalarStyle::Plain,
                self.scanner.done_pos,
                self.scanner.done_pos,
            ));
        }
        Ok(())
    }

    /// After a flow collection in block context, only spaces, a line
    /// break, a comment or EOF may follow.
    fn expect_flow_end_separation(&mut self) -> Result<(), Error> {
        let mut saw_space = false;
        while self.scanner.peek_char() == Some(' ') {
            saw_space = true;
            self.scanner.next_char();
        }
        match self.scanner.peek_char() {
            None | Some('\n') | Some('\r') => Ok(()),
            // The comment must be separated from the closing indicator
            // by a space (`]#comment` is an error).
            Some('#') if saw_space => Ok(()),
            Some(c) => Err(Error::new(
                ErrorKind::UnexpectedYamlNodeType,
                format!(
                    "Expecting a line break or comment after a flow \
                     collection, but got '{c}'"
                ),
                self.scanner.next_pos,
                self.scanner.next_pos,
            )),
        }
    }

    /// Handle a node inside a flow collection. The entry terminators
    /// (`,` plus `]` or `}`) are handled by the caller.
    pub(crate) fn handle_flow_node(&mut self) -> Result<(), Error> {
        let result = match self.enter_node() {
            Ok(()) => self.handle_flow_node_impl(),
            Err(e) => Err(e),
        };
        self.depth -= 1;
        result
    }

    fn handle_flow_node_impl(&mut self) -> Result<(), Error> {
        self.scanner.skip_flow_separation();
        // YAML 1.2.2 SPEC, 6.9. Node Properties:
        //      Node properties may appear in any order, but each at
        //      most once.
        let mut anchor = None;
        let mut tag = None;
        loop {
            match self.scanner.peek_char() {
                Some('&') if anchor.is_none() => {
                    anchor = Some(self.handle_anchor()?);
                    self.scanner.skip_flow_separation();
                }
                Some('!') if tag.is_none() => {
                    tag = self.handle_tag()?;
                    self.scanner.skip_flow_separation();
                }
                _ => break,
            }
        }
        match self.scanner.peek_char() {
            Some('[') => self.handle_flow_seq(anchor, tag)?,
            Some('{') => self.handle_flow_map(anchor, tag)?,
            Some('*') => {
                if anchor.is_some() || tag.is_some() {
                    return Err(Error::new(
                        ErrorKind::InvalidAnchor,
                        "Alias cannot carry node properties".to_string(),
                        self.scanner.next_pos,
                        self.scanner.next_pos,
                    ));
                }
                let name = self.handle_alias()?;
                self.push_event(YamlEvent::Alias(name, self.scanner.next_pos));
            }
            Some('"') => {
                self.handle_double_quoted_flow_scalar(anchor, tag, 0)?
            }
            Some('\'') => self.handle_single_quoted_flow_scalar(anchor, tag)?,
            Some(',') | Some(']') | Some('}') | None => {
                // Empty node, e.g. an omitted value in a flow mapping.
                let pos = self.scanner.next_pos;
                self.push_event(YamlEvent::Scalar(
                    anchor,
                    tag,
                    String::new(),
                    YamlScalarStyle::Plain,
                    pos,
                    pos,
                ));
            }
            Some(_) => self.handle_flow_plain_scalar(anchor, tag)?,
        }
        Ok(())
    }
}

/// Whether a line starting with a quoted scalar is followed by `:`,
/// i.e. the quoted scalar is a mapping key (`"foo": 23`) rather than a
/// standalone scalar (`"foo"`).
pub(crate) fn quoted_scalar_is_key(trimmed: &str) -> bool {
    let Some(quote) = trimmed.chars().next() else {
        return false;
    };
    if quote != '\'' && quote != '"' {
        return false;
    }
    let mut escaped = false;
    for (i, c) in trimmed.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quote == '"' {
            escaped = true;
            continue;
        }
        if c == quote {
            let rest = &trimmed[i + c.len_utf8()..];
            return rest.trim_start_matches(' ').starts_with(':');
        }
    }
    false
}

/// Whether a trimmed line is a flow collection used as a block
/// mapping key, e.g. `[flow]: block` or `{a: b}: value`: the line
/// starts with `[`/`{` and a `:` key separator follows the matching
/// closing indicator.
pub(crate) fn flow_collection_is_key(trimmed: &str) -> bool {
    let mut depth = 0usize;
    let mut in_squote = false;
    let mut in_dquote = false;
    let mut escaped = false;
    for (i, c) in trimmed.char_indices() {
        if in_dquote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_dquote = false;
            }
            continue;
        }
        if in_squote {
            if c == '\'' {
                in_squote = false;
            }
            continue;
        }
        match c {
            '"' => in_dquote = true,
            '\'' => in_squote = true,
            '[' | '{' => depth += 1,
            ']' | '}' => {
                if depth == 1 {
                    // Check for a `:` key separator right after the
                    // matching closing indicator.
                    let after: &str =
                        trimmed[i + c.len_utf8()..].trim_start_matches(' ');
                    return after == ":"
                        || after.starts_with(": ")
                        || after.starts_with(":\t")
                        || after.starts_with(":#");
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    false
}

/// Find the `:` that separates an implicit mapping key from its value
/// (`: ` or `:\t`); `None` when the line has no key/value separator.
/// A trailing `:` (key with no value) is handled separately by the
/// callers.
pub(crate) fn find_key_value_separator(line: &str) -> Option<usize> {
    line.find(": ").or_else(|| line.find(":\t"))
}

/// Whether a trimmed line is a document start marker `---`, optionally
/// followed by separation (space or tab), a comment or nothing
/// (YAML 1.2.2 SPEC, 6.3 / 9.1).
pub(crate) fn is_document_start_marker(trimmed: &str) -> bool {
    if trimmed == "---" {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("---") {
        return rest.starts_with([' ', '\t']) || rest.starts_with('#');
    }
    false
}

/// Find the start of an inline comment: a `#` preceded by a space or
/// a tab (YAML 1.2.2 SPEC, 7.1).
pub(crate) fn find_comment_start(line: &str) -> Option<usize> {
    let mut prev = '\0';
    for (i, c) in line.char_indices() {
        if c == '#' && matches!(prev, ' ' | '\t') {
            return Some(i);
        }
        prev = c;
    }
    None
}

/// Whether content that follows a tab separation looks like a block
/// node and is therefore invalid: a block collection indicator
/// (`- `, `? `, `: `) or a key-looking token ending with `:`
/// (yaml-test-suite: tabs-in-various-contexts,
/// tabs-that-look-like-indentation).
pub(crate) fn tab_content_is_block_node(content: &str) -> bool {
    let mut chars = content.chars();
    let Some(c) = chars.next() else {
        return false;
    };
    let second = chars.next();
    if matches!(c, '-' | '?' | ':')
        && matches!(
            second,
            None | Some(' ') | Some('\t') | Some('\n') | Some('\r')
        )
    {
        return true;
    }
    let token: String = content
        .chars()
        .take_while(|c| !matches!(c, ' ' | '\t' | '\n' | '\r'))
        .collect();
    token.ends_with(':')
}

/// Whether a trimmed line is a document end marker `...`, optionally
/// followed by a comment (YAML 1.2.2 SPEC, 9.3.2.3).
pub(crate) fn is_document_end_marker(trimmed: &str) -> bool {
    if trimmed == "..." {
        return true;
    }
    if let Some(rest) = trimmed.strip_prefix("...") {
        return rest.starts_with([' ', '\t'])
            && rest.trim_start_matches([' ', '\t']).starts_with('#');
    }
    false
}

/// Whether `s` looks like a YAML version `major.minor` with digit
/// components (any version is accepted, like libyaml).
fn is_valid_yaml_version(s: &str) -> bool {
    let Some((major, minor)) = s.split_once('.') else {
        return false;
    };
    !major.is_empty()
        && !minor.is_empty()
        && major.chars().all(|c| c.is_ascii_digit())
        && minor.chars().all(|c| c.is_ascii_digit())
}

/// Whether `s` is a valid tag handle: `!`, `!!`, or `!name!`.
fn is_valid_tag_handle(s: &str) -> bool {
    if s == "!" || s == "!!" {
        return true;
    }
    if let Some(name) = s.strip_prefix('!').and_then(|r| r.strip_suffix('!')) {
        return !name.is_empty()
            && !name.contains([' ', '\t', ',', '[', ']', '{', '}']);
    }
    false
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::YamlPosition;

    #[test]
    fn test_document_explcitly_start() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n---").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::Scalar(
                    None,
                    None,
                    String::new(),
                    YamlScalarStyle::Plain,
                    YamlPosition::new(3, 3),
                    YamlPosition::new(3, 3)
                ),
                YamlEvent::DocumentEnd(false, YamlPosition::new(3, 3)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_document_explcitly_start_and_end() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n---\na\n...").unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::Scalar(
                    None,
                    None,
                    "a".to_string(),
                    YamlScalarStyle::Plain,
                    YamlPosition::new(4, 1),
                    YamlPosition::new(4, 1)
                ),
                YamlEvent::DocumentEnd(true, YamlPosition::new(5, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_document_with_comment() {
        assert_eq!(
            YamlParser::parse_to_events("\n\r\n--- # test command\n...")
                .unwrap(),
            vec![
                YamlEvent::StreamStart,
                YamlEvent::DocumentStart(true, YamlPosition::new(3, 1)),
                YamlEvent::Scalar(
                    None,
                    None,
                    String::new(),
                    YamlScalarStyle::Plain,
                    YamlPosition::new(3, 19),
                    YamlPosition::new(3, 19)
                ),
                YamlEvent::DocumentEnd(true, YamlPosition::new(4, 1)),
                YamlEvent::StreamEnd,
            ]
        )
    }

    #[test]
    fn test_alias_with_trailing_comment_in_sequence() {
        // A trailing comment on the alias's own line must not end the
        // enclosing block sequence (yaml-test-suite:
        // spec-example-2-10-node-for-sammy-sosa-appears-twice-in-this-document).
        let events =
            YamlParser::parse_to_events("- *a # comment\n- b\n").unwrap();
        let mut events_str = String::new();
        for event in events {
            events_str.push_str(&event.to_string());
            events_str.push('\n');
        }
        assert_eq!(
            "+STR\n+DOC\n+SEQ\n=ALI *a\n=VAL :b\n-SEQ\n-DOC\n-STR\n",
            events_str
        );
    }
}
