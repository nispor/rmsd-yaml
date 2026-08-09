// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlEventIter, YamlPosition, YamlTag,
    YamlValue, YamlValueData, YamlValueMap,
};

impl YamlValue {
    pub(crate) fn compose(events: Vec<YamlEvent>) -> Result<Self, YamlError> {
        let mut documents = compose_documents(events)?;
        if documents.len() > 1 {
            return Err(YamlError::new(
                ErrorKind::NoSupportMultipleDocuments,
                format!(
                    "No support of multiple YAML documents, got {} documents",
                    documents.len()
                ),
                YamlPosition::default(),
                YamlPosition::default(),
            ));
        }
        Ok(documents.pop().unwrap_or_default())
    }
}

/// Compose every document of an event stream into a `YamlValue`.
///
/// Anchors are scoped per document per the YAML specification, so the
/// anchor table is reset between documents.
pub(crate) fn compose_documents(
    events: Vec<YamlEvent>,
) -> Result<Vec<YamlValue>, YamlError> {
    let mut events_iter = YamlEventIter::new(events);
    let mut documents = Vec::new();
    loop {
        match events_iter.peek() {
            None | Some(YamlEvent::StreamEnd) => break,
            Some(YamlEvent::DocumentStart(_, _)) => {
                let mut anchors: HashMap<String, YamlValue> = HashMap::new();
                documents.push(compose_value(&mut events_iter, &mut anchors)?);
            }
            Some(_) => {
                events_iter.next();
            }
        }
    }
    Ok(documents)
}

fn compose_value(
    events_iter: &mut YamlEventIter,
    anchors: &mut HashMap<String, YamlValue>,
) -> Result<YamlValue, YamlError> {
    let mut doc_started_pos: Option<YamlPosition> = None;
    while let Some(event) = events_iter.next() {
        match event {
            YamlEvent::StreamStart => (),
            YamlEvent::DocumentStart(_, pos) => {
                if let Some(_doc_started_pos) = doc_started_pos {
                    // Another document follows: the current one has
                    // ended (its DocumentEnd is implied).
                    return Ok(Default::default());
                } else {
                    doc_started_pos = Some(pos);
                }
            }
            YamlEvent::DocumentEnd(_, _) | YamlEvent::StreamEnd => {
                break;
            }
            YamlEvent::SequenceStart(anchor, tag, _style, pos) => {
                let array = compose_sequence(events_iter, anchors, pos)?;
                let mut ret = if let Some(tag) = tag {
                    YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: array.data,
                        })),
                        start: array.start,
                        end: array.end,
                        ..Default::default()
                    }
                } else {
                    array
                };
                ret.meta.anchor = anchor;
                if let Some(anchor) = &ret.meta.anchor {
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::SequenceEnd(pos) => {
                return Err(YamlError::new(
                    ErrorKind::Bug,
                    format!(
                        "Got unexpected event in compose_value(),
                        YamlEvent::SequenceEnd() should be consumed by
                        compose_sequence(): {:?}",
                        events_iter
                    ),
                    pos,
                    pos,
                ));
            }
            YamlEvent::MapStart(anchor, tag, _style, pos) => {
                let map = compose_map(events_iter, anchors, pos)?;
                let mut ret = if let Some(tag) = tag {
                    YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: map.data,
                        })),
                        start: map.start,
                        end: map.end,
                        ..Default::default()
                    }
                } else {
                    map
                };
                ret.meta.anchor = anchor;
                if let Some(anchor) = &ret.meta.anchor {
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::MapEnd(pos) => {
                return Err(YamlError::new(
                    ErrorKind::Bug,
                    format!(
                        "Got unexpected event in compose_value(),
                        YamlEvent::MapEnd() should be consumed by
                        compose_map(): {:?}",
                        events_iter
                    ),
                    pos,
                    pos,
                ));
            }
            YamlEvent::Scalar(anchor, tag, val, style, start, end) => {
                let mut ret = if let Some(tag) = tag {
                    YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: YamlValueData::String(val),
                        })),
                        start,
                        end,
                        ..Default::default()
                    }
                } else {
                    YamlValue {
                        data: YamlValueData::String(val),
                        start,
                        end,
                        ..Default::default()
                    }
                };
                ret.meta.scalar_style = Some(style);
                ret.meta.anchor = anchor;
                if let Some(anchor) = &ret.meta.anchor {
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::Alias(name, pos) => {
                if let Some(value) = anchors.get(&name) {
                    let mut ret = value.clone();
                    ret.meta.alias = Some(name);
                    return Ok(ret);
                } else {
                    return Err(YamlError::new(
                        ErrorKind::UnknownAlias,
                        format!(
                            "Alias *{name} does not reference any anchored \
                             node"
                        ),
                        pos,
                        pos,
                    ));
                }
            }
        }
    }

    Ok(Default::default())
}

fn compose_sequence(
    events_iter: &mut YamlEventIter,
    anchors: &mut HashMap<String, YamlValue>,
    start_pos: YamlPosition,
) -> Result<YamlValue, YamlError> {
    let mut ret: Vec<YamlValue> = Vec::new();
    let mut end_pos = YamlPosition::default();
    while let Some(event) = events_iter.peek() {
        match event {
            YamlEvent::SequenceEnd(pos) => {
                end_pos = *pos;
                events_iter.next();
                break;
            }
            _ => {
                ret.push(compose_value(events_iter, anchors)?);
            }
        }
    }

    Ok(YamlValue {
        data: YamlValueData::Array(ret),
        start: start_pos,
        end: end_pos,
        ..Default::default()
    })
}

