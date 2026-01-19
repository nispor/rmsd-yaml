// SPDX-License-Identifier: Apache-2.0

use crate::{YamlPosition, YamlTag};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum YamlEvent {
    StreamStart,
    StreamEnd,
    /// Whether document start with `---`
    DocumentStart(bool, YamlPosition),
    /// Whether document start with `...`
    DocumentEnd(bool, YamlPosition),
    SequenceStart(YamlPosition),
    SequenceEnd(YamlPosition),
    MapStart(YamlPosition),
    MapEnd(YamlPosition),
    Scalar(Option<YamlTag>, String, YamlPosition, YamlPosition),
}

impl std::fmt::Display for YamlEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StreamStart => write!(f, "+STR"),
            Self::StreamEnd => write!(f, "-STR"),
            Self::DocumentStart(true, _) => write!(f, "+DOC ---"),
            Self::DocumentStart(false, _) => write!(f, "+DOC"),
            Self::DocumentEnd(true, _) => write!(f, "-DOC ..."),
            Self::DocumentEnd(false, _) => write!(f, "-DOC"),
            Self::SequenceStart(_) => write!(f, "+SEQ"),
            Self::SequenceEnd(_) => write!(f, "-SEQ"),
            Self::MapStart(_) => write!(f, "+MAP"),
            Self::MapEnd(_) => write!(f, "-MAP"),
            Self::Scalar(tag, v, _, _) => {
                if let Some(tag) = tag {
                    write!(f, "=VAL {tag} {}", show_scalar_str(v))
                } else {
                    write!(f, "=VAL {}", show_scalar_str(v))
                }
            }
        }
    }
}

fn show_scalar_str(v: &str) -> String {
    if v.contains("\n") {
        format!("|{}", v.replace("\n", "\\n"))
    } else {
        format!(":{}", v)
    }
}
