// SPDX-License-Identifier: Apache-2.0

//! Fuzz target: feed raw binary input through a reader (e.g. a file
//! descriptor or file opened with `from_reader`).
//!
//! The bytes are passed as-is without any UTF-8 conversion, so this
//! exercises both the invalid-UTF-8 rejection path (`read_to_string`
//! failing on non-UTF-8 data) and the parser path when the raw bytes
//! happen to be valid UTF-8 (e.g. NUL and control characters). The
//! library must survive arbitrary binary input: no panic, abort or hang,
//! only `Ok` or a clean `Err`.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = rmsd_yaml::from_reader::<_, rmsd_yaml::Value>(std::io::Cursor::new(
        data,
    ));
});
