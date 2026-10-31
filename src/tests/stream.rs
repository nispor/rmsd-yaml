// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    Deserializer, ErrorKind, Serializer, Value, YamlParseOption, from_str,
    from_str_with_opt, from_value, to_writer,
};

fn document_value(document: crate::DocumentDeserializer) -> Value {
    Value::deserialize(document).unwrap()
}

#[test]
fn iterator_deserializes_each_document() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Doc {
        name: String,
    }

    let input = "---\nname: first\n---\nname: second\n...\n";
    let docs: Vec<Doc> = Deserializer::from_str(input)
        .map(|document| Doc::deserialize(document).unwrap())
        .collect();
    assert_eq!(
        docs,
        vec![
            Doc {
                name: "first".to_string()
            },
            Doc {
                name: "second".to_string()
            },
        ]
    );
}

#[test]
fn stream_deserializer_also_accepts_a_single_document() {
    let value = Value::deserialize(Deserializer::from_str("a: 1\n")).unwrap();
    assert_eq!(value.get("a").unwrap().as_str().unwrap(), "1");

    let error = Value::deserialize(Deserializer::from_str("---\na\n---\nb\n"))
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NoSupportMultipleDocuments);
}

#[test]
fn iterator_yields_no_documents_for_an_empty_stream() {
    assert_eq!(Deserializer::from_str("").count(), 0);
    assert_eq!(Deserializer::from_str("# comment only\n").count(), 0);

    // The single-document API keeps its historical behavior: an empty
    // stream deserializes as a null document.
    let value: Value = from_str("").unwrap();
    assert!(value.is_null());
}

#[test]
fn from_str_rejects_multiple_documents() {
    let error = from_str::<Value>("---\na\n---\nb\n").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NoSupportMultipleDocuments);
}

#[test]
fn from_reader_rejects_multiple_documents() {
    let error = crate::from_reader::<_, Value>(std::io::Cursor::new(
        "---\na\n---\nb\n",
    ))
    .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NoSupportMultipleDocuments);
}

#[test]
fn reader_stream_honors_max_input_bytes() {
    let option = YamlParseOption {
        max_input_bytes: 4,
        ..Default::default()
    };
    let mut documents = Deserializer::from_reader_with_opt(
        std::io::Cursor::new("more than four bytes"),
        option,
    );
    let error = Value::deserialize(documents.next().unwrap()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InputTooLarge);
}

#[test]
fn reader_stream_deserializes_multiple_documents() {
    let values: Vec<Value> = Deserializer::from_reader(std::io::Cursor::new(
        "---\na: 1\n---\na: 2\n",
    ))
    .map(document_value)
    .collect();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].get("a").unwrap().as_str().unwrap(), "1");
    assert_eq!(values[1].get("a").unwrap().as_str().unwrap(), "2");
}

#[test]
fn second_document_error_positions_are_absolute() {
    let mut documents = Deserializer::from_str("---\na: 1\n---\n[bad\n");
    let _first = document_value(documents.next().unwrap());
    let error = Value::deserialize(documents.next().unwrap()).unwrap_err();
    assert_eq!(error.start_pos().line, 5);
}

#[test]
fn from_str_reports_a_malformed_second_document() {
    let error = from_str::<Value>("---\na: 1\n---\n[bad\n").unwrap_err();
    assert_ne!(error.kind(), ErrorKind::NoSupportMultipleDocuments);
}

#[test]
fn consecutive_empty_documents_are_distinct() {
    let values: Vec<Value> = Deserializer::from_str("---\n---\n")
        .map(document_value)
        .collect();
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(Value::is_null));
}

#[test]
fn second_document_error_is_reported_by_the_iterator() {
    let mut documents = Deserializer::from_str("---\nname: first\n---\n[a\n");

    let first = document_value(documents.next().unwrap());
    assert_eq!(first.get("name").unwrap().as_str().unwrap(), "first");

    let second = documents.next().unwrap();
    assert!(Value::deserialize(second).is_err());
    assert!(documents.next().is_none());
}

#[test]
fn anchors_are_scoped_to_their_document() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Doc {
        a: Option<i32>,
        b: Option<i32>,
        c: Option<i32>,
    }

    let mut documents =
        Deserializer::from_str("---\na: &x 1\nb: *x\n---\nc: *x\n");

    let first: Doc = Doc::deserialize(documents.next().unwrap()).unwrap();
    assert_eq!(
        first,
        Doc {
            a: Some(1),
            b: Some(1),
            c: None
        }
    );

    let second = documents.next().unwrap();
    let error = Value::deserialize(second).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownAlias);
}

#[test]
fn tag_directives_are_scoped_to_their_document() {
    let input = "%TAG !e! tag:example.com,2000:app/\n---\n!e!foo \
                 one\n---\n!e!foo two\n";
    let mut documents = Deserializer::from_str(input);

    let first = document_value(documents.next().unwrap());
    let second = documents.next().unwrap();
    let error = Value::deserialize(second).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InvalidTag);
    assert!(matches!(first.data, crate::ValueData::Tag(_)));
}

