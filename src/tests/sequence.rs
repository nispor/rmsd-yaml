// SPDX-License-Identifier: Apache-2.0

use pretty_assertions::assert_eq;

use crate::{YamlEvent, YamlParser, YamlPosition};

#[test]
fn test_sequence_of_plain_scalar() {
    assert_eq!(
        YamlParser::parse_to_events("  - abc\n  - def\n").unwrap(),
        vec![
            YamlEvent::StreamStart,
            YamlEvent::DocumentStart(false, YamlPosition::new(1, 1)),
            YamlEvent::SequenceStart(None, YamlPosition::new(1, 1)),
            YamlEvent::Scalar(
                None,
                "abc".to_string(),
                YamlPosition::new(1, 5),
                YamlPosition::new(1, 7)
            ),
            YamlEvent::Scalar(
                None,
                "def".to_string(),
                YamlPosition::new(2, 5),
                YamlPosition::new(2, 7)
            ),
            YamlEvent::SequenceEnd(YamlPosition::new(2, 8)),
            YamlEvent::DocumentEnd(false, YamlPosition::new(2, 8)),
            YamlEvent::StreamEnd,
        ]
    )
}
