// SPDX-License-Identifier: Apache-2.0

use crate::RmsdPosition;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ErrorKind {
    #[default]
    Bug,
    /// Still have charters not parsed.
    TrailingCharacters,
    /// Invalid position string
    InvalidPosition,
    /// YAML reserved indicators( @ or `) can't start a plain scalar.
    StartWithReservedIndicator,
    /// Invalid escape scalar
    InvalidEscapeScalar,
    /// Unfinished quote
    UnfinishedQuote,
    /// Invalid ErrorKind
    InvalidErrorKind,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Bug => "bug",
                Self::TrailingCharacters => "trailing_characters",
                Self::InvalidPosition => "invalid_position",
                Self::StartWithReservedIndicator =>
                    "start_with_reserved_indicator",
                Self::InvalidEscapeScalar => "invalid_escape_scalar",
                Self::UnfinishedQuote => "unfinished_quote",
                Self::InvalidErrorKind => "invalid_error_kind",
            }
        )
    }
}

impl TryFrom<&str> for ErrorKind {
    type Error = RmsdError;

    fn try_from(value: &str) -> Result<Self, RmsdError> {
        match value {
            "bug" => Ok(Self::Bug),
            "trailing_characters" => Ok(Self::TrailingCharacters),
            "invalid_position" => Ok(Self::InvalidPosition),
            "start_with_reserved_indicator" => {
                Ok(Self::StartWithReservedIndicator)
            }
            "invalid_escape_scalar" => Ok(Self::InvalidEscapeScalar),
            "unfinished_quote" => Ok(Self::UnfinishedQuote),
            "invalid_error_kind" => Ok(Self::InvalidErrorKind),
            _ => Err(RmsdError::new(
                ErrorKind::InvalidErrorKind,
                format!("Invalid error kind: {value}"),
                RmsdPosition::default(),
            )),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RmsdError {
    kind: ErrorKind,
    msg: String,
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
    /// Still have trailing charters not parsed
    pub(crate) fn trailing_characters(pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::TrailingCharacters,
            pos,
            msg: "still have trailing charters".to_string(),
        }
    }

    pub(crate) fn invalid_pos(msg: &str) -> Self {
        Self {
            kind: ErrorKind::InvalidPosition,
            msg: msg.to_string(),
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
}
