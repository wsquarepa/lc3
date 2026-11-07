use crate::source::{Diagnostic, Severity};

/// Accepts UTF-8 lines of exactly 16 binary digits with spaces, tabs and `;`
/// comments. Returns the origin and image words, or errors for every malformed
/// line or a missing origin. Blank lines and CRLF endings are accepted.
pub(crate) fn bin(bytes: &[u8]) -> Result<(u16, Vec<u16>), Vec<Diagnostic>> {
    words(
        bytes,
        binary_word,
        "exactly 16 binary digits (0 or 1), with only spaces or tabs between digits",
    )
}

/// Accepts UTF-8 lines of one to four hex digits with an optional `x`/`X`,
/// surrounding spaces or tabs and `;` comments. Returns the origin and image
/// words, or errors for every malformed line or a missing origin. Blank lines
/// and CRLF endings are accepted.
pub(crate) fn hex(bytes: &[u8]) -> Result<(u16, Vec<u16>), Vec<Diagnostic>> {
    words(
        bytes,
        hex_word,
        "one to four hex digits with an optional x or X prefix and no internal whitespace or sign",
    )
}

fn words(
    bytes: &[u8],
    parse_word: fn(&str) -> Option<u16>,
    expected: &str,
) -> Result<(u16, Vec<u16>), Vec<Diagnostic>> {
    let mut words = Vec::new();
    let mut diagnostics = Vec::new();
    for (index, raw) in bytes.split(|&byte| byte == b'\n').enumerate() {
        let line = u32::try_from(index + 1).map_err(|_| {
            vec![Diagnostic {
                severity: Severity::Error,
                line: None,
                message: format!(
                    "source line {} exceeds the supported line number range",
                    index + 1
                ),
            }]
        })?;
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        let text = match std::str::from_utf8(raw) {
            Ok(text) => text,
            Err(error) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    line: Some(line),
                    message: format!(
                        "line {line}: invalid UTF-8 in {:?} (bytes {raw:02x?}): {error}; supply UTF-8 text",
                        String::from_utf8_lossy(raw)
                    ),
                });
                continue;
            }
        };
        let word = text.split_once(';').map_or(text, |(word, _)| word);
        let word = word.trim_matches([' ', '\t']);
        if word.is_empty() {
            continue;
        }
        match parse_word(word) {
            Some(word) => words.push(word),
            None => diagnostics.push(Diagnostic {
                severity: Severity::Error,
                line: Some(line),
                message: format!("line {line}: invalid word in {text:?}; expected {expected}"),
            }),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut words = words.into_iter();
    let origin = words.next().ok_or_else(|| {
        vec![Diagnostic {
            severity: Severity::Error,
            line: None,
            message: "source contains no words; supply an origin word".to_owned(),
        }]
    })?;
    Ok((origin, words.collect()))
}

fn binary_word(text: &str) -> Option<u16> {
    let mut word = 0_u16;
    let mut digits = 0;
    for byte in text.bytes() {
        match byte {
            b' ' | b'\t' => continue,
            b'0' | b'1' if digits < 16 => {
                word = (word << 1) | u16::from(byte - b'0');
                digits += 1;
            }
            _ => return None,
        }
    }
    (digits == 16).then_some(word)
}

fn hex_word(text: &str) -> Option<u16> {
    let digits = text.strip_prefix(['x', 'X']).unwrap_or(text);
    if !(1..=4).contains(&digits.len()) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(digits, 16).ok()
}
