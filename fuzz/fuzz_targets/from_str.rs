// SPDX-License-Identifier: Apache-2.0

//! Fuzz target: parse arbitrary character sequences (random text) as YAML.
//!
//! Any byte sequence is accepted and lossily converted to UTF-8 so the
//! parser sees every possible character, including control characters,
//! NUL bytes and invalid UTF-8 (mapped to U+FFFD). The parser must never
//! panic, abort or hang on random input — only ever return `Ok` or `Err`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let s = String::from_utf8_lossy(data);
    // Full parse + compose into a Value tree.
    let _ = rmsd_yaml::from_str::<rmsd_yaml::Value>(&s);
});
