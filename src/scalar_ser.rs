// SPDX-License-Identifier: Apache-2.0

/// Prefer unquoted string and use double quoted string if any of below:
///     * Line is longer than `max_width`
///     * Has non-printable character
///     * Has NS_ESC_XXX characters
///     * Would be resolved to a non-string scalar (bool, null, number)
pub(crate) fn to_scalar_string(
    indent_count: usize,
    input: &str,
    max_width: usize,
) -> String {
    if needs_double_quote(input) {
        return format!("\"{}\"", escape_double_quoted(input));
    }
    if indent_count + input.chars().count() < max_width {
        input.to_string()
    } else {
        // Break the long line with double-quoted folding: each folded
        // line is indented by one space relative to the opening quote.
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut current_width = 0usize;
        let limit = max_width.saturating_sub(indent_count + 2).max(1);
        for word in input.split(' ') {
            if !current.is_empty()
                && current_width + 1 + word.chars().count() > limit
            {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if !current.is_empty() {
                current.push(' ');
                current_width += 1;
            }
            current.push_str(word);
            current_width += word.chars().count();
        }
        lines.push(current);
        format!("\"{}\"", lines.join("\n "))
    }
}

/// Whether the string cannot be rendered as a plain scalar and must be
/// double quoted (YAML 1.2.2 SPEC, 7.3.3 Plain Style / 7.3.4.2
/// Double-Quoted Style).
fn needs_double_quote(input: &str) -> bool {
    if input.is_empty() {
        return true;
    }
    if input.starts_with("---") || input.starts_with("...") {
        return true;
    }
    let first = input.chars().next().unwrap();
    if matches!(
        first,
        '-' | '?'
            | ':'
            | ','
            | '['
            | ']'
            | '{'
            | '}'
            | '#'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '\''
            | '"'
            | '%'
            | '@'
            | '`'
            | ' '
            | '\t'
    ) {
        return true;
    }
    input.contains(": ")
        || input.contains(" #")
        || input.contains('"')
        || input.contains('\\')
        || input
            .chars()
            .any(|c| c.is_control() || is_yaml_line_break(c))
        || looks_like_non_string(input)
}

/// Whether the character is a YAML line break (`b-char`), which must not
/// appear literally in a double-quoted scalar as it would be folded.
fn is_yaml_line_break(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Whether the plain rendering of the string would be resolved to a
/// non-string scalar by the YAML Core Schema (bool, null, integer or
/// float), which would break string round-trip fidelity.
fn looks_like_non_string(input: &str) -> bool {
    match input {
        "~" | "null" | "Null" | "NULL" => return true,
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE" => {
            return true;
        }
        _ => {}
    }
    let unsigned = input.trim_start_matches(['-', '+']);
    if !unsigned.is_empty() && unsigned.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if unsigned.starts_with("0x")
        || unsigned.starts_with("0o")
        || unsigned.starts_with("0b")
    {
        return true;
    }
    if matches!(
        input,
        ".inf" | ".Inf" | ".INF" | "-.inf" | "-.Inf" | "-.INF"
    ) || matches!(input, ".nan" | ".NaN" | ".NAN")
    {
        return true;
    }
    input.parse::<f64>().is_ok()
}

/// Escape a string for the double-quoted style. The escapes follow
/// YAML 1.2.2 SPEC, 7.3.4.2 (NS_ESC_* productions). Non-ASCII
/// characters are escaped as `\uXXXX` (the generator that produced
/// the `out.yaml` test files did not allow unicode).
pub(crate) fn escape_double_quoted(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\0' => out.push_str("\\0"),
            '\u{07}' => out.push_str("\\a"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0b}' => out.push_str("\\v"),
            '\u{0c}' => out.push_str("\\f"),
            '\u{1b}' => out.push_str("\\e"),
            '\u{85}' => out.push_str("\\x85"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 || (0x7f..=0x9f).contains(&(c as u32)) => {
                out.push_str(&format!("\\x{:02X}", c as u32));
            }
            c if (c as u32) > 0xFF => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Scalar rendering for the `to_yaml` workflow (parsed `YamlValue` dump).
//
// The conventions follow the yaml-test-suite `out.yaml` files: plain
// scalars are preferred (including number/bool/null-looking text),
// then single-quoted, then double-quoted. Empty scalars stay empty.
// Non-ASCII is escaped (the generator that produced `out.yaml` did not
// allow unicode in scalar values).
// ---------------------------------------------------------------------------

/// Render a scalar value for the `to_yaml` dump.
pub(crate) fn to_out_yaml_scalar(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    if plain_safe(input) {
        return input.to_string();
    }
    if single_quote_safe(input) {
        return format!("'{}'", input.replace('\'', "''"));
    }
    format!("\"{}\"", escape_double_quoted(input))
}

/// Render a plain (or style-less) scalar for the `to_yaml` dump: a
/// single-line value keeps the plain/single/double choice of
/// [`to_out_yaml_scalar`]; a multiline value becomes a single-quoted
/// multiline scalar (the `out.yaml` convention, e.g.
/// `spec-example-7-12-plain-lines`).
pub(crate) fn to_out_yaml_scalar_plain(input: &str) -> String {
    if input.contains('\n') {
        to_out_yaml_scalar_sq(input, true)
    } else {
        to_out_yaml_scalar(input)
    }
}

/// Render a single-quoted scalar, keeping the style. Continuation
/// lines of a multiline value are indented by two spaces (unless
/// `indent_continuation` is false, used for keys).
pub(crate) fn to_out_yaml_scalar_sq(
    input: &str,
    indent_continuation: bool,
) -> String {
    if !input.contains('\n') {
        return format!("'{}'", input.replace('\'', "''"));
    }
    let mut out = String::from("'");
    let lines: Vec<&str> = input.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && indent_continuation && !line.is_empty() {
            out.push_str("  ");
        }
        out.push_str(&line.replace('\'', "''"));
        if i + 1 < lines.len() {
            out.push('\n');
        }
    }
    out.push('\'');
    out
}

/// Whether the string can be rendered as a plain (unquoted) scalar
/// (YAML 1.2.2 SPEC, 7.3.3 Plain Style), matching `out.yaml`.
fn plain_safe(input: &str) -> bool {
    if input.is_empty() || input.starts_with("---") || input.starts_with("...")
    {
        return false;
    }
    let mut chars = input.char_indices().peekable();
    let (_, first) = chars.next().unwrap();
    if matches!(
        first,
        ',' | '['
            | ']'
            | '{'
            | '}'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '\''
            | '"'
            | '%'
            | '@'
            | '`'
    ) {
        return false;
    }
    if matches!(first, '-' | '?' | ':') {
        // Only a block indicator when followed by blank or end.
        if input.len() == first.len_utf8()
            || is_blank(input.as_bytes()[first.len_utf8()] as char)
        {
            return false;
        }
    }
    if first == ' ' || first == '\t' {
        return false;
    }
    for (i, c) in input.char_indices() {
        if c.is_control() || is_yaml_line_break(c) || c == '\t' {
            return false;
        }
        if !c.is_ascii() {
            return false;
        }
        if c == ':' {
            let next = input[i + 1..].chars().next();
            if next.is_none_or(|n| n == ' ' || is_yaml_line_break(n)) {
                return false;
            }
        }
        if c == '#' && i > 0 {
            let prev = input[..i].chars().next_back().unwrap();
            if prev == ' ' || prev == '\t' {
                return false;
            }
        }
    }
    let last = input.chars().next_back().unwrap();
    if last == ' ' || last == '\t' {
        return false;
    }
    true
}

/// Whether the string can be rendered as a single-quoted scalar.
fn single_quote_safe(input: &str) -> bool {
    input.chars().all(|c| {
        c.is_ascii()
            && !c.is_control()
            && !matches!(c, '\u{85}' | '\u{2028}' | '\u{2029}')
    })
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\t'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_scalar_kept_as_is() {
        assert_eq!(to_scalar_string(0, "hello world", 80), "hello world");
        assert_eq!(to_scalar_string(0, "forty-two", 80), "forty-two");
    }

    #[test]
    fn test_empty_and_whitespace_strings_are_quoted() {
        assert_eq!(to_scalar_string(0, "", 80), "\"\"");
        assert_eq!(to_scalar_string(0, "  ", 80), "\"  \"");
    }

    #[test]
    fn test_ambiguous_strings_are_quoted() {
        assert_eq!(to_scalar_string(0, "a: b", 80), "\"a: b\"");
        assert_eq!(to_scalar_string(0, "a # b", 80), "\"a # b\"");
        assert_eq!(to_scalar_string(0, "- item", 80), "\"- item\"");
        assert_eq!(to_scalar_string(0, "---", 80), "\"---\"");
    }

    #[test]
    fn test_non_string_lookalikes_are_quoted() {
        assert_eq!(to_scalar_string(0, "42", 80), "\"42\"");
        assert_eq!(to_scalar_string(0, "-1.5", 80), "\"-1.5\"");
        assert_eq!(to_scalar_string(0, "0x1F", 80), "\"0x1F\"");
        assert_eq!(to_scalar_string(0, "true", 80), "\"true\"");
        assert_eq!(to_scalar_string(0, "null", 80), "\"null\"");
        assert_eq!(to_scalar_string(0, "~", 80), "\"~\"");
        assert_eq!(to_scalar_string(0, ".inf", 80), "\".inf\"");
    }

    #[test]
    fn test_special_characters_are_escaped() {
        assert_eq!(to_scalar_string(0, "a\nb", 80), "\"a\\nb\"");
        assert_eq!(to_scalar_string(0, "a\tb", 80), "\"a\\tb\"");
        assert_eq!(to_scalar_string(0, "a\"b", 80), "\"a\\\"b\"");
        assert_eq!(to_scalar_string(0, "a\\b", 80), "\"a\\\\b\"");
        assert_eq!(to_scalar_string(0, "a\u{1b}b", 80), "\"a\\eb\"");
        assert_eq!(to_scalar_string(0, "a\u{01}b", 80), "\"a\\x01b\"");
        assert_eq!(to_scalar_string(0, "a\u{85}b", 80), "\"a\\x85b\"");
        assert_eq!(to_scalar_string(0, "a\u{2028}b", 80), "\"a\\u2028b\"");
    }

    #[test]
    fn test_long_line_is_folded() {
        let long = "word ".repeat(30) + "end";
        let rendered = to_scalar_string(0, &long, 40);
        assert!(rendered.starts_with('"'));
        assert!(rendered.ends_with('"'));
        assert!(rendered.contains('\n'));
        for line in rendered.lines() {
            assert!(line.chars().count() <= 42);
        }
    }
}
