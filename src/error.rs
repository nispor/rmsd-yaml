// SPDX-License-Identifier: Apache-2.0

use crate::YamlPosition;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ErrorKind {
    #[default]
    Bug,
    /// Found character not for start of any token. e.g. (\t cannot be used
    /// as start of token)
    InvalidStartOfToken,
    /// Invalid position string
    InvalidPosition,
    /// YAML reserved indicators( @ or `) can't start a plain scalar.
    StartWithReservedIndicator,
    /// Invalid escape scalar
    InvalidEscapeScalar,
    /// Unfinished quote
    UnfinishedQuote,
    /// Invalid ErrorKind
    InvalidErrorType,
    /// Unexpected YAML node type, e.g. Expecting a scalar but got sequence.
    UnexpectedYamlNodeType,
    /// Invalid bool string, should be `true` or `false`
    InvalidBool,
    /// Invalid number
    InvalidNumber,
    /// Number overflow
    NumberOverflow,
    /// Unfinished map indicator `{`
    UnfinishedMapIndicator,
    /// Unfinished map indicator `[`
    UnfinishedSequenceIndicator,
    /// Desired indent count should be bigger or equal to minimum indent count
    /// 2.
    IndentTooSmall,
    /// Expecting comment or line break. For example YAML string `|9>abc` will
    /// trigger this error because after `|9>`, we are expecting a line break
    /// or comment.
    ExpectingCommentOrLineBreak,
    /// Plain style scalar should not start with invalid indicators.
    InvalidPlainScalarStart,
    /// Plain style scalar should not contains ": " or " #" as it will cause
    /// ambiguity for mapping key or comment.
    /// In addition, inside flow collections, or when used as implicit keys,
    /// plain scalars must not contain the '[', ']', '{', '}' and ','
    /// characters.
    AmbiguityPlainScalar,
    /// Implicit key should contains ": " within single line
    InvalidImplicitKey,
    /// Expecting '-\n' or '- ' as sequence start Indicator
    InvalidSequnceStartIndicator,
    /// Got less indented data without parent container
    LessIndentedWithoutParent,
    /// No support of multiple documents
    NoSupportMultipleDocuments,
    /// Invalid anchor: empty anchor name, flow indicator character in
    /// anchor name, or anchor attached to an alias.
    InvalidAnchor,
    /// Invalid alias: empty alias name or flow indicator character in
    /// alias name.
    InvalidAlias,
    /// Alias does not reference any anchored node.
    UnknownAlias,
    /// Deserializing bytes is not supported.
    BytesUnsupported,
    /// Directive is only allowed before document start marker `---` or
    /// after document end marker `...`.
    MissingDocumentEndMarkerBeforeDirective,
    /// Malformed `%YAML` or `%TAG` directive (duplicate, extra words,
    /// bad version, duplicate tag handle, ...).
    InvalidDirective,
    /// Invalid tag: unknown tag handle, empty verbatim tag, or illegal
    /// character in a tag.
    InvalidTag,
    /// YAML node nesting (block or flow collections) exceeds the
    /// maximum supported depth. Guards against a stack overflow on
    /// pathologically deep input (e.g. thousands of nested `[` or
    /// `- ` indicators).
    RecursionLimitExceeded,
    /// Composing the document would produce more `Value` nodes than
    /// supported. Guards against the "billion laughs" pattern, where a
    /// chain of anchors each aliasing the previous one several times
    /// duplicates its content exponentially without the input itself
    /// growing much.
    AliasExpansionLimitExceeded,
    /// A mapping defines the same key more than once. The YAML
    /// specification (3.2.1.1) requires mapping keys to be unique.
    DuplicateMapKey,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Bug => "bug",
                Self::InvalidPosition => "invalid_position",
                Self::StartWithReservedIndicator =>
                    "start_with_reserved_indicator",
                Self::InvalidEscapeScalar => "invalid_escape_scalar",
                Self::UnfinishedQuote => "unfinished_quote",
                Self::InvalidErrorType => "invalid_error_type",
                Self::UnexpectedYamlNodeType => "unexpected_yaml_node_type",
                Self::InvalidBool => "invalid_bool",
                Self::InvalidNumber => "invalid_number",
                Self::NumberOverflow => "number_overflow",
                Self::UnfinishedMapIndicator => "unfinished_map_indicator",
                Self::UnfinishedSequenceIndicator =>
                    "unfinished_sequence_indicator",
                Self::IndentTooSmall => "indent_too_small",
                Self::InvalidStartOfToken => "invalid_start_of_token",
                Self::ExpectingCommentOrLineBreak =>
                    "expecting_comment_or_linebreak",
                Self::InvalidPlainScalarStart => "invalid_plain_scalar_start",
                Self::AmbiguityPlainScalar => "ambiguity_plain_scalar",
                Self::InvalidImplicitKey => "invalid_implicit_key",
                Self::InvalidSequnceStartIndicator =>
                    "invalid_sequence_start_indicator",
                Self::LessIndentedWithoutParent =>
                    "less_indented_without_parent",
                Self::NoSupportMultipleDocuments =>
                    "no_support_mulitple_documents",
                Self::InvalidAnchor => "invalid_anchor",
                Self::InvalidAlias => "invalid_alias",
                Self::UnknownAlias => "unknown_alias",
                Self::BytesUnsupported => "bytes_unsupported",
                Self::MissingDocumentEndMarkerBeforeDirective =>
                    "missing_document_end_marker_before_directive",
                Self::InvalidDirective => "invalid_directive",
                Self::InvalidTag => "invalid_tag",
                Self::RecursionLimitExceeded => "recursion_limit_exceeded",
                Self::AliasExpansionLimitExceeded =>
                    "alias_expansion_limit_exceeded",
                Self::DuplicateMapKey => "duplicate_map_key",
            }
        )
    }
}

