// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

// Only used by the `#[cfg(test)]` `Value::compose` convenience wrapper
// and its own tests; production code takes `max_depth` via
// `YamlParseOption` instead.
#[cfg(test)]
use crate::parser::MAX_NESTING_DEPTH;
use crate::{
    Error, ErrorKind, Mapping, Value, ValueData, YamlEvent, YamlEventIter,
    YamlPosition, YamlTag,
};

/// Maximum number of `Value` nodes a single document may realize during
/// composition. Every scalar, sequence and mapping produced counts as
/// one node, and resolving an alias counts as the full number of nodes
/// in the anchor it copies (since `Value::clone()` duplicates all of
/// it). Storing an anchor in the anchor table counts as another full
/// copy of the anchored subtree, since the composer keeps that copy in
/// addition to the node that appears in the document. This bounds the
/// "billion laughs" pattern, where a chain of anchors each aliasing the
/// previous one several times duplicates its content exponentially
/// without the input itself growing much.
pub(crate) const MAX_COMPOSED_NODES: usize = 100_000;

/// Depth and node-count limits enforced while composing a single
/// document, configurable via `YamlParseOption`. A fresh instance is
/// created per document, since anchors (and therefore the node
/// budget) are scoped per document per the YAML specification.
struct ComposeBudget {
    /// Maximum nesting depth a `SequenceStart`/`MapStart` may reach.
    max_depth: usize,
    /// Node budget remaining, initialized from the configured maximum
    /// and charged down by `charge_budget()` as nodes are composed.
    remaining_nodes: usize,
    /// The configured maximum node count, kept around only to report
    /// an accurate value in the error message.
    max_nodes: usize,
}

impl ComposeBudget {
    fn new(max_depth: usize, max_nodes: usize) -> Self {
        Self {
            max_depth,
            remaining_nodes: max_nodes,
            max_nodes,
        }
    }
}

/// Number of nodes in `value`, including itself and, recursively, its
/// children (sequence items, mapping keys/values, and tagged content).
fn count_nodes(value: &Value) -> usize {
    1 + count_child_nodes(&value.data)
}

fn count_child_nodes(data: &ValueData) -> usize {
    match data {
        ValueData::Null | ValueData::String(_) => 0,
        ValueData::Array(items) => items.iter().map(count_nodes).sum(),
        ValueData::Map(map) => map
            .iter()
            .map(|(key, val)| count_nodes(key) + count_nodes(val))
            .sum(),
        ValueData::Tag(tag) => count_child_nodes(&tag.data),
    }
}

/// Charge `cost` nodes against the remaining composition budget,
/// returning `ErrorKind::AliasExpansionLimitExceeded` if that would
/// overdraw it.
fn charge_budget(
    budget: &mut ComposeBudget,
    cost: usize,
    pos: YamlPosition,
) -> Result<(), Error> {
    match budget.remaining_nodes.checked_sub(cost) {
        Some(remaining) => {
            budget.remaining_nodes = remaining;
            Ok(())
        }
        None => Err(Error::new(
            ErrorKind::AliasExpansionLimitExceeded,
            format!(
                "YAML document composes more than {} value nodes; this is \
                 likely an anchor/alias expansion bomb",
                budget.max_nodes
            ),
            pos,
            pos,
        )),
    }
}

impl Value {
    /// Test-only convenience wrapper for `compose_with_limits` using
    /// the default `MAX_NESTING_DEPTH`/`MAX_COMPOSED_NODES`;
    /// production code goes through `YamlParseOption` (see
    /// `from_str_with_opt`) instead.
    #[cfg(test)]
    pub(crate) fn compose(events: Vec<YamlEvent>) -> Result<Self, Error> {
        Self::compose_with_limits(events, MAX_NESTING_DEPTH, MAX_COMPOSED_NODES)
    }