#[test]
fn duplicate_anchors_use_the_latest_completed_node() {
    let value: Value = from_str("a: &x 1\nb: &x 2\nc: *x\n").unwrap();
    assert_eq!(value.get("c").unwrap().as_str().unwrap(), "2");

    let value: Value = from_str("a: &x 1\nb: *x\nc: &x 2\nd: *x\n").unwrap();
    assert_eq!(value.get("b").unwrap().as_str().unwrap(), "1");
    assert_eq!(value.get("d").unwrap().as_str().unwrap(), "2");
}

#[test]
fn forward_alias_is_rejected() {
    let error = from_str::<Value>("a: *x\nb: &x 1\n").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnknownAlias);
}

#[test]
fn explicit_document_end_and_directives_are_preserved_per_document() {
    let input = "a: 1\n...\n%YAML 1.2\n---\nb: 2\n";
    let values: Vec<Value> =
        Deserializer::from_str(input).map(document_value).collect();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].get("a").unwrap().as_str().unwrap(), "1");
    assert_eq!(values[1].get("b").unwrap().as_str().unwrap(), "2");
}

#[test]
fn tagged_empty_scalar_keeps_its_string_type() {
    let value: Value = from_str("{ foo: !!str }\n").unwrap();
    let foo = value.get("foo").unwrap();
    match &foo.data {
        crate::ValueData::Tag(tag) => match &tag.data {
            crate::ValueData::String(text) => assert!(text.is_empty()),
            other => panic!("expected an empty string, got {other:?}"),
        },
        other => panic!("expected a tag, got {other:?}"),
    }
}

#[test]
fn event_deserializer_enforces_alias_node_budget() {
    let option = YamlParseOption {
        max_nodes: 2,
        ..Default::default()
    };
    let mut documents =
        Deserializer::from_str_with_opt("a: &x [1, 2, 3, 4]\nb: *x\n", option);
    let error = Value::deserialize(documents.next().unwrap()).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::AliasExpansionLimitExceeded);
}

#[test]
fn event_deserializer_enforces_depth_limit() {
    let option = YamlParseOption {
        max_depth: 1,
        ..Default::default()
    };
    let error = from_str_with_opt::<Value>("[[1]]", option).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::RecursionLimitExceeded);
}

#[test]
fn from_value_deserializes_an_owned_value() {
    let value: Value = from_str("a: 1\nb: 2\n").unwrap();
    let map: BTreeMap<String, i32> = from_value(value).unwrap();
    assert_eq!(map.get("a"), Some(&1));
    assert_eq!(map.get("b"), Some(&2));

    let tagged: Value = from_str("!!str 5\n").unwrap();
    let text: String = from_value(tagged).unwrap();
    assert_eq!(text, "5");
}

#[test]
fn multi_document_dump_inserts_separators() {
    let mut serializer = Serializer::new(Vec::new());
    serializer.serialize(&vec![1, 2]).unwrap();
    serializer.serialize(&vec![3]).unwrap();
    assert_eq!(serializer.into_inner(), b"- 1\n- 2\n---\n- 3\n".to_vec());
}

#[test]
fn multi_document_dump_honors_leading_start_indicator() {
    let option = crate::YamlSerializeOption {
        leading_start_indicator: true,
        ..Default::default()
    };
    let mut serializer = Serializer::with_opt(Vec::new(), option);
    serializer.serialize(&"first").unwrap();
    serializer.serialize(&"second").unwrap();
    assert_eq!(
        serializer.into_inner(),
        b"---\nfirst\n---\nsecond\n".to_vec()
    );
}

#[test]
fn multi_document_dump_round_trips_each_document() {
    let input = "---\na: 1\n---\nb: 2\n";
    let values: Vec<Value> =
        Deserializer::from_str(input).map(document_value).collect();

    let mut serializer = Serializer::new(Vec::new());
    for value in &values {
        serializer.serialize(value).unwrap();
    }
    let dumped = String::from_utf8(serializer.into_inner()).unwrap();

    let back: Vec<Value> = Deserializer::from_str(&dumped)
        .map(document_value)
        .collect();
    assert_eq!(values, back);
}

#[test]
fn to_writer_writes_a_single_document() {
    let mut output = Vec::new();
    to_writer(&mut output, &vec![1, 2]).unwrap();
    assert_eq!(output, b"- 1\n- 2\n".to_vec());
}

#[test]
fn malformed_second_document_does_not_poison_the_first() {
    let input = "---\nok: true\n---\nname: [unterminated\n";
    let mut documents = Deserializer::from_str(input);
    let first: Value = document_value(documents.next().unwrap());
    assert!(first.get("ok").unwrap().as_bool().unwrap());
    assert!(Value::deserialize(documents.next().unwrap()).is_err());
}
