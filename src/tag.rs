// SPDX-License-Identifier: Apache-2.0

use crate::{Error, ErrorKind, ValueData, YamlParser};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct YamlTag {
    pub name: String,
    pub data: ValueData,
}

impl<'a> YamlParser<'a> {
    /// Parse a tag shorthand and resolve it against the `%TAG`
    /// directives of the current document (YAML 1.2.2 SPEC, 6.18
    /// Tag Shorthands):
    ///
    /// * `!<verbatim>` is taken as-is;
    /// * `!!suffix` uses the `!!` handle (default prefix `tag:yaml.org,2002:`);
    /// * `!suffix` uses the `!` handle (default local prefix `!`);
    /// * `!name!suffix` uses the `!name!` handle, which must be declared by a
    ///   `%TAG` directive.
    ///
    /// The returned tag string is `name` wrapped in `<...>`, matching
    /// the event representation used by the yaml-test-suite.
    pub(crate) fn handle_tag(&mut self) -> Result<Option<String>, Error> {
        // Scan the tag token, stopping at separation characters (a
        // space, line break or a flow indicator like `,`). A verbatim
        // tag `!<...>` may contain flow indicators inside the brackets.
        let mut tag_name = String::new();
        let mut in_verbatim = false;
        while let Some(c) = self.scanner.peek_char() {
            if !in_verbatim && tag_name.starts_with("!<") {
                in_verbatim = true;
            }
            if !in_verbatim
                && (c.is_whitespace() || matches!(c, ',' | ']' | '}'))
            {
                break;
            }
            tag_name.push(c);
            self.scanner.next_char();
            if in_verbatim && c == '>' {
                break;
            }
        }
        // Consume the separating character (space or line break)
        // following the tag token.
        if matches!(
            self.scanner.peek_char(),
            Some(' ') | Some('\t') | Some('\n') | Some('\r')
        ) {
            self.scanner.next_char();
        }

        let Some(rest) = tag_name.strip_prefix('!') else {
            log::trace!("Unknown tag {tag_name}");
            return Ok(None);
        };

        // Verbatim tag `!<...>`.
        if let Some(inner) = rest.strip_prefix('<') {
            let Some(inner) = inner.strip_suffix('>') else {
                return Err(self.invalid_tag(&tag_name));
            };
            if inner.is_empty() || inner.contains([' ', '\t']) {
                return Err(self.invalid_tag(&tag_name));
            }
            return Ok(Some(format!("<{inner}>")));
        }

        // Split into handle and suffix. The handle is everything up to
        // and including the second `!` (for `!!suffix` and `!name!suffix`).
        let (handle, suffix) = if let Some(idx) = rest.find('!') {
            let mut handle = String::with_capacity(idx + 2);
            handle.push('!');
            handle.push_str(&rest[..idx]);
            handle.push('!');
            (handle, &rest[idx + 1..])
        } else {
            ("!".to_string(), rest)
        };

        if suffix.is_empty() && handle != "!" {
            // A lone `!` is the valid non-specific tag; an empty
            // `!!` or `!name!` shorthand is not.
            return Err(self.invalid_tag(&tag_name));
        }
        if !suffix.is_empty() && !is_valid_tag_suffix(suffix) {
            return Err(self.invalid_tag(&tag_name));
        }

        let prefix = match self.tag_handles.get(&handle) {
            Some(prefix) => prefix.clone(),
            None => match handle.as_str() {
                "!!" => "tag:yaml.org,2002:".to_string(),
                "!" => "!".to_string(),
                _ => {
                    // A named handle requires a `%TAG` directive.
                    return Err(self.invalid_tag(&tag_name));
                }
            },
        };

        Ok(Some(format!("<{prefix}{}>", decode_percent(suffix))))
    }

    fn invalid_tag(&self, tag: &str) -> Error {
        Error::new(
            ErrorKind::InvalidTag,
            format!("Invalid tag: {tag}"),
            self.scanner.done_pos,
            self.scanner.done_pos,
        )
    }
}

/// Characters allowed in a tag suffix (c-tag-char excludes the flow
/// indicators and separation spaces).
fn is_valid_tag_suffix(s: &str) -> bool {
    !s.contains([' ', '\t', ',', '[', ']', '{', '}'])
}

/// Decode `%XX` URI escapes in a tag suffix (e.g. `%21` -> `!`).
fn decode_percent(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2
                && hex.chars().all(|h| h.is_ascii_hexdigit())
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                out.push(byte as char);
                continue;
            }
            out.push('%');
            out.push_str(&hex);
        } else {
            out.push(c);
        }
    }
    out
}
