// SPDX-License-Identifier: Apache-2.0

use crate::YamlPosition;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct YamlEventIter {
    data: Vec<Option<YamlEvent>>,
    next_index: usize,
}

impl YamlEventIter {
    pub(crate) fn new(events: Vec<YamlEvent>) -> Self {
        Self {
            data: events.into_iter().map(Some).collect(),
            next_index: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Option<YamlEvent> {
        if self.next_index >= self.data.len() {
            None
        } else {
            let ret = self.data[self.next_index].take();
            self.next_index += 1;
            ret
        }
    }

    pub(crate) fn peek(&self) -> Option<&YamlEvent> {
        if self.next_index >= self.data.len() {
            None
        } else {
            self.data.get(self.next_index).unwrap_or(&None).as_ref()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum YamlEvent {
    StreamStart,
    StreamEnd,
    /// Whether document start with `---`
    DocumentStart(bool, YamlPosition),
    /// Whether document start with `...`
    DocumentEnd(bool, YamlPosition),
    /// Tag and position
    SequenceStart(Option<String>, YamlPosition),
    SequenceEnd(YamlPosition),
    /// Tag and position
    MapStart(Option<String>, YamlPosition),
    MapEnd(YamlPosition),
    Scalar(Option<String>, String, YamlPosition, YamlPosition),
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
            Self::SequenceStart(tag, _) => {
                if let Some(tag) = tag {
                    write!(f, "+SEQ {tag}")
                } else {
                    write!(f, "+SEQ")
                }
            }
            Self::SequenceEnd(_) => write!(f, "-SEQ"),
            Self::MapStart(tag, _) => {
                if let Some(tag) = tag {
                    write!(f, "+MAP {tag}")
                } else {
                    write!(f, "+MAP")
                }
            }
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