fn compose_map(
    events_iter: &mut YamlEventIter,
    anchors: &mut HashMap<String, YamlValue>,
    start_pos: YamlPosition,
) -> Result<YamlValue, YamlError> {
    let mut ret: YamlValueMap = YamlValueMap::new();
    let mut end_pos = YamlPosition::default();
    let mut key: Option<YamlValue> = None;
    while let Some(event) = events_iter.peek() {
        match event {
            YamlEvent::MapEnd(pos) => {
                end_pos = *pos;
                events_iter.next();
                break;
            }
            _ => {
                if let Some(key) = key.take() {
                    let value = compose_value(events_iter, anchors)?;
                    ret.insert(key, value);
                } else {
                    key = Some(compose_value(events_iter, anchors)?);
                }
            }
        }
    }

    Ok(YamlValue {
        data: YamlValueData::Map(Box::new(ret)),
        start: start_pos,
        end: end_pos,
        ..Default::default()
    })
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{YamlCollectionStyle, YamlScalarStyle};

    #[test]
    fn test_compose_single_scalar() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "abc".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 3),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(1, 3)),
            YamlEvent::StreamEnd,
        ];

        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::String("abc".to_string()),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(1, 3),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_compose_single_layer_sequence() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::SequenceStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "abc".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "def".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::SequenceEnd(YamlPosition::new(2, 5)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(2, 5)),
            YamlEvent::StreamEnd,
        ];

        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Array(vec![
                    YamlValue {
                        data: YamlValueData::String("abc".into()),
                        start: YamlPosition::new(1, 3),
                        end: YamlPosition::new(1, 5),
                        ..Default::default()
                    },
                    YamlValue {
                        data: YamlValueData::String("def".into()),
                        start: YamlPosition::new(2, 3),
                        end: YamlPosition::new(2, 5),
                        ..Default::default()
                    }
                ]),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_compose_single_layer_map() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "abc".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "def".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::MapEnd(YamlPosition::new(2, 5)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(2, 5)),
            YamlEvent::StreamEnd,
        ];

        let mut map = YamlValueMap::new();
        map.insert(
            YamlValue {
                data: YamlValueData::String("abc".into()),
                start: YamlPosition::new(1, 3),
                end: YamlPosition::new(1, 5),
                ..Default::default()
            },
            YamlValue {
                data: YamlValueData::String("def".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            },
        );

        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Map(Box::new(map)),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_compose_sequence_of_map() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::SequenceStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1),
            ),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "abc".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "def".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 8),
                YamlPosition::new(1, 10),
            ),
            YamlEvent::MapEnd(YamlPosition::new(1, 10)),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(2, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "hig".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "klm".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 8),
                YamlPosition::new(2, 10),
            ),
            YamlEvent::MapEnd(YamlPosition::new(2, 10)),
            YamlEvent::SequenceEnd(YamlPosition::new(2, 10)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(3, 1)),
            YamlEvent::StreamEnd,
        ];

        let mut map1 = YamlValueMap::new();
        map1.insert(
            YamlValue {
                data: YamlValueData::String("abc".into()),
                start: YamlPosition::new(1, 3),
                end: YamlPosition::new(1, 5),
                ..Default::default()
            },
            YamlValue {
                data: YamlValueData::String("def".into()),
                start: YamlPosition::new(1, 8),
                end: YamlPosition::new(1, 10),
                ..Default::default()
            },
        );
        let mut map2 = YamlValueMap::new();
        map2.insert(
            YamlValue {
                data: YamlValueData::String("hig".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            },
            YamlValue {
                data: YamlValueData::String("klm".into()),
                start: YamlPosition::new(2, 8),
                end: YamlPosition::new(2, 10),
                ..Default::default()
            },
        );

        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Array(vec![
                    YamlValue {
                        data: YamlValueData::Map(Box::new(map1)),
                        start: YamlPosition::new(1, 1),
                        end: YamlPosition::new(1, 10),
                        ..Default::default()
                    },
                    YamlValue {
                        data: YamlValueData::Map(Box::new(map2)),
                        start: YamlPosition::new(2, 1),
                        end: YamlPosition::new(2, 10),
                        ..Default::default()
                    },
                ]),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 10),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_compose_map_ofsequence_of() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(1, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "abc".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 3),
            ),
            YamlEvent::SequenceStart(
                None,
                None,
                YamlCollectionStyle::Block,
                YamlPosition::new(2, 1),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "def".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "hig".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(3, 3),
                YamlPosition::new(3, 5),
            ),
            YamlEvent::Scalar(
                None,
                None,
                "klm".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(4, 3),
                YamlPosition::new(4, 5),
            ),
            YamlEvent::SequenceEnd(YamlPosition::new(4, 5)),
            YamlEvent::MapEnd(YamlPosition::new(4, 5)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(4, 5)),
            YamlEvent::StreamEnd,
        ];

        let mut map = YamlValueMap::new();
        map.insert(
            YamlValue {
                data: YamlValueData::String("abc".into()),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(1, 3),
                ..Default::default()
            },
            YamlValue {
                data: YamlValueData::Array(vec![
                    YamlValue {
                        data: YamlValueData::String("def".into()),
                        start: YamlPosition::new(2, 3),
                        end: YamlPosition::new(2, 5),
                        ..Default::default()
                    },
                    YamlValue {
                        data: YamlValueData::String("hig".into()),
                        start: YamlPosition::new(3, 3),
                        end: YamlPosition::new(3, 5),
                        ..Default::default()
                    },
                    YamlValue {
                        data: YamlValueData::String("klm".into()),
                        start: YamlPosition::new(4, 3),
                        end: YamlPosition::new(4, 5),
                        ..Default::default()
                    },
                ]),
                start: YamlPosition::new(2, 1),
                end: YamlPosition::new(4, 5),
                ..Default::default()
            },
        );
        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Map(Box::new(map)),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(4, 5),
                ..Default::default()
            }
        );
    }
}
