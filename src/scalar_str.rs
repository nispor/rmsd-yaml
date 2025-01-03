// SPDX-License-Identifier: Apache-2.0

use crate::{CharsIter, RmsdError, RmsdPosition, YAML_CHAR_INDICATORS};

const YAML_CHAR_ESCAPE: char = '\\';

/// Read till reach another `"`
/// The starting " has already removed from CharsIter.
pub(crate) fn read_double_quoted_str(
    iter: &mut CharsIter,
) -> Result<String, RmsdError> {
    let mut ret = String::new();

    while let Some(c) = iter.next() {
        if c == '"' {
            return Ok(ret);
        } else if c == YAML_CHAR_ESCAPE {
            ret.push(read_escaped_char(iter)?);
        } else {
            ret.push(c);
        }
    }
    Err(RmsdError::unfinished_quote(
        format!("Unfinished double quote: {ret}"),
        iter.pos(),
    ))
}

// YAML 1.2.2: 7.3.2. Single-Quoted Style
/// Read till reach next `'` or EOF. Two ' are treated as escaped ' as YAML
/// 1.2.2 specified.
/// The starting ' has already removed from CharsIter.
pub(crate) fn read_single_quoted_str(
    iter: &mut CharsIter,
) -> Result<String, RmsdError> {
    let mut ret = String::new();
    let mut pending_whitespace: Vec<char> = Vec::new();
    let mut droped_first_newline = false;

    while let Some(c) = iter.peek() {
        if c == '\'' {
            iter.next();
            if Some('\'') == iter.peek() {
                iter.next();
                ret.push('\'');
            } else {
                for s in pending_whitespace.drain(..) {
                    ret.push(s);
                }
                return Ok(ret);
            }
        } else {
            process_with_line_folding(
                &mut ret,
                iter,
                &mut pending_whitespace,
                &mut droped_first_newline,
            );
        }
    }

    Err(RmsdError::unfinished_quote(
        format!("Unfinished single quote: {ret}"),
        iter.pos(),
    ))
}

/// Read till end of line or any c-indicator defined in YAML 1.2.2
/// White space trimmed both start and end.
///
/// For unquoted string after map value indicator `:`, we should not
/// process line folding, so the string is limited before \n. Set
/// skip_line_folding to true in this case.
///
/// The ending char might be new line, which should not be considered as
/// position range, hence we return the final non-whitespace position.
pub(crate) fn read_unquoted_str(
    indent: usize,
    iter: &mut CharsIter,
    skip_line_folding: bool,
) -> Result<(String, RmsdPosition), RmsdError> {
    let mut ret = String::new();
    let mut droped_first_newline = false;
    let mut pending_whitespace: Vec<char> = Vec::new();
    let mut pos;

    // node.rs already has checks, so first char can be any(except new line will
    // be discard).
    if let Some(c) = iter.next() {
        pos = iter.pos();
        if c == '\n' {
            if skip_line_folding {
                return Ok((ret, RmsdPosition::EOF));
            } else {
                droped_first_newline = true;
            }
        } else {
            ret.push(c);
        }
    } else {
        return Ok((ret, RmsdPosition::EOF));
    }

    while let Some(c) = iter.peek() {
        if YAML_CHAR_INDICATORS.contains(&c) {
            return Ok((ret, pos));
        } else if c == '\n'
            && (skip_line_folding
                || !iter
                    .as_str()
                    .starts_with(&format!("\n{}", " ".repeat(indent))))
        {
            iter.next();
            return Ok((ret, pos));
        } else if let Some(p) = process_with_line_folding(
            &mut ret,
            iter,
            &mut pending_whitespace,
            &mut droped_first_newline,
        ) {
            pos = p;
        }
    }
    Ok((ret, pos))
}

