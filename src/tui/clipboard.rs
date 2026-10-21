//! Copying text through the terminal itself (OSC 52), so it works over ssh and needs no helper
//! program. Terminals that do not support it ignore the sequence.

use std::io::Write;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Ask the terminal to put `text` on the system clipboard.
pub fn copy(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout();
    out.write_all(sequence(text).as_bytes())?;
    out.flush()
}

fn sequence(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

fn base64(bytes: &[u8]) -> String {
    bytes.chunks(3).map(group).collect()
}

/// Up to three bytes as four characters, padded with `=`.
fn group(chunk: &[u8]) -> String {
    let [a, b, c] = [0, 1, 2].map(|i| chunk.get(i).copied().unwrap_or(0));
    let bits = u32::from_be_bytes([0, a, b, c]);
    let sextet = |shift: u32| ALPHABET[(bits >> shift & 63) as usize] as char;
    let mut text: String = [18, 12, 6, 0].map(sextet).into_iter().collect();
    for pad in 0..3 - chunk.len() {
        text.replace_range(3 - pad..4 - pad, "=");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_vectors() {
        let encoded: Vec<String> = ["", "f", "fo", "foo", "foob"]
            .iter()
            .map(|s| base64(s.as_bytes()))
            .collect();

        // RFC 4648 test vectors cover every padding length.
        assert_eq!(encoded, ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg=="]);
    }

    #[test]
    fn the_sequence_wraps_the_payload_for_the_clipboard() {
        let text = sequence("foo");

        // OSC 52, clipboard selection `c`, BEL terminator.
        assert_eq!(text, "\x1b]52;c;Zm9v\x07");
    }
}
