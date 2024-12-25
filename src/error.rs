// SPDX-License-Identifier: Apache-2.0

use crate::RmsdPosition;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ErrorKind {
    #[default]
    Bug,
    /// Reach end of file, internally.
    Eof,
    /// Still have charters not parsed.
    TrailingCharacters,
    /// Expecting boolean: true or false
    NotBoolean,
    /// Invalid position string
    InvalidPosition,
    /// YAML reserved indicators( @ or `) can't start a plain scalar.
    StartWithReservedIndicator,
    /// Invalid escape scalar
    InvalidEscapeScalar,
    /// Unfinished quote
    UnfinishedQuote,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RmsdError {
    kind: ErrorKind,
    msg: String,
    pos: RmsdPosition,
}

impl RmsdError {
    pub fn new(kind: ErrorKind, msg: &str, pos: RmsdPosition) -> Self {
        Self {
            kind,
            msg: msg.to_string(),
            pos,
        }
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
        write!(f, "{} error: {}", self.pos, self.msg)
    }
}

impl std::error::Error for RmsdError {}

impl serde::de::Error for RmsdError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        let error_message = msg.to_string();
        if error_message.contains("error: ") {
            if let Some((pos_str, msg_str)) =
                error_message.split_once("error: ")
            {
                return Self {
                    pos: RmsdPosition::try_from(pos_str)
                        .unwrap_or(RmsdPosition::default()),
                    msg: msg_str.to_string(),
                    ..Default::default()
                };
            }
        }
        Self {
            msg: msg.to_string(),
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

    pub(crate) fn not_boolean(pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::NotBoolean,
            pos,
            msg: "still have trailing charters".to_string(),
        }
    }

    pub(crate) fn eof() -> Self {
        Self {
            kind: ErrorKind::Eof,
            msg: "Reach end of file".to_string(),
            ..Default::default()
        }
    }

    pub(crate) fn invalid_pos(msg: &str) -> Self {
        Self {
            kind: ErrorKind::InvalidPosition,
            msg: msg.to_string(),
            ..Default::default()
        }
    }

    pub(crate) fn bug(msg: String, pos: RmsdPosition) -> Self {
        Self {
            kind: ErrorKind::Bug,
            msg,
            pos,
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