// Escaped ASCII null (x00) character.
const NS_ESC_NULL: char = '0';
// Escaped ASCII bell (x07) character.
const NS_ESC_BELL: char = '7';
// Escaped ASCII backspace (x08) character.
const NS_ESC_BACKSPACE: char = '8';
// Escaped ASCII horizontal tab (x09) character. This is useful at the start or
// the end of a line to force a leading or trailing tab to become part of the
// content.
const NS_ESC_HORIZONTAL_TAB: char = '9';
const NS_ESC_HORIZONTAL_TAB_2: char = 't';
// Escaped ASCII line feed (x0A) character.
const NS_ESC_LINE_FEED: char = 'n';
// Escaped ASCII vertical tab (x0B) character.
const NS_ESC_VERTICAL_TAB: char = 'v';
// Escaped ASCII form feed (x0C) character.
const NS_ESC_FORM_FEED: char = 'f';
// Escaped ASCII carriage return (x0D) character.
const NS_ESC_CARRIAGE_RETURN: char = 'r';
// Escaped ASCII escape (x1B) character.
const NS_ESC_ESCAPE: char = 'e';
// Escaped ASCII slash (x2F), for JSON compatibility.
const NS_ESC_SLASH: char = '/';
// Escaped ASCII back slash (x5C).
const NS_ESC_BACKSLASH: char = '\\';
// Escaped Unicode next line (x85) character.
const NS_ESC_NEXT_LINE: char = 'N';
// Escaped Unicode non_breaking space (xA0) character.
const NS_ESC_NON_BREAKING_SPACE: char = '_';
// Escaped Unicode line separator (x2028) character.
const NS_ESC_LINE_SEPARATOR: char = 'L';
// Escaped Unicode paragraph separator (x2029) character.
const NS_ESC_PARAGRAPH_SEPARATOR: char = 'P';
// Escaped 8_bit Unicode character. 2 chars.
const NS_ESC_8_BIT: char = 'x';
// Escaped 16_bit Unicode character. 4 chars.
const NS_ESC_16_BIT: char = 'u';
// Escaped 32_bit Unicode character. 8 chars.
const NS_ESC_32_BIT: char = 'U';

pub(crate) fn read_escaped_char(
    iter: &mut CharsIter,
) -> Result<char, RmsdError> {
    let c = if let Some(c) = iter.next() {
        c
    } else {
        return Err(RmsdError::invalid_escape_scalar(
            "No character after escape \\".to_string(),
            iter.pos(),
        ));
    };

    let pos = iter.pos();
    Ok(match c {
        NS_ESC_NULL => '\0',
        NS_ESC_BELL => '\u{07}',
        NS_ESC_BACKSPACE => '\u{08}',
        NS_ESC_HORIZONTAL_TAB | NS_ESC_HORIZONTAL_TAB_2 => '\t',
        NS_ESC_LINE_FEED => '\n',
        NS_ESC_VERTICAL_TAB => '\u{0b}',
        NS_ESC_FORM_FEED => '\u{0c}',
        NS_ESC_CARRIAGE_RETURN => '\u{0d}',
        NS_ESC_ESCAPE => '\u{1b}',
        NS_ESC_SLASH => '/',
        NS_ESC_BACKSLASH => '\\',
        NS_ESC_NEXT_LINE => '\u{85}',
        NS_ESC_NON_BREAKING_SPACE => '\u{a0}',
        NS_ESC_LINE_SEPARATOR => '\u{2028}',
        NS_ESC_PARAGRAPH_SEPARATOR => '\u{2029}',
        NS_ESC_8_BIT | NS_ESC_16_BIT | NS_ESC_32_BIT => {
            let expected_count: usize = match c {
                NS_ESC_8_BIT => 2,
                NS_ESC_16_BIT => 4,
                NS_ESC_32_BIT => 8,
                _ => unreachable!(),
            };
            let mut val = String::new();
            for _ in 0..expected_count {
                if let Some(i) = iter.next() {
                    val.push(i)
                } else {
                    break;
                }
            }
            if val.chars().count() == expected_count {
                let val_u32 =
                    u32::from_str_radix(val.as_str(), 16).map_err(|_| {
                        RmsdError::invalid_escape_scalar(
                            format!(
                                "Escaped unicode \\x{} is \
                                not a valid unsigned integer in hex",
                                val
                            ),
                            pos,
                        )
                    })?;
                char::from_u32(val_u32).ok_or(
                    RmsdError::invalid_escape_scalar(
                        format!(
                            "Escaped unicode: \\x{} is not a valid unicode",
                            val
                        ),
                        pos,
                    ),
                )?
            } else {
                return Err(RmsdError::invalid_escape_scalar(
                    format!(
                        "Expecting {expected_count} characters after \
                        escape \\x, but got:{val}",
                    ),
                    pos,
                ));
            }
        }
        _ => {
            return Err(RmsdError::invalid_escape_scalar(
                format!("Not supported escape \\{c}"),
                pos,
            ));
        }
    })
}

