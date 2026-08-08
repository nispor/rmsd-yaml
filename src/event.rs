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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum YamlScalarStyle {
    #[default]
    Plain,
    // Constructed once single-quoted scalar parsing is implemented.
    #[allow(dead_code)]
    SingleQuoted,
    DoubleQuoted,
    Literal,
    Folded,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum YamlEvent {
    StreamStart,
    StreamEnd,
    /// Whether document start with `---`
    DocumentStart(bool, YamlPosition),
    /// Whether document start with `...`
    DocumentEnd(bool, YamlPosition),
    /// Anchor, Tag and position
    SequenceStart(Option<String>, Option<String>, YamlPosition),
    SequenceEnd(YamlPosition),
    /// Anchor, Tag and position
    MapStart(Option<String>, Option<String>, YamlPosition),
    MapEnd(YamlPosition),
    /// Anchor, Tag, value, style, start and end
    Scalar(
        Option<String>,
        Option<String>,
        String,
        YamlScalarStyle,
        YamlPosition,
        YamlPosition,
    ),
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
            Self::SequenceStart(anchor, tag, _) => {
                let mut s = String::from("+SEQ");
                if let Some(a) = anchor {
                    s.push_str(&format!(" &{a}"));
                }
                if let Some(t) = tag {
                    s.push_str(&format!(" {t}"));
                }
                write!(f, "{s}")
            }
            Self::SequenceEnd(_) => write!(f, "-SEQ"),
            Self::MapStart(anchor, tag, _) => {
                let mut s = String::from("+MAP");
                if let Some(a) = anchor {
                    s.push_str(&format!(" &{a}"));
                }
                if let Some(t) = tag {
                    s.push_str(&format!(" {t}"));
                }
                write!(f, "{s}")
            }
            Self::MapEnd(_) => write!(f, "-MAP"),
            Self::Scalar(anchor, tag, v, style, _, _) => {
                let mut s = String::from("=VAL");
                if let Some(a) = anchor {
                    s.push_str(&format!(" &{a}"));
                }
                if let Some(t) = tag {
                    s.push_str(&format!(" {t}"));
                }
                s.push_str(&format!(" {}", show_scalar_str(v, style)));
                write!(f, "{s}")
            }
        }
    }
}

fn show_scalar_str(v: &str, style: &YamlScalarStyle) -> String {
    let mut ret = match style {
        YamlScalarStyle::Plain => String::from(":"),
        YamlScalarStyle::SingleQuoted => String::from("'"),
        YamlScalarStyle::DoubleQuoted => String::from("\""),
        YamlScalarStyle::Literal => String::from("|"),
        YamlScalarStyle::Folded => String::from(">"),
    };
    ret.push_str(&v.replace("\n", "\\n"));
    ret
}
