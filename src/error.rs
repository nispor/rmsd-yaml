// SPDX-License-Identifier: Apache-2.0

use crate::RmsdPosition;

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RmsdError {
    //    kind: ErrorKind,
    msg: Box<String>,
    position: RmsdPosition,
}

impl std::fmt::Display for RmsdError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> Result<(), std::fmt::Error> {
        write!(f, "{} error: {}", self.position, self.msg)
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
                    position: pos_str.into(),
                    msg: Box::new(msg_str.to_string()),
                };
            }
        }
        Self {
            msg: Box::new(msg.to_string()),
            ..Default::default()
        }
    }
}

impl RmsdError {
    /// Still have trailing charters not parsed
    pub(crate) fn trailing_characters(position: RmsdPosition) -> Self {
        Self {
            position,
            msg: Box::new("still have trailing charters".to_string()),
        }
    }

    pub(crate) fn eof() -> Self {
        Self {
            msg: Box::new("Reach end of file".to_string()),
            ..Default::default()
        }
    }
}
