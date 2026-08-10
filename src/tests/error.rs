// SPDX-License-Identifier: Apache-2.0

use crate::{Error, ErrorKind};

#[test]
fn test_custom_deserialize_error_message_preserved_verbatim() {
    // Regression: `Error::from(&str)` (used by
    // `serde::de::Error::custom()`) used to reconstruct
    // `kind`/`start_pos`/`end_pos` by pattern-matching this crate's
    // own `Display` output (splitting on `" kind: "` and `"error:
    // "`). A custom error message that happens to be shaped like that
    // output had its kind/position silently reinterpreted and the
    // leading part of the message discarded. It must now survive
    // untouched.
    let tricky = "0:0 kind: bug error: field `count` must be positive";
    let err = <Error as serde::de::Error>::custom(tricky);
    assert_eq!(err.msg(), tricky);
    assert_eq!(err.kind(), ErrorKind::Custom);
}

#[test]
fn test_custom_serialize_error_message_preserved_verbatim() {
    // Same regression as above, for `serde::ser::Error::custom()`.
    let tricky = "1:2 kind: recursion_limit_exceeded error: value too deep";
    let err = <Error as serde::ser::Error>::custom(tricky);
    assert_eq!(err.msg(), tricky);
    assert_eq!(err.kind(), ErrorKind::Custom);
}

#[test]
fn test_ordinary_custom_message_preserved_too() {
    // Messages that do not look like the crate's `Display` output
    // must of course also survive unchanged.
    let err = Error::from("field `x` is required");
    assert_eq!(err.msg(), "field `x` is required");
    assert_eq!(err.kind(), ErrorKind::Custom);
}
