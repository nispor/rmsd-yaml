// SPDX-License-Identifier: Apache-2.0

use crate::{
    process_indent, read_double_quoted_str, read_single_quoted_str,
    read_unquoted_str, CharsIter, RmsdError, RmsdPosition,
};

const YAML_CHAR_SEQUENCE_ENTRY: char = '-';
const YAML_CHAR_MAPPING_KEY: char = '?';
const YAML_CHAR_MAPPING_VALUE: char = ':';
const YAML_CHAR_COLLECT_ENTRY: char = ',';
const YAML_CHAR_SEQUENCE_START: char = '[';
const YAML_CHAR_SEQUENCE_END: char = ']';
const YAML_CHAR_MAPPING_START: char = '{';
const YAML_CHAR_MAPPING_END: char = '}';
const YAML_CHAR_COMMENT: char = '#';
const YAML_CHAR_ANCHOR: char = '&';
const YAML_CHAR_ALIAS: char = '*';
const YAML_CHAR_TAG: char = '!';
const YAML_CHAR_LITERAL: char = '|';
const YAML_CHAR_FOLDED: char = '>';
const YAML_CHAR_SINGLE_QUOTE: char = '\'';
const YAML_CHAR_DOUBLE_QUOTE: char = '"';
const YAML_CHAR_DIRECTIVE: char = '%';
const YAML_CHAR_RESERVED: char = '@';
const YAML_CHAR_RESERVED2: char = '`';

pub(crate) const YAML_CHAR_INDICATORS: [char; 19] = [
    YAML_CHAR_SEQUENCE_ENTRY,
    YAML_CHAR_MAPPING_KEY,
    YAML_CHAR_MAPPING_VALUE,
    YAML_CHAR_COLLECT_ENTRY,
    YAML_CHAR_SEQUENCE_START,
    YAML_CHAR_SEQUENCE_END,
    YAML_CHAR_MAPPING_START,
    YAML_CHAR_MAPPING_END,
    YAML_CHAR_COMMENT,
    YAML_CHAR_ANCHOR,
    YAML_CHAR_ALIAS,
    YAML_CHAR_TAG,
    YAML_CHAR_LITERAL,
    YAML_CHAR_FOLDED,
    YAML_CHAR_SINGLE_QUOTE,
    YAML_CHAR_DOUBLE_QUOTE,
    YAML_CHAR_DIRECTIVE,
    YAML_CHAR_RESERVED,
    YAML_CHAR_RESERVED2,
];

/// YAML Token Data
/// Tokenization input data with white spaces and comments removed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum YamlTokenData {
    /// Empty
    Null,
    /// The `-` character for sequence in block collection
    BlockSequenceIndicator,
    /// The `[` character for sequence start in flow style
    FlowSequenceStart,
    /// The `]` character for sequence end in flow style
    FlowSequenceEnd,
    /// The `?` character for mapping key in block collection
    MapKeyIndicator,
    /// The `:` character for mapping value in block collection
    MapValueIndicator,
    /// The `{` character for mapping start in flow style
    FlowMapStart,
    /// The `}` character for mapping end in flow style
    FlowMapEnd,
    // We need to convert escaped UTF-8 char like `\0001F600` to
    /// Scalar content
    Scalar(String),
    /*
    /// Global tag using `tag:`
    GlobalTag(String),
    /// Local tag using `!`
    LocalTag(String),
    /// Directive using `%TAG`
    DirectiveTag(String),
    /// Directive using `%YAML`
    DirectiveYaml(String),
    /// Node anchor `&`
    Anchor(String),
    /// Refer to anchor by `*`
    Alias(String),
    */
}

impl std::fmt::Display for YamlTokenData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlToken {
    pub indent: usize,
    pub start: RmsdPosition,
    pub end: RmsdPosition,
    pub data: YamlTokenData,
}

impl std::fmt::Display for YamlToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Improve this
        write!(f, "{self:?}")
    }
}

