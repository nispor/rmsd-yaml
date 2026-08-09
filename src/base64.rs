// SPDX-License-Identifier: Apache-2.0

//! Minimal standard base64 (RFC 4648) encoder/decoder used for the
//! `!!binary` tag, kept dependency-free per the project goals.

const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode a byte slice into standard base64 with padding.
pub(crate) fn encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(triple >> 6) as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[triple as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decode a standard base64 string (whitespace and padding tolerated).
pub(crate) fn decode(input: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut quad = [0u8; 4];
    let mut quad_len = 0usize;
    for c in input.chars() {
        if c.is_whitespace() {
            continue;
        }
        let value = match c {
            'A'..='Z' => c as u8 - b'A',
            'a'..='z' => c as u8 - b'a' + 26,
            '0'..='9' => c as u8 - b'0' + 52,
            '+' => 62,
            '/' => 63,
            '=' => break,
            _ => return Err(format!("invalid base64 character {c:?}")),
        };
        quad[quad_len] = value;
        quad_len += 1;
        if quad_len == 4 {
            let triple = ((quad[0] as u32) << 18)
                | ((quad[1] as u32) << 12)
                | ((quad[2] as u32) << 6)
                | quad[3] as u32;
            out.push((triple >> 16) as u8);
            out.push((triple >> 8) as u8);
            out.push(triple as u8);
            quad_len = 0;
        }
    }
    if quad_len == 1 {
        return Err("invalid base64 length".to_string());
    }
    if quad_len == 2 {
        let triple = ((quad[0] as u32) << 18) | ((quad[1] as u32) << 12);
        out.push((triple >> 16) as u8);
    } else if quad_len == 3 {
        let triple = ((quad[0] as u32) << 18)
            | ((quad[1] as u32) << 12)
            | ((quad[2] as u32) << 6);
        out.push((triple >> 16) as u8);
        out.push((triple >> 8) as u8);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        for input in [
            &[][..],
            b"a",
            b"ab",
            b"abc",
            b"abcd",
            b"hello world",
            &[0u8, 1, 2, 3, 254, 255],
        ] {
            let encoded = encode(input);
            assert_eq!(decode(&encoded).unwrap(), input, "for {input:?}");
        }
    }

    #[test]
    fn test_known_vectors() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foob"), "Zm9vYg==");
        assert_eq!(encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(decode("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn test_invalid() {
        assert!(decode("!!!!").is_err());
        assert!(decode("Z").is_err());
        assert!(decode("Zg*").is_err());
    }
}
