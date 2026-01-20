// SPDX-License-Identifier: Apache-2.0

use crate::{
    ErrorKind, YamlError, YamlEvent, YamlEventIter, YamlPosition, YamlTag,
    YamlValue, YamlValueData, YamlValueMap,
};

impl YamlValue {
    pub(crate) fn compose(events: Vec<YamlEvent>) -> Result<Self, YamlError> {
        let mut events_iter = YamlEventIter::new(events);
        compose_value(&mut events_iter)
    }
}

fn compose_value(
    events_iter: &mut YamlEventIter,
) -> Result<YamlValue, YamlError> {
    let mut doc_started_pos: Option<YamlPosition> = None;
    while let Some(event) = events_iter.next() {
        match event {
            YamlEvent::StreamStart => (),
            YamlEvent::DocumentStart(_, pos) => {
                if let Some(doc_started_pos) = doc_started_pos {
                    return Err(YamlError::new(
                        ErrorKind::NoSupportMultipleDocuments,
                        "No support of multiple YAML documents".to_string(),
                        doc_started_pos,
                        pos,
                    ));
                } else {
                    doc_started_pos = Some(pos);
                }
            }
            YamlEvent::DocumentEnd(_, _) | YamlEvent::StreamEnd => {
                break;
            }
            YamlEvent::SequenceStart(tag, pos) => {
                let array = compose_sequence(events_iter, pos)?;
                if let Some(tag) = tag {
                    return Ok(YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: array.data,
                        })),
                        start: array.start,
                        end: array.end,
                    });
                } else {
                    return Ok(array);
                }
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
            YamlEvent::MapStart(tag, pos) => {
                let map = compose_map(events_iter, pos)?;
                if let Some(tag) = tag {
                    return Ok(YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: map.data,
                        })),
                        start: map.start,
                        end: map.end,
                    });
                } else {
                    return Ok(map);
                }
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
            YamlEvent::Scalar(tag, val, start, end) => {
                if let Some(tag) = tag {
                    return Ok(YamlValue {
                        data: YamlValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: YamlValueData::String(val),
                        })),
                        start,
                        end,
                    });
                } else {
                    return Ok(YamlValue {
                        data: YamlValueData::String(val),
                        start,
                        end,
                    });
                }
            }
        }
    }

    Ok(Default::default())
}

fn compose_sequence(
    events_iter: &mut YamlEventIter,
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
                ret.push(compose_value(events_iter)?);
            }
        }
    }

    Ok(YamlValue {
        data: YamlValueData::Array(ret),
        start: start_pos,
        end: end_pos,
    })
}

fn compose_map(
    events_iter: &mut YamlEventIter,
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
                    let value = compose_value(events_iter)?;
                    ret.insert(key, value);
                } else {
                    key = Some(compose_value(events_iter)?);
                }
            }
        }
    }

    Ok(YamlValue {
        data: YamlValueData::Map(Box::new(ret)),
        start: start_pos,
        end: end_pos,
    })
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_compose_single_scalar() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
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
                end: YamlPosition::new(1, 3)
            }
        );
    }

    #[test]
    fn test_compose_single_layer_sequence() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::SequenceStart(None, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                "def".to_string(),
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
                    },
                    YamlValue {
                        data: YamlValueData::String("def".into()),
                        start: YamlPosition::new(2, 3),
                        end: YamlPosition::new(2, 5),
                    }
                ]),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 5),
            }
        );
    }

    #[test]
    fn test_compose_single_layer_map() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(None, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                "def".to_string(),
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
            },
            YamlValue {
                data: YamlValueData::String("def".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
            },
        );

        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Map(Box::new(map)),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 5),
            }
        );
    }

    #[test]
    fn test_compose_sequence_of_map() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::SequenceStart(None, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(None, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
                YamlPosition::new(1, 3),
                YamlPosition::new(1, 5),
            ),
            YamlEvent::Scalar(
                None,
                "def".to_string(),
                YamlPosition::new(1, 8),
                YamlPosition::new(1, 10),
            ),
            YamlEvent::MapEnd(YamlPosition::new(1, 10)),
            YamlEvent::MapStart(None, YamlPosition::new(2, 1)),
            YamlEvent::Scalar(
                None,
                "hig".to_string(),
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::Scalar(
                None,
                "klm".to_string(),
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
            },
            YamlValue {
                data: YamlValueData::String("def".into()),
                start: YamlPosition::new(1, 8),
                end: YamlPosition::new(1, 10),
            },
        );
        let mut map2 = YamlValueMap::new();
        map2.insert(
            YamlValue {
                data: YamlValueData::String("hig".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
            },
            YamlValue {
                data: YamlValueData::String("klm".into()),
                start: YamlPosition::new(2, 8),
                end: YamlPosition::new(2, 10),
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
                    },
                    YamlValue {
                        data: YamlValueData::Map(Box::new(map2)),
                        start: YamlPosition::new(2, 1),
                        end: YamlPosition::new(2, 10),
                    },
                ]),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(2, 10),
            }
        );
    }

    #[test]
    fn test_compose_map_ofsequence_of() {
        let events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::MapStart(None, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 3),
            ),
            YamlEvent::SequenceStart(None, YamlPosition::new(2, 1)),
            YamlEvent::Scalar(
                None,
                "def".to_string(),
                YamlPosition::new(2, 3),
                YamlPosition::new(2, 5),
            ),
            YamlEvent::Scalar(
                None,
                "hig".to_string(),
                YamlPosition::new(3, 3),
                YamlPosition::new(3, 5),
            ),
            YamlEvent::Scalar(
                None,
                "klm".to_string(),
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
            },
            YamlValue {
                data: YamlValueData::Array(vec![
                    YamlValue {
                        data: YamlValueData::String("def".into()),
                        start: YamlPosition::new(2, 3),
                        end: YamlPosition::new(2, 5),
                    },
                    YamlValue {
                        data: YamlValueData::String("hig".into()),
                        start: YamlPosition::new(3, 3),
                        end: YamlPosition::new(3, 5),
                    },
                    YamlValue {
                        data: YamlValueData::String("klm".into()),
                        start: YamlPosition::new(4, 3),
                        end: YamlPosition::new(4, 5),
                    },
                ]),
                start: YamlPosition::new(2, 1),
                end: YamlPosition::new(4, 5),
            },
        );
        assert_eq!(
            YamlValue::compose(events).unwrap(),
            YamlValue {
                data: YamlValueData::Map(Box::new(map)),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(4, 5),
            }
        );
    }
}