impl YamlToken {
    pub(crate) fn parse(input: &str) -> Result<Vec<Self>, RmsdError> {
        if input.is_empty() {
            return Ok(vec![Self {
                indent: 0,
                start: RmsdPosition::new(1, 0),
                end: RmsdPosition::new(1, 0),
                data: YamlTokenData::Null,
            }]);
        }
        let mut iter = CharsIter::new(input);
        let mut ret: Vec<Self> = Vec::new();
        let mut indent = 0usize;

        while let Some(mut c) = iter.peek() {
            if iter.next_pos().column == 1 {
                indent = process_indent(&mut iter);
                if indent > 0 {
                    c = if let Some(c) = iter.peek() {
                        c
                    } else {
                        break;
                    }
                }
            }
            match c {
                // New lines at document start
                '\n' => {
                    iter.next();
                }
                YAML_CHAR_SEQUENCE_ENTRY
                | YAML_CHAR_MAPPING_KEY
                | YAML_CHAR_MAPPING_VALUE => {
                    let indicator = match c {
                        YAML_CHAR_SEQUENCE_ENTRY => {
                            YamlTokenData::BlockSequenceIndicator
                        }
                        YAML_CHAR_MAPPING_KEY => YamlTokenData::MapKeyIndicator,
                        YAML_CHAR_MAPPING_VALUE => {
                            YamlTokenData::MapValueIndicator
                        }
                        _ => unreachable!(),
                    };
                    iter.next();
                    if let Some(c) = iter.peek() {
                        if c == ' ' || c == '\t' || c == '\n' {
                            let start = iter.pos();
                            iter.next();
                            ret.push(YamlToken {
                                indent,
                                start,
                                end: start,
                                data: indicator,
                            });
                        } else {
                            ret.push(read_unquoted_str_token(
                                &mut iter,
                                indent,
                                is_after_map_indicator(&ret),
                            )?);
                        }
                    } else {
                        ret.push(YamlToken {
                            indent,
                            start: iter.pos(),
                            end: iter.pos(),
                            data: indicator,
                        });
                        ret.push(YamlToken {
                            indent,
                            start: RmsdPosition::EOF,
                            end: RmsdPosition::EOF,
                            data: YamlTokenData::Null,
                        });
                        break;
                    }
                }
                YAML_CHAR_SEQUENCE_START => {
                    iter.next();
                    ret.push(YamlToken {
                        indent,
                        start: iter.pos(),
                        end: iter.pos(),
                        data: YamlTokenData::FlowSequenceStart,
                    })
                }
                YAML_CHAR_COLLECT_ENTRY => {
                    iter.next();
                    // no special action required for `,`.
                }
                YAML_CHAR_SEQUENCE_END => {
                    iter.next();
                    ret.push(YamlToken {
                        indent,
                        start: iter.pos(),
                        end: iter.pos(),
                        data: YamlTokenData::FlowSequenceEnd,
                    })
                }
                YAML_CHAR_MAPPING_START => {
                    iter.next();
                    ret.push(YamlToken {
                        indent,
                        start: iter.pos(),
                        end: iter.pos(),
                        data: YamlTokenData::FlowMapStart,
                    })
                }
                YAML_CHAR_MAPPING_END => {
                    iter.next();
                    ret.push(YamlToken {
                        indent,
                        start: iter.pos(),
                        end: iter.pos(),
                        data: YamlTokenData::FlowMapEnd,
                    })
                }
                YAML_CHAR_TAG => {
                    iter.next();
                    ret.push(YamlToken {
                        indent,
                        start: iter.pos(),
                        end: iter.pos(),
                        data: YamlTokenData::MapValueIndicator,
                    })
                }

                YAML_CHAR_COMMENT => {
                    // Discard all comments
                    break;
                }
                YAML_CHAR_ANCHOR => {
                    iter.next();
                    todo!()
                }
                YAML_CHAR_ALIAS => {
                    iter.next();
                    todo!()
                }
                YAML_CHAR_LITERAL => {
                    iter.next();
                    todo!()
                }
                YAML_CHAR_FOLDED => {
                    iter.next();
                    todo!()
                }
                YAML_CHAR_DIRECTIVE => {
                    iter.next();
                    todo!()
                }
                YAML_CHAR_RESERVED | YAML_CHAR_RESERVED2 => {
                    return Err(RmsdError::reserved_indicator(iter.pos()));
                }
                YAML_CHAR_SINGLE_QUOTE => {
                    iter.next();
                    let start = iter.pos();
                    let quoted_string = read_single_quoted_str(&mut iter)?;

                    ret.push(YamlToken {
                        indent,
                        start,
                        end: iter.pos(),
                        data: YamlTokenData::Scalar(quoted_string),
                    });
                }
                YAML_CHAR_DOUBLE_QUOTE => {
                    iter.next();
                    let start = iter.pos();
                    let quoted_string = read_double_quoted_str(&mut iter)?;

                    ret.push(YamlToken {
                        indent,
                        start,
                        end: iter.pos(),
                        data: YamlTokenData::Scalar(quoted_string),
                    });
                }
                ' ' => {
                    // discard whitespace
                    iter.next();
                }
                _ => {
                    ret.push(read_unquoted_str_token(
                        &mut iter,
                        indent,
                        is_after_map_indicator(&ret),
                    )?);
                }
            }
        }
        Ok(ret)
    }
}

