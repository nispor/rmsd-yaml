// SPDX-License-Identifier: Apache-2.0

//! Deterministic smoke fuzz tests that run in CI without cargo-fuzz.
//!
//! The cargo-fuzz targets in `fuzz/` do the heavy lifting; these tests
//! provide a fast, reproducible regression check that random character
//! input and raw binary input through a reader (`from_reader`, e.g. a
//! file descriptor) never panic — they only ever return `Ok` or `Err`.

use std::io::Cursor;

use crate::{Value, documents, from_reader, from_str};

/// Tiny deterministic PRNG (xorshift64*) so results are reproducible.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn byte(&mut self) -> u8 {
        (self.next() >> 56) as u8
    }

    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

const SEED: u64 = 0x4d59_5df4_d0f3_3173;

#[test]
fn random_characters_never_panic() {
    let mut rng = Rng(SEED);
    // Mostly short inputs; occasionally a longer one to exercise
    // multi-line and multi-document paths.
    for i in 0..10_000u32 {
        let len = match i % 100 {
            99 => (rng.next() % 4_096) as usize,
            _ => (rng.next() % 64) as usize,
        };
        let bytes = rng.bytes(len);
        // Random bytes, lossily converted to random characters, so every
        // possible character (including NUL and control characters) is
        // fed to the parser.
        let s = String::from_utf8_lossy(&bytes);
        let _ = s.parse::<Value>();
        let _ = from_str::<Value>(&s);
        let _ = documents(&s);
    }
}

#[test]
fn raw_binary_reader_never_panics() {
    let mut rng = Rng(SEED ^ 0x5DEECE66D);
    for i in 0..10_000u32 {
        let len = match i % 100 {
            99 => (rng.next() % 4_096) as usize,
            _ => (rng.next() % 64) as usize,
        };
        let bytes = rng.bytes(len);
        // Raw bytes as-is through a reader (e.g. a file descriptor).
        // Invalid UTF-8 must be rejected cleanly; bytes that happen to
        // be valid UTF-8 reach the parser.
        let _ = from_reader::<_, Value>(Cursor::new(&bytes));
    }
}

#[test]
fn binary_edge_inputs_never_panic() {
    // NUL and control characters are valid UTF-8 and reach the parser;
    // high bytes are invalid UTF-8 and exercise the reader rejection path.
    for bytes in [
        &[0x00u8][..],
        &[0x00, 0x00],
        &[0x01, 0x02, 0x1f, 0x7f],
        &[0xff, 0xfe, 0x80, 0x00, 0x41],
        &[0xff; 256],
        &[0x00; 256],
        &[0x80; 256],
    ] {
        let _ = from_str::<Value>(&String::from_utf8_lossy(bytes));
        let _ = from_reader::<_, Value>(Cursor::new(bytes));
    }
}

#[test]
fn reader_rejects_invalid_utf8_with_error() {
    // A file descriptor fed raw binary must produce a clean Err (not a
    // panic, and not a silently parsed document).
    let bytes = [0xff, 0xfe, 0x00, 0x80, 0x41];
    let err = from_reader::<_, Value>(Cursor::new(&bytes)).unwrap_err();
    assert_eq!(err.kind(), crate::ErrorKind::Bug);
    assert!(
        err.msg().contains("stream did not contain valid UTF-8"),
        "got: {}",
        err
    );
}

#[test]
fn reader_accepts_valid_utf8_binary() {
    // NUL bytes are valid UTF-8: from_reader must parse them instead of
    // rejecting the stream.
    let value: Value = from_reader(Cursor::new(b"a: \x00b\n")).unwrap();
    assert_eq!(value, "a: \0b\n".parse::<Value>().unwrap());
}

#[test]
fn document_start_marker_with_trailing_space_at_eof_never_hangs() {
    // Regression: `--- ` (marker + trailing space at EOF) used to leave
    // the scanner stuck, looping forever and pushing events until the
    // process ran out of memory. It must terminate quickly with Ok or
    // Err, never allocate unboundedly.
    for input in [
        "--- ",
        "---  ",
        "---\t",
        "a\n--- ",
        "--- \n--- ",
        "--- #c",
        "--- \t",
        "---  \t",
    ] {
        let _ = input.parse::<Value>();
        let _ = from_str::<Value>(input);
        let _ = documents(input);
        let _ = from_reader::<_, Value>(Cursor::new(input.as_bytes()));
    }
    // Same-line document start with content still parses.
    assert_eq!(
        from_str::<Value>("--- x").unwrap(),
        "x".parse::<Value>().unwrap()
    );
}