impl TryFrom<&str> for ErrorKind {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        Ok(match value {
            s if s == Self::Bug.to_string() => Self::Bug,
            _ => {
                return Err(Error::new(
                    ErrorKind::InvalidErrorType,
                    format!("Invalid error type: {value}"),
                    YamlPosition::default(),
                    YamlPosition::default(),
                ));
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Error {
    kind: ErrorKind,
    msg: String,
    start_pos: YamlPosition,
    end_pos: YamlPosition,
}

impl Error {
    pub fn new(
        kind: ErrorKind,
        msg: String,
        start_pos: YamlPosition,
        end_pos: YamlPosition,
    ) -> Self {
        Self {
            kind,
            msg,
            start_pos,
            end_pos,
        }
    }

    /// Prefix the message with `path: ` like serde_yaml (e.g.
    /// `cwnd: invalid type: ...`) unless the path is already present.
    pub(crate) fn with_path(mut self, path: &str) -> Self {
        if !path.is_empty() && !self.msg.starts_with(path) {
            self.msg = format!("{path}: {}", self.msg);
        }
        self
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn msg(&self) -> &str {
        self.msg.as_str()
    }

    pub fn start_pos(&self) -> YamlPosition {
        self.start_pos
    }

    pub fn end_pos(&self) -> YamlPosition {
        self.end_pos
    }
}

impl std::fmt::Display for Error {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "{}:{} kind: {} error: {}",
            self.start_pos, self.end_pos, self.kind, self.msg
        )
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        if let Some((pos_kind_str, msg_str)) = msg.split_once("error: ")
            && let Some((pos_str, kind_str)) =
                pos_kind_str.split_once(" kind: ")
            && let Some((start_pos_str, end_pos_str)) = pos_str.split_once(":")
        {
            Self {
                start_pos: YamlPosition::try_from(start_pos_str)
                    .unwrap_or_default(),
                end_pos: YamlPosition::try_from(end_pos_str)
                    .unwrap_or_default(),
                msg: msg_str.to_string(),
                kind: ErrorKind::try_from(kind_str).unwrap_or_default(),
            }
        } else {
            Self {
                msg: msg.to_string(),
                ..Default::default()
            }
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Bug,
            msg: e.to_string(),
            ..Default::default()
        }
    }
}

impl std::error::Error for Error {}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::from(msg.to_string().as_str())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::from(msg.to_string().as_str())
    }

    // TOOD: Implement more functions of this trait with position stored in
    // error.
}