/// Prefer unquoted string and use double quoted string if any of below:
///     * Line is longer than `max_width`
///     * Has non-printable character
///     * Has NS_ESC_XXX characters
pub(crate) fn to_scalar_string(
    indent_count: usize,
    input: &str,
    max_width: usize,
) -> String {
    // TODO: Escape non-printable character
    // TODO: Escape NS_ESC_XXX characters
    // TODO: Break long line
    if indent_count + input.chars().count() < max_width {
        input.to_string()
    } else {
        format!("\"{input}\"")
    }
}

// YAML 1.2.2: 6.5. Line Folding:
//      Line folding allows long lines to be broken for readability, while
//      retaining the semantics of the original long line. If a line break is
//      followed by an empty line, it is trimmed; the first line break is
//      discarded and the rest are retained as content.
fn process_with_line_folding(
    output: &mut String,
    iter: &mut CharsIter,
    pending_whitespace: &mut Vec<char>,
    droped_first_newline: &mut bool,
) -> Option<RmsdPosition> {
    let mut pos = RmsdPosition::default();
    if let Some(c) = iter.next() {
        match c {
            ' ' | '\t' => {
                if iter.pos().column == 1 {
                    iter.dicard_whitespace();
                } else {
                    // We are not sure this is trailing whitespace
                    // or not. So save it till non-whitespace found.
                    pending_whitespace.push(c);
                }
            }
            '\n' => {
                pending_whitespace.clear();
                if *droped_first_newline {
                    output.push(c);
                } else if Some('\n') == iter.peek() {
                    *droped_first_newline = true;
                } else {
                    pending_whitespace.push(' ');
                }
            }
            _ => {
                pos = iter.pos();
                *droped_first_newline = false;
                // Previously stored white spaces are not trailing white space,
                // hence store them.
                for s in pending_whitespace.drain(..) {
                    output.push(s);
                }
                output.push(c);
            }
        }
    }
    if pos != RmsdPosition::default() {
        Some(pos)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_double_quoted_string() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new(r#"abc dfd a ""#);
        assert_eq!(read_double_quoted_str(&mut iter)?, "abc dfd a ");
        Ok(())
    }

    #[test]
    fn test_double_quoted_string_with_escape() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new(r#"abc dfd a\n \x20\u2665\U0001F600""#);
        assert_eq!(read_double_quoted_str(&mut iter)?, "abc dfd a\n  ♥😀");
        Ok(())
    }

    #[test]
    fn test_normal_single_quoted_string() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc dfd a '");
        assert_eq!(read_single_quoted_str(&mut iter)?, "abc dfd a ");
        Ok(())
    }

    #[test]
    fn test_single_quoted_string_with_escape() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc dfd a'' '");
        assert_eq!(read_single_quoted_str(&mut iter)?, "abc dfd a' ");
        Ok(())
    }

    #[test]
    fn test_single_quoted_string_with_folding() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc\n\n\n \nabc\nd'");
        assert_eq!(read_single_quoted_str(&mut iter)?, "abc\n\n\nabc d");
        Ok(())
    }

    #[test]
    fn test_unquoted_string() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc d");
        let ret = read_unquoted_str(0, &mut iter, false)?;
        assert_eq!(ret.0, "abc d");
        assert_eq!(ret.1.line, 1);
        assert_eq!(ret.1.column, 5);
        Ok(())
    }

    #[test]
    fn test_unquoted_string_with_folding() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc\n\n\n \nabc\nd\n");
        let ret = read_unquoted_str(0, &mut iter, false)?;
        assert_eq!(ret.0, "abc\n\n\nabc d");
        assert_eq!(ret.1.line, 6);
        assert_eq!(ret.1.column, 1);
        Ok(())
    }

    #[test]
    fn test_unquoted_string_with_leading_new_line() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("\nabc");
        let ret = read_unquoted_str(0, &mut iter, false)?;
        assert_eq!(ret.0, "abc");
        assert_eq!(ret.1.line, 2);
        assert_eq!(ret.1.column, 3);
        Ok(())
    }

    #[test]
    fn test_unquoted_string_skip_line_folding() -> Result<(), RmsdError> {
        let mut iter = CharsIter::new("abc\n  d");
        let ret = read_unquoted_str(0, &mut iter, true)?;
        assert_eq!(ret.0, "abc");
        assert_eq!(ret.1.line, 1);
        assert_eq!(ret.1.column, 3);
        Ok(())
    }
}