    /// Like [`Self::compose`], but with configurable `max_depth`/
    /// `max_nodes` limits instead of the defaults. Backs
    /// `YamlParseOption`.
    pub(crate) fn compose_with_limits(
        events: Vec<YamlEvent>,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<Self, Error> {
        let mut documents =
            compose_documents_with_limits(events, max_depth, max_nodes)?;
        if documents.len() > 1 {
            return Err(Error::new(
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

/// Compose every document of an event stream into a `Value`, with
/// configurable `max_depth`/`max_nodes` limits (see
/// `YamlParseOption`).
///
/// Anchors are scoped per document per the YAML specification, so the
/// anchor table is reset between documents.
///
/// Multi-document streams are not part of the public API (see the
/// "Limitations" section in `README.md`); this composes every
/// document anyway so [`Value::compose_with_limits`] can tell whether
/// the stream held more than one and reject it accordingly.
fn compose_documents_with_limits(
    events: Vec<YamlEvent>,
    max_depth: usize,
    max_nodes: usize,
) -> Result<Vec<Value>, Error> {
    let mut events_iter = YamlEventIter::new(events);
    let mut documents = Vec::new();
    loop {
        match events_iter.peek() {
            None | Some(YamlEvent::StreamEnd) => break,
            Some(YamlEvent::DocumentStart(_, _)) => {
                let implicit = match events_iter.next() {
                    Some(YamlEvent::DocumentStart(implicit, _)) => implicit,
                    _ => unreachable!(),
                };
                let mut anchors: HashMap<String, Value> = HashMap::new();
                let mut budget = ComposeBudget::new(max_depth, max_nodes);
                let mut value = compose_value(
                    &mut events_iter,
                    &mut anchors,
                    0,
                    &mut budget,
                )?;
                // Record the explicit `---` / `...` document markers so
                // the dump can reproduce them.
                value.meta.doc_explicit = implicit;
                if let Some(YamlEvent::DocumentEnd(end_implicit, _)) =
                    events_iter.peek()
                {
                    let end_implicit = *end_implicit;
                    events_iter.next();
                    value.meta.doc_end_explicit = end_implicit;
                }
                documents.push(value);
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
    anchors: &mut HashMap<String, Value>,
    depth: usize,
    budget: &mut ComposeBudget,
) -> Result<Value, Error> {
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
                if depth >= budget.max_depth {
                    return Err(Error::new(
                        ErrorKind::RecursionLimitExceeded,
                        format!(
                            "YAML node nesting exceeds the maximum supported \
                             depth of {}",
                            budget.max_depth
                        ),
                        pos,
                        pos,
                    ));
                }
                charge_budget(budget, 1, pos)?;
                let array = compose_sequence(
                    events_iter,
                    anchors,
                    pos,
                    depth + 1,
                    budget,
                )?;
                let mut ret = if let Some(tag) = tag {
                    Value {
                        data: ValueData::Tag(Box::new(YamlTag {
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
                    charge_budget(budget, count_nodes(&ret), pos)?;
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::SequenceEnd(pos) => {
                return Err(Error::new(
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
                if depth >= budget.max_depth {
                    return Err(Error::new(
                        ErrorKind::RecursionLimitExceeded,
                        format!(
                            "YAML node nesting exceeds the maximum supported \
                             depth of {}",
                            budget.max_depth
                        ),
                        pos,
                        pos,
                    ));
                }
                charge_budget(budget, 1, pos)?;
                let map =
                    compose_map(events_iter, anchors, pos, depth + 1, budget)?;
                let mut ret = if let Some(tag) = tag {
                    Value {
                        data: ValueData::Tag(Box::new(YamlTag {
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
                    charge_budget(budget, count_nodes(&ret), pos)?;
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::MapEnd(pos) => {
                return Err(Error::new(
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
                charge_budget(budget, 1, start)?;
                let mut ret = if let Some(tag) = tag {
                    Value {
                        data: ValueData::Tag(Box::new(YamlTag {
                            name: tag,
                            data: ValueData::String(val),
                        })),
                        start,
                        end,
                        ..Default::default()
                    }
                } else {
                    Value {
                        data: ValueData::String(val),
                        start,
                        end,
                        ..Default::default()
                    }
                };
                ret.meta.scalar_style = Some(style);
                ret.meta.anchor = anchor;
                if let Some(anchor) = &ret.meta.anchor {
                    charge_budget(budget, count_nodes(&ret), start)?;
                    anchors.insert(anchor.clone(), ret.clone());
                }
                return Ok(ret);
            }
            YamlEvent::Alias(name, pos) => {
                if let Some(value) = anchors.get(&name) {
                    charge_budget(budget, count_nodes(value), pos)?;
                    let mut ret = value.clone();
                    ret.meta.alias = Some(name);
                    return Ok(ret);
                } else {
                    return Err(Error::new(
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
    anchors: &mut HashMap<String, Value>,
    start_pos: YamlPosition,
    depth: usize,
    budget: &mut ComposeBudget,
) -> Result<Value, Error> {
    let mut ret: Vec<Value> = Vec::new();
    let mut end_pos = YamlPosition::default();
    while let Some(event) = events_iter.peek() {
        match event {
            YamlEvent::SequenceEnd(pos) => {
                end_pos = *pos;
                events_iter.next();
                break;
            }
            _ => {
                ret.push(compose_value(events_iter, anchors, depth, budget)?);
            }
        }
    }

    Ok(Value {
        data: ValueData::Array(ret),
        start: start_pos,
        end: end_pos,
        ..Default::default()
    })
}

fn compose_map(
    events_iter: &mut YamlEventIter,
    anchors: &mut HashMap<String, Value>,
    start_pos: YamlPosition,
    depth: usize,
    budget: &mut ComposeBudget,
) -> Result<Value, Error> {
    let mut ret: Mapping = Mapping::new();
    let mut end_pos = YamlPosition::default();
    let mut key: Option<Value> = None;
    while let Some(event) = events_iter.peek() {
        match event {
            YamlEvent::MapEnd(pos) => {
                end_pos = *pos;
                events_iter.next();
                break;
            }
            _ => {
                if let Some(key) = key.take() {
                    let value =
                        compose_value(events_iter, anchors, depth, budget)?;
                    if ret.contains_key(&key) {
                        return Err(Error::new(
                            ErrorKind::DuplicateMapKey,
                            format!(
                                "Mapping key `{}` is duplicated; YAML \
                                 requires unique mapping keys",
                                key.data
                            ),
                            key.start,
                            key.end,
                        ));
                    }
                    ret.insert(key, value);
                } else {
                    key = Some(compose_value(
                        events_iter,
                        anchors,
                        depth,
                        budget,
                    )?);
                }
            }
        }
    }

    Ok(Value {
        data: ValueData::Map(Box::new(ret)),
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
            Value::compose(events).unwrap(),
            Value {
                data: ValueData::String("abc".to_string()),
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
            Value::compose(events).unwrap(),
            Value {
                data: ValueData::Array(vec![
                    Value {
                        data: ValueData::String("abc".into()),
                        start: YamlPosition::new(1, 3),
                        end: YamlPosition::new(1, 5),
                        ..Default::default()
                    },
                    Value {
                        data: ValueData::String("def".into()),
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

        let mut map = Mapping::new();
        map.insert(
            Value {
                data: ValueData::String("abc".into()),
                start: YamlPosition::new(1, 3),
                end: YamlPosition::new(1, 5),
                ..Default::default()
            },
            Value {
                data: ValueData::String("def".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            },
        );

        assert_eq!(
            Value::compose(events).unwrap(),
            Value {
                data: ValueData::Map(Box::new(map)),
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

        let mut map1 = Mapping::new();
        map1.insert(
            Value {
                data: ValueData::String("abc".into()),
                start: YamlPosition::new(1, 3),
                end: YamlPosition::new(1, 5),
                ..Default::default()
            },
            Value {
                data: ValueData::String("def".into()),
                start: YamlPosition::new(1, 8),
                end: YamlPosition::new(1, 10),
                ..Default::default()
            },
        );
        let mut map2 = Mapping::new();
        map2.insert(
            Value {
                data: ValueData::String("hig".into()),
                start: YamlPosition::new(2, 3),
                end: YamlPosition::new(2, 5),
                ..Default::default()
            },
            Value {
                data: ValueData::String("klm".into()),
                start: YamlPosition::new(2, 8),
                end: YamlPosition::new(2, 10),
                ..Default::default()
            },
        );

        assert_eq!(
            Value::compose(events).unwrap(),
            Value {
                data: ValueData::Array(vec![
                    Value {
                        data: ValueData::Map(Box::new(map1)),
                        start: YamlPosition::new(1, 1),
                        end: YamlPosition::new(1, 10),
                        ..Default::default()
                    },
                    Value {
                        data: ValueData::Map(Box::new(map2)),
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

        let mut map = Mapping::new();
        map.insert(
            Value {
                data: ValueData::String("abc".into()),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(1, 3),
                ..Default::default()
            },
            Value {
                data: ValueData::Array(vec![
                    Value {
                        data: ValueData::String("def".into()),
                        start: YamlPosition::new(2, 3),
                        end: YamlPosition::new(2, 5),
                        ..Default::default()
                    },
                    Value {
                        data: ValueData::String("hig".into()),
                        start: YamlPosition::new(3, 3),
                        end: YamlPosition::new(3, 5),
                        ..Default::default()
                    },
                    Value {
                        data: ValueData::String("klm".into()),
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
            Value::compose(events).unwrap(),
            Value {
                data: ValueData::Map(Box::new(map)),
                start: YamlPosition::new(1, 1),
                end: YamlPosition::new(4, 5),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_compose_rejects_hand_built_events_nested_past_the_limit() {
        // Regression: `compose_value`/`compose_sequence`/`compose_map`
        // used to recurse once per `SequenceStart`/`MapStart` with no
        // depth limit. This directly builds an event stream nested
        // past `MAX_NESTING_DEPTH` (bypassing the parser's own guard)
        // to prove the composer rejects it independently instead of
        // overflowing the stack.
        let nesting = MAX_NESTING_DEPTH + 1;
        let mut events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
        ];
        for _ in 0..nesting {
            events.push(YamlEvent::SequenceStart(
                None,
                None,
                YamlCollectionStyle::Flow,
                YamlPosition::new(1, 1),
            ));
        }
        events.push(YamlEvent::Scalar(
            None,
            None,
            "1".to_string(),
            YamlScalarStyle::Plain,
            YamlPosition::new(1, 1),
            YamlPosition::new(1, 1),
        ));
        for _ in 0..nesting {
            events.push(YamlEvent::SequenceEnd(YamlPosition::new(1, 1)));
        }
        events.push(YamlEvent::DocumentEnd(false, YamlPosition::new(1, 1)));
        events.push(YamlEvent::StreamEnd);

        let err = Value::compose(events).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::RecursionLimitExceeded);
    }

    #[test]
    fn test_anchor_storage_counts_against_node_budget() {
        // The composer returns an anchored value to the document *and*
        // keeps a second full copy in the anchor table. Both copies are
        // real `Value` nodes, so storing the anchor must charge the
        // budget too; otherwise a document can retain nearly twice its
        // declared node limit and still be accepted (the slow
        // 8-level alias-chain fuzz artifacts).
        let anchored_events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                Some("a".to_string()),
                None,
                "x".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 1),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(1, 1)),
            YamlEvent::StreamEnd,
        ];
        let err =
            Value::compose_with_limits(anchored_events, 128, 1).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::AliasExpansionLimitExceeded);

        // The same scalar without an anchor fits within the same budget.
        let plain_events = vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                None,
                "x".to_string(),
                YamlScalarStyle::Plain,
                YamlPosition::new(1, 1),
                YamlPosition::new(1, 1),
            ),
            YamlEvent::DocumentEnd(false, YamlPosition::new(1, 1)),
            YamlEvent::StreamEnd,
        ];
        assert!(Value::compose_with_limits(plain_events, 128, 1).is_ok());
    }
}