fn is_after_map_indicator(tokens: &[YamlToken]) -> bool {
    tokens
        .iter()
        .last()
        .map(|token| token.data == YamlTokenData::MapValueIndicator)
        .unwrap_or_default()
}

fn read_unquoted_str_token(
    iter: &mut CharsIter,
    indent: usize,
    skip_line_folding: bool,
) -> Result<YamlToken, RmsdError> {
    let start = iter.next_pos();
    let (unquoted_string, end) = read_unquoted_str(iter, skip_line_folding)?;
    Ok(YamlToken {
        indent,
        start,
        end,
        data: YamlTokenData::Scalar(unquoted_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_null() {
        assert_eq!(
            YamlToken::parse("").unwrap(),
            vec![YamlToken {
                indent: 0,
                start: RmsdPosition::new(1, 0),
                end: RmsdPosition::new(1, 0),
                data: YamlTokenData::Null,
            }]
        )
    }

    #[test]
    fn test_double_quoted_str_with_document() {
        assert_eq!(
            YamlToken::parse(r#""abc" # testing document"#).unwrap(),
            vec![YamlToken {
                indent: 0,
                start: RmsdPosition::new(1, 1),
                end: RmsdPosition::new(1, 5),
                data: YamlTokenData::Scalar("abc".to_string()),
            }]
        )
    }

    #[test]
    fn test_sequence_unquoted() {
        assert_eq!(
            YamlToken::parse("- a\n- b\n- c \n- d").unwrap(),
            vec![
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(1, 1),
                    end: RmsdPosition::new(1, 1),
                    data: YamlTokenData::BlockSequenceIndicator,
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(1, 3),
                    end: RmsdPosition::new(1, 3),
                    data: YamlTokenData::Scalar("a".into()),
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(2, 1),
                    end: RmsdPosition::new(2, 1),
                    data: YamlTokenData::BlockSequenceIndicator,
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(2, 3),
                    end: RmsdPosition::new(2, 3),
                    data: YamlTokenData::Scalar("b".into()),
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(3, 1),
                    end: RmsdPosition::new(3, 1),
                    data: YamlTokenData::BlockSequenceIndicator,
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(3, 3),
                    end: RmsdPosition::new(3, 3),
                    data: YamlTokenData::Scalar("c".into()),
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(4, 1),
                    end: RmsdPosition::new(4, 1),
                    data: YamlTokenData::BlockSequenceIndicator,
                },
                YamlToken {
                    indent: 0,
                    start: RmsdPosition::new(4, 3),
                    end: RmsdPosition::new(4, 3),
                    data: YamlTokenData::Scalar("d".into()),
                },
            ]
        )
    }

    #[test]
    fn test_map_indented() {
        assert_eq!(
            YamlToken::parse("\n  abc : d\n  abd:\n    abe: f").unwrap(),
            vec![
                YamlToken {
                    indent: 2,
                    start: RmsdPosition::new(2, 3),
                    end: RmsdPosition::new(2, 5),
                    data: YamlTokenData::Scalar("abc".into()),
                },
                YamlToken {
                    indent: 2,
                    start: RmsdPosition::new(2, 7),
                    end: RmsdPosition::new(2, 7),
                    data: YamlTokenData::MapValueIndicator,
                },
                YamlToken {
                    indent: 2,
                    start: RmsdPosition::new(2, 9),
                    end: RmsdPosition::new(2, 9),
                    data: YamlTokenData::Scalar("d".into()),
                },
                YamlToken {
                    indent: 2,
                    start: RmsdPosition::new(3, 3),
                    end: RmsdPosition::new(3, 5),
                    data: YamlTokenData::Scalar("abd".into()),
                },
                YamlToken {
                    indent: 2,
                    start: RmsdPosition::new(3, 6),
                    end: RmsdPosition::new(3, 6),
                    data: YamlTokenData::MapValueIndicator,
                },
                YamlToken {
                    indent: 4,
                    start: RmsdPosition::new(4, 5),
                    end: RmsdPosition::new(4, 7),
                    data: YamlTokenData::Scalar("abe".into()),
                },
                YamlToken {
                    indent: 4,
                    start: RmsdPosition::new(4, 8),
                    end: RmsdPosition::new(4, 8),
                    data: YamlTokenData::MapValueIndicator,
                },
                YamlToken {
                    indent: 4,
                    start: RmsdPosition::new(4, 10),
                    end: RmsdPosition::new(4, 10),
                    data: YamlTokenData::Scalar("f".into()),
                },
            ]
        )
    }
}