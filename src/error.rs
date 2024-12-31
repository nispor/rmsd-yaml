// SPDX-License-Identifier: Apache-2.0

use crate::RmsdPosition;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ErrorKind {
    #[default]
    Bug,
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
            }
        )
    }
}

impl TryFrom<&str> for ErrorKind {
    type Error = RmsdError;

    fn try_from(value: &str) -> Result<Self, RmsdError> {
        Ok(match value {
            "bug" => Self::Bug,
            "invalid_position" => Self::InvalidPosition,
            "start_with_reserved_indicator" => Self::StartWithReservedIndicator,
            "invalid_escape_scalar" => Self::InvalidEscapeScalar,
            "unfinished_quote" => Self::UnfinishedQuote,
            "invalid_error_type" => Self::InvalidErrorType,
            "unexpected_yaml_node_type" => Self::UnexpectedYamlNodeType,
            "invalid_bool" => Self::InvalidBool,
            "invalid_number" => Self::InvalidNumber,
            "number_overflow" => Self::NumberOverflow,
            "unfinished_map_indicator" => Self::UnfinishedMapIndicator,
            "unfinished_sequence_indicator" => {
                Self::UnfinishedSequenceIndicator
            }
            _ => {
                return Err(RmsdError::new(
                    ErrorKind::InvalidErrorType,
                    format!("Invalid error type: {value}"),
                    RmsdPosition::default(),
                ))
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RmsdError {
    kind: ErrorKind,
    msg: String,
    // TODO: Should we support pos range instead of single point?
    pos: RmsdPosition,
}

impl RmsdError {
    pub fn new(kind: ErrorKind, msg: String, pos: RmsdPosition) -> Self {
        Self { kind, msg, pos }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn msg(&self) -> &str {
        self.msg.as_str()
    }

    pub fn pos(&self) -> RmsdPosition {
        self.pos
    }
}

impl std::fmt::Display for RmsdError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        write!(f, "{} kind: {} error: {}", self.pos, self.kind, self.msg)
    }
}

impl std::error::Error for RmsdError {}

impl serde::de::Error for RmsdError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        let msg = msg.to_string();
        if let Some((pos_kind_str, msg_str)) = msg.split_once("error: ") {
            if let Some((pos_str, kind_str)) = pos_kind_str.split_once("kind: ")
            {
                return Self {
                    pos: RmsdPosition::try_from(pos_str).unwrap_or_default(),
                    msg: msg_str.to_string(),
                    kind: ErrorKind::try_from(kind_str).unwrap_or_default(),
                };
            }
        }
        Self {
            msg,
            ..Default::default()
        }
    }
}

impl RmsdError {
    pub(crate) fn bug(msg: String, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::Bug,
            msg,
            pos,
        }
    }

    pub(crate) fn invalid_pos(msg: String) -> Self {
        Self {
            kind: ErrorKind::InvalidPosition,
            msg,
            ..Default::default()
        }
    }

    pub(crate) fn reserved_indicator(pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::StartWithReservedIndicator,
            msg: "YAML reserved indicators( @ or `) can't start a plain scalar"
                .to_string(),
            pos,
        }
    }

    pub(crate) fn invalid_escape_scalar(
        msg: String,
        pos: RmsdPosition,
    ) -> Self {
        Self {
            kind: ErrorKind::InvalidEscapeScalar,
            msg,
            pos,
        }
    }

    pub(crate) fn unfinished_quote(msg: String, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::UnfinishedQuote,
            msg,
            pos,
        }
    }

    pub(crate) fn unexpected_yaml_node_type(
        msg: String,
        pos: RmsdPosition,
    ) -> Self {
        Self {
            kind: ErrorKind::UnexpectedYamlNodeType,
            msg,
            pos,
        }
    }

    pub(crate) fn invalid_bool(input: &str, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::InvalidBool,
            msg: format!(
                "Invalid bool string `{input}`, should be true or false"
            ),
            pos,
        }
    }

    pub(crate) fn invalid_number(input: &str, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::InvalidNumber,
            msg: format!("Invalid number string `{input}`"),
            pos,
        }
    }

    pub(crate) fn number_overflow(msg: String, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::NumberOverflow,
            msg,
            pos,
        }
    }

    pub(crate) fn unfinished_map_indicator(pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::UnfinishedMapIndicator,
            msg: "Unfinished map indicator `}`".to_string(),
            pos,
        }
    }

    pub(crate) fn unfinished_seq_indicator(pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::UnfinishedSequenceIndicator,
            msg: "Unfinished sequence indicator `]`".to_string(),
            pos,
        }
    }
}
